import { useQuery } from '@tanstack/react-query';
import { getMySession, listSessions } from '../api/sessions';

const POLL_INTERVAL_MS = 5000;

/**
 * Fetches the active session list and the user's current session ID, polling
 * every 5 seconds via TanStack Query (PR-C2).
 *
 * The legacy `Promise.all([listSessions(), getMySession()])` is split into two
 * independent queries: `['sessions']` and `['my-session']`. The split lets
 * `['my-session']` be shared with `BottomNav` — same key means one fetch,
 * deduped by TanStack Query — and lets the session mutations (create / join /
 * leave) invalidate just the affected key. `refetchIntervalInBackground: false`
 * pauses both while the tab is hidden, replacing the old Page-Visibility
 * listener.
 *
 * The legacy `{ sessions, mySessionId, loading }` shape is preserved so
 * `Home.tsx` is untouched. `loading` stays true until both queries resolve,
 * matching the old single-`Promise.all` gate.
 */
export function useSessions() {
  const sessionsQuery = useQuery({
    queryKey: ['sessions'],
    queryFn: ({ signal }) => listSessions(signal),
    refetchInterval: POLL_INTERVAL_MS,
    refetchIntervalInBackground: false,
  });

  const mySessionQuery = useQuery({
    queryKey: ['my-session'],
    queryFn: ({ signal }) => getMySession(signal),
    refetchInterval: POLL_INTERVAL_MS,
    refetchIntervalInBackground: false,
  });

  return {
    sessions: sessionsQuery.data ?? [],
    mySessionId: mySessionQuery.data ?? null,
    loading: sessionsQuery.isPending || mySessionQuery.isPending,
  };
}
