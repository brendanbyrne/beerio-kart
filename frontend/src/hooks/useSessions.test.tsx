import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { server } from '../mocks/server';
import { useSessions } from './useSessions';

// PR-C2 (Issue #186) split useSessions' single `Promise.all` into two TanStack
// Query queries (['sessions'] + ['my-session']) so the my-session piece is
// shared with BottomNav. Per react.md § 13, the test mocks both endpoints with
// MSW and asserts the public `{ sessions, mySessionId, loading }` contract.

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

const sessionSummary = {
  id: 's1',
  host_username: 'alice',
  participant_count: 2,
  race_number: 1,
  ruleset: 'random',
};

describe('useSessions', () => {
  it('returns the session list and my-session id once both queries resolve', async () => {
    server.use(
      http.get('/api/v1/sessions', () => HttpResponse.json([sessionSummary])),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: 's1' }),
      ),
    );

    const { result } = renderHook(() => useSessions(), {
      wrapper: createWrapper(),
    });

    expect(result.current.loading).toBe(true);
    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.sessions).toHaveLength(1);
    expect(result.current.mySessionId).toBe('s1');
  });

  it('reports no active session when /mine returns null', async () => {
    server.use(
      http.get('/api/v1/sessions', () => HttpResponse.json([])),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: null }),
      ),
    );

    const { result } = renderHook(() => useSessions(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.sessions).toEqual([]);
    expect(result.current.mySessionId).toBeNull();
  });
});
