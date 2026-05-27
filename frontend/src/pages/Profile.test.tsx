import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { Profile } from './Profile';

// The password change form is what PR-E1 migrated; the other Profile cards
// delegate to sub-components covered by their own tests. So this file
// focuses on PasswordChangeForm: happy path (success message + auto-close)
// and error path (backend error message shown, form stays open).

const { changePassword, logout } = vi.hoisted(() => ({
  changePassword: vi.fn(),
  logout: vi.fn(),
}));

vi.mock('../hooks/useAuth', () => ({
  useAuth: () => ({
    user: { id: 'u1', username: 'alice' },
    logout,
    changePassword,
  }),
}));

vi.mock('../hooks/useUserProfile', () => ({
  useUserProfile: () => ({
    profile: {
      id: 'u1',
      username: 'alice',
      preferred_character_id: null,
      preferred_body_id: null,
      preferred_wheel_id: null,
      preferred_glider_id: null,
      preferred_drink_type: null,
      created_at: '2026-05-21T00:00:00.000Z',
    },
    refresh: vi.fn(),
  }),
}));

vi.mock('../hooks/useGameData', () => ({
  useCharacters: () => ({ items: [] }),
  useBodies: () => ({ items: [] }),
  useWheels: () => ({ items: [] }),
  useGliders: () => ({ items: [] }),
}));

vi.mock('../components/BottomNav', () => ({
  BottomNav: () => <nav data-testid="bottom-nav" />,
}));

function renderProfile() {
  return render(
    <MemoryRouter>
      <Profile />
    </MemoryRouter>,
  );
}

describe('Profile password change form', () => {
  beforeEach(() => {
    changePassword.mockReset();
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
});
