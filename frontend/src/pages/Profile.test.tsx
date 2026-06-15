import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import type * as RouterDom from 'react-router-dom';
import { Profile } from './Profile';

// Mutable mock state, reset per test. Lets one set of module-level mocks serve
// both the all-null default profile (most tests) and a fully-populated one
// (the preferences-rendering test) without re-declaring `vi.mock` per case.
const state = vi.hoisted(() => ({
  profile: null as Record<string, unknown> | null,
  characters: [] as { id: number; name: string; image_path: string }[],
  bodies: [] as { id: number; name: string; image_path: string }[],
  wheels: [] as { id: number; name: string; image_path: string }[],
  gliders: [] as { id: number; name: string; image_path: string }[],
}));

const { changePassword, logout, refresh, apiFetch, navigate } = vi.hoisted(
  () => ({
    changePassword: vi.fn(),
    logout: vi.fn(),
    refresh: vi.fn(),
    apiFetch: vi.fn(),
    navigate: vi.fn(),
  }),
);

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof RouterDom>('react-router-dom');
  return { ...actual, useNavigate: () => navigate };
});

vi.mock('../hooks/useAuth', () => ({
  useAuth: () => ({
    user: { id: 'u1', username: 'alice' },
    logout,
    changePassword,
  }),
}));

vi.mock('../hooks/useUserProfile', () => ({
  useUserProfile: () => ({ profile: state.profile, refresh }),
}));

vi.mock('../hooks/useGameData', () => ({
  useCharacters: () => ({ items: state.characters }),
  useBodies: () => ({ items: state.bodies }),
  useWheels: () => ({ items: state.wheels }),
  useGliders: () => ({ items: state.gliders }),
}));

vi.mock('../api/client', () => ({ apiFetch }));

vi.mock('../components/BottomNav', () => ({
  BottomNav: () => <nav data-testid="bottom-nav" />,
}));

// The two picker children are exercised by their own tests; here they stand in
// as a single button that fires the callback Profile passes, so the edit-mode
// save handlers can be driven without re-testing the pickers themselves.
vi.mock('../components/RaceSetupPicker', () => ({
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
        onComplete({ characterId: 1, bodyId: 2, wheelId: 3, gliderId: 4 });
      }}
    >
      complete-setup
    </button>
  ),
}));

vi.mock('../components/DrinkTypeSelector', () => ({
  DrinkTypeSelector: ({
    onSelect,
  }: {
    onSelect: (d: { id: string; name: string; alcoholic: boolean }) => void;
  }) => (
    <button
      onClick={() => {
        onSelect({ id: 'd1', name: 'Cider', alcoholic: true });
      }}
    >
      select-drink
    </button>
  ),
}));

const ALL_NULL_PROFILE = {
  id: 'u1',
  username: 'alice',
  preferred_character_id: null,
  preferred_body_id: null,
  preferred_wheel_id: null,
  preferred_glider_id: null,
  preferred_drink_type: null,
  created_at: '2026-05-21T00:00:00.000Z',
};

function renderProfile() {
  return render(
    <MemoryRouter>
      <Profile />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  state.profile = { ...ALL_NULL_PROFILE };
  state.characters = [];
  state.bodies = [];
  state.wheels = [];
  state.gliders = [];
});

// The password block opts into fake timers; restore real ones after every test
// so the later real-timer blocks (waitFor) aren't left on a frozen clock.
afterEach(() => {
  vi.useRealTimers();
});

describe('Profile password change form', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  it('submits the entered passwords and closes the form on success', async () => {
    changePassword.mockResolvedValue(null);
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderProfile();

    await user.click(screen.getByRole('button', { name: /change/i }));
    await user.type(
      screen.getByPlaceholderText('Current password'),
      'old-secret',
    );
    await user.type(
      screen.getByPlaceholderText(/new password/i),
      'new-secret!',
    );
    await user.click(screen.getByRole('button', { name: /change password/i }));

    await waitFor(() => {
      expect(changePassword).toHaveBeenCalledWith('old-secret', 'new-secret!');
    });
    expect(await screen.findByText('Password changed!')).toBeInTheDocument();

    // After ~1.5s the form auto-closes; the Change-trigger button reappears.
    vi.advanceTimersByTime(1600);
    await waitFor(() => {
      expect(
        screen.queryByPlaceholderText('Current password'),
      ).not.toBeInTheDocument();
    });
  });

  it('shows the backend error and stays on the form when the change fails', async () => {
    changePassword.mockResolvedValue('Current password is incorrect');
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderProfile();

    await user.click(screen.getByRole('button', { name: /change/i }));
    await user.type(
      screen.getByPlaceholderText('Current password'),
      'wrong-old',
    );
    await user.type(
      screen.getByPlaceholderText(/new password/i),
      'new-secret!',
    );
    await user.click(screen.getByRole('button', { name: /change password/i }));

    expect(
      await screen.findByText('Current password is incorrect'),
    ).toBeInTheDocument();
    // Form is still open — the inputs are still in the DOM.
    expect(screen.getByPlaceholderText('Current password')).toBeInTheDocument();
  });

  it('clears the form when Cancel is clicked', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderProfile();

    await user.click(screen.getByRole('button', { name: /change/i }));
    const cancel = screen.getByRole('button', { name: /cancel/i });
    await user.click(cancel);

    expect(
      screen.queryByPlaceholderText('Current password'),
    ).not.toBeInTheDocument();
    expect(changePassword).not.toHaveBeenCalled();
  });

  it('catches a short new password with the Zod backstop at submit', async () => {
    changePassword.mockResolvedValue(null);
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderProfile();

    await user.click(screen.getByRole('button', { name: /change/i }));
    // jsdom does not enforce `minLength` at submit, so the submit reaches
    // the action and Zod is the only gate that fires — the "submit-time
    // backstop" react.md § 8 mandates.
    await user.type(
      screen.getByPlaceholderText('Current password'),
      'old-secret',
    );
    await user.type(screen.getByPlaceholderText(/new password/i), 'short');
    await user.click(screen.getByRole('button', { name: /change password/i }));

    expect(
      await screen.findByText('New password must be at least 8 characters'),
    ).toBeInTheDocument();
    expect(changePassword).not.toHaveBeenCalled();
  });
});

describe('Profile cards, edit modes, and logout', () => {
  it('renders the saved character setup and drink preference when present', () => {
    state.profile = {
      ...ALL_NULL_PROFILE,
      preferred_character_id: 1,
      preferred_body_id: 2,
      preferred_wheel_id: 3,
      preferred_glider_id: 4,
      preferred_drink_type: { id: 'd1', name: 'Cider', alcoholic: true },
    };
    state.characters = [{ id: 1, name: 'Mario', image_path: 'c.png' }];
    state.bodies = [{ id: 2, name: 'Standard', image_path: 'b.png' }];
    state.wheels = [{ id: 3, name: 'Slick', image_path: 'w.png' }];
    state.gliders = [{ id: 4, name: 'Cloud', image_path: 'g.png' }];
    renderProfile();

    expect(screen.getByText('Mario')).toBeInTheDocument();
    expect(screen.getByText('Cider')).toBeInTheDocument();
    expect(screen.getByText('(Alcoholic)')).toBeInTheDocument();
  });

  it('shows "Not set yet" when no setup or drink preference is saved', () => {
    renderProfile();
    expect(screen.getAllByText('Not set yet')).toHaveLength(2);
  });

  it('opens the race-setup editor from its card and returns on Back', async () => {
    const user = userEvent.setup();
    renderProfile();

    await user.click(screen.getByRole('button', { name: /race setup/i }));
    // The mocked picker stands in for the edit view.
    expect(
      screen.getByRole('button', { name: 'complete-setup' }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /back/i }));
    expect(
      screen.getByRole('button', { name: /log out/i }),
    ).toBeInTheDocument();
  });

  it('saves the race setup and returns to the profile view', async () => {
    apiFetch.mockResolvedValue({ ok: true });
    const user = userEvent.setup();
    renderProfile();

    await user.click(screen.getByRole('button', { name: /race setup/i }));
    await user.click(screen.getByRole('button', { name: 'complete-setup' }));

    // Pin the request body, not just method — it's the only thing that
    // distinguishes a race-setup save from a drink save to the same URL.
    await waitFor(() => {
      expect(apiFetch).toHaveBeenCalledWith(
        '/api/v1/users/u1',
        expect.objectContaining({
          method: 'PUT',
          body: JSON.stringify({
            preferred_character_id: 1,
            preferred_body_id: 2,
            preferred_wheel_id: 3,
            preferred_glider_id: 4,
          }),
        }),
      );
    });
    expect(refresh).toHaveBeenCalled();
    expect(
      await screen.findByRole('button', { name: /log out/i }),
    ).toBeInTheDocument();
  });

  it('surfaces a save error and stays on the race-setup editor when the PUT fails', async () => {
    apiFetch.mockResolvedValue({
      ok: false,
      status: 500,
      json: () =>
        Promise.resolve({ error: 'Server exploded', code: 'internal' }),
    });
    const user = userEvent.setup();
    renderProfile();

    await user.click(screen.getByRole('button', { name: /race setup/i }));
    await user.click(screen.getByRole('button', { name: 'complete-setup' }));

    // The backend error is surfaced and the editor stays open (refresh skipped).
    expect(await screen.findByText('Server exploded')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'complete-setup' }),
    ).toBeInTheDocument();
    expect(refresh).not.toHaveBeenCalled();
  });

  it('saves the drink preference from its editor', async () => {
    apiFetch.mockResolvedValue({ ok: true });
    const user = userEvent.setup();
    renderProfile();

    await user.click(screen.getByRole('button', { name: /preferred drink/i }));
    await user.click(screen.getByRole('button', { name: 'select-drink' }));

    await waitFor(() => {
      expect(apiFetch).toHaveBeenCalledWith(
        '/api/v1/users/u1',
        expect.objectContaining({
          method: 'PUT',
          body: JSON.stringify({ preferred_drink_type_id: 'd1' }),
        }),
      );
    });
    expect(refresh).toHaveBeenCalled();
  });

  it('surfaces a save error and stays on the drink editor when the PUT fails', async () => {
    apiFetch.mockResolvedValue({
      ok: false,
      status: 400,
      json: () =>
        Promise.resolve({ error: 'Unknown drink type', code: 'validation' }),
    });
    const user = userEvent.setup();
    renderProfile();

    await user.click(screen.getByRole('button', { name: /preferred drink/i }));
    await user.click(screen.getByRole('button', { name: 'select-drink' }));

    expect(await screen.findByText('Unknown drink type')).toBeInTheDocument();
    expect(refresh).not.toHaveBeenCalled();
  });

  it('opens the drink editor from its card and returns on Back', async () => {
    const user = userEvent.setup();
    renderProfile();

    await user.click(screen.getByRole('button', { name: /preferred drink/i }));
    expect(
      screen.getByRole('button', { name: 'select-drink' }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /back/i }));
    expect(
      screen.getByRole('button', { name: /log out/i }),
    ).toBeInTheDocument();
  });

  it('logs out and redirects to the login page', async () => {
    logout.mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderProfile();

    await user.click(screen.getByRole('button', { name: /log out/i }));

    await waitFor(() => {
      expect(logout).toHaveBeenCalledTimes(1);
    });
    expect(navigate).toHaveBeenCalledWith('/login');
  });
});
