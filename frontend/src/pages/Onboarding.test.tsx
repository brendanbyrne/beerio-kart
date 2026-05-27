import { http, HttpResponse } from 'msw';
import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import type * as RouterDom from 'react-router-dom';
import { server } from '../mocks/server';
import { Onboarding } from './Onboarding';

const { navigate } = vi.hoisted(() => ({ navigate: vi.fn() }));
vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof RouterDom>('react-router-dom');
  return { ...actual, useNavigate: () => navigate };
});

// Onboarding saves the race setup, then the drink type, via PUT /users/:id.
// Covered behavior: when a save fails the user sees the backend's error and
// is NOT advanced past the current phase. The picker children are heavy and
// tested elsewhere, so they are mocked down to a single button that fires
// their completion callback (the Login.test.tsx useAuth-mock pattern).

vi.mock('../hooks/useAuth', () => ({
  useAuth: () => ({ user: { id: 'u1', username: 'alice' } }),
}));

vi.mock('../components/RaceSetupPicker', () => ({
  RaceSetupPicker: ({ onComplete }: { onComplete: (s: unknown) => void }) => (
    <button
      onClick={() =>
        onComplete({ characterId: 1, bodyId: 2, wheelId: 3, gliderId: 4 })
      }
    >
      complete-setup
    </button>
  ),
}));

vi.mock('../components/DrinkTypeSelector', () => ({
  DrinkTypeSelector: ({ onSelect }: { onSelect: (d: unknown) => void }) => (
    <button onClick={() => onSelect({ id: 'd1' })}>select-drink</button>
  ),
}));

describe('Onboarding', () => {
  it('shows the backend error and stays on the setup phase when the save fails', async () => {
    server.use(
      http.put('/api/v1/users/u1', () =>
        HttpResponse.json({ error: 'Invalid character' }, { status: 400 }),
      ),
    );
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <Onboarding />
      </MemoryRouter>,
    );

    await user.click(screen.getByText('complete-setup'));

    expect(await screen.findByText('Invalid character')).toBeInTheDocument();
    // Still on race-setup; the drink phase never rendered.
    expect(screen.queryByText('select-drink')).not.toBeInTheDocument();
  });

  it('navigates home after both saves succeed', async () => {
    navigate.mockReset();
    server.use(http.put('/api/v1/users/u1', () => HttpResponse.json({})));
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <Onboarding />
      </MemoryRouter>,
    );

    await user.click(screen.getByText('complete-setup'));
    await user.click(await screen.findByText('select-drink'));

    await waitFor(() => {
      expect(navigate).toHaveBeenCalledWith('/');
    });
  });

  it('shows the backend error when the drink-type save fails', async () => {
    // First PUT (race setup) succeeds and advances to the drink phase; the
    // second (drink type) fails.
    let calls = 0;
    server.use(
      http.put('/api/v1/users/u1', () => {
        calls += 1;
        return calls === 1
          ? HttpResponse.json({})
          : HttpResponse.json({ error: 'Unknown drink type' }, { status: 400 });
      }),
    );
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <Onboarding />
      </MemoryRouter>,
    );

    await user.click(screen.getByText('complete-setup'));
    await user.click(await screen.findByText('select-drink'));

    expect(await screen.findByText('Unknown drink type')).toBeInTheDocument();
  });
});
