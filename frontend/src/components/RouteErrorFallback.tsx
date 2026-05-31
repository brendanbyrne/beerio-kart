import { Link, isRouteErrorResponse, useRouteError } from 'react-router-dom';

// Route-scoped errorElement (react.md § 9, § 11). react-router renders this in
// place of a route's element when that element throws during render, catching
// the crash *before* it bubbles to the app-level AppErrorFallback. That keeps
// failures isolated to one route — a busted page can't blank the whole app.
//
// Two recovery paths, and the order matters. With code-split routes the most
// common real trigger here isn't a logic crash — it's a dynamic-import failure
// after a redeploy: a user on a stale index.html navigates to a route whose
// chunk hash no longer exists, lazy() throws, and we land here. "Go home" is a
// client-side <Link> that re-hits the same stale manifest and can loop, so the
// primary action is a hard Reload (re-fetches index.html + a fresh manifest —
// the reliable fix for the chunk-load case); "Go home" stays as the secondary
// way out for an actual render crash on one page.
export function RouteErrorFallback() {
  const error = useRouteError();

  // A route Response (e.g. a 404 thrown by a future loader) carries a status;
  // a render-time crash is a thrown Error. Everything else stringifies.
  const detail = isRouteErrorResponse(error)
    ? `${String(error.status)} ${error.statusText}`
    : error instanceof Error
      ? error.message
      : String(error);

  return (
    <div
      role="alert"
      className="min-h-screen flex flex-col items-center justify-center gap-4 bg-gray-50 px-4 text-center"
    >
      <h1 className="text-xl font-bold text-gray-900">
        This page ran into a problem
      </h1>
      <p className="text-sm text-gray-500">
        Something on this screen failed to load. The rest of the app is fine.
      </p>
      {import.meta.env.DEV && (
        <pre className="max-w-full overflow-auto text-xs text-danger">
          {detail}
        </pre>
      )}
      <div className="flex flex-col items-center gap-3">
        <button
          type="button"
          onClick={() => {
            window.location.reload();
          }}
          className="min-h-touch px-6 py-2.5 bg-brand-primary text-white font-semibold rounded-xl hover:bg-brand-primary-strong transition-colors"
        >
          Reload
        </button>
        <Link
          to="/"
          className="min-h-touch flex items-center text-sm text-brand-primary hover:underline font-medium"
        >
          Go home
        </Link>
      </div>
    </div>
  );
}
