## Data Model

### Design Decisions

- **UUID vs INTEGER primary keys.** INTEGER for pre-seeded static data (characters, tracks, cups, bodies, wheels, gliders) — stable, small, human-readable. UUID for user-generated runtime data (users, runs, drink_types) — globally unique, can be generated client-side without a database round trip (important for future offline support).
- **RaceSetup stored inline, not normalized.** Character, body, wheels, and glider IDs are stored directly on the `runs` and `users` tables rather than in separate junction tables. With ~3 million possible combinations (most never used), a reference table is wasteful. Inline storage costs 4 integer columns (16 bytes) — negligible. Migration to a normalized form later is straightforward if needed.
- **Images stored on disk, paths in the database.** Pre-seeded assets (characters, tracks, kart parts) ship as static files. User-uploaded photos (run verification) are saved to a configurable uploads directory. Database stores relative paths (e.g., `images/characters/mario.png`).
- **Fixed-size arrays use separate columns or relational joins.** Lap times (always 3) become `lap1_time`, `lap2_time`, `lap3_time` — simple to query. Cup-to-track relationships use the `cup_id` foreign key on the `tracks` table, not an array on `cups`.
- **Leaderboards separate alcoholic and non-alcoholic runs by default**, with a combined view available.
- **Nullability defaults to NOT NULL** unless there is a clear reason for the data to be optional. Nullable columns map to `Option<T>` in Rust, adding handling overhead.
- **"Previous" setup is derived, not stored.** The user's last-used race setup and drink type are queried from their most recent run, not duplicated on the users table. Only "preferred" (explicitly set) values are stored on users.
- **Database encryption** via SQLCipher is possible but deferred past v1.
- **No separate `created_by` column on sessions.** `host_id` carries the original creator until host transfers on leave. No current product feature uses the original-creator information. If needed later, re-adding the column is one append-only migration.

### Naming Conventions

- **Table names:** plural, snake_case (`drink_types`, `characters`).
- **Column names:** snake_case (`track_time`, `created_at`).
- **Foreign keys:** `{referenced_table_singular}_id` (`character_id`, `cup_id`).
- **Primary keys:** `id`.

### Users

User-modifiable: yes (own profile, preferred race setup).

```
users
├── id: UUID (primary key)
├── username: TEXT (unique, not null, 1-30 characters)
├── email: TEXT (unique, nullable — for account recovery)
├── password_hash: TEXT (not null)
├── preferred_character_id: INTEGER (foreign key -> characters, nullable)
├── preferred_body_id: INTEGER (foreign key -> bodies, nullable)
├── preferred_wheel_id: INTEGER (foreign key -> wheels, nullable)
├── preferred_glider_id: INTEGER (foreign key -> gliders, nullable)
├── preferred_drink_type_id: UUID (foreign key -> drink_types, nullable)
├── refresh_token_version: INTEGER (not null, default 0)
├── created_at: TIMESTAMP (not null)
└── updated_at: TIMESTAMP (not null)
```

Notes:
- Preferred race setup columns are nullable (new user hasn't picked yet). All-or-nothing: either all four are set or none are. Enforced in application code, not the database.
- Preferred drink type is nullable (new user hasn't picked yet).
- SQLite allows multiple NULLs in a UNIQUE column (email), which is the desired behavior.
- `email` validated as valid format in application code if provided.
- "Previous" race setup and drink type are derived from the user's most recent run — not stored here. The run form defaults to previous (last run), falling back to preferred (profile), falling back to empty (new user).
- Preferred race setup will eventually be retired once OCR extracts setup from TV screen photos.

### Characters

Pre-seeded, read-only. All MK8 Deluxe characters (including DLC).

```
characters
├── id: INTEGER (primary key, not null)
├── name: TEXT (unique, not null)
└── image_path: TEXT (not null)
```

### Bodies

Pre-seeded, read-only. All MK8 Deluxe vehicle bodies.

```
bodies
├── id: INTEGER (primary key, not null)
├── name: TEXT (unique, not null)
└── image_path: TEXT (not null)
```

### Wheels

Pre-seeded, read-only. All MK8 Deluxe wheel sets.

```
wheels
├── id: INTEGER (primary key, not null)
├── name: TEXT (unique, not null)
└── image_path: TEXT (not null)
```

Note: FK column is `wheel_id` (singular, per naming convention). The UI displays the label as "Wheels" since each entry represents a set of four.

### Gliders

Pre-seeded, read-only. All MK8 Deluxe glider attachments.

```
gliders
├── id: INTEGER (primary key, not null)
├── name: TEXT (unique, not null)
└── image_path: TEXT (not null)
```

### Cups

Pre-seeded, read-only. All MK8 Deluxe cups (including DLC).

```
cups
├── id: INTEGER (primary key, not null)
├── name: TEXT (unique, not null)
└── image_path: TEXT (not null)
```

Note: Cup-to-track mapping is handled by the `cup_id` foreign key on the `tracks` table. Application-level validation ensures each cup has exactly 4 tracks after seeding. Cup IDs are assigned in game grid order (top-left to bottom-right, originals first, then DLC).

### Tracks

Pre-seeded, read-only. All MK8 Deluxe tracks (including DLC). Track names include console prefix for retro tracks (e.g., "GBA Rainbow Road", "SNES Rainbow Road"). MK8-native tracks have no prefix (e.g., "Rainbow Road").

```
tracks
├── id: INTEGER (primary key, not null)
├── name: TEXT (unique, not null)
├── cup_id: INTEGER (foreign key -> cups, not null)
├── position: INTEGER (not null, 1-4, order within the cup)
└── image_path: TEXT (not null)
```

Constraints:
- Composite unique on `(cup_id, position)` — no two tracks in the same slot of a cup.

### Drink Types

User-created. Specific beverages used during runs (e.g., "Molson Canadian", "LaCroix Pamplemousse"). Users can submit new drink types anywhere a drink selector appears. Deduplication is handled via deterministic UUID.

```
drink_types
├── id: UUID (primary key, deterministic via uuid_v5 of (uppercased name, alcoholic))
├── name: TEXT (not null, stored as-entered by first creator)
├── alcoholic: BOOLEAN (not null)
├── created_by: UUID (foreign key -> users, nullable — null for pre-seeded entries)
└── created_at: TIMESTAMP (not null)
```

Notes:
- UUID derived from `uuid_v5(DRINK_TYPE_NAMESPACE, "{uppercase(name)}\x1f{alcoholic}")`. Name matched case-insensitively; the `alcoholic` flag is part of the identity, so the two forms of the same name (e.g. "Punch") are distinct drinks with distinct IDs.
- A composite `UNIQUE(name, alcoholic)` index backstops the app-level dedup at the DB layer (the derived PK is the primary guard).
- If a user submits a drink whose `(name, alcoholic)` already exists (any casing of the name), the app returns the existing entry. The same name with the *other* flag creates a distinct drink.
- `alcoholic` must be explicitly set by the user (no default).
- Image support for drink types deferred to a future phase.

### Sessions

The organizational unit for group play. A session is like a lobby — players join, race tracks together, and leave when done. All run recording happens within session context.

```
sessions
├── id: UUID (primary key)
├── host_id: UUID (foreign key -> users, not null — starts as the user who created the session; transfers on leave)
├── ruleset: TEXT (not null — "random", "default", "least_played", "round_robin")
├── least_played_drink_category: TEXT (nullable — "alcoholic" or "non_alcoholic"; only used when ruleset is "least_played")
├── status: TEXT (not null — "active", "closed")
└── created_at: TIMESTAMP (not null)
```

Notes:
- `host_id` is set to the creating user when the session is created. If the host leaves, host role transfers to the earliest-joined remaining participant.
- `least_played_drink_category` stores values as `"alcoholic"` or `"non_alcoholic"` (snake_case, per database convention). The frontend maps these to display text with hyphens ("non-alcoholic").
- Ruleset-specific config uses explicit nullable columns rather than a JSON blob. With four well-defined rulesets, explicit columns are safer (database can enforce CHECK constraints) and queryable. New config options require a migration, but that's the right tradeoff for known rulesets.
- Session liveness is **activity-derived** per [ADR-0035](./decisions/0035-race-anchored-session-lifetime.md) and [ADR-0037](./decisions/0037-pending-races-dropped-on-session-close.md). There is no maintained `last_activity_at` column — each meaningful user action lands its own row (`session_races`, `runs`, `session_participants`, `session_race_participations`) and liveness reads from those. Two related predicates:
  - **Read-path lockout** (`check_not_in_any_session`, `get_active_session_id`): a user is held in a session iff `status = 'active'` AND (a race exists within the last hour OR `created_at` is within the last hour). This decouples user lockout from sweep timing.
  - **Stale-session sweeper** (`close_stale_sessions`, ADR-0037): a session is *stale* iff `status = 'active'`, `created_at` is over an hour old, AND no activity of any kind happened within the last hour — no new race, no submitted run, no join or leave, no skipped pending race (five signals). With the per-race timer gone from pending derivation (see Pending Race Tracking), the sweeper is the sole gatekeeper for closing dormant sessions.
- A periodic Tokio task (`close_stale_sessions`, every 15 minutes) flips `status` to `'closed'` for stale sessions. Clean exits (last participant leaves) close the session inline via `leave_session`'s `transfer_host_or_close` path; the sweeper handles only the abandoned case (tab closed, phone died). **Closing a session — by either path — drops every unresolved pending race in it** (`dropped_at` stamped on the `session_race_participations` row) and records a `notifications` row per affected user (ADR-0037 / ADR-0038). The drop and the notifications are atomic with the close.
- Future consideration: `password_hash` column for session passwords. Deferred — the `POST /sessions/:id/join` endpoint is designed as a dedicated action so password checking can be added later without restructuring the join flow.
- A user can only be active in one session at a time (enforced by a partial unique index on `session_participants(user_id) WHERE left_at IS NULL`).
- **Session UI icons:** The host is indicated by a 🏠 (house) icon, not a crown. The 👑 (crown) is reserved for the player with the most fastest track times in the session — an earned distinction, not a role.

### Session Participants

Tracks current participation state for each (session, user) pair. **One row per (session, user)** — leave/rejoin mutates this row rather than appending a new one.

```
session_participants
├── id: UUID (primary key)
├── session_id: UUID (foreign key -> sessions, not null)
├── user_id: UUID (foreign key -> users, not null)
├── joined_at: TIMESTAMP (not null — start of current presence segment)
└── left_at: TIMESTAMP (nullable — null means currently in session)
```

Constraints:
- Composite unique on `(session_id, user_id)` — at most one row per (session, user).

Notes:
- "Currently in session" = `left_at` is null.
- `joined_at` is **monotonic** — set when the row is first inserted, never reset on rejoin. Genuinely means "when did this user first join this session."
- On rejoin (any duration): clear `left_at` (set to NULL). `joined_at` is untouched. Pending races remain accessible for as long as the session is alive (per [ADR-0037](./decisions/0037-pending-races-dropped-on-session-close.md)) — a user who leaves with pending races can rejoin and act on them as long as someone else kept the session alive. If everyone leaves, the session closes and the pending races are dropped. No 5-minute grace concept, and no per-race timer — the session is the deadline.
- **Settle-on-new-join.** The schema's partial unique index `UNIQUE(user_id) WHERE left_at IS NULL` allows at most one active participation per user. When a user starts or joins a *new* session (`create_session` / `join_session`), the same transaction also sets `left_at = NOW()` on any pre-existing `left_at IS NULL` row for that user in a *different* session. Semantically: starting or joining a new session is an implicit leave of any abandoned one. Without this, the application-level race-derived liveness predicate would let the check pass while the INSERT collided with the partial unique index — so lockout would still be sweep-bound. The stale session's `status` and other participants' rows are left to the sweeper (eventual consistency).
- Per-race presence (which races the user was actually present for at creation time) is NOT derived from this table — see `session_race_participations`.

### Session Races

Each race within a session. Tracks the sequence of tracks raced and who chose them.

```
session_races
├── id: UUID (primary key)
├── session_id: UUID (foreign key -> sessions, not null)
├── race_number: INTEGER (not null — sequential within session, starting at 1)
├── track_id: INTEGER (foreign key -> tracks, not null)
├── chosen_by: UUID (foreign key -> users, nullable — null for random/automatic selection)
└── created_at: TIMESTAMP (not null)
```

Constraints:
- Composite unique on `(session_id, race_number)` — no duplicate race numbers within a session.

Notes:
- `chosen_by` is null when the track was selected automatically (random ruleset, or everyone recused in default/round-robin).
- Race numbers are sequential and gapless within a session.
- On race creation, the server inserts one `session_race_participations` row per currently-present user in the same transaction — see below.

### Session Race Participations

Captures which users were present when each session race was created, plus per-(race, user) skip status. This table is what makes pending-race tracking possible without a participation history walk.

```
session_race_participations
├── session_race_id: UUID (foreign key -> session_races, not null)
├── user_id: UUID (foreign key -> users, not null)
├── created_at: TIMESTAMP (not null)
├── skipped_at: TIMESTAMP (nullable — set when the user explicitly skips this race)
└── dropped_at: TIMESTAMP (nullable — set when the session closes around an unresolved pending row)
```

Constraints:
- Primary key: `(session_race_id, user_id)` — at most one row per (race, user). Idempotent skip via PK conflict handling.
- Index: `(user_id)` for "what's pending for this user" queries.

Notes:
- Inserted at race-creation time, in the same transaction as the `session_races` INSERT, for every user with `session_participants.left_at IS NULL` in this session.
- Existence of a row = "this user was present when this race was created" (the primary fact this table proves).
- A `runs` row for `(session_race_id, user_id)` = user submitted; pending state cleared by row presence in `runs`.
- `skipped_at IS NOT NULL` = user explicitly forfeited this race.
- `dropped_at IS NOT NULL` = the session closed (clean last-leave or sweeper) while this row was still an unresolved pending race ([ADR-0037](./decisions/0037-pending-races-dropped-on-session-close.md)). Distinct from `skipped` — `skipped` is "the user chose to forfeit," `dropped` is "the session ended around them." `skipped` and `raced` both beat `dropped`: a row that already has `skipped_at` set, or a `runs` row, is never stamped `dropped_at`.
- Rows are never deleted. After a session closes, rows remain in the DB for history; they just become inaccessible via normal API paths (the close stamps `dropped_at`, which the Pending Race Tracking derivation below filters on).

**Per-(race, user) status enum** (derived, not stored as such). These four mutually-exclusive states apply only to `(race, user)` pairs where a `session_race_participations` row exists — i.e. where the user was present at race creation. A user who joined the session *after* a race was created has no row for it and none of the states apply.

| Status | Derivation |
|---|---|
| `unraced` | row exists, no `runs` row, `skipped_at IS NULL`, `dropped_at IS NULL` (this is "pending") |
| `raced` | a `runs` row exists for `(race, user)` |
| `skipped` | `skipped_at IS NOT NULL` |
| `dropped` | `dropped_at IS NOT NULL` |

### Runs

The core table. One row per player per race attempt. User-created, immutable for regular users (times cannot be edited after creation; admin can edit), deletable by owner or admin.

```
runs
├── id: UUID (primary key)
├── user_id: UUID (foreign key -> users, not null)
├── session_race_id: UUID (foreign key -> session_races, not null)
├── track_id: INTEGER (foreign key -> tracks, not null)
├── character_id: INTEGER (foreign key -> characters, not null)
├── body_id: INTEGER (foreign key -> bodies, not null)
├── wheel_id: INTEGER (foreign key -> wheels, not null)
├── glider_id: INTEGER (foreign key -> gliders, not null)
├── track_time: INTEGER (milliseconds, not null, must be positive)
├── lap1_time: INTEGER (milliseconds, not null, must be positive and non-zero)
├── lap2_time: INTEGER (milliseconds, not null, must be positive and non-zero)
├── lap3_time: INTEGER (milliseconds, not null, must be positive and non-zero)
├── drink_type_id: UUID (foreign key -> drink_types, not null)
├── disqualified: BOOLEAN (not null, default false)
├── photo_path: TEXT (nullable — optional but encouraged; required for record-breaking runs)
├── created_at: TIMESTAMP (not null, defaults to current time, optionally user-provided)
└── notes: TEXT (nullable — freeform; may be mined for future structured columns)
```

Notes:
- `session_race_id` is NOT NULL — all runs belong to a session race in the MVP. Solo racing is a one-person session. Future enhancement: make nullable to allow standalone runs without session context, streamlining the solo experience.
- `track_id` is technically redundant with `session_race_id` (the session race knows the track), but stored for query convenience and to allow future standalone runs.
- `disqualified` marks runs where the player didn't finish their drink before finishing the race. Self-reported (honor system). DQ'd runs are recorded but excluded from H2H win/loss tallies and leaderboard positions.

Validation (application-level):
- `track_time` must be positive.
- All three lap times must be positive and non-zero.
- Lap times must sum exactly to `track_time`. The frontend warns if they don't match; the backend rejects the submission.
- Race setup columns pre-fill from previous run (or preferred from profile), but are all required.
- `track_id` must match the track on the referenced `session_race_id`.

Record-breaking run enforcement:
- When a run is created, the backend checks if the time is a new track record (per drink category).
- If it is a record and no photo is attached, the run is saved and an auto-generated flag is created in `run_flags` with `hide_while_pending = true`.
- When a photo is uploaded via `POST /runs/:id/photo`, the auto-flag is resolved automatically.
- If the photo never arrives, the run remains flagged and hidden from leaderboards. Admin can see and act on it.
- DQ'd runs cannot be track records.

Future (OCR):
- The end-of-race TV screen shows race setup, track, and all 3 lap times. OCR will eventually extract all of this automatically.
- Photos on all runs provide training data for OCR, even when not required.
- Once OCR is reliable, the `created_at` override becomes unnecessary (live capture only).

### Run Flags

Tracks review requests for runs. Supports both user-initiated flags and auto-generated flags (e.g., record-breaking runs without photos).

```
run_flags
├── id: UUID (primary key)
├── run_id: UUID (foreign key -> runs, not null)
├── reason: TEXT (not null — from preset list or auto-generated)
├── note: TEXT (nullable — user-provided context)
├── hide_while_pending: BOOLEAN (not null, default false)
├── auto_generated: BOOLEAN (not null, default false)
├── created_at: TIMESTAMP (not null)
└── resolved_at: TIMESTAMP (nullable — set when admin acts)
```

Preset flag reasons (user-initiated):
- "Time is incorrect"
- "Wrong track"
- "Wrong race setup"
- "Wrong drink type"
- "Other"

Auto-generated flag reasons:
- "Record requires photo verification"

Storage values (snake_case): `time_is_incorrect`, `wrong_track`, `wrong_race_setup`, `wrong_drink_type`, `other`, `record_requires_photo_verification`. The frontend renders the display text shown above; the wire and DB use the snake_case form.

Notes:
- A run is considered flagged if it has an unresolved entry in `run_flags` (where `resolved_at` is null).
- Users can only flag their own runs, and only if the run has a photo attached.
- When flagging, users choose whether the run stays visible or is hidden while under review (`hide_while_pending`).
- Auto-generated flags always set `hide_while_pending = true`.
- The `flagged_for_review` column on the `runs` table is removed — flag status is determined by the presence of an unresolved `run_flags` row.
- `run_id` is NOT unique — a run can have multiple flags, both resolved and unresolved. Different issues (e.g., wrong time and wrong race setup) are tracked as separate flags and resolved independently. Resolved flags are kept as audit history. Application code prevents duplicate flags (same run + same reason while unresolved).

### Notifications

A per-user inbox of asynchronous events, per [ADR-0038](./decisions/0038-notifications-system.md). Each event materializes one row; dismissal flips `read_at`. The first (MVP) consumer is the pending-races-dropped event from ADR-0037.

```
notifications
├── id: UUID (primary key)
├── user_id: UUID (foreign key -> users, not null — recipient)
├── kind: TEXT (not null — discriminator, snake_case)
├── payload: TEXT (not null — kind-specific structured data, JSON)
├── created_at: TIMESTAMP (not null)
└── read_at: TIMESTAMP (nullable — set when the user dismisses)
```

Constraints:
- Foreign key on `user_id` with **ON DELETE CASCADE** — when a user is deleted their inbox goes with them (notifications carry no cross-user audit value, unlike `session_race_participations`).
- Index `idx_notifications_user_unread` on `(user_id, created_at DESC) WHERE read_at IS NULL` — the "list my unread, newest first" hot path.
- Index `idx_notifications_user_created` on `(user_id, created_at DESC)` — paginated "show me everything, including read."

Notes:
- **Append-only.** Each event is its own row; there is no supersession. "Three drops" yields three rows.
- **Keep forever.** No retention task for MVP — bounded well below "concern" at friend-group scale. A future Issue adds retention if it ever matters.
- `kind` is a snake_case discriminator (`pending_races_dropped`, …) lifted out of the JSON `payload` for indexing. `payload` stores the kind-specific structured body — the serde-tagged `NotificationPayload` enum on the Rust side. JSON-as-TEXT follows [ADR-0028](./decisions/0028-timestamp-storage-as-iso8601-text.md)'s same-shape rule; the DB never queries inside it.
- Every notification INSERT runs in the same transaction as the triggering write (ADR-0038 § Atomicity) — no worker, no queue. The pending-drops consumer writes one row per affected user inside the session-close transaction.

### Head-to-Head Derivation

Head-to-head records are derived from session data, not stored separately. A "win" is when two players both submitted non-DQ'd runs for the same `session_race_id` and one had a faster `track_time`. Ties (identical times) count as 0-0 — neither a win nor a loss.

The query: find all `session_race_id` values where both User A and User B have a non-DQ'd run, compare `track_time`, tally wins and losses. The relationship is symmetric — A's wins are B's losses and vice versa.

H2H does not distinguish between alcoholic and non-alcoholic runs. If a drinker and a non-drinker race in the same session, their result counts for H2H. Drink category separation only applies to leaderboards.

This approach supports an unbounded number of rivals — no tracking table needed, no cap on how many people you can have H2H records with. Performance is a simple indexed join at the scale of a friend group. If it ever needs optimization, a materialized cache can be added later.

### Pending Race Tracking

Within a session, a participant may have "pending" races — session races they were present for but haven't yet submitted a time. The UI caps pending races at 3, but the schema places no limit, allowing this cap to be adjusted later.

**Derivation** (per [ADR-0037](./decisions/0037-pending-races-dropped-on-session-close.md)). A `session_race_participations` row represents pending state for user `U` and race `SR` iff **all** of the following hold:

1. The row exists (i.e. `U` was present when `SR` was created).
2. `skipped_at IS NULL` on the row.
3. No `runs` row exists for `(SR.id, U.id)`.
4. `dropped_at IS NULL` on the row.

Four clauses, no per-race timer and no `sessions.status` filter. **The session is the deadline** (ADR-0037): a pending race stays submittable for as long as the session is alive. When a session closes — clean last-leave or sweeper — the close transaction stamps `dropped_at` on every unresolved row, so clause 4 alone captures "the session is over." A race created four hours ago in a still-active session is still pending. This is the narrowing of [ADR-0035](./decisions/0035-race-anchored-session-lifetime.md)'s per-race 1-hour window, which is gone.

Pending races are returned ordered by `session_races.race_number ASC`. The API returns all; the UI applies the 3-cap.

**Submission rules.** When a participant has pending races, they submit in order (oldest first). For each pending race, they can submit a time or skip. Submitting out of order is not allowed — this prevents cherry-picking favorable tracks to game H2H records. Skipping is permitted in any order (skipping doesn't help cherry-pick because no time is recorded).

**Session advancement.** If the session advances while a participant hasn't submitted, they see their pending list (oldest first) when they go to submit, and must resolve them in order before submitting for the current race. This ensures no one person holds up the group, consistent with "never feel rushed."

**Forfeiture vs. deletion.** Resolved pending records — `skipped`, `raced`, or `dropped` — are **not deleted** from `session_race_participations`. They remain as historical state and become inaccessible via the derivation above. This preserves the audit trail of "what was pending at any moment" for debugging and future analytics.

### Session Rulesets

Each session uses one ruleset that determines how tracks are selected. The ruleset is chosen at session creation. Development order: Random → Default → Least Played → Round-robin.

**Random:** A random track is chosen each time, without replacement. The host triggers each new track selection. For when you just don't care what you're racing.

**Default:** The person with the fewest points on the global leaderboard always chooses. This biases the game toward "self-leveling" — keeping the pack tight and the leaderboard interesting. Tiebreaker: the user with the oldest account creation time is chosen (increasing the likelihood that the chooser has the most site experience and will keep things moving). The chosen person can recuse — they're removed from consideration until a race is completed or they leave and rejoin. If everyone recuses, a random track is chosen.

**Least Played:** The track with the fewest submitted runs is chosen automatically. "Fewest" is scoped to a drink category (alcoholic or non-alcoholic), chosen at session creation. The player's preferred drink type determines which option is pre-selected. The host triggers each new track selection.

**Round-robin:** Two groups: "Can Choose" and "Can't Choose." Everyone starts in "Can Choose." The earliest session joiner in "Can Choose" picks the track. After choosing, they move to "Can't Choose." Recusing also moves you to "Can't Choose." If "Can Choose" is empty, a random track is chosen and everyone resets to "Can Choose." This prevents the decision from stalling on someone not paying attention.

**Skip turn vs. recusal:** "Skip turn" (`POST /sessions/:id/skip-turn`) and self-recusal are distinct actions. Recusal means "I don't want to choose" — the chooser opts out. Skip turn means "I don't want to wait for you" — any participant can trigger it to move past a stalled chooser. Per-ruleset behavior of skip-turn: in Default, the next eligible player (by fewest leaderboard points) is offered the choice. In Round-robin, the skipped player moves to "Can't Choose," same as if they'd chosen or recused. If skip-turn or recusal cycles through everyone (Default) or empties "Can Choose" (Round-robin), a random track is selected automatically.

**Chooser state persistence:** Round-robin chooser rotation is derived from `session_races.chosen_by` — the "Can Choose" group is everyone who hasn't chosen since the last reset. Default recusal is transient (in-memory, resets per race) — if the server restarts mid-race, the chooser gets re-offered, which is harmless.

**Timeout handling (all rulesets):** Every decision point is event-driven — no timers pressuring players. If a chooser stalls, any participant can trigger "skip turn" to pass to the next person per the ruleset's logic. MVP fallback for truly stuck sessions: leave and start a new one. Vote-to-kick deferred.

## Document history

- 2026-05-05 — Extracted from `docs/design.md` as part of PR 1 (docs restructure foundation). See `docs/designs/archive/2026-05-04-design-doc-restructure.md` §5.1.
- 2026-05-13 — § `run_flags` now enumerates the canonical snake_case storage values for `reason` alongside the display text, mirroring the storage-vs-display split already documented for `sessions.{ruleset,status,least_played_drink_category}`. The values are load-bearing as of PR-D3 ([#120](https://github.com/brendanbyrne/beerio-kart/issues/120) / PR [#148](https://github.com/brendanbyrne/beerio-kart/pull/148)), which committed them via `RunFlagReason::string_value` annotations on the `DeriveActiveEnum` backing the column. Review feedback from PR #148.
- 2026-05-15 — Updated the 2026-05-05 history entry's path reference for the design-doc-restructure record (now archived under `designs/archive/`). Companion to PR [#160](https://github.com/brendanbyrne/beerio-kart/pull/160) / Issue [#159](https://github.com/brendanbyrne/beerio-kart/issues/159).
- 2026-05-16 — ADR-0037 + ADR-0038. `session_race_participations` gains a nullable `dropped_at` column and the four-state per-(race, user) status enum (`unraced` / `raced` / `skipped` / `dropped`). Pending Race Tracking derivation simplified to four clauses — the per-race 1-hour timer and the `sessions.status` filter are gone; the session is the deadline. Session liveness note rewritten: the stale-session sweeper now uses a five-signal activity predicate, and closing a session drops its unresolved pending races + records notifications. Session Participants rejoin rule updated to "rejoin while the session is alive." New `notifications` table section added (ADR-0038 inbox). Issues [#51](https://github.com/brendanbyrne/beerio-kart/issues/51), [#58](https://github.com/brendanbyrne/beerio-kart/issues/58), [#164](https://github.com/brendanbyrne/beerio-kart/issues/164).
- 2026-05-28 — § Drink Types: a drink's identity is now `(uppercased name, alcoholic)`, not name alone. The derived UUID hashes `"{uppercase(name)}\x1f{alcoholic}"`, the standalone `UNIQUE(name)` constraint became composite `UNIQUE(name, alcoholic)`, and the dedup note now distinguishes a same-`(name, alcoholic)` collision (returns existing) from the same name with the other flag (distinct drink). Companion to PR [#213](https://github.com/brendanbyrne/beerio-kart/pull/213) / Issue [#212](https://github.com/brendanbyrne/beerio-kart/issues/212). Review feedback from PR #213.
- 2026-05-31 — Added a `### Naming Conventions` subsection (table/column/FK/PK snake_case rules), making this file the canonical home (#220/#223). The rules previously lived in `design.md` § Naming Conventions (now removed) and were duplicated in `backend/CLAUDE.md` § Naming (now a pointer here).
