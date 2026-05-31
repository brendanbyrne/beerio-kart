// Suspense fallback for lazily-loaded route pages (react.md § 11). Shown for
// the brief moment a route's code chunk is in flight. `role="status"` +
// the visually-hidden label make the loading state audible to screen readers;
// the pulsing bars give a sighted user a content placeholder rather than a
// blank screen.
export function PageSkeleton() {
  return (
    <div
      role="status"
      aria-live="polite"
      className="min-h-screen flex flex-col items-center justify-center gap-3 bg-gray-50 px-4"
    >
      <span className="sr-only">Loading…</span>
      <div className="w-40 h-6 bg-gray-200 rounded animate-pulse" />
      <div className="w-64 h-4 bg-gray-200 rounded animate-pulse" />
      <div className="w-52 h-4 bg-gray-200 rounded animate-pulse" />
    </div>
  );
}
