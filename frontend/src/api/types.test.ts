import { describe, expect, it } from 'vitest';
import {
  DrinkTypeSchema,
  NotificationPayloadSchema,
  SessionDetailSchema,
} from './types';

// Verifies the response schemas in types.ts. Per typescript.md § 12 every
// schema test covers the happy path, at least one rejection case, and a JSON
// round-trip. The brand transforms are erased at runtime — a parsed value is
// byte-identical to the wire JSON — so a round-trip is a deep-equality check.

const sessionDetail = {
  id: 's1',
  host_id: 'u1',
  host_username: 'alice',
  ruleset: 'random',
  status: 'active',
  created_at: '2026-05-18T00:00:00.000Z',
  participants: [
    {
      user_id: 'u1',
      username: 'alice',
      joined_at: '2026-05-18T00:00:00.000Z',
      left_at: null,
    },
  ],
  race_number: 0,
  current_race: null,
  races: [],
};

describe('SessionDetailSchema', () => {
  it('parses a valid API response into the inferred type', () => {
    const parsed = SessionDetailSchema.parse(sessionDetail);
    expect(parsed.id).toBe('s1');
    expect(parsed.status).toBe('active');
    expect(parsed.participants[0]?.user_id).toBe('u1');
  });

  it('rejects a response missing a required field', () => {
    const incomplete: Record<string, unknown> = { ...sessionDetail };
    delete incomplete.status;
    expect(SessionDetailSchema.safeParse(incomplete).success).toBe(false);
  });

  it('rejects a status outside the SessionStatus enum', () => {
    expect(
      SessionDetailSchema.safeParse({ ...sessionDetail, status: 'paused' })
        .success,
    ).toBe(false);
  });

  it('round-trips a parsed value through JSON unchanged', () => {
    const parsed = SessionDetailSchema.parse(sessionDetail);
    expect(JSON.parse(JSON.stringify(parsed))).toEqual(sessionDetail);
  });
});

describe('DrinkTypeSchema', () => {
  it('accepts a null created_by', () => {
    const parsed = DrinkTypeSchema.parse({
      id: 'd1',
      name: 'Water',
      alcoholic: false,
      created_by: null,
      created_at: '2026-05-18T00:00:00.000Z',
    });
    expect(parsed.created_by).toBeNull();
  });

  it('rejects a non-boolean alcoholic field', () => {
    expect(
      DrinkTypeSchema.safeParse({
        id: 'd1',
        name: 'Water',
        alcoholic: 'yes',
        created_by: null,
        created_at: '2026-05-18T00:00:00.000Z',
      }).success,
    ).toBe(false);
  });
});

describe('NotificationPayloadSchema', () => {
  it('parses a known payload kind', () => {
    const parsed = NotificationPayloadSchema.parse({
      kind: 'pending_races_dropped',
      session_id: 's1',
      dropped_count: 2,
    });
    expect(parsed.kind).toBe('pending_races_dropped');
  });

  it('rejects an unknown payload kind', () => {
    expect(
      NotificationPayloadSchema.safeParse({ kind: 'mystery_event' }).success,
    ).toBe(false);
  });
});
