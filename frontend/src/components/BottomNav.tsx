import { useQuery } from '@tanstack/react-query';
import { NavLink, useLocation } from 'react-router-dom';
import { clsx } from 'clsx';
import { getMySession } from '../api/sessions';

const TAB_BASE =
  'flex-1 flex flex-col items-center py-2 min-h-[52px] transition-colors';

// NavLink drives the active styling and sets aria-current="page" itself
// (react.md § 11); this only supplies the colors per active state.
function tabClass(isActive: boolean): string {
  return clsx(
    TAB_BASE,
    isActive ? 'text-brand-primary' : 'text-gray-400 hover:text-gray-600',
  );
}

export function BottomNav() {
  const location = useLocation();

  // Shares the ['my-session'] key with useSessions, so the two dedupe to one
  // fetch and the session mutations' invalidation keeps this tab in sync —
  // replacing the old re-fetch-on-navigation useEffect (PR-C2).
  //
  // Deliberately no refetchInterval here: on Home, useSessions polls this key
  // at 5s; on Session/Profile this stays fresh via mount, local-mutation
  // invalidation, and refetchOnWindowFocus (on by default). The old effect
  // only re-fetched on navigation — never on a timer — so a sit-idle tab
  // missing another participant's close is not a regression, and refocus
  // covers it.
  const { data: mySessionId } = useQuery({
    queryKey: ['my-session'],
    queryFn: ({ signal }) => getMySession(signal),
  });

  const sessionMatch = /^\/session\/(.+)$/.exec(location.pathname);
  // When viewing a session, link to the current one; otherwise deep-link to the
  // active session if one exists. Null means no session to reach → disabled.
  const sessionPath = sessionMatch
    ? location.pathname
    : mySessionId
      ? `/session/${mySessionId}`
      : null;

  return (
    <nav className="fixed bottom-0 left-0 right-0 bg-white border-t border-gray-200 safe-area-pb">
      <div className="flex max-w-lg mx-auto">
        {/* `end` so "/" is active only on the home route, not every path. */}
        <NavLink to="/" end className={({ isActive }) => tabClass(isActive)}>
          <span className="text-xl leading-none">{'🏠'}</span>
          <span className="text-[10px] font-medium mt-0.5">Home</span>
        </NavLink>

        {sessionPath ? (
          <NavLink
            to={sessionPath}
            className={({ isActive }) => tabClass(isActive)}
          >
            <span className="text-xl leading-none">{'🎮'}</span>
            <span className="text-[10px] font-medium mt-0.5">Session</span>
          </NavLink>
        ) : (
          // No reachable session yet: a NavLink has no disabled state, so the
          // inert tab is a disabled <button> (skipped by keyboard, greyed out).
          <button
            type="button"
            disabled
            className={clsx(TAB_BASE, 'text-gray-300 cursor-not-allowed')}
          >
            <span className="text-xl leading-none">{'🎮'}</span>
            <span className="text-[10px] font-medium mt-0.5">Session</span>
          </button>
        )}

        <NavLink to="/profile" className={({ isActive }) => tabClass(isActive)}>
          <span className="text-xl leading-none">{'👤'}</span>
          <span className="text-[10px] font-medium mt-0.5">Profile</span>
        </NavLink>
      </div>
    </nav>
  );
}
