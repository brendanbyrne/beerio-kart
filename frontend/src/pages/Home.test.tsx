import { http, HttpResponse } from 'msw';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes, useParams } from 'react-router-dom';
import { server } from '../mocks/server';
import { Home } from './Home';

// PR-C2 (Issue #186): creating a session must invalidate the membership and
// session-list queries so the bottom-nav and Home list update without waiting
// for the next poll. The test spies on the QueryClient the provider hands the
// component and asserts the invalidation fires after the create succeeds.
// useAuth is mocked to a fixed user (the Login/Onboarding test pattern).
vi.mock('../hooks/useAuth', () => ({
  useAuth: () => ({ user: { id: 'u1', username: 'alice' } }),
}));

const profile = {
  id: 'u1',
  username: 'alice',
  preferred_character_id: null,
  preferred_body_id: null,
  preferred_wheel_id: null,
  preferred_glider_id: null,
  preferred_drink_type: null,
  created_at: '2026-05-21T00:00:00.000Z',
};

const createdSession = {
  id: 's1',
  host_id: 'u1',
  host_username: 'alice',
  ruleset: 'random',
  status: 'active',
  created_at: '2026-05-22T00:00:00.000Z',
  participants: [],
  race_number: 0,
  current_race: null,
  races: [],
};

// Minimal stand-in for the session route, so the create flow's navigation is
// observable and we can read back the id it navigated to.
function SessionRouteProbe() {
  const { id } = useParams();
  return <div>session route: {id}</div>;
}

describe('Home', () => {
  it('navigates to the new session and refreshes membership/list after creating one', async () => {
    server.use(
      http.get('/api/v1/users/u1', () => HttpResponse.json(profile)),
      http.get('/api/v1/characters', () => HttpResponse.json([])),
      http.get('/api/v1/sessions', () => HttpResponse.json([])),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: null }),
      ),
      http.post('/api/v1/sessions', () => HttpResponse.json(createdSession)),
    );

    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');
    const user = userEvent.setup();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={['/']}>
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/session/:id" element={<SessionRouteProbe />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    // No active session → the primary button opens the create-session modal.
    await user.click(
      await screen.findByRole('button', { name: /start a session/i }),
    );
    await user.click(screen.getByRole('button', { name: /^random$/i }));

    // Observable outcome: a successful create navigates to the new session's
    // page — not merely "invalidateQueries was called".
    expect(await screen.findByText('session route: s1')).toBeInTheDocument();
    // It also invalidated the membership + session-list keys so the bottom-nav
    // and Home list refresh on return instead of waiting for the next poll —
    // the documented purpose of the invalidation (#186).
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['my-session'] });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['sessions'] });
  });

  it('jumps straight to the current session when the user already has one', async () => {
    // With an active membership the primary button becomes "Jump to Current
    // Session" and navigates directly, skipping the create modal.
    server.use(
      http.get('/api/v1/users/u1', () => HttpResponse.json(profile)),
      http.get('/api/v1/characters', () => HttpResponse.json([])),
      http.get('/api/v1/sessions', () => HttpResponse.json([])),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: 's1' }),
      ),
    );
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const user = userEvent.setup();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={['/']}>
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/session/:id" element={<SessionRouteProbe />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.click(
      await screen.findByRole('button', { name: /jump to current session/i }),
    );
    expect(await screen.findByText('session route: s1')).toBeInTheDocument();
  });

  it('navigates to a session picked from the active-sessions list', async () => {
    server.use(
      http.get('/api/v1/users/u1', () => HttpResponse.json(profile)),
      http.get('/api/v1/characters', () => HttpResponse.json([])),
      http.get('/api/v1/sessions', () =>
        HttpResponse.json([
          {
            id: 's2',
            host_username: 'bob',
            participant_count: 2,
            race_number: 3,
            ruleset: 'random',
          },
        ]),
      ),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: null }),
      ),
    );
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const user = userEvent.setup();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={['/']}>
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/session/:id" element={<SessionRouteProbe />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.click(
      await screen.findByRole('button', { name: /bob's session/i }),
    );
    expect(await screen.findByText('session route: s2')).toBeInTheDocument();
  });

  it('closes the create-session modal on Escape and restores focus to the trigger', async () => {
    // PR-G2 (Issue #184): the create-session modal is a real dialog — Escape
    // closes it and focus returns to the button that opened it (react.md § 10).
    server.use(
      http.get('/api/v1/users/u1', () => HttpResponse.json(profile)),
      http.get('/api/v1/characters', () => HttpResponse.json([])),
      http.get('/api/v1/sessions', () => HttpResponse.json([])),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: null }),
      ),
    );
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const user = userEvent.setup();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>
          <Home />
        </MemoryRouter>
      </QueryClientProvider>,
    );

    const trigger = await screen.findByRole('button', {
      name: /start a session/i,
    });
    await user.click(trigger);
    expect(await screen.findByRole('dialog')).toBeInTheDocument();

    fireEvent.keyDown(document, { key: 'Escape' });

    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    });
    expect(trigger).toHaveFocus();
  });
});
