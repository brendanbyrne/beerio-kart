import { describe, expect, it } from 'vitest';
import {
  BodyId,
  BodyIdSchema,
  CharacterId,
  CharacterIdSchema,
  CupId,
  CupIdSchema,
  DrinkTypeId,
  DrinkTypeIdSchema,
  GliderId,
  GliderIdSchema,
  RaceId,
  RaceIdSchema,
  RunId,
  RunIdSchema,
  SessionId,
  SessionIdSchema,
  TrackId,
  TrackIdSchema,
  UserId,
  UserIdSchema,
  WheelId,
  WheelIdSchema,
} from './brand';
import type * as z from 'zod';

// Branded-type constructors must be runtime *identity* functions: the brand
// is a compile-time-only phantom, so the value coming out must be byte-for-
// byte the value going in. If a constructor ever did real work (wrapped,
// normalized, or copied the value) it would silently corrupt every JSON
// payload the branded value rides in — a bug the type system cannot catch,
// since the brand is erased at runtime. typescript.md § 12 requires a happy-
// path and a JSON round-trip test for every branded-type constructor.

const stringBrands: [string, (value: string) => string][] = [
  ['UserId', UserId],
  ['SessionId', SessionId],
  ['RunId', RunId],
  ['RaceId', RaceId],
  ['DrinkTypeId', DrinkTypeId],
];

const numberBrands: [string, (value: number) => number][] = [
  ['CharacterId', CharacterId],
  ['BodyId', BodyId],
  ['WheelId', WheelId],
  ['GliderId', GliderId],
  ['TrackId', TrackId],
  ['CupId', CupId],
];

describe('string-brand constructors', () => {
  it.each(stringBrands)('%s returns its input unchanged', (_name, ctor) => {
    expect(ctor('018f2a3b-uuid-value')).toBe('018f2a3b-uuid-value');
  });

  it.each(stringBrands)('%s round-trips through JSON', (_name, ctor) => {
    const branded = ctor('some-id');
    expect(JSON.parse(JSON.stringify(branded))).toBe('some-id');
  });

  it.each(stringBrands)('%s preserves the empty string', (_name, ctor) => {
    expect(ctor('')).toBe('');
  });
});

describe('number-brand constructors', () => {
  it.each(numberBrands)('%s returns its input unchanged', (_name, ctor) => {
    expect(ctor(42)).toBe(42);
  });

  it.each(numberBrands)('%s round-trips through JSON', (_name, ctor) => {
    const branded = ctor(7);
    expect(JSON.parse(JSON.stringify(branded))).toBe(7);
  });

  it.each(numberBrands)('%s preserves zero', (_name, ctor) => {
    expect(ctor(0)).toBe(0);
  });
});

describe('brands are nominally distinct', () => {
  it('rejects passing one brand where a different brand is expected', () => {
    const takesSessionId = (value: SessionId): SessionId => value;
    // @ts-expect-error a UserId is not assignable to a SessionId — this is
    // the whole point of branding. If this line ever stops erroring, the
    // brands have collapsed back into structurally-identical strings.
    expect(takesSessionId(UserId('s1'))).toBe('s1');
  });
});

// The Zod brand schemas (PR-B2) are the mint point used by api/types.ts.
// Like the constructors, the transform must be runtime identity — the brand
// is erased — so `.parse(x)` returns `x` unchanged; only the wire-primitive
// validation (string vs. number) is enforced.

const stringBrandSchemas: [string, z.ZodType][] = [
  ['UserIdSchema', UserIdSchema],
  ['SessionIdSchema', SessionIdSchema],
  ['RunIdSchema', RunIdSchema],
  ['RaceIdSchema', RaceIdSchema],
  ['DrinkTypeIdSchema', DrinkTypeIdSchema],
];

const numberBrandSchemas: [string, z.ZodType][] = [
  ['CharacterIdSchema', CharacterIdSchema],
  ['BodyIdSchema', BodyIdSchema],
  ['WheelIdSchema', WheelIdSchema],
  ['GliderIdSchema', GliderIdSchema],
  ['TrackIdSchema', TrackIdSchema],
  ['CupIdSchema', CupIdSchema],
];

describe('string-brand schemas', () => {
  it.each(stringBrandSchemas)(
    '%s parses a string to the branded value unchanged',
    (_name, schema) => {
      expect(schema.parse('018f2a3b-uuid-value')).toBe('018f2a3b-uuid-value');
    },
  );

  it.each(stringBrandSchemas)('%s rejects a number input', (_name, schema) => {
    expect(() => schema.parse(42)).toThrow();
  });
});

describe('number-brand schemas', () => {
  it.each(numberBrandSchemas)(
    '%s parses a number to the branded value unchanged',
    (_name, schema) => {
      expect(schema.parse(7)).toBe(7);
    },
  );

  it.each(numberBrandSchemas)('%s rejects a string input', (_name, schema) => {
    expect(() => schema.parse('not-a-number')).toThrow();
  });
});
