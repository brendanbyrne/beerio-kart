import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { PageSkeleton } from './PageSkeleton';

// PageSkeleton is the Suspense fallback for lazily-loaded routes (react.md
// § 11). It's presentational, but the one behavior worth pinning is the
// screen-reader-facing loading announcement — a blank or unlabeled loading
// state is the silent regression this guards against.
describe('PageSkeleton', () => {
  it('exposes a labelled loading status to assistive tech', () => {
    render(<PageSkeleton />);

    const status = screen.getByRole('status');
    expect(status).toBeInTheDocument();
    expect(status).toHaveTextContent(/loading/i);
  });
});
