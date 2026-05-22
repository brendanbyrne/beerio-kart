import { useCallback } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from '../api/client';
import { parseBody } from '../api/result';
import { UserDetailProfileSchema } from '../api/types';
import type { UserDetailProfile } from '../api/types';

/**
 * Fetch a user's detail profile via TanStack Query. The legacy
 * `{ profile, loading, refresh }` shape is preserved so call sites (Home,
 * Profile) are untouched: `profile` defaults to `null` until the query
 * resolves, and `refresh()` invalidates the cache instead of bumping a
 * version counter.
 *
 * `userId` may be undefined while the auth context is still resolving the
 * signed-in user; the query is disabled until it's known, and stays in its
 * pending state (so `loading` is `true`) in the meantime — matching the
 * legacy hook, which left `loading` true and never fetched without an id.
 */
export function useUserProfile(userId: string | undefined): {
  profile: UserDetailProfile | null;
  loading: boolean;
  refresh: () => void;
} {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: ['user-profile', userId],
    queryFn: async ({ signal }) => {
      // `enabled` guarantees this only runs with a defined id; the guard
      // narrows the type and is defensive against a future caller that
      // forgets the gate.
      if (userId === undefined) throw new Error('userId is required');
      const res = await apiFetch(`/api/v1/users/${userId}`, { signal });
      return parseBody(UserDetailProfileSchema, res);
    },
    enabled: userId !== undefined,
  });

  const refresh = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: ['user-profile', userId] });
  }, [queryClient, userId]);

  return { profile: query.data ?? null, loading: query.isPending, refresh };
}
