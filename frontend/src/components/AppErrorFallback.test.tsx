import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AppErrorFallback } from './AppErrorFallback';

// AppErrorFallback is the app-level react-error-boundary fallback (react.md
// § 9). Its user-visible contract: tell the user the app broke, and offer a
// reload that actually reloads the page.

const originalLocation = window.location;

afterEach(() => {
  vi.unstubAllEnvs();
  Object.defineProperty(window, 'location', {
    configurable: true,
    value: originalLocation,
  });
});

describe('AppErrorFallback', () => {
  it('tells the user something went wrong', () => {
    render(
      <AppErrorFallback
        error={new Error('boom')}
        resetErrorBoundary={vi.fn()}
      />,
    );

    expect(
      screen.getByRole('heading', { name: /something went wrong/i }),
    ).toBeInTheDocument();
  });

  it('renders a non-Error thrown value without crashing', () => {
    // react-error-boundary types `error` as unknown — code can throw a string,
    // object, anything. The fallback must stringify it, not assume `.message`.
    vi.stubEnv('DEV', true);
    render(
      <AppErrorFallback
        error="boom as a string"
        resetErrorBoundary={vi.fn()}
      />,
    );

    expect(screen.getByText(/boom as a string/)).toBeInTheDocument();
  });

  it('reloads the page when Reload is pressed', async () => {
    const reload = vi.fn();
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { reload },
    });
    const user = userEvent.setup();

    render(
      <AppErrorFallback
        error={new Error('boom')}
        resetErrorBoundary={vi.fn()}
      />,
    );
    await user.click(screen.getByRole('button', { name: /reload/i }));

    expect(reload).toHaveBeenCalledOnce();
  });
});
