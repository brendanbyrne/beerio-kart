import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { server } from '../mocks/server';
import {
  useBodies,
  useCharacters,
  useDrinkTypes,
  useGliders,
  useWheels,
} from './useGameData';

// PR-C1 (Issue #176) migrated the static-data hooks to TanStack Query. Per
// react.md § 13, hook tests use `renderHook` wrapped in a QueryClientProvider
// and mock the network at the fetch boundary with MSW. The assertions target
// the public contract the hooks hand to components (`items` / `loading` /
// `refresh`), not TanStack Query's internal query state.

function createWrapper() {
  // A fresh client per test isolates the cache. `retry: false` so a
  // deliberately-failing request settles at once instead of waiting out the
  // app-level retry backoff.
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
  };
}

const mario = { id: 1, name: 'Mario', image_path: '/characters/mario.png' };

describe('useGameData', () => {
  it('returns the fetched list once the query resolves', async () => {
    server.use(
      http.get('/api/v1/characters', () => HttpResponse.json([mario])),
    );

    const { result } = renderHook(() => useCharacters(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.items).toEqual([mario]);
    });
  });

  it('exposes a loading state while the query is pending', async () => {
    server.use(
      http.get('/api/v1/characters', () => HttpResponse.json([mario])),
    );

    const { result } = renderHook(() => useCharacters(), {
      wrapper: createWrapper(),
    });

    // First render: the fetch is in flight, so the list is empty and loading.
    expect(result.current.loading).toBe(true);
    expect(result.current.items).toEqual([]);

    // Once it resolves, loading clears and the list is populated.
    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.items).toEqual([mario]);
  });

  // The four pick-list hooks are thin wrappers over the same useSimpleList
  // body, each pointed at its own endpoint. Verify each hits its endpoint and
  // surfaces the result.
  it.each([
    [useBodies, '/api/v1/bodies', 'useBodies'],
    [useWheels, '/api/v1/wheels', 'useWheels'],
    [useGliders, '/api/v1/gliders', 'useGliders'],
  ])('%s fetches its list from its endpoint', async (hook, endpoint) => {
    server.use(http.get(endpoint, () => HttpResponse.json([mario])));

    const { result } = renderHook(() => hook(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.items).toEqual([mario]);
    });
  });

  it('degrades to an empty list when the request fails', async () => {
    server.use(
      http.get(
        '/api/v1/characters',
        () => new HttpResponse(null, { status: 500 }),
      ),
    );

    const { result } = renderHook(() => useCharacters(), {
      wrapper: createWrapper(),
    });

    // The old useEffect hook swallowed failures and left the list empty;
    // TanStack Query keeps the error in its own state, so components still
    // render an empty pick-list rather than crashing.
    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.items).toEqual([]);
  });
});

const drink = (n: number) => ({
  id: `d${String(n)}`,
  name: `Drink ${String(n)}`,
  alcoholic: false,
  created_by: null,
  created_at: '2026-05-21T00:00:00.000Z',
});

describe('useDrinkTypes', () => {
  it('refetches the list when refresh() is called', async () => {
    let calls = 0;
    server.use(
      http.get('/api/v1/drink-types', () => {
        calls += 1;
        return HttpResponse.json([drink(calls)]);
      }),
    );

    const { result } = renderHook(() => useDrinkTypes(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.items[0]?.name).toBe('Drink 1');
    });

    // refresh() invalidates the cache, which triggers a refetch — the user
    // adding a custom drink type relies on this to see it appear.
    act(() => {
      result.current.refresh();
    });

    await waitFor(() => {
      expect(result.current.items[0]?.name).toBe('Drink 2');
    });
    expect(calls).toBe(2);
  });
});
