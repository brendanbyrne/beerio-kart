import { describe, expect, it } from 'vitest';
import {
  BodyId,
  CharacterId,
  CupId,
  DrinkTypeId,
  GliderId,
  RaceId,
  RunId,
  SessionId,
  TrackId,
  UserId,
  WheelId,
} from './brand';

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
