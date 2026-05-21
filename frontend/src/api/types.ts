/**
 * API response shapes, as Zod schemas with their TypeScript types inferred
 * from them (`z.infer`) — never declared separately.
 *
 * Why schemas, not hand-written types (typescript.md § 8): `await res.json()`
 * is `any`. Assigning it to a hand-written `SessionDetail` is a lie the
 * compiler can't catch — the next backend field rename surfaces as a bug at
 * the first downstream read, far from the cause. A schema parses *and* infers,
 * so the runtime check and the type stay in lockstep, and the branded IDs
 * (brand.ts) are minted here, once, as the wire JSON is parsed.
 *
 * `CreateRunRequest` is the lone hand-written type: it is a *request* body the
 * frontend serializes, not a response it parses, so it needs no schema.
 *
 * PR-B2 (Issue #191).
 */
import * as z from 'zod';
import {
  BodyIdSchema,
  CharacterIdSchema,
  CupIdSchema,
  DrinkTypeIdSchema,
  GliderIdSchema,
  RaceIdSchema,
  RunIdSchema,
  SessionIdSchema,
  TrackIdSchema,
  UserIdSchema,
  WheelIdSchema,
} from './brand';

// ── Session enums ───────────────────────────────────────────────────────
//
// String-literal-union mirrors of the backend's `DeriveActiveEnum` types in
// backend/src/domain/enums.rs. The backend serializes each as a bare string;
// these unions are the wire-faithful frontend counterparts. typescript.md
// § 4: model "one of several known values" as a literal union, not `string`.

/**
 * Lifecycle state of a session — mirrors the backend `SessionStatus` enum
 * (serialized lowercase). A session accepts joins / leaves / submissions
 * while `active`; `closed` is terminal.
 */
export const SessionStatusSchema = z.enum(['active', 'closed']);
export type SessionStatus = z.infer<typeof SessionStatusSchema>;

/**
 * How a session selects each race's track — mirrors the backend
 * `SessionRuleset` enum (serialized snake_case). Only `random` is wired
 * through session creation today; the other three are valid values of the
 * type but the backend service rejects them until their behavior lands.
 */
export const SessionRulesetSchema = z.enum([
  'random',
  'default',
  'least_played',
  'round_robin',
]);
export type SessionRuleset = z.infer<typeof SessionRulesetSchema>;

// ── DTOs ────────────────────────────────────────────────────────────────

/**
 * A shared shape for the character / body / wheel / glider pick-lists.
 * `id` is intentionally a raw `number`: the type is structurally polymorphic
 * (it stands in for four distinct entities), so it has no one "corresponding"
 * brand. The branded CharacterId / BodyId / WheelId / GliderId types appear
 * on every *specific* field that references these entities by identity.
 */
export const SimpleItemSchema = z.object({
  id: z.number(),
  name: z.string(),
  image_path: z.string(),
});
export type SimpleItem = z.infer<typeof SimpleItemSchema>;

export const TrackItemSchema = z.object({
  id: TrackIdSchema,
  name: z.string(),
  cup_id: CupIdSchema,
  position: z.number(),
  image_path: z.string(),
});
export type TrackItem = z.infer<typeof TrackItemSchema>;

export const CupWithTracksSchema = z.object({
  id: CupIdSchema,
  name: z.string(),
  image_path: z.string(),
  tracks: z.array(TrackItemSchema),
});
export type CupWithTracks = z.infer<typeof CupWithTracksSchema>;

export const DrinkTypeSchema = z.object({
  id: DrinkTypeIdSchema,
  name: z.string(),
  alcoholic: z.boolean(),
  created_by: UserIdSchema.nullable(),
  created_at: z.string(),
});
export type DrinkType = z.infer<typeof DrinkTypeSchema>;

export const DrinkTypeInfoSchema = z.object({
  id: DrinkTypeIdSchema,
  name: z.string(),
  alcoholic: z.boolean(),
});
export type DrinkTypeInfo = z.infer<typeof DrinkTypeInfoSchema>;

export const UserPublicProfileSchema = z.object({
  id: UserIdSchema,
  username: z.string(),
  preferred_character_id: CharacterIdSchema.nullable(),
  preferred_body_id: BodyIdSchema.nullable(),
  preferred_wheel_id: WheelIdSchema.nullable(),
  preferred_glider_id: GliderIdSchema.nullable(),
  preferred_drink_type_id: DrinkTypeIdSchema.nullable(),
  created_at: z.string(),
});
export type UserPublicProfile = z.infer<typeof UserPublicProfileSchema>;

export const UserDetailProfileSchema = z.object({
  id: UserIdSchema,
  username: z.string(),
  preferred_character_id: CharacterIdSchema.nullable(),
  preferred_body_id: BodyIdSchema.nullable(),
  preferred_wheel_id: WheelIdSchema.nullable(),
  preferred_glider_id: GliderIdSchema.nullable(),
  preferred_drink_type: DrinkTypeInfoSchema.nullable(),
  created_at: z.string(),
});
export type UserDetailProfile = z.infer<typeof UserDetailProfileSchema>;

export const RaceSetupSchema = z.object({
  preferred_character_id: CharacterIdSchema,
  preferred_body_id: BodyIdSchema,
  preferred_wheel_id: WheelIdSchema,
  preferred_glider_id: GliderIdSchema,
});
export type RaceSetup = z.infer<typeof RaceSetupSchema>;

export const SessionSummarySchema = z.object({
  id: SessionIdSchema,
  host_username: z.string(),
  participant_count: z.number(),
  race_number: z.number(),
  ruleset: SessionRulesetSchema,
});
export type SessionSummary = z.infer<typeof SessionSummarySchema>;

export const ParticipantInfoSchema = z.object({
  user_id: UserIdSchema,
  username: z.string(),
  joined_at: z.string(),
  left_at: z.string().nullable(),
});
export type ParticipantInfo = z.infer<typeof ParticipantInfoSchema>;

export const RaceSubmissionSchema = z.object({
  user_id: UserIdSchema,
  username: z.string(),
  track_time: z.number(),
  disqualified: z.boolean(),
});
export type RaceSubmission = z.infer<typeof RaceSubmissionSchema>;

export const SessionRaceInfoSchema = z.object({
  id: RaceIdSchema,
  race_number: z.number(),
  track_id: TrackIdSchema,
  track_name: z.string(),
  cup_name: z.string(),
  image_path: z.string(),
  created_at: z.string(),
  submissions: z.array(RaceSubmissionSchema),
});
export type SessionRaceInfo = z.infer<typeof SessionRaceInfoSchema>;

/**
 * Request body for `POST /runs`. Unlike the response DTOs, the ID fields are
 * raw `number` / `string`, not branded: this object is assembled inside the
 * run-entry form from raw component state (race-setup picks, a drink-type
 * pick) and serialized straight to JSON. Branding it would force every form
 * component to mint brands at the call site — exactly what typescript.md § 3
 * ("brand at the parse boundary, not at each call site") tells us not to do.
 * Branded IDs are for data crossing *into* the typed layer (the responses);
 * this is the one type that stays a hand-written `type` with no schema, since
 * the frontend serializes it rather than parsing it.
 */
export type CreateRunRequest = {
  session_race_id: string;
  track_time: number;
  lap1_time: number;
  lap2_time: number;
  lap3_time: number;
  character_id: number;
  body_id: number;
  wheel_id: number;
  glider_id: number;
  drink_type_id: string;
  disqualified: boolean;
};

export const RunDetailSchema = z.object({
  id: RunIdSchema,
  user_id: UserIdSchema,
  session_race_id: RaceIdSchema,
  track_id: TrackIdSchema,
  track_time: z.number(),
  lap1_time: z.number(),
  lap2_time: z.number(),
  lap3_time: z.number(),
  character_id: CharacterIdSchema,
  body_id: BodyIdSchema,
  wheel_id: WheelIdSchema,
  glider_id: GliderIdSchema,
  drink_type_id: DrinkTypeIdSchema,
  drink_type_name: z.string(),
  disqualified: z.boolean(),
  created_at: z.string(),
});
export type RunDetail = z.infer<typeof RunDetailSchema>;

export const RunDefaultsSchema = z.object({
  drink_type_id: DrinkTypeIdSchema.nullable(),
  character_id: CharacterIdSchema.nullable(),
  body_id: BodyIdSchema.nullable(),
  wheel_id: WheelIdSchema.nullable(),
  glider_id: GliderIdSchema.nullable(),
  source: z.enum(['previous_run', 'preferences', 'none']),
});
export type RunDefaults = z.infer<typeof RunDefaultsSchema>;

export const RaceInfoSchema = z.object({
  id: RaceIdSchema,
  race_number: z.number(),
  track_id: TrackIdSchema,
  track_name: z.string(),
  cup_name: z.string(),
  run_count: z.number(),
  created_at: z.string(),
});
export type RaceInfo = z.infer<typeof RaceInfoSchema>;

/** `GET /sessions/mine` — the lone `{ session_id }` field. */
export const MySessionResponseSchema = z.object({
  session_id: SessionIdSchema.nullable().optional(),
});
export type MySessionResponse = z.infer<typeof MySessionResponseSchema>;

export const SessionDetailSchema = z.object({
  id: SessionIdSchema,
  host_id: UserIdSchema,
  host_username: z.string(),
  ruleset: SessionRulesetSchema,
  status: SessionStatusSchema,
  created_at: z.string(),
  participants: z.array(ParticipantInfoSchema),
  race_number: z.number(),
  current_race: SessionRaceInfoSchema.nullable(),
  races: z.array(RaceInfoSchema),
});
export type SessionDetail = z.infer<typeof SessionDetailSchema>;

// ── Notifications (ADR-0038) ──────────────────────────────────────────
//
// Hand-written counterpart of the Rust `NotificationPayload` enum
// (backend/src/services/notifications.rs). Kept in sync via PR review —
// no codegen for MVP. Add a new payload schema + union member here
// whenever a variant is added on the Rust side.

export const PendingRacesDroppedPayloadSchema = z.object({
  kind: z.literal('pending_races_dropped'),
  session_id: SessionIdSchema,
  dropped_count: z.number(),
});
export type PendingRacesDroppedPayload = z.infer<
  typeof PendingRacesDroppedPayloadSchema
>;

// Discriminated union of notification payload kinds. MVP carries one
// variant; future kinds (h2h_lead_changed, track_record_lost,
// leaderboard_rank_changed) join this union as they land.
export const NotificationPayloadSchema = z.discriminatedUnion('kind', [
  PendingRacesDroppedPayloadSchema,
]);
export type NotificationPayload = z.infer<typeof NotificationPayloadSchema>;

export const NotificationSchema = z.object({
  // Notification IDs have no branded type yet — no `NotificationId` is in
  // the PR-B1 brand set, and nothing consumes this id by identity.
  id: z.string(),
  created_at: z.string(),
  read_at: z.string().nullable(),
  payload: NotificationPayloadSchema,
});
export type Notification = z.infer<typeof NotificationSchema>;

export const UnreadCountResponseSchema = z.object({
  count: z.number(),
});
export type UnreadCountResponse = z.infer<typeof UnreadCountResponseSchema>;

// ── Auth responses ──────────────────────────────────────────────────────
//
// Bodies returned by the `/auth/*` endpoints. `user.id` stays a raw string
// rather than a branded `UserId`: it feeds the `User` shape the auth context
// holds, which PR-B1 deliberately left unbranded (it is not an api/types.ts
// DTO). The access token is an opaque JWT string.

export const AuthUserSchema = z.object({
  id: z.string(),
  username: z.string(),
});
export type AuthUser = z.infer<typeof AuthUserSchema>;

/** `POST /auth/login` and `POST /auth/register` — token plus the user. */
export const AuthSessionSchema = z.object({
  access_token: z.string(),
  user: AuthUserSchema,
});
export type AuthSession = z.infer<typeof AuthSessionSchema>;

/** `POST /auth/refresh` — a fresh access token only (no user payload; the
 *  caller decodes the JWT for identity). */
export const TokenRefreshSchema = z.object({
  access_token: z.string(),
});
export type TokenRefresh = z.infer<typeof TokenRefreshSchema>;

/**
 * The decoded JWT access-token payload the frontend reads after a silent
 * refresh (it does not verify the signature — api-contract.md § 5). `sub` is
 * the user id, `username` the handle. Parsed through Zod like every other
 * untyped boundary, since `JSON.parse(atob(...))` is `any`.
 */
export const AccessTokenPayloadSchema = z.object({
  sub: z.string(),
  username: z.string(),
});
export type AccessTokenPayload = z.infer<typeof AccessTokenPayloadSchema>;
