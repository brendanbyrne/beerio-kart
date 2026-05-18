import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import Login from './Login';

// Login reads `login` from the auth context. Mock the hook so the test
// exercises the form's behavior without a real AuthProvider or network.
// `vi.hoisted` is required because `vi.mock` is hoisted above this file's
// other statements — a plain `const login = vi.fn()` would not yet exist
// when the mock factory runs.
const { login } = vi.hoisted(() => ({ login: vi.fn() }));
vi.mock('../hooks/useAuth', () => ({
  useAuth: () => ({ login }),
}));

function renderLogin() {
  // MemoryRouter supplies the router context Login needs for <Link> and
  // useNavigate().
  return render(
    <MemoryRouter>
      <Login />
    </MemoryRouter>,
  );
}

describe('Login', () => {
  beforeEach(() => {
    login.mockReset();
  });

  it('submits the entered username and password', async () => {
    login.mockResolvedValue(null);
    const user = userEvent.setup();
    renderLogin();

    await user.type(screen.getByLabelText('Username'), 'alice');
    await user.type(screen.getByLabelText('Password'), 'hunter2');
    await user.click(screen.getByRole('button', { name: /log in/i }));

    expect(login).toHaveBeenCalledWith('alice', 'hunter2');
  });

  it('shows the error message when login fails', async () => {
    login.mockResolvedValue('Invalid credentials');
    const user = userEvent.setup();
    renderLogin();

    await user.type(screen.getByLabelText('Username'), 'alice');
    await user.type(screen.getByLabelText('Password'), 'wrong-password');
    await user.click(screen.getByRole('button', { name: /log in/i }));

    expect(await screen.findByText('Invalid credentials')).toBeInTheDocument();
  });

  it('requires both fields before the form submits', async () => {
    login.mockResolvedValue(null);
    const user = userEvent.setup();
    renderLogin();

    // Native `required` on the inputs blocks submission when they are empty.
    await user.click(screen.getByRole('button', { name: /log in/i }));

    expect(login).not.toHaveBeenCalled();
  });
});
