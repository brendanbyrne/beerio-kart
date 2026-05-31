import { http, HttpResponse } from 'msw';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { server } from '../mocks/server';
import { Session } from './Session';
import { formatTime } from '../utils/time';

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
  RunEntrySheet: ({ onSubmitted }: { onSubmitted: () => void }) => (
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
  it('redirects home when the route param is missing', async () => {
    // useParams is typed as Partial<{ id: SessionId }> regardless of the route's
    // declared path, so the component guards with `if (!id) <Navigate to="/" />`.
    // Rendering at a path that has no :id segment exercises that guard.
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={['/session']}>
          <Routes>
            <Route path="/session" element={<Session />} />
            <Route path="/" element={<div>home page</div>} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(await screen.findByText('home page')).toBeInTheDocument();
  });

  it('reflects membership and refreshes the detail when joining', async () => {
    // After the join POST, the server reports u1 as a participant; the detail
    // refetch (triggered by the invalidation) must surface that.
    const joinedSession = {
      ...otherSession,
      participants: [
        ...otherSession.participants,
        {
          user_id: 'u1',
          username: 'alice',
          joined_at: '2026-05-22T00:00:00.000Z',
          left_at: null,
        },
      ],
    };
    let joined = false;
    server.use(
      http.get('/api/v1/sessions/s1', () =>
        HttpResponse.json(joined ? joinedSession : otherSession),
      ),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: joined ? 's1' : null }),
      ),
      http.post('/api/v1/sessions/s1/join', () => {
        joined = true; // the join creates membership server-side
        return new HttpResponse(null, { status: 204 });
      }),
    );
    const user = userEvent.setup();
    const { invalidate } = renderSession();

    await user.click(
      await screen.findByRole('button', { name: /join session/i }),
    );

    // Observable outcome: the detail refetches and now shows u1 as a member, so
    // the Join button is replaced by Leave — not merely "a spy was called".
    expect(
      await screen.findByRole('button', { name: /leave session/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /join session/i }),
    ).not.toBeInTheDocument();
    // It also invalidated the membership + detail keys (the documented purpose).
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['my-session'] });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['sessions'] });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['session', 's1'] });
  });

  it('refreshes the track on next-track and skip, and navigates home on leave', async () => {
    // The server advances the current track on each mutation; the detail
    // refetch (triggered by the invalidation) must show the new track — a spy
    // on invalidateQueries would pass even if invalidation no longer refreshed.
    let currentTrack = 'Mario Circuit';
    const raceWithTrack = (track_name: string) => ({
      ...currentRace,
      track_name,
    });
    server.use(
      http.get('/api/v1/sessions/s1', () =>
        HttpResponse.json({
          ...hostSession,
          current_race: raceWithTrack(currentTrack),
        }),
      ),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: 's1' }),
      ),
      http.post('/api/v1/sessions/s1/next-track', () => {
        currentTrack = 'Rainbow Road';
        return HttpResponse.json(raceWithTrack(currentTrack));
      }),
      http.post('/api/v1/sessions/s1/skip-turn', () => {
        currentTrack = 'Bowser Castle';
        return HttpResponse.json(raceWithTrack(currentTrack));
      }),
      http.post(
        '/api/v1/sessions/s1/leave',
        () => new HttpResponse(null, { status: 204 }),
      ),
    );
    const user = userEvent.setup();
    renderSession();

    // Initial track from the first fetch.
    expect(
      await screen.findByRole('heading', { name: 'Mario Circuit' }),
    ).toBeInTheDocument();

    // Next Track → the detail refetches and the track card shows the new track.
    await user.click(
      await screen.findByRole('button', { name: /next track/i }),
    );
    expect(
      await screen.findByRole('heading', { name: 'Rainbow Road' }),
    ).toBeInTheDocument();

    // Skip Track → refetches again, showing yet another track.
    await user.click(screen.getByRole('button', { name: /skip track/i }));
    expect(
      await screen.findByRole('heading', { name: 'Bowser Castle' }),
    ).toBeInTheDocument();

    // Leave → navigates home.
    await user.click(screen.getByRole('button', { name: /leave session/i }));
    expect(await screen.findByText('home page')).toBeInTheDocument();
  });

  it('shows your submitted time after a run is submitted', async () => {
    // The real RunEntrySheet POSTs /runs; the stub skips that, so we model the
    // server-side effect by flipping the detail to include u1's submission just
    // before firing onSubmitted. The invalidation must then surface it.
    let submitted = false;
    server.use(
      http.get('/api/v1/sessions/s1', () =>
        HttpResponse.json(
          submitted ? sessionWithMySubmission(false) : hostSession,
        ),
      ),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: 's1' }),
      ),
    );
    const user = userEvent.setup();
    renderSession();

    // Open the run-entry sheet (stubbed), then fire its success callback.
    await user.click(
      await screen.findByRole('button', { name: /submit time/i }),
    );
    submitted = true;
    await user.click(screen.getByRole('button', { name: /submit-run-stub/i }));

    // Observable outcome: the detail refetches and the Submit-Time button is
    // replaced by the "Your Time" card with the formatted time.
    expect(await screen.findByText('Your Time')).toBeInTheDocument();
    expect(screen.getByText(formatTime(83456))).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /submit time/i }),
    ).not.toBeInTheDocument();
  });

  // Returns hostSession with u1's submission on the current race, in the
  // given DQ state. Drives the "Your Time" card (the hasSubmitted branch),
  // which replaces the Submit-Time button once you've logged a run.
  function sessionWithMySubmission(disqualified: boolean) {
    return {
      ...hostSession,
      current_race: {
        ...currentRace,
        submissions: [
          {
            user_id: 'u1',
            username: 'alice',
            track_time: 83456,
            disqualified,
          },
        ],
      },
    };
  }

  it('shows your finished time when you have a non-DQ submission', async () => {
    server.use(
      http.get('/api/v1/sessions/s1', () =>
        HttpResponse.json(sessionWithMySubmission(false)),
      ),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: 's1' }),
      ),
    );
    renderSession();

    // Non-DQ branch: the plain "Your Time" label (not "Your Time (DQ)") and
    // the formatted track time, rendered with the success styling.
    expect(await screen.findByText('Your Time')).toBeInTheDocument();
    expect(screen.getByText(formatTime(83456))).toBeInTheDocument();
  });

  it('shows a DQ treatment when your submission is disqualified', async () => {
    server.use(
      http.get('/api/v1/sessions/s1', () =>
        HttpResponse.json(sessionWithMySubmission(true)),
      ),
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: 's1' }),
      ),
    );
    renderSession();

    // DQ branch: the "(DQ)" label variant and the struck-through time.
    expect(await screen.findByText('Your Time (DQ)')).toBeInTheDocument();
    expect(screen.getByText(formatTime(83456))).toBeInTheDocument();
  });
});
