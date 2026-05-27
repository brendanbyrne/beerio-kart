import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { Register } from './Register';

// Register reads `register` from the auth context. Mock the hook so the
// test exercises the form's behavior without a real AuthProvider or
// network. The hoisted-fn pattern mirrors Login.test.tsx.
const { register } = vi.hoisted(() => ({ register: vi.fn() }));
vi.mock('../hooks/useAuth', () => ({
  useAuth: () => ({ register }),
}));

function renderRegister() {
  return render(
    <MemoryRouter>
      <Register />
    </MemoryRouter>,
  );
}

describe('Register', () => {
  beforeEach(() => {
    register.mockReset();
  });

  it('submits the entered username and password', async () => {
    register.mockResolvedValue(null);
    const user = userEvent.setup();
    renderRegister();

    await user.type(screen.getByLabelText('Username'), 'alice');
    await user.type(screen.getByLabelText('Password'), 'hunter2!');
    await user.click(screen.getByRole('button', { name: /register/i }));

    // The action runs inside a transition; waitFor lets it settle before we
    // check that register was invoked with the FormData-read values.
    await waitFor(() => {
      expect(register).toHaveBeenCalledWith('alice', 'hunter2!');
    });
  });

  it('shows the error message when registration fails', async () => {
    register.mockResolvedValue('Username is taken');
    const user = userEvent.setup();
    renderRegister();

    await user.type(screen.getByLabelText('Username'), 'alice');
    await user.type(screen.getByLabelText('Password'), 'hunter2!');
    await user.click(screen.getByRole('button', { name: /register/i }));

    expect(await screen.findByText('Username is taken')).toBeInTheDocument();
  });

  it('requires both fields before the form submits', async () => {
    register.mockResolvedValue(null);
    const user = userEvent.setup();
    renderRegister();

    // Native `required` on the inputs blocks submission when they are empty.
    await user.click(screen.getByRole('button', { name: /register/i }));

    expect(register).not.toHaveBeenCalled();
  });

  // The 8-character `minLength` on the password input is enforced by the
  // browser at submit time, but jsdom does not block submission on
  // `minLength` violations (it does on `required`), so a behavioural test
  // here would lie about what real browsers do. The constraint is verified
  // by manual smoke test instead; this test asserts the attribute is wired
  // so a future rename can't silently drop it.
  it('exposes minLength=8 on the password input', () => {
    renderRegister();
    expect(screen.getByLabelText('Password')).toHaveAttribute('minLength', '8');
  });
});
