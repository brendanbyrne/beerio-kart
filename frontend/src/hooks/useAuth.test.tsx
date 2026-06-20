import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { server } from '../mocks/server';
import { AuthProvider, useAuth } from './useAuth';

// Covers the auth-context HTTP calls (`login`, `register`, `changePassword`)
// at the hook boundary: render `useAuth` inside an `AuthProvider`, drive MSW
// to return either a success body or an error body, assert on the returned
// `Promise<string | null>` shape every consumer relies on.

function Wrapper({ children }: { children: ReactNode }) {
  return <AuthProvider>{children}</AuthProvider>;
}

const session = {
  access_token: 'access.token.body',
  user: { id: 'u1', username: 'alice' },
};

function setupHook() {
  // The provider fires a silent refresh on mount — let it fail fast so the
  // hook settles into the "logged out" state before the test acts.
  server.use(
    http.post(
      '/api/v1/auth/refresh',
      () => new HttpResponse(null, { status: 401 }),
    ),
  );
  return renderHook(() => useAuth(), { wrapper: Wrapper });
}

describe('useAuth', () => {
  describe('login', () => {
    it('returns null on success and stores the user', async () => {
      const { result } = setupHook();
      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      server.use(
        http.post('/api/v1/auth/login', () => HttpResponse.json(session)),
      );

      let err: string | null = 'unset';
      await act(async () => {
        err = await result.current.login('alice', 'hunter2');
      });
      expect(err).toBeNull();
      expect(result.current.user).toEqual(session.user);
      expect(result.current.isAuthenticated).toBe(true);
    });

    it('returns the backend error message on failure', async () => {
      const { result } = setupHook();
      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      server.use(
        http.post('/api/v1/auth/login', () =>
          HttpResponse.json({ error: 'Invalid credentials' }, { status: 401 }),
        ),
      );

      let err: string | null = null;
      await act(async () => {
        err = await result.current.login('alice', 'wrong');
      });
      expect(err).toBe('Invalid credentials');
    });
  });

  describe('register', () => {
    it('returns null on success and stores the new user', async () => {
      const { result } = setupHook();
      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      server.use(
        http.post('/api/v1/auth/register', () => HttpResponse.json(session)),
      );

      let err: string | null = 'unset';
      await act(async () => {
        err = await result.current.register('alice', 'hunter2!');
      });
      expect(err).toBeNull();
      expect(result.current.user).toEqual(session.user);
    });

    it('returns the backend error message on failure', async () => {
      const { result } = setupHook();
      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      server.use(
        http.post('/api/v1/auth/register', () =>
          HttpResponse.json({ error: 'Username is taken' }, { status: 409 }),
        ),
      );

      let err: string | null = null;
      await act(async () => {
        err = await result.current.register('alice', 'hunter2!');
      });
      expect(err).toBe('Username is taken');
    });
  });

  describe('authNotice (reuse detection sign-out)', () => {
    it('surfaces a security notice when reuse detection forces a sign-out, and clears it on the next login', async () => {
      const { result } = setupHook();
      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      // Establish a session so the API client has an access token to send.
      server.use(
        http.post('/api/v1/auth/login', () => HttpResponse.json(session)),
      );
      await act(async () => {
        await result.current.login('alice', 'hunter2');
      });
      expect(result.current.isAuthenticated).toBe(true);
      expect(result.current.authNotice).toBeNull();

      // An authenticated request 401s; the refresh reports reuse detection, so
      // the client signs out with reason 'reuse' → the provider sets the notice.
      server.use(
        http.put(
          '/api/v1/auth/password',
          () => new HttpResponse(null, { status: 401 }),
        ),
        http.post('/api/v1/auth/refresh', () =>
          HttpResponse.json(
            {
              error: 'Refresh token reuse detected',
              code: 'token_reuse_detected',
            },
            { status: 401 },
          ),
        ),
      );
      await act(async () => {
        await result.current.changePassword('old', 'new-secret!');
      });

      await waitFor(() => {
        expect(result.current.isAuthenticated).toBe(false);
        expect(result.current.authNotice).toMatch(/security/i);
      });

      // Logging back in clears the notice.
      server.use(
        http.post('/api/v1/auth/login', () => HttpResponse.json(session)),
      );
      await act(async () => {
        await result.current.login('alice', 'hunter2');
      });
      expect(result.current.authNotice).toBeNull();
    });

    it('signs out without a security notice on an ordinary expiry', async () => {
      const { result } = setupHook();
      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      server.use(
        http.post('/api/v1/auth/login', () => HttpResponse.json(session)),
      );
      await act(async () => {
        await result.current.login('alice', 'hunter2');
      });
      expect(result.current.isAuthenticated).toBe(true);

      // The refresh cookie is genuinely gone (not reuse) → sign out, no notice.
      server.use(
        http.put(
          '/api/v1/auth/password',
          () => new HttpResponse(null, { status: 401 }),
        ),
        http.post('/api/v1/auth/refresh', () =>
          HttpResponse.json(
            { error: 'Refresh token has been revoked', code: 'token_invalid' },
            { status: 401 },
          ),
        ),
      );
      await act(async () => {
        await result.current.changePassword('old', 'new-secret!');
      });

      await waitFor(() => {
        expect(result.current.isAuthenticated).toBe(false);
      });
      expect(result.current.authNotice).toBeNull();
    });
  });

  describe('changePassword', () => {
    it('returns null on success', async () => {
      const { result } = setupHook();
      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      server.use(
        http.put(
          '/api/v1/auth/password',
          () => new HttpResponse(null, { status: 204 }),
        ),
      );

      let err: string | null = 'unset';
      await act(async () => {
        err = await result.current.changePassword('old', 'new-secret!');
      });
      expect(err).toBeNull();
    });

    it('returns the backend error message on failure', async () => {
      const { result } = setupHook();
      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      server.use(
        http.put('/api/v1/auth/password', () =>
          HttpResponse.json(
            { error: 'Current password is incorrect' },
            { status: 400 },
          ),
        ),
      );

      let err: string | null = null;
      await act(async () => {
        err = await result.current.changePassword('wrong', 'new-secret!');
      });
      expect(err).toBe('Current password is incorrect');
    });
  });

  describe('logout (#209 — leave session first)', () => {
    // Log in so the API client holds an access token; `getMySession` /
    // `leaveSession` go through `apiFetch`, which attaches it.
    async function loggedInHook() {
      const view = setupHook();
      await waitFor(() => {
        expect(view.result.current.isLoading).toBe(false);
      });
      server.use(
        http.post('/api/v1/auth/login', () => HttpResponse.json(session)),
      );
      await act(async () => {
        await view.result.current.login('alice', 'hunter2');
      });
      expect(view.result.current.isAuthenticated).toBe(true);
      return view;
    }

    it('leaves the current session before POSTing logout, then clears auth', async () => {
      const { result } = await loggedInHook();

      const calls: string[] = [];
      let leftSessionId: string | undefined;
      server.use(
        http.get('/api/v1/sessions/mine', () => {
          calls.push('mine');
          return HttpResponse.json({ session_id: 's1' });
        }),
        http.post('/api/v1/sessions/:id/leave', ({ params }) => {
          calls.push('leave');
          leftSessionId = params.id as string;
          return new HttpResponse(null, { status: 204 });
        }),
        http.post('/api/v1/auth/logout', () => {
          calls.push('logout');
          return new HttpResponse(null, { status: 204 });
        }),
      );

      await act(async () => {
        await result.current.logout();
      });

      // Leave is called with the live session id, strictly before the logout
      // POST (which revokes the refresh token), then local auth is cleared.
      expect(leftSessionId).toBe('s1');
      expect(calls).toEqual(['mine', 'leave', 'logout']);
      expect(result.current.isAuthenticated).toBe(false);
      expect(result.current.user).toBeNull();
    });

    it('still signs out after attempting the leave when leaving fails', async () => {
      const { result } = await loggedInHook();

      const calls: string[] = [];
      server.use(
        http.get('/api/v1/sessions/mine', () => {
          calls.push('mine');
          return HttpResponse.json({ session_id: 's1' });
        }),
        // Leave errors (500, not a 401 — no refresh path); logout must proceed.
        http.post('/api/v1/sessions/:id/leave', () => {
          calls.push('leave');
          return HttpResponse.json({ error: 'leave failed' }, { status: 500 });
        }),
        http.post('/api/v1/auth/logout', () => {
          calls.push('logout');
          return new HttpResponse(null, { status: 204 });
        }),
      );

      await act(async () => {
        await result.current.logout();
      });

      // Leave was attempted (and failed) strictly before the logout POST; the
      // failure was swallowed and sign-out completed regardless.
      expect(calls).toEqual(['mine', 'leave', 'logout']);
      expect(result.current.isAuthenticated).toBe(false);
      expect(result.current.user).toBeNull();
    });

    it('does not call leaveSession when not in a session', async () => {
      const { result } = await loggedInHook();

      let leaveCalled = false;
      let logoutPosted = false;
      server.use(
        http.get('/api/v1/sessions/mine', () =>
          HttpResponse.json({ session_id: null }),
        ),
        http.post('/api/v1/sessions/:id/leave', () => {
          leaveCalled = true;
          return new HttpResponse(null, { status: 204 });
        }),
        http.post('/api/v1/auth/logout', () => {
          logoutPosted = true;
          return new HttpResponse(null, { status: 204 });
        }),
      );

      await act(async () => {
        await result.current.logout();
      });

      expect(leaveCalled).toBe(false);
      expect(logoutPosted).toBe(true);
      expect(result.current.isAuthenticated).toBe(false);
    });

    it('skips leaveSession and still signs out when fetching the current session errors', async () => {
      const { result } = await loggedInHook();

      let leaveCalled = false;
      let logoutPosted = false;
      server.use(
        // getMySession swallows a non-ok /mine and resolves to null, so leave
        // is skipped — a different path to the no-session outcome than a null
        // body, and one a slow/erroring sessions service hits in practice.
        http.get('/api/v1/sessions/mine', () =>
          HttpResponse.json({ error: 'boom' }, { status: 500 }),
        ),
        http.post('/api/v1/sessions/:id/leave', () => {
          leaveCalled = true;
          return new HttpResponse(null, { status: 204 });
        }),
        http.post('/api/v1/auth/logout', () => {
          logoutPosted = true;
          return new HttpResponse(null, { status: 204 });
        }),
      );

      await act(async () => {
        await result.current.logout();
      });

      expect(leaveCalled).toBe(false);
      expect(logoutPosted).toBe(true);
      expect(result.current.isAuthenticated).toBe(false);
    });
  });
});
