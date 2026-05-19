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
 * Every ID has a same-named constructor and a same-named Zod schema
 * (`UserId` / `UserIdSchema`). The schema is the sanctioned mint point: it
 * validates the wire primitive and transforms it through the constructor, so
 * api/types.ts mints a branded ID exactly once — when the response JSON is
 * parsed. PR-B1 cast at the API-helper boundary; PR-B2 (Issue #191) moved the
 * mint into these `.transform()` schemas.
 */

import * as z from 'zod';

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

// ── Zod brand schemas (the PR-B2 mint point) ────────────────────────────
//
// Each schema validates the wire primitive (a string for UUID IDs, a number
// for lookup-table keys) and transforms it through the matching constructor
// above. api/types.ts composes these into its response-DTO schemas; nothing
// else should mint a brand.

export const UserIdSchema = z.string().transform(UserId);
export const SessionIdSchema = z.string().transform(SessionId);
export const RunIdSchema = z.string().transform(RunId);
export const RaceIdSchema = z.string().transform(RaceId);
export const DrinkTypeIdSchema = z.string().transform(DrinkTypeId);

export const CharacterIdSchema = z.number().transform(CharacterId);
export const BodyIdSchema = z.number().transform(BodyId);
export const WheelIdSchema = z.number().transform(WheelId);
export const GliderIdSchema = z.number().transform(GliderId);
export const TrackIdSchema = z.number().transform(TrackId);
export const CupIdSchema = z.number().transform(CupId);
