import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { RouterProvider, createMemoryRouter } from 'react-router-dom';
import type { ReactNode } from 'react';
import { AppProviders, routes } from '../App';

// PR-F1 (Issue #190): the router maps each path to its lazily-loaded page.
// This verifies the wiring for an authenticated visitor without dragging in
// each page's data dependencies: useAuth is stubbed to a signed-in user and
// every page is stubbed to a trivial marker, so the test asserts only that the
// right lazy chunk resolves under the right route (and exercises the
// named-export → React.lazy adapter for each page). The unauthenticated path
// and the real Login render live in App.test.tsx.

vi.mock('../hooks/useAuth', () => ({
  AuthProvider: ({ children }: { children: ReactNode }) => children,
  useAuth: () => ({
    user: { id: 'u1', username: 'alice' },
    isAuthenticated: true,
    isLoading: false,
    login: vi.fn(),
    register: vi.fn(),
    changePassword: vi.fn(),
    logout: vi.fn(),
  }),
}));
vi.mock('../pages/Home', () => ({ Home: () => <div>home page</div> }));
vi.mock('../pages/Profile', () => ({ Profile: () => <div>profile page</div> }));
vi.mock('../pages/Session', () => ({ Session: () => <div>session page</div> }));
vi.mock('../pages/Onboarding', () => ({
  Onboarding: () => <div>onboarding page</div>,
}));
vi.mock('../pages/Register', () => ({
  Register: () => <div>register page</div>,
}));

// Devtools touch browser APIs jsdom lacks; keep the dev-only branch off.
vi.stubEnv('DEV', false);

describe('authenticated routing', () => {
  it.each([
    ['/', 'home page'],
    ['/profile', 'profile page'],
    ['/session/abc', 'session page'],
    ['/onboarding', 'onboarding page'],
    ['/register', 'register page'],
  ])('renders the lazy page mounted at %s', async (path, marker) => {
    const router = createMemoryRouter(routes, { initialEntries: [path] });
    render(
      <AppProviders>
        <RouterProvider router={router} />
      </AppProviders>,
    );

    expect(await screen.findByText(marker)).toBeInTheDocument();
  });
});
