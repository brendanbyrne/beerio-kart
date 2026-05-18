/**
 * Branded (nominal) ID types and their constructors.
 *
 * TypeScript's type system is structural: `SessionId` and `UserId` are both
 * strings, so nothing stops one being passed where the other is expected. A
 * brand attaches a phantom symbol property that exists only at the type level
 * (it is erased at runtime), which makes each ID type *nominally* distinct
 * while leaving the underlying value a plain JSON-round-trippable string or
 * number. See docs/coding-standards/typescript.md § 3.
 *
 * The Rust backend already has a `nutype` newtype for every domain ID; these
 * are the TypeScript mirrors, so the type safety survives the wire crossing.
 *
 * Every ID has a same-named constructor — the only sanctioned way to mint a
 * brand. PR-B1 calls them (or casts) at the API-helper boundary; PR-B2
 * (Issue #191) replaces those mint sites with Zod `.transform()` parses.
 */

declare const brand: unique symbol;

/** Attaches a compile-time-only phantom brand `B` to a base type `T`. */
export type Brand<T, B> = T & { readonly [brand]: B };

// ── String-shaped IDs (UUIDs on the wire) ───────────────────────────────

export type UserId = Brand<string, 'UserId'>;
export type SessionId = Brand<string, 'SessionId'>;
export type RunId = Brand<string, 'RunId'>;
export type RaceId = Brand<string, 'RaceId'>;
export type DrinkTypeId = Brand<string, 'DrinkTypeId'>;

export const UserId = (value: string): UserId => value as UserId;
export const SessionId = (value: string): SessionId => value as SessionId;
export const RunId = (value: string): RunId => value as RunId;
export const RaceId = (value: string): RaceId => value as RaceId;
export const DrinkTypeId = (value: string): DrinkTypeId => value as DrinkTypeId;

// ── Number-shaped IDs (lookup-table primary keys) ───────────────────────

export type CharacterId = Brand<number, 'CharacterId'>;
export type BodyId = Brand<number, 'BodyId'>;
export type WheelId = Brand<number, 'WheelId'>;
export type GliderId = Brand<number, 'GliderId'>;
export type TrackId = Brand<number, 'TrackId'>;
export type CupId = Brand<number, 'CupId'>;

export const CharacterId = (value: number): CharacterId => value as CharacterId;
export const BodyId = (value: number): BodyId => value as BodyId;
export const WheelId = (value: number): WheelId => value as WheelId;
export const GliderId = (value: number): GliderId => value as GliderId;
export const TrackId = (value: number): TrackId => value as TrackId;
export const CupId = (value: number): CupId => value as CupId;
