import { http, HttpResponse } from 'msw';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';
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

describe('Home', () => {
  it('invalidates the membership and session-list queries after creating a session', async () => {
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
        <MemoryRouter>
          <Home />
        </MemoryRouter>
      </QueryClientProvider>,
    );

    // No active session → the primary button opens the create-session modal.
    await user.click(
      await screen.findByRole('button', { name: /start a session/i }),
    );
    await user.click(screen.getByRole('button', { name: /^random$/i }));

    await waitFor(() => {
      expect(invalidate).toHaveBeenCalledWith({ queryKey: ['my-session'] });
    });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['sessions'] });
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
