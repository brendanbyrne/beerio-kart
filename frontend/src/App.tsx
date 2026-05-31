import { Suspense, lazy } from 'react';
import type { ReactNode } from 'react';
import { QueryClientProvider } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import {
  Navigate,
  RouterProvider,
  createBrowserRouter,
} from 'react-router-dom';
import type { RouteObject } from 'react-router-dom';
import { ErrorBoundary } from 'react-error-boundary';
import { createQueryClient } from './api/queryClient';
import { AuthProvider, useAuth } from './hooks/useAuth';
import { PageSkeleton } from './components/PageSkeleton';
import { AppErrorFallback } from './components/AppErrorFallback';
import { RouteErrorFallback } from './components/RouteErrorFallback';

// Route pages are code-split (react.md § 11): each lazy() import becomes its
// own chunk, so the initial bundle ships only the app shell + whatever route
// the user landed on. The `.then(m => ({ default: m.X }))` adapter bridges our
// named exports (typescript.md § 5) to React.lazy's default-export contract.
const Login = lazy(() =>
  import('./pages/Login').then((m) => ({ default: m.Login })),
);
const Register = lazy(() =>
  import('./pages/Register').then((m) => ({ default: m.Register })),
);
const Onboarding = lazy(() =>
  import('./pages/Onboarding').then((m) => ({ default: m.Onboarding })),
);
const Home = lazy(() =>
  import('./pages/Home').then((m) => ({ default: m.Home })),
);
const Profile = lazy(() =>
  import('./pages/Profile').then((m) => ({ default: m.Profile })),
);
const Session = lazy(() =>
  import('./pages/Session').then((m) => ({ default: m.Session })),
);

// One QueryClient for the whole app, created at module scope so it survives
// re-renders (a client recreated in render would drop the cache every time).
// Config + the contract-drift logging live in createQueryClient (api layer).
const queryClient = createQueryClient();

/** Shows a loading spinner while the initial silent refresh is in progress. */
function AuthGate({ children }: { children: ReactNode }) {
  const { isLoading } = useAuth();

  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-50">
        <p className="text-gray-400">Loading...</p>
      </div>
    );
  }

  return children;
}

/** Redirects to /login if not authenticated. */
function RequireAuth({ children }: { children: ReactNode }) {
  const { isAuthenticated } = useAuth();
  if (!isAuthenticated) return <Navigate to="/login" replace />;
  return children;
}

/** Redirects to / if already authenticated. */
function GuestOnly({ children }: { children: ReactNode }) {
  const { isAuthenticated } = useAuth();
  if (isAuthenticated) return <Navigate to="/" replace />;
  return children;
}

/** Wraps a lazy page in the shared Suspense fallback (react.md § 11). */
function page(node: ReactNode): ReactNode {
  return <Suspense fallback={<PageSkeleton />}>{node}</Suspense>;
}

/**
 * Builds a route with the shared route-scoped errorElement attached, so a
 * render crash in one page shows RouteErrorFallback in place of that page
 * (caught here, before it reaches the app-level boundary in AppProviders).
 */
function route(path: string, element: ReactNode): RouteObject {
  return { path, element, errorElement: <RouteErrorFallback /> };
}

// eslint-disable-next-line react-refresh/only-export-components -- `routes` is a static route table consumed by createBrowserRouter (and by tests, which build a memory router from it), not a fast-refreshable component. Mirrors the useAuth.tsx precedent for a justified react-refresh opt-out.
export const routes: RouteObject[] = [
  route('/login', <GuestOnly>{page(<Login />)}</GuestOnly>),
  route('/register', page(<Register />)),
  route('/onboarding', <RequireAuth>{page(<Onboarding />)}</RequireAuth>),
  route('/profile', <RequireAuth>{page(<Profile />)}</RequireAuth>),
  route('/session/:id', <RequireAuth>{page(<Session />)}</RequireAuth>),
  route('/', <RequireAuth>{page(<Home />)}</RequireAuth>),
  // Catch-all: unknown paths (e.g. /session with no :id) redirect home rather
  // than dead-ending on a 404. Per design.md ("never a burden", "sensible
  // defaults"), bouncing a mistyped URL to the home screen beats a 404 page.
  // This is distinct from the errorElement above, which handles render crashes
  // on a *matched* route — unmatched-path handling and crash handling are
  // separate concerns.
  route('*', <Navigate to="/" replace />),
];

const router = createBrowserRouter(routes);

/**
 * The provider stack that wraps the router. Exported so tests can mount it
 * around a `createMemoryRouter(routes, …)` for a specific initial path — the
 * module-scope `router` above uses real browser history, which is a shared
 * singleton and awkward to drive per-test.
 *
 * ErrorBoundary is outermost: it's the app-level catch-all for crashes outside
 * the routed tree (a provider, the router itself). Render crashes *inside* a
 * route are caught earlier by that route's errorElement (RouteErrorFallback).
 */
export function AppProviders({ children }: { children: ReactNode }) {
  return (
    <ErrorBoundary FallbackComponent={AppErrorFallback}>
      <QueryClientProvider client={queryClient}>
        <AuthProvider>
          <AuthGate>{children}</AuthGate>
        </AuthProvider>
        {import.meta.env.DEV && <ReactQueryDevtools initialIsOpen={false} />}
      </QueryClientProvider>
    </ErrorBoundary>
  );
}

export function App() {
  return (
    <AppProviders>
      <RouterProvider router={router} />
    </AppProviders>
  );
}
