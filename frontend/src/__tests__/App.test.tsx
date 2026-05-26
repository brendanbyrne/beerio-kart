import { http, HttpResponse } from 'msw';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { server } from '../mocks/server';
import { App } from '../App';

// Smoke test for the app shell wired up in PR-C1 (Issue #176): the full
// provider tree — QueryClientProvider → BrowserRouter → AuthProvider — must
// compose and mount. With no valid refresh cookie the silent refresh 401s,
// AuthGate finishes loading, and an unauthenticated visitor is redirected to
// the login screen. MSW mocks the network at the fetch boundary (react.md
// § 13).

// Force the dev-only React Query Devtools branch off: the panel touches
// browser APIs jsdom doesn't implement, and the test only needs the shell to
// mount. The `import.meta.env.DEV && …` line still evaluates either way.
vi.stubEnv('DEV', false);

afterEach(() => {
  vi.unstubAllEnvs();
});

describe('App shell', () => {
  it('renders the login screen for an unauthenticated visitor', async () => {
    server.use(
      http.post(
        '/api/v1/auth/refresh',
        () => new HttpResponse(null, { status: 401 }),
      ),
    );

    render(<App />);

    expect(
      await screen.findByRole('button', { name: /log in/i }),
    ).toBeInTheDocument();
  });

  it('routes unknown paths through the catch-all instead of rendering blank', async () => {
    // PR-D2 / Issue #192 added <Route path="*" /> so paths that don't match
    // any declared route (e.g. /session with no :id segment) redirect to "/"
    // rather than rendering an empty <Routes>. With no auth, the / route
    // bounces to /login, so the user-visible end state is the login screen.
    server.use(
      http.post(
        '/api/v1/auth/refresh',
        () => new HttpResponse(null, { status: 401 }),
      ),
    );

    window.history.pushState({}, '', '/session');
    render(<App />);

    expect(
      await screen.findByRole('button', { name: /log in/i }),
    ).toBeInTheDocument();
  });
});
