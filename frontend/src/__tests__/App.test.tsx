import { http, HttpResponse } from 'msw';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { RouterProvider, createMemoryRouter } from 'react-router-dom';
import { ErrorBoundary } from 'react-error-boundary';
import { server } from '../mocks/server';
import { App, AppProviders, routes } from '../App';
import { AppErrorFallback } from '../components/AppErrorFallback';
import { RouteErrorFallback } from '../components/RouteErrorFallback';

// App shell wired up across PR-C1 (provider tree) and PR-F1 (Issue #190 —
// createBrowserRouter + RouterProvider, lazy routes, app-level + per-route
// error boundaries). The full stack — ErrorBoundary → QueryClientProvider →
// AuthProvider → AuthGate → RouterProvider — must compose and mount, an
// unauthenticated visitor must land on the login screen, unknown paths must
// route through the catch-all, and a render crash in a route must be caught by
// that route's scoped fallback rather than the global one. MSW mocks the
// network at the fetch boundary (react.md § 13).

// Force the dev-only React Query Devtools branch off: the panel touches
// browser APIs jsdom doesn't implement, and the test only needs the shell to
// mount. The `import.meta.env.DEV && …` line still evaluates either way.
vi.stubEnv('DEV', false);

function unauthenticated() {
  server.use(
    http.post(
      '/api/v1/auth/refresh',
      () => new HttpResponse(null, { status: 401 }),
    ),
  );
}

afterEach(() => {
  vi.unstubAllEnvs();
});

describe('App shell', () => {
  it('renders the login screen for an unauthenticated visitor', async () => {
    unauthenticated();

    render(<App />);

    expect(
      await screen.findByRole('button', { name: /log in/i }),
    ).toBeInTheDocument();
  });

  it('routes unknown paths through the catch-all instead of rendering blank', async () => {
    // Paths that don't match any declared route (e.g. /session with no :id
    // segment) redirect to "/" rather than dead-ending. With no auth the /
    // route bounces to /login, so the user-visible end state is the login
    // screen. Driven through a memory router so the initial path is explicit
    // (the module-scope browser router shares real history across renders).
    unauthenticated();

    const router = createMemoryRouter(routes, {
      initialEntries: ['/session'],
    });
    render(
      <AppProviders>
        <RouterProvider router={router} />
      </AppProviders>,
    );

    expect(
      await screen.findByRole('button', { name: /log in/i }),
    ).toBeInTheDocument();
  });

  it('attaches the route-scoped errorElement to every top-level route', () => {
    expect(routes.length).toBeGreaterThan(0);
    for (const r of routes) {
      // toEqual, not toBeDefined: pin that it's specifically RouteErrorFallback
      // (route-scoped), so a regression wiring the global AppErrorFallback — or
      // any other element — onto a route fails here. React elements compare
      // structurally under toEqual.
      expect(r.errorElement).toEqual(<RouteErrorFallback />);
    }
  });

  it('catches a route render crash with the route-scoped fallback, not the global one', async () => {
    // A thrown render error inside a route element must be caught by that
    // route's errorElement (RouteErrorFallback) — the app-level ErrorBoundary
    // (AppErrorFallback) stays dormant. This is the layering the app relies on
    // so one broken page can't blank the whole tree.
    const errorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);

    function Boom(): never {
      throw new Error('kaboom');
    }
    const router = createMemoryRouter(
      [{ path: '/', element: <Boom />, errorElement: <RouteErrorFallback /> }],
      { initialEntries: ['/'] },
    );
    render(
      <ErrorBoundary FallbackComponent={AppErrorFallback}>
        <RouterProvider router={router} />
      </ErrorBoundary>,
    );

    expect(
      await screen.findByText(/this page ran into a problem/i),
    ).toBeInTheDocument();
    expect(screen.queryByText(/something went wrong/i)).not.toBeInTheDocument();

    errorSpy.mockRestore();
  });
});
