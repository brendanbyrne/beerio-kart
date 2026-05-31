import type { FallbackProps } from 'react-error-boundary';

// App-level fallback for the react-error-boundary that wraps the whole tree
// (react.md § 9). This is the last-resort catch-all: react-router's per-route
// `errorElement` (RouteErrorFallback) handles render crashes inside a route,
// so this only fires for failures outside the routed tree (a provider, the
// router itself). A full page reload is the honest recovery for an app-shell
// crash — `resetErrorBoundary` would just re-render into the same broken state.
//
// `FallbackProps.error` is untyped at the library boundary; narrow before use.
export function AppErrorFallback({ error }: FallbackProps) {
  const detail = error instanceof Error ? error.message : String(error);

  return (
    <div
      role="alert"
      className="min-h-screen flex flex-col items-center justify-center gap-4 bg-gray-50 px-4 text-center"
    >
      <h1 className="text-xl font-bold text-gray-900">Something went wrong</h1>
      <p className="text-sm text-gray-500">
        The app hit an unexpected error. Reloading usually fixes it.
      </p>
      {import.meta.env.DEV && (
        <pre className="max-w-full overflow-auto text-xs text-red-500">
          {detail}
        </pre>
      )}
      <button
        type="button"
        onClick={() => {
          window.location.reload();
        }}
        className="min-h-[44px] px-6 py-2.5 bg-blue-500 text-white font-semibold rounded-xl hover:bg-blue-600 transition-colors"
      >
        Reload
      </button>
    </div>
  );
}
