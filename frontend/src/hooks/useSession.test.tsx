import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { server } from '../mocks/server';
import { SessionId } from '../api/brand';
import { useSession } from './useSession';

// PR-C2 (Issue #186) migrated useSession from a setInterval/visibility-API
// polling loop to TanStack Query. Per react.md § 13, hook tests use
// `renderHook` wrapped in a QueryClientProvider and mock the network at the
// fetch boundary with MSW. The assertions target the public contract
// (`session` / `loading` / `ended`) and the observable polling behavior
// (request counts), not TanStack Query's internal query state or timers.

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
  };
}

const sid = SessionId('s1');

const activeSession = {
  id: 's1',
  host_id: 'u1',
  host_username: 'alice',
  ruleset: 'random',
  status: 'active',
  created_at: '2026-05-22T00:00:00.000Z',
  participants: [],
  race_number: 1,
  current_race: null,
  races: [],
};

const closedSession = { ...activeSession, status: 'closed' };

describe('useSession', () => {
  it('returns the session and clears loading once the query resolves', async () => {
    server.use(
      http.get('/api/v1/sessions/:id', () => HttpResponse.json(activeSession)),
    );

    const { result } = renderHook(() => useSession(sid), {
      wrapper: createWrapper(),
    });

    expect(result.current.loading).toBe(true);
    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.session?.status).toBe('active');
    expect(result.current.ended).toBe(false);
  });

  it('marks the session ended and stops polling when it is closed', async () => {
    let calls = 0;
    server.use(
      http.get('/api/v1/sessions/:id', () => {
        calls += 1;
        return HttpResponse.json(closedSession);
      }),
    );

    const { result } = renderHook(() => useSession(sid), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.ended).toBe(true);
    });
    expect(result.current.session?.status).toBe('closed');

    // refetchInterval returns false for a closed session, so the poll loop
    // never schedules another request — the count stops climbing.
    const after = calls;
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(calls).toBe(after);
  });

  it('marks the session ended when it is not found (404)', async () => {
    server.use(
      http.get(
        '/api/v1/sessions/:id',
        () => new HttpResponse(null, { status: 404 }),
      ),
    );

    const { result } = renderHook(() => useSession(sid), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.ended).toBe(true);
    });
    expect(result.current.session).toBeNull();
  });

  it('keeps polling while the session stays active', async () => {
    let calls = 0;
    server.use(
      http.get('/api/v1/sessions/:id', () => {
        calls += 1;
        return HttpResponse.json(activeSession);
      }),
    );

    renderHook(() => useSession(sid), { wrapper: createWrapper() });

    await waitFor(() => {
      expect(calls).toBe(1);
    });
    // The 2.5s refetchInterval fires at least one more request over time.
    await waitFor(
      () => {
        expect(calls).toBeGreaterThan(1);
      },
      { timeout: 4000 },
    );
  });
});
