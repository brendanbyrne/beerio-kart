import { http, HttpResponse } from 'msw';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { server } from '../mocks/server';
import { RunEntrySheet } from './RunEntrySheet';
import { RaceId, TrackId } from '../api/brand';
import type { SessionRaceInfo } from '../api/types';

// PR-C2 (Issue #186) moved RunEntrySheet's defaults-loading useEffect to a
// useQuery(['run-defaults']) and layers the pre-fill in via `picked ?? default`
// rather than copying it into state. Per react.md § 13 the test mocks every
// endpoint the sheet hits with MSW and asserts the user-visible pre-fill:
// defaults populate the drink + setup, and a failed defaults request degrades
// the fields to blank (same end state as the old `source: 'none'` fallback).

// Stub RaceSetupPicker to a button that fires onComplete with a known setup —
// its own multi-step flow is tested in its own file; here we only need the
// completion signal to verify the user's pick overrides the default setup.
vi.mock('./RaceSetupPicker', () => ({
  RaceSetupPicker: ({
    onComplete,
  }: {
    onComplete: (s: {
      characterId: number;
      bodyId: number;
      wheelId: number;
      gliderId: number;
    }) => void;
  }) => (
    <button
      onClick={() => {
        onComplete({ characterId: 2, bodyId: 2, wheelId: 2, gliderId: 2 });
      }}
    >
      complete-setup-stub
    </button>
  ),
}));

function Wrapper({ children }: { children: ReactNode }) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

const race: SessionRaceInfo = {
  id: RaceId('r1'),
  race_number: 1,
  track_id: TrackId(1),
  track_name: 'Mario Circuit',
  cup_name: 'Mushroom Cup',
  image_path: 'tracks/mario-circuit.png',
  created_at: '2026-05-22T00:00:00.000Z',
  submissions: [],
};

const lager = {
  id: 'd1',
  name: 'Lager',
  alcoholic: true,
  created_by: null,
  created_at: '2026-05-22T00:00:00.000Z',
};
const water = {
  id: 'd2',
  name: 'Sparkling Water',
  alcoholic: false,
  created_by: null,
  created_at: '2026-05-22T00:00:00.000Z',
};
// Two of each list item (id 1 = the default, id 2 = the user's override) so
// the rendered setup summary differs depending on which is selected.
const characters = [
  { id: 1, name: 'Mario', image_path: 'characters/mario.png' },
  { id: 2, name: 'Luigi', image_path: 'characters/luigi.png' },
];
const bodies = [
  { id: 1, name: 'Standard Kart', image_path: 'bodies/std.png' },
  { id: 2, name: 'Pipe Frame', image_path: 'bodies/pipe.png' },
];
const wheels = [
  { id: 1, name: 'Standard Wheels', image_path: 'wheels/std.png' },
  { id: 2, name: 'Roller Wheels', image_path: 'wheels/roller.png' },
];
const gliders = [
  { id: 1, name: 'Super Glider', image_path: 'gliders/std.png' },
  { id: 2, name: 'Cloud Glider', image_path: 'gliders/cloud.png' },
];

/** Register MSW handlers for the game-data lists the sheet always loads. */
function mockGameData(opts: { drinkTypes?: unknown[] } = {}) {
  server.use(
    http.get('/api/v1/drink-types', () =>
      HttpResponse.json(opts.drinkTypes ?? [lager]),
    ),
    http.get('/api/v1/characters', () => HttpResponse.json(characters)),
    http.get('/api/v1/bodies', () => HttpResponse.json(bodies)),
    http.get('/api/v1/wheels', () => HttpResponse.json(wheels)),
    http.get('/api/v1/gliders', () => HttpResponse.json(gliders)),
  );
}

describe('RunEntrySheet', () => {
  it('pre-fills the drink and setup from the run defaults', async () => {
    mockGameData();
    server.use(
      http.get('/api/v1/runs/defaults', () =>
        HttpResponse.json({
          drink_type_id: 'd1',
          character_id: 1,
          body_id: 1,
          wheel_id: 1,
          glider_id: 1,
          source: 'previous_run',
        }),
      ),
    );

    render(
      <RunEntrySheet race={race} onClose={vi.fn()} onSubmitted={vi.fn()} />,
      { wrapper: Wrapper },
    );

    // The default drink id resolves to its name via the drink-types list.
    expect(await screen.findByText('Lager')).toBeInTheDocument();
    // The four setup ids resolve to the joined setup summary.
    expect(
      await screen.findByText(/Mario.*Standard Kart.*Standard Wheels/),
    ).toBeInTheDocument();
    // The "previous_run" source label shows under both the drink and setup.
    expect(screen.getAllByText('From your last run')).toHaveLength(2);
  });

  it('lets the user override the default drink', async () => {
    mockGameData({ drinkTypes: [lager, water] });
    server.use(
      http.get('/api/v1/runs/defaults', () =>
        HttpResponse.json({
          drink_type_id: 'd1',
          character_id: 1,
          body_id: 1,
          wheel_id: 1,
          glider_id: 1,
          source: 'previous_run',
        }),
      ),
    );
    const user = userEvent.setup();

    render(
      <RunEntrySheet race={race} onClose={vi.fn()} onSubmitted={vi.fn()} />,
      { wrapper: Wrapper },
    );

    // The default (previous-run) drink shows first.
    expect(await screen.findByText('Lager')).toBeInTheDocument();

    // Open the drink picker and choose a different drink; the user's pick wins
    // over the default (`picked ?? default`).
    await user.click(screen.getByRole('button', { name: /change/i }));
    await user.click(
      await screen.findByRole('button', { name: /sparkling water/i }),
    );

    expect(await screen.findByText('Sparkling Water')).toBeInTheDocument();
  });

  it('lets the user override the default race setup', async () => {
    mockGameData();
    server.use(
      http.get('/api/v1/runs/defaults', () =>
        HttpResponse.json({
          drink_type_id: 'd1',
          character_id: 1,
          body_id: 1,
          wheel_id: 1,
          glider_id: 1,
          source: 'previous_run',
        }),
      ),
    );
    const user = userEvent.setup();

    render(
      <RunEntrySheet race={race} onClose={vi.fn()} onSubmitted={vi.fn()} />,
      { wrapper: Wrapper },
    );

    // Default setup summary (the id-1 items) shows first.
    expect(
      await screen.findByText(/Mario.*Standard Kart.*Standard Wheels/),
    ).toBeInTheDocument();

    // Open the setup picker and complete it with the id-2 setup; the pick wins.
    await user.click(screen.getByRole('button', { name: /edit/i }));
    await user.click(
      await screen.findByRole('button', { name: /complete-setup-stub/i }),
    );

    expect(
      await screen.findByText(/Luigi.*Pipe Frame.*Roller Wheels/),
    ).toBeInTheDocument();
  });

  it('hides the track thumbnail when the image fails to load', async () => {
    mockGameData();
    server.use(
      http.get(
        '/api/v1/runs/defaults',
        () => new HttpResponse(null, { status: 500 }),
      ),
    );

    render(
      <RunEntrySheet race={race} onClose={vi.fn()} onSubmitted={vi.fn()} />,
      { wrapper: Wrapper },
    );

    const img = await screen.findByAltText('Mario Circuit');
    expect(img.style.display).not.toBe('none');

    fireEvent.error(img);

    expect(img.style.display).toBe('none');
  });

  it('degrades to blank fields when the run-defaults request fails', async () => {
    mockGameData({ drinkTypes: [] });
    server.use(
      http.get(
        '/api/v1/runs/defaults',
        () => new HttpResponse(null, { status: 500 }),
      ),
    );

    render(
      <RunEntrySheet race={race} onClose={vi.fn()} onSubmitted={vi.fn()} />,
      { wrapper: Wrapper },
    );

    expect(await screen.findByText('Select drink')).toBeInTheDocument();
    expect(screen.getByText('Select race setup')).toBeInTheDocument();
    expect(screen.queryByText('From your last run')).not.toBeInTheDocument();
  });
});
