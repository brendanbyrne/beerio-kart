import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useModalA11y } from './useModalA11y';

// A minimal dialog that wires the hook the way real consumers do: attach the
// ref to a role="dialog" container holding a few focusable controls.
function Dialog({
  onClose,
  active,
}: {
  onClose: () => void;
  active?: boolean;
}) {
  const ref = useModalA11y(onClose, active);
  return (
    <div ref={ref} role="dialog" aria-modal="true" aria-label="Test dialog">
      <button>first</button>
      <button>second</button>
      <button>third</button>
    </div>
  );
}

// A dialog with no focusable descendants — exercises the fallback that seats
// focus on the container itself and the Tab no-op when there's nothing to cycle.
function EmptyDialog({ onClose }: { onClose: () => void }) {
  const ref = useModalA11y(onClose);
  return (
    <div ref={ref} role="dialog" aria-modal="true" aria-label="Empty dialog">
      <p>nothing focusable here</p>
    </div>
  );
}

// Opens the dialog from a trigger button, so the open→close→restore cycle can
// be exercised end to end (focus must return to `open` after the modal closes).
function Harness({ onClose }: { onClose?: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <div>
      <button
        onClick={() => {
          setOpen(true);
        }}
      >
        open
      </button>
      {open && (
        <Dialog
          onClose={() => {
            setOpen(false);
            onClose?.();
          }}
        />
      )}
    </div>
  );
}

describe('useModalA11y', () => {
  it('moves focus into the dialog on open', () => {
    render(<Dialog onClose={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'first' })).toHaveFocus();
  });

  it('wraps focus from the last element back to the first on Tab', () => {
    render(<Dialog onClose={vi.fn()} />);
    const first = screen.getByRole('button', { name: 'first' });
    const third = screen.getByRole('button', { name: 'third' });

    third.focus();
    fireEvent.keyDown(third, { key: 'Tab' });

    expect(first).toHaveFocus();
  });

  it('wraps focus from the first element to the last on Shift+Tab', () => {
    render(<Dialog onClose={vi.fn()} />);
    const first = screen.getByRole('button', { name: 'first' });
    const third = screen.getByRole('button', { name: 'third' });

    first.focus();
    fireEvent.keyDown(first, { key: 'Tab', shiftKey: true });

    expect(third).toHaveFocus();
  });

  it('closes on Escape', () => {
    const onClose = vi.fn();
    render(<Dialog onClose={onClose} />);

    fireEvent.keyDown(document, { key: 'Escape' });

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('suspends the trap and Escape when inactive', () => {
    const onClose = vi.fn();
    render(<Dialog onClose={onClose} active={false} />);

    fireEvent.keyDown(document, { key: 'Escape' });

    expect(onClose).not.toHaveBeenCalled();
  });

  it('seats focus on the container and still closes when nothing is focusable', () => {
    const onClose = vi.fn();
    render(<EmptyDialog onClose={onClose} />);
    const dialog = screen.getByRole('dialog');

    // No focusable children → focus falls back to the dialog container.
    expect(dialog).toHaveFocus();
    // Tab has nothing to cycle to, so it's a no-op (focus stays put).
    fireEvent.keyDown(dialog, { key: 'Tab' });
    expect(dialog).toHaveFocus();
    // Escape still closes.
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('restores focus to the trigger when the modal closes', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const trigger = screen.getByRole('button', { name: 'open' });

    await user.click(trigger);
    // On open, focus is seated on the first control inside the dialog.
    expect(screen.getByRole('button', { name: 'first' })).toHaveFocus();

    // Closing (Escape → unmount) hands focus back to the element that opened it.
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(trigger).toHaveFocus();
  });
});
