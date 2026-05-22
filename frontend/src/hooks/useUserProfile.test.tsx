import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { server } from '../mocks/server';
import { useUserProfile } from './useUserProfile';

// PR-C1 (Issue #176) migrated useUserProfile to TanStack Query. Hook tests
// follow react.md § 13: renderHook under a QueryClientProvider, network mocked
// with MSW, assertions on the public `{ profile, loading, refresh }` contract.

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

const profile = {
  id: 'u1',
  username: 'alice',
  preferred_character_id: null,
  preferred_body_id: null,
  preferred_wheel_id: null,
  preferred_glider_id: null,
  preferred_drink_type: null,
  created_at: '2026-05-21T00:00:00.000Z',
};

describe('useUserProfile', () => {
  it('returns the profile once the query resolves', async () => {
    server.use(http.get('/api/v1/users/u1', () => HttpResponse.json(profile)));

    const { result } = renderHook(() => useUserProfile('u1'), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.profile).toEqual(profile);
    });
  });

  it('stays loading and never fetches without a user id', async () => {
    let called = false;
    server.use(
      http.get('/api/v1/users/:id', () => {
        called = true;
        return HttpResponse.json(profile);
      }),
    );

    const { result } = renderHook(() => useUserProfile(undefined), {
      wrapper: createWrapper(),
    });

    // The query is disabled until an id is known: it stays pending (loading)
    // with no profile, and crucially issues no request — matching the legacy
    // hook, which returned early before fetching.
    expect(result.current.loading).toBe(true);
    expect(result.current.profile).toBeNull();
    // Let any erroneous fetch fire before asserting it didn't.
    await Promise.resolve();
    expect(called).toBe(false);
  });

  it('refetches the profile when refresh() is called', async () => {
    let calls = 0;
    server.use(
      http.get('/api/v1/users/u1', () => {
        calls += 1;
        return HttpResponse.json({
          ...profile,
          username: `alice${String(calls)}`,
        });
      }),
    );

    const { result } = renderHook(() => useUserProfile('u1'), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.profile?.username).toBe('alice1');
    });

    act(() => {
      result.current.refresh();
    });

    await waitFor(() => {
      expect(result.current.profile?.username).toBe('alice2');
    });
  });
});
