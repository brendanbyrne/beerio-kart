import { http, HttpResponse } from 'msw';
import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { server } from '../mocks/server';
import { RaceSetupPicker } from './RaceSetupPicker';

// PR-D2 (Issue #192) replaced four `characterId! / bodyId! / wheelId! /
// gliderId!` reads in the Confirm-button handler with a single `setup` value
// that is non-null only when all four ids are picked. The user-visible
// behavior — Confirm disabled until all four pieces are chosen, then fires
// `onComplete` with the full setup — matches the pre-PR `allSelected` flag,
// but the narrowing is now load-bearing for the no-`!` invariant. This test
// pins that contract so a future refactor (e.g., wrapping `setup` in a
// useMemo that captures stale ids) fails loudly.

function Wrapper({ children }: { children: ReactNode }) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

// One item per list — the picker auto-advances after the first selection in
// each step, so a single-element grid keeps the test deterministic.
const characters = [
  { id: 1, name: 'Mario', image_path: 'characters/mario.png' },
];
const bodies = [{ id: 1, name: 'Standard Kart', image_path: 'bodies/std.png' }];
const wheels = [
  { id: 1, name: 'Standard Wheels', image_path: 'wheels/std.png' },
];
const gliders = [
  { id: 1, name: 'Super Glider', image_path: 'gliders/std.png' },
];

describe('RaceSetupPicker', () => {
  it('keeps Confirm disabled until all four pieces are picked, then fires onComplete with the full setup', async () => {
    server.use(
      http.get('/api/v1/characters', () => HttpResponse.json(characters)),
      http.get('/api/v1/bodies', () => HttpResponse.json(bodies)),
      http.get('/api/v1/wheels', () => HttpResponse.json(wheels)),
      http.get('/api/v1/gliders', () => HttpResponse.json(gliders)),
    );
    const onComplete = vi.fn();
    const user = userEvent.setup();

    render(<RaceSetupPicker onComplete={onComplete} />, { wrapper: Wrapper });

    // Initial state: the Confirm button renders disabled — `setup` is null
    // because no ids have been picked.
    const confirm = await screen.findByRole('button', {
      name: /confirm setup/i,
    });
    expect(confirm).toBeDisabled();

    // Walk through the four steps. `handleSelect` auto-advances 150 ms after
    // each pick; `findByRole` polls long enough to catch the next grid render.
    await user.click(await screen.findByRole('button', { name: /mario/i }));
    await user.click(
      await screen.findByRole('button', { name: /standard kart/i }),
    );
    await user.click(
      await screen.findByRole('button', { name: /standard wheels/i }),
    );
    await user.click(
      await screen.findByRole('button', { name: /super glider/i }),
    );

    // All four picked: `setup` is non-null and Confirm enables.
    await waitFor(() => {
      expect(confirm).toBeEnabled();
    });

    await user.click(confirm);
    expect(onComplete).toHaveBeenCalledWith({
      characterId: 1,
      bodyId: 1,
      wheelId: 1,
      gliderId: 1,
    });
  });
});
