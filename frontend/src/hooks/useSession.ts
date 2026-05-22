import { useQuery } from '@tanstack/react-query';
import { getSession } from '../api/sessions';
import type { SessionId } from '../api/brand';

const POLL_INTERVAL_MS = 2500;

/**
 * Polls `GET /sessions/:id` every 2.5 seconds via TanStack Query (PR-C2).
 *
 * `refetchInterval` returns `false` once the session is over — `getSession`
 * resolves to `null` on a 404 and the closed session carries
 * `status === 'closed'` (there is no `ended_at` field; this is the real
 * contract, not the plan's illustrative example). `refetchIntervalInBackground:
 * false` pauses polling while the tab is hidden, replacing the old
 * Page-Visibility listener; on return TanStack Query fires a single catch-up
 * fetch rather than backfilling every missed tick.
 *
 * The legacy `{ session, loading, ended }` shape is preserved so `Session.tsx`
 * is untouched.
 */
export function useSession(id: SessionId) {
  const query = useQuery({
    queryKey: ['session', id],
    queryFn: ({ signal }) => getSession(id, signal),
    refetchInterval: (q) =>
      q.state.data == null || q.state.data.status === 'closed'
        ? false
        : POLL_INTERVAL_MS,
    refetchIntervalInBackground: false,
  });

  const data = query.data;
  return {
    session: data ?? null,
    loading: query.isPending,
    ended: data === null || data?.status === 'closed',
  };
}
