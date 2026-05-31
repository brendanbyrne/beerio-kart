import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RouterProvider, createMemoryRouter } from 'react-router-dom';
import { RouteErrorFallback } from './RouteErrorFallback';

// RouteErrorFallback is the per-route errorElement (react.md § 9, § 11). It
// reads the error via useRouteError, so it can only render inside a data
// router's error path — these tests drive it through createMemoryRouter.

// react-router logs caught route errors to console.error; silence the expected
// noise so the suite output stays readable.
const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => undefined);
const originalLocation = window.location;

afterEach(() => {
  errorSpy.mockClear();
  vi.unstubAllEnvs();
  Object.defineProperty(window, 'location', {
    configurable: true,
    value: originalLocation,
  });
});

describe('RouteErrorFallback', () => {
  it('shows a route-scoped message with a way back home when a route crashes', async () => {
    function Boom(): never {
      throw new Error('render crash');
    }
    const router = createMemoryRouter(
      [{ path: '/', element: <Boom />, errorElement: <RouteErrorFallback /> }],
      { initialEntries: ['/'] },
    );

    render(<RouterProvider router={router} />);

    expect(
      await screen.findByRole('heading', {
        name: /this page ran into a problem/i,
      }),
    ).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /go home/i })).toHaveAttribute(
      'href',
      '/',
    );
  });

  it('hard-reloads the page when Reload is pressed', async () => {
    // Reload is the primary recovery for the chunk-load-after-redeploy case: a
    // full reload re-fetches index.html + a fresh manifest, unlike the "Go
    // home" client-side nav.
    const reload = vi.fn();
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { reload },
    });
    function Boom(): never {
      throw new Error('Failed to fetch dynamically imported module');
    }
    const router = createMemoryRouter(
      [{ path: '/', element: <Boom />, errorElement: <RouteErrorFallback /> }],
      { initialEntries: ['/'] },
    );
    const user = userEvent.setup();

    render(<RouterProvider router={router} />);
    await user.click(await screen.findByRole('button', { name: /reload/i }));

    expect(reload).toHaveBeenCalledOnce();
  });

  it('surfaces the status when a route throws an error Response', async () => {
    // The status detail renders only in the dev build (we don't leak error
    // internals to users in prod); pin DEV on so the branch is observable.
    vi.stubEnv('DEV', true);
    const router = createMemoryRouter(
      [
        {
          path: '/',
          loader: () => {
            // react-router treats a thrown Response as a route error response
            // (the loader 404 pattern); only-throw-error doesn't know that.
            // eslint-disable-next-line @typescript-eslint/only-throw-error
            throw new Response('Nope', {
              status: 404,
              statusText: 'Not Found',
            });
          },
          element: <div>unreachable</div>,
          errorElement: <RouteErrorFallback />,
        },
      ],
      { initialEntries: ['/'] },
    );

    render(<RouterProvider router={router} />);

    // The dev-only detail line renders the status of the route error response.
    expect(await screen.findByText(/404 Not Found/)).toBeInTheDocument();
  });
});
