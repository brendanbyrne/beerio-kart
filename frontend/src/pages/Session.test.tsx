import { http, HttpResponse } from 'msw';
import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { server } from '../mocks/server';
import Session from './Session';

// PR-C2 (Issue #186): the membership/state mutations must invalidate the keys
// they make stale — join/leave touch ['my-session'], ['sessions'] and the
// session detail; next-track/skip-turn touch only the session detail. The
// tests spy on the provider's QueryClient and assert each action invalidates
// the right keys. useAuth is mocked to a fixed user (the Login/Onboarding
// pattern); the session shape comes from MSW.
vi.mock('../hooks/useAuth', () => ({
  useAuth: () => ({ user: { id: 'u1', username: 'alice' } }),
}));

// Stub the run-entry sheet down to a button that fires its onSubmitted
// callback — the full form is exercised in RunEntrySheet's own test; here we
// only need the success signal to verify Session invalidates the detail.
vi.mock('../components/RunEntrySheet', () => ({
  default: ({ onSubmitted }: { onSubmitted: () => void }) => (
    <button onClick={onSubmitted}>submit-run-stub</button>
  ),
}));

const currentRace = {
  id: 'r1',
  race_number: 1,
  track_id: 1,
  track_name: 'Mario Circuit',
  cup_name: 'Mushroom Cup',
  image_path: 'tracks/mario-circuit.png',
  created_at: '2026-05-22T00:00:00.000Z',
  submissions: [],
};

const baseSession = {
  id: 's1',
  ruleset: 'random',
  status: 'active',
  created_at: '2026-05-22T00:00:00.000Z',
  race_number: 1,
};

// u1 is the host and a participant, with a current race in progress.
const hostSession = {
  ...baseSession,
  host_id: 'u1',
  host_username: 'alice',
  participants: [
    {
      user_id: 'u1',
      username: 'alice',
      joined_at: '2026-05-22T00:00:00.000Z',
      left_at: null,
    },
  ],
  current_race: currentRace,
  // `races` is parsed with RaceInfoSchema (needs run_count), which is a
  // different shape from current_race's SessionRaceInfo — keep it empty.
  races: [],
};

// u1 is not a participant — bob's session, so u1 sees the Join button.
const otherSession = {
  ...baseSession,
  host_id: 'u2',
  host_username: 'bob',
  participants: [
    {
      user_id: 'u2',
      username: 'bob',
      joined_at: '2026-05-22T00:00:00.000Z',
      left_at: null,
    },
  ],
  current_race: null,
  races: [],
};

function renderSession() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const invalidate = vi.spyOn(queryClient, 'invalidateQueries');
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={['/session/s1']}>
        <Routes>
          <Route path="/session/:id" element={<Session />} />
          <Route path="/" element={<div>home page</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return { invalidate };
}

describe('Session', () => {
  it('invalidates membership and session keys when joining', async () => {
    server.use(
      http.get('/api/v1/sessions/s1', () => HttpResponse.json(otherSession)),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: null }),
      ),
      http.post(
        '/api/v1/sessions/s1/join',
        () => new HttpResponse(null, { status: 204 }),
      ),
    );
    const user = userEvent.setup();
    const { invalidate } = renderSession();

    await user.click(
      await screen.findByRole('button', { name: /join session/i }),
    );

    await waitFor(() => {
      expect(invalidate).toHaveBeenCalledWith({ queryKey: ['my-session'] });
    });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['sessions'] });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['session', 's1'] });
  });

  it('invalidates the session detail on next-track and skip-track, and membership on leave', async () => {
    server.use(
      http.get('/api/v1/sessions/s1', () => HttpResponse.json(hostSession)),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: 's1' }),
      ),
      http.post('/api/v1/sessions/s1/next-track', () =>
        HttpResponse.json(currentRace),
      ),
      http.post('/api/v1/sessions/s1/skip-turn', () =>
        HttpResponse.json(currentRace),
      ),
      http.post(
        '/api/v1/sessions/s1/leave',
        () => new HttpResponse(null, { status: 204 }),
      ),
    );
    const user = userEvent.setup();
    const { invalidate } = renderSession();

    await user.click(
      await screen.findByRole('button', { name: /next track/i }),
    );
    await waitFor(() => {
      expect(invalidate).toHaveBeenCalledWith({ queryKey: ['session', 's1'] });
    });

    invalidate.mockClear();
    await user.click(screen.getByRole('button', { name: /skip track/i }));
    await waitFor(() => {
      expect(invalidate).toHaveBeenCalledWith({ queryKey: ['session', 's1'] });
    });

    await user.click(screen.getByRole('button', { name: /leave session/i }));
    await waitFor(() => {
      expect(invalidate).toHaveBeenCalledWith({ queryKey: ['my-session'] });
    });
    // Leaving navigates home.
    expect(await screen.findByText('home page')).toBeInTheDocument();
  });

  it('invalidates the session detail after a run is submitted', async () => {
    server.use(
      http.get('/api/v1/sessions/s1', () => HttpResponse.json(hostSession)),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: 's1' }),
      ),
    );
    const user = userEvent.setup();
    const { invalidate } = renderSession();

    // Opening the run-entry sheet, then submitting (the stub fires onSubmitted).
    await user.click(
      await screen.findByRole('button', { name: /submit time/i }),
    );
    invalidate.mockClear();
    await user.click(screen.getByRole('button', { name: /submit-run-stub/i }));

    await waitFor(() => {
      expect(invalidate).toHaveBeenCalledWith({ queryKey: ['session', 's1'] });
    });
  });
});
