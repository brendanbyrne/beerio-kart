import { useEffect, useRef } from 'react';

// Tab-order members inside a modal. Mirrors the set browsers treat as
// sequentially focusable; `[tabindex="-1"]` is programmatically focusable but
// not part of Tab order, so every clause excludes it (a focusable tag can also
// carry tabindex="-1" to opt out — e.g. a backdrop button).
const FOCUSABLE_SELECTOR = [
  'a[href]:not([tabindex="-1"])',
  'button:not([disabled]):not([tabindex="-1"])',
  'textarea:not([disabled]):not([tabindex="-1"])',
  'input:not([disabled]):not([tabindex="-1"])',
  'select:not([disabled]):not([tabindex="-1"])',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

/**
 * Accessibility plumbing for a modal overlay (react.md § 10): trap Tab focus
 * inside the dialog, close on Escape, and restore focus to the trigger on
 * close. The caller attaches the returned ref to the dialog container and sets
 * `role="dialog"` / `aria-modal="true"` itself (kept in JSX so the markup reads
 * as a dialog without inspecting this hook).
 *
 * We hand-roll this rather than use native `<dialog>` because jsdom doesn't
 * implement `showModal()`'s focus trap / Escape / focus-restore, so the native
 * route would leave these behaviors untestable.
 *
 * @param onClose called on Escape.
 * @param active when false, the trap and Escape handler suspend (focus is left
 *   alone) — used when the modal hands off to a full-screen sub-view that owns
 *   focus, e.g. RunEntrySheet's drink/setup pickers. Focus is still restored to
 *   the original trigger when the modal unmounts, regardless of `active`.
 */
export function useModalA11y<T extends HTMLElement = HTMLDivElement>(
  onClose: () => void,
  active = true,
) {
  const ref = useRef<T>(null);
  // Lets the keydown handler read the latest onClose without re-installing the
  // listener each render. Updated in an effect (never during render) so it
  // doesn't trip react-hooks' refs-in-render rule.
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  // Capture the trigger on mount; restore focus to it on unmount. Runs once for
  // the modal's whole lifetime (empty deps) so toggling `active` for a sub-view
  // doesn't bounce focus back to the page.
  useEffect(() => {
    const trigger = document.activeElement as HTMLElement | null;
    return () => {
      trigger?.focus();
    };
  }, []);

  // Trap focus and handle Escape while active. Re-runs on `active` flips so a
  // suspended trap resumes (and re-seats focus) when a sub-view closes.
  useEffect(() => {
    const node = ref.current;
    if (!active || !node) return;

    const getFocusable = () =>
      Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));

    // Seat focus inside the dialog on open: first focusable, else the container.
    const initial = getFocusable()[0];
    if (initial) {
      initial.focus();
    } else {
      node.tabIndex = -1;
      node.focus();
    }

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onCloseRef.current();
        return;
      }
      if (e.key !== 'Tab') return;

      const els = getFocusable();
      const first = els[0];
      const last = els[els.length - 1];
      if (!first || !last) {
        // Nothing focusable — keep focus pinned to the container.
        e.preventDefault();
        return;
      }
      const focused = document.activeElement;
      if (e.shiftKey) {
        if (focused === first || !node.contains(focused)) {
          e.preventDefault();
          last.focus();
        }
      } else if (focused === last || !node.contains(focused)) {
        e.preventDefault();
        first.focus();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [active]);

  return ref;
}
