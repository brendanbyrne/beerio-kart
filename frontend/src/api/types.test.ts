import { describe, it } from 'vitest';

// Placeholder — the schema-test pattern.
//
// PR-B2 (Issue #191) replaces the hand-written types in api/types.ts with Zod
// schemas and inferred types. When it lands, fill these in: per
// docs/coding-standards/typescript.md § 12, every schema test covers the happy
// path, at least one rejection case, and a JSON round-trip.
describe('API response schemas', () => {
  it.todo('parses a valid API response into the inferred type');
  it.todo('rejects a response missing a required field');
  it.todo('round-trips a parsed value through JSON');
});
