import type {
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
export type SessionStatus = 'active' | 'closed';

/**
 * How a session selects each race's track — mirrors the backend
 * `SessionRuleset` enum (serialized snake_case). Only `random` is wired
 * through session creation today; the other three are valid values of the
 * type but the backend service rejects them until their behavior lands.
 */
export type SessionRuleset =
  | 'random'
  | 'default'
  | 'least_played'
  | 'round_robin';

// ── DTOs ────────────────────────────────────────────────────────────────

export type SimpleItem = {
  // A shared shape for the character / body / wheel / glider pick-lists.
  // `id` is intentionally a raw `number`: the type is structurally
  // polymorphic (it stands in for four distinct entities), so it has no one
  // "corresponding" brand. The branded CharacterId / BodyId / WheelId /
  // GliderId types appear on every *specific* field that references these
  // entities by identity (see UserDetailProfile, RunDetail, RaceSetup).
  id: number;
  name: string;
  image_path: string;
};

export type TrackItem = {
  id: TrackId;
  name: string;
  cup_id: CupId;
  position: number;
  image_path: string;
};

export type CupWithTracks = {
  id: CupId;
  name: string;
  image_path: string;
  tracks: TrackItem[];
};

export type DrinkType = {
  id: DrinkTypeId;
  name: string;
  alcoholic: boolean;
  created_by: UserId | null;
  created_at: string;
};

export type DrinkTypeInfo = {
  id: DrinkTypeId;
  name: string;
  alcoholic: boolean;
};

export type UserPublicProfile = {
  id: UserId;
  username: string;
  preferred_character_id: CharacterId | null;
  preferred_body_id: BodyId | null;
  preferred_wheel_id: WheelId | null;
  preferred_glider_id: GliderId | null;
  preferred_drink_type_id: DrinkTypeId | null;
  created_at: string;
};

export type UserDetailProfile = {
  id: UserId;
  username: string;
  preferred_character_id: CharacterId | null;
  preferred_body_id: BodyId | null;
  preferred_wheel_id: WheelId | null;
  preferred_glider_id: GliderId | null;
  preferred_drink_type: DrinkTypeInfo | null;
  created_at: string;
};

export type RaceSetup = {
  preferred_character_id: CharacterId;
  preferred_body_id: BodyId;
  preferred_wheel_id: WheelId;
  preferred_glider_id: GliderId;
};

export type SessionSummary = {
  id: SessionId;
  host_username: string;
  participant_count: number;
  race_number: number;
  ruleset: SessionRuleset;
};

export type ParticipantInfo = {
  user_id: UserId;
  username: string;
  joined_at: string;
  left_at: string | null;
};

export type RaceSubmission = {
  user_id: UserId;
  username: string;
  track_time: number;
  disqualified: boolean;
};

export type SessionRaceInfo = {
  id: RaceId;
  race_number: number;
  track_id: TrackId;
  track_name: string;
  cup_name: string;
  image_path: string;
  created_at: string;
  submissions: RaceSubmission[];
};

/**
 * Request body for `POST /runs`. Unlike the response DTOs, the ID fields are
 * raw `number` / `string`, not branded: this object is assembled inside the
 * run-entry form from raw component state (race-setup picks, a drink-type
 * pick) and serialized straight to JSON. Branding it would force every form
 * component to mint brands at the call site — exactly what typescript.md § 3
 * ("brand at the parse boundary, not at each call site") and the compliance
 * plan's "mint at the API-helper boundary" rule tell us not to do. Branded
 * IDs are for data crossing *into* the typed layer (the response DTOs).
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

export type RunDetail = {
  id: RunId;
  user_id: UserId;
  session_race_id: RaceId;
  track_id: TrackId;
  track_time: number;
  lap1_time: number;
  lap2_time: number;
  lap3_time: number;
  character_id: CharacterId;
  body_id: BodyId;
  wheel_id: WheelId;
  glider_id: GliderId;
  drink_type_id: DrinkTypeId;
  drink_type_name: string;
  disqualified: boolean;
  created_at: string;
};

export type RunDefaults = {
  drink_type_id: DrinkTypeId | null;
  character_id: CharacterId | null;
  body_id: BodyId | null;
  wheel_id: WheelId | null;
  glider_id: GliderId | null;
  source: 'previous_run' | 'preferences' | 'none';
};

export type RaceInfo = {
  id: RaceId;
  race_number: number;
  track_id: TrackId;
  track_name: string;
  cup_name: string;
  run_count: number;
  created_at: string;
};

export type SessionDetail = {
  id: SessionId;
  host_id: UserId;
  host_username: string;
  ruleset: SessionRuleset;
  status: SessionStatus;
  created_at: string;
  participants: ParticipantInfo[];
  race_number: number;
  current_race: SessionRaceInfo | null;
  races: RaceInfo[];
};

// ── Notifications (ADR-0038) ──────────────────────────────────────────
//
// Hand-written counterpart of the Rust `NotificationPayload` enum
// (backend/src/services/notifications.rs). Kept in sync via PR review —
// no codegen for MVP. Add a new payload type + union member here
// whenever a variant is added on the Rust side.

export type PendingRacesDroppedPayload = {
  kind: 'pending_races_dropped';
  session_id: SessionId;
  dropped_count: number;
};

// Discriminated union of notification payload kinds. MVP carries one
// variant; future kinds (h2h_lead_changed, track_record_lost,
// leaderboard_rank_changed) join this union as they land.
export type NotificationPayload = PendingRacesDroppedPayload;

export type Notification = {
  // Notification IDs have no branded type yet — no `NotificationId` is in
  // the PR-B1 brand set, and nothing consumes this id by identity.
  id: string;
  created_at: string;
  read_at: string | null;
  payload: NotificationPayload;
};

export type UnreadCountResponse = {
  count: number;
};
