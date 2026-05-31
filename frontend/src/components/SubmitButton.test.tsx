import { describe, expect, it } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { SubmitButton } from './SubmitButton';

// useFormStatus reads pending state from the nearest <form>. Outside a form
// it returns { pending: false, data: null, method: null, action: null }, so
// the rendered button shows children and is enabled. That's what we assert
// here; the pending=true branch is exercised by Login.test.tsx and the other
// form tests where MSW holds the request open.

describe('SubmitButton', () => {
  it('renders children and is enabled outside a form', () => {
    render(<SubmitButton>Log In</SubmitButton>);
    const button = screen.getByRole('button', { name: 'Log In' });
    expect(button).toBeEnabled();
    expect(button).toHaveAttribute('type', 'submit');
  });

  it('keeps showing children while pending when no pendingLabel is given', async () => {
    // The pending fallback (`pending && pendingLabel !== undefined ? … : children`)
    // only matters when pending=true. Render inside a form whose action is held
    // open so useFormStatus reports pending, then assert the button is disabled
    // and STILL shows its children. Rendering outside a form (the prior version
    // of this test) leaves pending=false and never exercises this branch.
    let release!: () => void;
    const action = () =>
      new Promise<void>((resolve) => {
        release = resolve;
      });
    const user = userEvent.setup();

    render(
      <form action={action}>
        <SubmitButton>Save</SubmitButton>
      </form>,
    );
    const button = screen.getByRole('button', { name: 'Save' });

    await user.click(button);

    await waitFor(() => expect(button).toBeDisabled());
    expect(button).toHaveTextContent('Save');

    // Let the action settle so the pending state clears inside act().
    release();
    await waitFor(() => expect(button).toBeEnabled());
  });

  it('applies the supplied className alongside the disabled-state classes', () => {
    render(<SubmitButton className="bg-blue-500">Save</SubmitButton>);
    const button = screen.getByRole('button', { name: 'Save' });
    expect(button).toHaveClass('bg-blue-500');
    expect(button).toHaveClass('disabled:bg-gray-300');
  });
});
