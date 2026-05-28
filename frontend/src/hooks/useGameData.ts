import { useQuery, useQueryClient } from '@tanstack/react-query';
import * as z from 'zod';
import { apiFetch } from '../api/client';
import { parseBody } from '../api/result';
import { DrinkTypeSchema, SimpleItemSchema } from '../api/types';
import type { DrinkType, SimpleItem } from '../api/types';

/**
 * Reference data (characters / bodies / wheels / gliders / drink types) is
 * effectively static within a session: the backend seeds it and it changes
 * only when an admin edits it. Cache it for an hour and never poll. Drink
 * types are the one mutable case — a user can add a custom one — so its hook
 * exposes a `refresh()` that invalidates the cache (see `useDrinkTypes`).
 */
const STATIC_STALE_TIME = 60 * 60 * 1000;

/**
 * Fetch a simple game-data pick-list via TanStack Query. `queryKey` is the
 * stable cache key (react.md § 4); same key across components means one shared
 * fetch. The legacy `{ items, loading }` shape is preserved so call sites are
 * untouched: `items` defaults to `[]` until the query resolves, and a fetch or
 * schema failure degrades to an empty list (TanStack Query holds the error in
 * its own state rather than throwing — the pick-lists render empty, matching
 * the old useEffect hooks' behavior).
 */
function useSimpleList(queryKey: string, endpoint: string) {
  const query = useQuery({
    queryKey: [queryKey],
    queryFn: async ({ signal }) => {
      const res = await apiFetch(endpoint, { signal });
      return parseBody(z.array(SimpleItemSchema), res);
    },
    staleTime: STATIC_STALE_TIME,
  });

  return { items: query.data ?? [], loading: query.isPending };
}

export function useCharacters(): { items: SimpleItem[]; loading: boolean } {
  return useSimpleList('characters', '/api/v1/characters');
}

export function useBodies(): { items: SimpleItem[]; loading: boolean } {
  return useSimpleList('bodies', '/api/v1/bodies');
}

export function useWheels(): { items: SimpleItem[]; loading: boolean } {
  return useSimpleList('wheels', '/api/v1/wheels');
}

export function useGliders(): { items: SimpleItem[]; loading: boolean } {
  return useSimpleList('gliders', '/api/v1/gliders');
}

export function useDrinkTypes(): {
  items: DrinkType[];
  loading: boolean;
  refresh: () => void;
} {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: ['drink-types'],
    queryFn: async ({ signal }) => {
      const res = await apiFetch('/api/v1/drink-types', { signal });
      return parseBody(z.array(DrinkTypeSchema), res);
    },
    staleTime: STATIC_STALE_TIME,
  });

  // A user adding a custom drink type invalidates the cache so the list
  // refetches — replaces the version-counter `useEffect` re-run from the
  // legacy hook.
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ['drink-types'] });
  };

  return { items: query.data ?? [], loading: query.isPending, refresh };
}
