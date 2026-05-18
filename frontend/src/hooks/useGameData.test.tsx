import { describe, it } from 'vitest';

// Placeholder — the hook-test pattern.
//
// PR-C1 (Issue #176) migrates the static-data hooks in useGameData.ts to
// TanStack Query. When it lands, fill these in: per
// docs/coding-standards/react.md § 13, hook tests use `renderHook` wrapped in
// QueryClientProvider and assert rendered output / side effects (network
// requests via MSW), not the hook's internal return-value shape.
describe('useGameData', () => {
  it.todo('returns the fetched list once the query resolves');
  it.todo('exposes a loading state while the query is pending');
});
