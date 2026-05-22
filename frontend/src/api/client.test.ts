import { http, HttpResponse } from 'msw';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { server } from '../mocks/server';
import { apiFetch, setAccessToken, setOnAuthFailure } from './client';

// Verifies the access-token + 401-refresh behavior of `apiFetch`. The
// user-visible promise: an expired access token is refreshed and the request
// retried transparently, so the user is not bounced to /login mid-session;
// when the refresh cookie itself is gone, the session ends and the auth-
// failure callback fires. MSW intercepts at the fetch boundary (react.md
// § 13), so the refresh cookie / token plumbing is exercised for real.

afterEach(() => {
  // Module-level token + callback are global state; reset so tests don't leak.
  setAccessToken(null);
  setOnAuthFailure(vi.fn());
});

describe('apiFetch', () => {
  it('attaches the Bearer token when one is set', async () => {
    setAccessToken('tok-1');
    let seen: string | null = null;
    server.use(
      http.get('/api/v1/ping', ({ request }) => {
        seen = request.headers.get('Authorization');
        return HttpResponse.json({ ok: true });
      }),
    );

    await apiFetch('/api/v1/ping');

    expect(seen).toBe('Bearer tok-1');
  });

  it('omits the Authorization header when no token is set', async () => {
    let hasAuth = true;
    server.use(
      http.get('/api/v1/ping', ({ request }) => {
        hasAuth = request.headers.has('Authorization');
        return HttpResponse.json({ ok: true });
      }),
    );

    await apiFetch('/api/v1/ping');

    expect(hasAuth).toBe(false);
  });

  it('forwards an AbortSignal to fetch', async () => {
    const controller = new AbortController();
    server.use(http.get('/api/v1/ping', () => HttpResponse.json({})));
    controller.abort();

    await expect(
      apiFetch('/api/v1/ping', { signal: controller.signal }),
    ).rejects.toThrow();
  });

  it('refreshes and retries once on a 401, using the new token', async () => {
    setAccessToken('expired');
    const authHeaders: (string | null)[] = [];
    let calls = 0;
    server.use(
      http.get('/api/v1/me', ({ request }) => {
        authHeaders.push(request.headers.get('Authorization'));
        calls += 1;
        // First call (expired token) 401s; the retry (fresh token) succeeds.
        return calls === 1
          ? new HttpResponse(null, { status: 401 })
          : HttpResponse.json({ id: 'u1' });
      }),
      http.post('/api/v1/auth/refresh', () =>
        HttpResponse.json({ access_token: 'fresh' }),
      ),
    );

    const res = await apiFetch('/api/v1/me');

    expect(res.status).toBe(200);
    expect(authHeaders).toEqual(['Bearer expired', 'Bearer fresh']);
  });

  it('clears the token and fires onAuthFailure when refresh fails', async () => {
    setAccessToken('expired');
    const onFailure = vi.fn();
    setOnAuthFailure(onFailure);
    server.use(
      http.get('/api/v1/me', () => new HttpResponse(null, { status: 401 })),
      http.post(
        '/api/v1/auth/refresh',
        () => new HttpResponse(null, { status: 401 }),
      ),
    );

    const res = await apiFetch('/api/v1/me');

    expect(res.status).toBe(401);
    expect(onFailure).toHaveBeenCalledOnce();
  });

  it('treats a malformed refresh body as a failed refresh', async () => {
    setAccessToken('expired');
    const onFailure = vi.fn();
    setOnAuthFailure(onFailure);
    // 2xx but missing access_token — parseBody throws, refresh is treated as
    // failed rather than crashing the request.
    const err = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    server.use(
      http.get('/api/v1/me', () => new HttpResponse(null, { status: 401 })),
      http.post('/api/v1/auth/refresh', () => HttpResponse.json({ nope: 1 })),
    );

    const res = await apiFetch('/api/v1/me');

    expect(res.status).toBe(401);
    expect(onFailure).toHaveBeenCalledOnce();
    expect(err).toHaveBeenCalled();
    err.mockRestore();
  });

  it('does not attempt a refresh on a 401 when no token was set', async () => {
    let refreshCalls = 0;
    server.use(
      http.get('/api/v1/me', () => new HttpResponse(null, { status: 401 })),
      http.post('/api/v1/auth/refresh', () => {
        refreshCalls += 1;
        return HttpResponse.json({ access_token: 'fresh' });
      }),
    );

    const res = await apiFetch('/api/v1/me');

    expect(res.status).toBe(401);
    expect(refreshCalls).toBe(0);
  });
});
