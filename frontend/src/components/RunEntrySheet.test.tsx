import { http, HttpResponse } from 'msw';
import { afterEach, describe, expect, it, vi } from 'vitest';
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

  // pinSliderGeometry pollutes HTMLElement.prototype globally, so cleanup must
  // run even if an assertion throws mid-test — an end-of-body restore() would
  // leak the mock into later tests. afterEach always drains it (react.md § 13).
  let restoreGeometry: (() => void) | null = null;
  afterEach(() => {
    restoreGeometry?.();
    restoreGeometry = null;
  });

  // The slide-to-DQ control measures its track from `offsetWidth` (a layout
  // value jsdom reports as 0) and maps the pointer's clientX through
  // `getBoundingClientRect`. Pin both so the drag math is deterministic: a
  // 300px-wide track with its left edge at x=0. The confirm threshold is
  // `width - thumb(44) - 4 = 252`.
  function pinSliderGeometry() {
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
      configurable: true,
      value: 300,
    });
    const rect = vi
      .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockReturnValue({
        left: 0,
        top: 0,
        right: 300,
        bottom: 48,
        width: 300,
        height: 48,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      } as DOMRect);
    restoreGeometry = () => {
      rect.mockRestore();
      delete (HTMLElement.prototype as { offsetWidth?: number }).offsetWidth;
    };
  }

  it('disqualifies the run when the slider is dragged past the threshold', async () => {
    mockGameData();
    server.use(
      http.get(
        '/api/v1/runs/defaults',
        () => new HttpResponse(null, { status: 500 }),
      ),
    );
    pinSliderGeometry();

    render(
      <RunEntrySheet race={race} onClose={vi.fn()} onSubmitted={vi.fn()} />,
      { wrapper: Wrapper },
    );

    const thumb = (await screen.findByText('»')).parentElement;
    if (!thumb?.parentElement) throw new Error('slider track not found');
    const track = thumb.parentElement;

    // Grab the thumb, drag past the 252px threshold, and release.
    fireEvent.mouseDown(thumb);
    fireEvent.mouseMove(track, { clientX: 300 });
    fireEvent.mouseUp(track);

    expect(await screen.findByText('Disqualified')).toBeInTheDocument();
  });

  it('disqualifies the run when the slider is dragged past the threshold by touch', async () => {
    // Touch is the primary input on the mobile reference device, so exercise
    // the touch drag path too (the mouse path is covered above).
    mockGameData();
    server.use(
      http.get(
        '/api/v1/runs/defaults',
        () => new HttpResponse(null, { status: 500 }),
      ),
    );
    pinSliderGeometry();

    render(
      <RunEntrySheet race={race} onClose={vi.fn()} onSubmitted={vi.fn()} />,
      { wrapper: Wrapper },
    );

    const thumb = (await screen.findByText('»')).parentElement;
    if (!thumb?.parentElement) throw new Error('slider track not found');
    const track = thumb.parentElement;

    fireEvent.touchStart(thumb);
    fireEvent.touchMove(track, { touches: [{ clientX: 300 }] });
    fireEvent.touchEnd(track);

    expect(await screen.findByText('Disqualified')).toBeInTheDocument();
  });

  it('snaps back without disqualifying when released before the threshold', async () => {
    mockGameData();
    server.use(
      http.get(
        '/api/v1/runs/defaults',
        () => new HttpResponse(null, { status: 500 }),
      ),
    );
    pinSliderGeometry();

    render(
      <RunEntrySheet race={race} onClose={vi.fn()} onSubmitted={vi.fn()} />,
      { wrapper: Wrapper },
    );

    const thumb = (await screen.findByText('»')).parentElement;
    if (!thumb?.parentElement) throw new Error('slider track not found');
    const track = thumb.parentElement;

    // A short drag (well under the 252px threshold) releases without confirming.
    fireEvent.mouseDown(thumb);
    fireEvent.mouseMove(track, { clientX: 50 });
    fireEvent.mouseUp(track);

    expect(screen.queryByText('Disqualified')).not.toBeInTheDocument();
  });

  it('closes the sheet on Escape', async () => {
    // PR-G2 (Issue #184): the sheet is a modal dialog — Escape calls onClose
    // (react.md § 10). useModalA11y installs the document-level handler.
    mockGameData();
    server.use(
      http.get(
        '/api/v1/runs/defaults',
        () => new HttpResponse(null, { status: 500 }),
      ),
    );
    const onClose = vi.fn();

    render(
      <RunEntrySheet race={race} onClose={onClose} onSubmitted={vi.fn()} />,
      { wrapper: Wrapper },
    );
    // Let the sheet settle before asserting on the key handler.
    await screen.findByText('Select drink');

    fireEvent.keyDown(document, { key: 'Escape' });

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('disqualifies the run from the keyboard (Enter on the slider)', async () => {
    // The slide-to-DQ control is a role="button" with an Enter/Space shortcut
    // so it's operable without a pointer gesture (PR-G2).
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

    const slider = await screen.findByRole('button', {
      name: /slide or press enter to disqualify/i,
    });
    fireEvent.keyDown(slider, { key: 'Enter' });

    expect(await screen.findByText('Disqualified')).toBeInTheDocument();
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
