import { Link, isRouteErrorResponse, useRouteError } from 'react-router-dom';

// Route-scoped errorElement (react.md § 9, § 11). react-router renders this in
// place of a route's element when that element throws during render, catching
// the crash *before* it bubbles to the app-level AppErrorFallback. That keeps
// failures isolated to one route — a busted page can't blank the whole app —
// and gives the user a way out via the "Go home" link rather than a dead end.
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
        <pre className="max-w-full overflow-auto text-xs text-red-500">
          {detail}
        </pre>
      )}
      <Link
        to="/"
        className="min-h-[44px] flex items-center px-6 py-2.5 bg-blue-500 text-white font-semibold rounded-xl hover:bg-blue-600 transition-colors"
      >
        Go home
      </Link>
    </div>
  );
}
