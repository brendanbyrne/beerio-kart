import { describe, expect, it } from 'vitest';
import type * as z from 'zod';
import {
  AccessTokenPayloadSchema,
  AuthSessionSchema,
  DrinkTypeSchema,
  NotificationPayloadSchema,
  RunDefaultsSchema,
  RunDetailSchema,
  SessionDetailSchema,
  TokenRefreshSchema,
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

// The remaining response schemas the user-facing flows in this PR exercise.
// Each gets a happy-path parse, a JSON round-trip (brands erase, so it's a
// deep-equality check), and one targeted rejection — per typescript.md § 12.

const runDetail = {
  id: 'run1',
  user_id: 'u1',
  session_race_id: 'r1',
  track_id: 1,
  track_time: 90_000,
  lap1_time: 30_000,
  lap2_time: 30_000,
  lap3_time: 30_000,
  character_id: 1,
  body_id: 2,
  wheel_id: 3,
  glider_id: 4,
  drink_type_id: 'd1',
  drink_type_name: 'Water',
  disqualified: false,
  created_at: '2026-05-18T00:00:00.000Z',
};

const runDefaults = {
  drink_type_id: 'd1',
  character_id: 1,
  body_id: 2,
  wheel_id: 3,
  glider_id: 4,
  source: 'previous_run',
};

const authSession = {
  access_token: 'header.payload.signature',
  user: { id: 'u1', username: 'alice' },
};

const tokenRefresh = { access_token: 'header.payload.signature' };

const accessTokenPayload = { sub: 'u1', username: 'alice' };

const validPayloads: [string, z.ZodType, unknown][] = [
  ['RunDetailSchema', RunDetailSchema, runDetail],
  ['RunDefaultsSchema', RunDefaultsSchema, runDefaults],
  ['AuthSessionSchema', AuthSessionSchema, authSession],
  ['TokenRefreshSchema', TokenRefreshSchema, tokenRefresh],
  ['AccessTokenPayloadSchema', AccessTokenPayloadSchema, accessTokenPayload],
];

describe('response schemas — happy path + JSON round-trip', () => {
  it.each(validPayloads)(
    '%s parses a valid payload',
    (_name, schema, payload) => {
      expect(schema.safeParse(payload).success).toBe(true);
    },
  );

  it.each(validPayloads)(
    '%s round-trips through JSON unchanged',
    (_name, schema, payload) => {
      expect(JSON.parse(JSON.stringify(schema.parse(payload)))).toEqual(
        payload,
      );
    },
  );
});

describe('response schemas — rejections', () => {
  it('RunDetailSchema rejects a non-numeric track_time', () => {
    expect(
      RunDetailSchema.safeParse({ ...runDetail, track_time: 'fast' }).success,
    ).toBe(false);
  });

  it('RunDefaultsSchema rejects a source outside the enum', () => {
    expect(
      RunDefaultsSchema.safeParse({ ...runDefaults, source: 'magic' }).success,
    ).toBe(false);
  });

  it('AuthSessionSchema rejects a missing access_token', () => {
    expect(
      AuthSessionSchema.safeParse({ user: { id: 'u1', username: 'alice' } })
        .success,
    ).toBe(false);
  });

  it('TokenRefreshSchema rejects a non-string access_token', () => {
    expect(TokenRefreshSchema.safeParse({ access_token: 42 }).success).toBe(
      false,
    );
  });

  it('AccessTokenPayloadSchema rejects a missing sub', () => {
    expect(
      AccessTokenPayloadSchema.safeParse({ username: 'alice' }).success,
    ).toBe(false);
  });
});
