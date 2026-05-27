import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
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

  it('falls back to children when no pendingLabel is provided', () => {
    render(<SubmitButton>Save</SubmitButton>);
    expect(screen.getByRole('button', { name: 'Save' })).toBeInTheDocument();
  });

  it('applies the supplied className alongside the disabled-state classes', () => {
    render(<SubmitButton className="bg-blue-500">Save</SubmitButton>);
    const button = screen.getByRole('button', { name: 'Save' });
    expect(button).toHaveClass('bg-blue-500');
    expect(button).toHaveClass('disabled:bg-gray-300');
  });
});
