# Beerio Kart - Architecture Design Document

## Overview

Beerio Kart is a mobile-first web app for tracking times and stats for the Mario Kart 8 Deluxe drinking game. Players race one at a time in Time Trial mode (150cc only). You can't touch the controller while touching your drink (one 12oz beer or sparkling water). The app tracks personal times per track, head-to-head records, and leaderboards.

## Rules of the Game

1. Players race one at a time using Time Trial mode in Mario Kart 8 Deluxe.
2. You cannot touch the controller while touching your drink.
3. The drink is one 12oz beer or one 12oz sparkling water, poured into a cup.
4. You may restart the race if it is before the end of the first lap AND you haven't had any of your drink yet.
5. If you finish the race before you finish your drink, your run is disqualified (DQ). This is self-reported on the honor system.
6. Played round robin — all players race the same track.
7. Fastest non-DQ'd time wins.

## High Level Principles

Consideration of these principles should go into every design decision made. If a decision can't tie itself back to at least one of these principles, then it should be questioned.

- **First class experience even if you don't drink.** Tailor each player's experience to their preference to drink or not.
- **Never a burden to use.** What's the point if it's not making things easier.
- **You should never feel rushed, unless absolutely necessary.** Enjoying each others' company is the point.
- **You can play in the same room as much as across the world.** Nothing should require that you're using different TVs or the same TV.
- **Should be usable by only one hand.** The other one could be wet.

## Design Goals

- **Minimize number of choices in any moment.** It's hard to use wrong, if you can only do what you need to.
- **Prefer simple interactions over complex ones.** Doing nothing > "single use button" > swipe screen > togglable button > swipe specific objects > multiple buttons > typing.
- **Provide sensible defaults whenever possible.** You can usually assume what most likely decision a person will make.

## Technical Constraints

- **Don't overengineer before OCR.** Many corner cases (time validation, race setup entry, session tracking) will be solved by OCR. Design the MVP for manual entry with hooks for OCR to slot in later.
- **SQLite STRICT mode on lookup / static tables.** Enforces type checking at the DB level where it adds real safety (static game data tables: `characters`, `bodies`, `wheels`, `gliders`, `cups`, `tracks`). Dropped on tables with timestamp columns (`users`, `sessions`, `session_participants`, `session_races`, `runs`, `drink_types`, `run_flags`) because SQLite STRICT does not accept the `DATETIME` type, and proper Rust-side `DateTime<Utc>` typing outweighs the duplicative safety (Rust's type system already enforces column types for this Rust-only codebase). Requires SQLite 3.37+ (2021).
- **Must work on Firefox.** Firefox is a target browser alongside Chrome/Safari mobile. Avoid Chrome-only APIs or `-webkit-` prefixes without Firefox equivalents. Test on Firefox before shipping UI changes.

## Tech Stack

| Layer       | Technology                    | Rationale                                                    |
|-------------|-------------------------------|--------------------------------------------------------------|
| Backend     | Rust + Axum                   | Learning opportunity; strong async/WebSocket support          |
| Frontend    | React + Vite                  | Largest ecosystem for mobile-web; camera API support          |
| Styling     | Tailwind CSS                  | Utility-first; fast iteration; mobile-first by convention     |
| ORM         | SeaORM (backed by sqlx)       | Rust-native query API; database-agnostic; easier SQLite->PostgreSQL migration |
| Database    | SQLite                        | File-based; no separate server; sufficient for this scale     |
| Package mgr | Bun                           | Drop-in npm replacement; faster installs and script running   |
| Containers  | Dockerfile + compose.yaml    | Works with Docker or Podman                                  |
| Serving     | Axum (single container)       | Axum serves both the API and the frontend static files via `tower-http::ServeDir`. No separate nginx or frontend container. |

### ORM Usage

Use SeaORM's builder API for single-table reads and writes (`Entity::find()`, `Entity::find_by_id`, `ActiveModel::insert` / `update`). Drop to raw SQL via `find_by_statement` only for multi-table JOINs where the builder's JOIN ergonomics become clumsy (most of `get_session_detail`'s helper queries, `list_runs`'s dynamic filters). Avoid hand-rolling SQL for single-table ops — the builder gives you type safety and refactor-proofness for free.

## Observability

### Crates

- **`tracing`** — structured, leveled logging facade (used throughout application code)
- **`tracing-subscriber`** — formats and emits log output (with `fmt` and `env-filter` features)
- **`tower-http`** — provides `TraceLayer` middleware for automatic HTTP request/response logging (method, path, status, duration)

### Log level conventions

- `error` — unexpected failures (DB errors, hashing failures, token creation failures)
- `warn` — suspicious but recoverable (e.g., rate-limit warnings in the future)
- `info` — request lifecycle, startup, seeding complete
- `debug` — detailed diagnostics during development

### Error response pattern

All route handlers return `Result<impl IntoResponse, AppError>` where `AppError` (`src/error.rs`) is a unified error type that implements Axum's `IntoResponse` trait. This enables idiomatic `?` error propagation instead of verbose match arms.

**`AppError` variants:**

| Variant | HTTP Status | User-facing message |
|---------|-------------|---------------------|
| `BadRequest(msg)` | 400 | The provided `msg` |
| `Unauthorized(msg)` | 401 | The provided `msg` |
| `Forbidden(msg)` | 403 | The provided `msg` |
| `NotFound(msg)` | 404 | The provided `msg` |
| `Conflict(msg)` | 409 | The provided `msg` |
| `Internal(log_msg)` | 500 | Generic `"Internal server error"` |

**Key behaviors:**
- `Internal` logs the real error via `tracing::error!` but returns a generic message to the client — internal details are never exposed.
- `From` impls for `sea_orm::DbErr`, `jsonwebtoken::errors::Error`, and `argon2::password_hash::Error` auto-convert library errors into `Internal`, so `?` works directly on DB queries, token operations, and password hashing.
- Client-facing errors (`BadRequest`, `Unauthorized`, etc.) are always constructed explicitly — they require human judgment about the appropriate status code and message.

**Response format:** All errors return JSON: `{ "error": "<message>" }`

### Configuration

Log output is controlled via the `RUST_LOG` environment variable. Defaults to `info` if not set. Examples:
- `RUST_LOG=debug` — all debug-level output
- `RUST_LOG=beerio_kart=debug` — debug only for application code, info for dependencies

## Coverage & CI

- **Local:** `just coverage` generates an HTML report; `just coverage-summary` prints a text summary.
- **CI:** GitHub Actions runs `cargo-llvm-cov` on every PR and push to main. Results upload to Codecov.
- **Exclusions:** `entities/` (SeaORM codegen), `migration/`, `main.rs` (wiring), `seed.rs` (startup), `frontend/` (not yet instrumented). Only business logic counts.
- **Policy:** No regression from the base branch (`target: auto`, 0.5% threshold). New/changed code must be 80% covered (`patch: 80%`). As coverage rises from the audit, we'll lock in a hard floor.
- **Reports:** Codecov posts a PR comment with coverage delta, patch coverage, and per-file breakdown.

## Naming Conventions

- Table names: plural, snake_case (`drink_types`, `characters`)
- Column names: snake_case (`track_time`, `created_at`)
- Foreign keys: `{referenced_table_singular}_id` (`character_id`, `cup_id`)
- Primary keys: `id`

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
├── id: UUID (primary key, deterministic via uuid_v5 of uppercased name)
├── name: TEXT (unique, not null, stored as-entered by first creator)
├── alcoholic: BOOLEAN (not null)
├── created_by: UUID (foreign key -> users, nullable — null for pre-seeded entries)
└── created_at: TIMESTAMP (not null)
```

Notes:
- UUID derived from `uuid_v5(DRINK_TYPE_NAMESPACE, uppercase(name))`. Ensures case-insensitive deduplication at the database level.
- If a user submits a drink that already exists (different casing), the app detects the UUID collision, shows the existing entry, and offers to use it.
- `alcoholic` must be explicitly set by the user (no default).
- Image support for drink types deferred to a future phase.

### Sessions

The organizational unit for group play. A session is like a lobby — players join, race tracks together, and leave when done. All run recording happens within session context.

```
sessions
├── id: UUID (primary key)
├── created_by: UUID (foreign key -> users, not null)
├── host_id: UUID (foreign key -> users, not null — current host, transfers on leave)
├── ruleset: TEXT (not null — "random", "default", "least_played", "round_robin")
├── least_played_drink_category: TEXT (nullable — "alcoholic" or "non_alcoholic"; only used when ruleset is "least_played")
├── status: TEXT (not null — "active", "closed")
├── created_at: TIMESTAMP (not null)
└── last_activity_at: TIMESTAMP (not null)
```

Notes:
- `host_id` starts as `created_by`. If the host leaves, host role transfers to the earliest-joined remaining participant.
- `least_played_drink_category` stores values as `"alcoholic"` or `"non_alcoholic"` (snake_case, per database convention). The frontend maps these to display text with hyphens ("non-alcoholic").
- Ruleset-specific config uses explicit nullable columns rather than a JSON blob. With four well-defined rulesets, explicit columns are safer (database can enforce CHECK constraints) and queryable. New config options require a migration, but that's the right tradeoff for known rulesets.
- Session auto-closes after 1 hour of no activity. No further run submissions accepted after close. A lightweight Tokio background task checks for and closes stale sessions periodically (e.g., every 5 minutes) so they don't linger in the active sessions list. Actions that update `last_activity_at`: run submission, track selection (next-track, choose-track), join, leave, skip-turn.
- Future consideration: `password_hash` column for session passwords. Deferred — the `POST /sessions/:id/join` endpoint is designed as a dedicated action so password checking can be added later without restructuring the join flow.
- A user can only be active in one session at a time (enforced by a partial unique index on `session_participants(user_id) WHERE left_at IS NULL`).
- **Session UI icons:** The host is indicated by a 🏠 (house) icon, not a crown. The 👑 (crown) is reserved for the player with the most fastest track times in the session — an earned distinction, not a role.

### Session Participants

Tracks who is in a session and when they joined/left. A user can rejoin a session (creating a new row).

```
session_participants
├── id: UUID (primary key)
├── session_id: UUID (foreign key -> sessions, not null)
├── user_id: UUID (foreign key -> users, not null)
├── joined_at: TIMESTAMP (not null)
└── left_at: TIMESTAMP (nullable — null means currently in session)
```

Notes:
- A user can have multiple rows for the same session (left and rejoined).
- "Currently in session" = has a row where `left_at` is null.
- On leave, pending races enter a 5-minute grace period. If the user rejoins within that window, their pending races are preserved. After the grace period, pending races expire. Grace period is checked lazily at submission time (compare `now` against `left_at`) — no background timer needed.

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

Notes:
- A run is considered flagged if it has an unresolved entry in `run_flags` (where `resolved_at` is null).
- Users can only flag their own runs, and only if the run has a photo attached.
- When flagging, users choose whether the run stays visible or is hidden while under review (`hide_while_pending`).
- Auto-generated flags always set `hide_while_pending = true`.
- The `flagged_for_review` column on the `runs` table is removed — flag status is determined by the presence of an unresolved `run_flags` row.
- `run_id` is NOT unique — a run can have multiple flags, both resolved and unresolved. Different issues (e.g., wrong time and wrong race setup) are tracked as separate flags and resolved independently. Resolved flags are kept as audit history. Application code prevents duplicate flags (same run + same reason while unresolved).

### Head-to-Head Derivation

Head-to-head records are derived from session data, not stored separately. A "win" is when two players both submitted non-DQ'd runs for the same `session_race_id` and one had a faster `track_time`. Ties (identical times) count as 0-0 — neither a win nor a loss.

The query: find all `session_race_id` values where both User A and User B have a non-DQ'd run, compare `track_time`, tally wins and losses. The relationship is symmetric — A's wins are B's losses and vice versa.

H2H does not distinguish between alcoholic and non-alcoholic runs. If a drinker and a non-drinker race in the same session, their result counts for H2H. Drink category separation only applies to leaderboards.

This approach supports an unbounded number of rivals — no tracking table needed, no cap on how many people you can have H2H records with. Performance is a simple indexed join at the scale of a friend group. If it ever needs optimization, a materialized cache can be added later.

### Pending Race Tracking

Within a session, a participant may have "pending" races — session races they were present for but haven't yet submitted a time. The UI caps pending races at 3 (oldest expire first), but the schema places no limit, allowing this cap to be adjusted later.

When a participant has pending races, they submit in order (oldest first). For each pending race, they can submit a time or skip. Submitting out of order is not allowed — this prevents cherry-picking favorable tracks to game H2H records.

If the session advances while a participant hasn't submitted, they get a choice: submit for the original track (default) or the current one. This ensures no one person holds up the group, consistent with "never feel rushed."

### Session Rulesets

Each session uses one ruleset that determines how tracks are selected. The ruleset is chosen at session creation. Development order: Random → Default → Least Played → Round-robin.

**Random:** A random track is chosen each time, without replacement. The host triggers each new track selection. For when you just don't care what you're racing.

**Default:** The person with the fewest points on the global leaderboard always chooses. This biases the game toward "self-leveling" — keeping the pack tight and the leaderboard interesting. Tiebreaker: the user with the oldest account creation time is chosen (increasing the likelihood that the chooser has the most site experience and will keep things moving). The chosen person can recuse — they're removed from consideration until a race is completed or they leave and rejoin. If everyone recuses, a random track is chosen.

**Least Played:** The track with the fewest submitted runs is chosen automatically. "Fewest" is scoped to a drink category (alcoholic or non-alcoholic), chosen at session creation. The player's preferred drink type determines which option is pre-selected. The host triggers each new track selection.

**Round-robin:** Two groups: "Can Choose" and "Can't Choose." Everyone starts in "Can Choose." The earliest session joiner in "Can Choose" picks the track. After choosing, they move to "Can't Choose." Recusing also moves you to "Can't Choose." If "Can Choose" is empty, a random track is chosen and everyone resets to "Can Choose." This prevents the decision from stalling on someone not paying attention.

**Skip turn vs. recusal:** "Skip turn" (`POST /sessions/:id/skip-turn`) and self-recusal are distinct actions. Recusal means "I don't want to choose" — the chooser opts out. Skip turn means "I don't want to wait for you" — any participant can trigger it to move past a stalled chooser. Per-ruleset behavior of skip-turn: in Default, the next eligible player (by fewest leaderboard points) is offered the choice. In Round-robin, the skipped player moves to "Can't Choose," same as if they'd chosen or recused. If skip-turn or recusal cycles through everyone (Default) or empties "Can Choose" (Round-robin), a random track is selected automatically.

**Chooser state persistence:** Round-robin chooser rotation is derived from `session_races.chosen_by` — the "Can Choose" group is everyone who hasn't chosen since the last reset. Default recusal is transient (in-memory, resets per race) — if the server restarts mid-race, the chooser gets re-offered, which is harmless.

**Timeout handling (all rulesets):** Every decision point is event-driven — no timers pressuring players. If a chooser stalls, any participant can trigger "skip turn" to pass to the next person per the ruleset's logic. MVP fallback for truly stuck sessions: leave and start a new one. Vote-to-kick deferred.

## User Workflows

### Workflow 1: New User Joins

1. Gets URL from a friend, opens on phone.
2. Registers (username + password), auto-logged-in.
3. Lands on home/dashboard — empty state.
4. Prompted to set up preferred race setup (character, body, wheels, glider) and preferred drink type. Drink type selector includes "not listed? add new" option.
5. Home screen shows active sessions. If friends are already playing, the most natural next step is "tap to join." If nobody's playing, "Start a Session" is the primary action.

### Workflow 2: Starting a Session

1. Taps "Start a Session" on home screen.
2. Selects a ruleset: Random (default for MVP), Default, Least Played, or Round-robin. Brief explanation of each shown inline.
3. Session is created. User is the host and first participant.
4. Lands on the session screen — waiting for others to join, or can proceed solo.
5. For Random and Least Played rulesets: host taps to trigger the first track selection. For Default and Round-robin: the chooser is determined by the ruleset and prompted to pick.

### Workflow 3: Joining a Session

1. Home screen shows list of active sessions, sorted by most recent activity. Each shows: host name, participant count, current race number, ruleset.
2. Taps a session to join.
3. Lands on the session screen, sees current state: what track is being raced, who's in, who has pending races.
4. Can immediately submit a time for the current race.

Future enhancement: prioritize sessions containing players you've competed with before (sort by known rivals). Future consideration: session passwords via the `POST /sessions/:id/join` endpoint.

### Workflow 4: The Session Loop (Core Play Loop)

1. A track is selected (by the chooser or automatically, depending on ruleset).
2. Everyone in the session sees the track. Each person races on their TV in Time Trial mode.
3. After racing, each person submits their time:
   - Track is already known (from the session race) — no track selection needed.
   - Enter time (M:SS.mmm — single digit minutes, no leading zero, manual entry for v1, camera/OCR later). Auto-advance moves focus forward through all 12 fields (total → L1 → L2 → L3); backspace on an empty field moves backward. Lap times must sum exactly to total time.
   - Drink defaults to previous, fallback to preferred. Can change or add new inline.
   - Race setup defaults to previous, fallback to preferred. Can change.
   - Option to mark the run as DQ'd (didn't finish drink before finishing race).
   - Optional photo upload.
   - If time is a track record and no photo: prompt, then auto-flag if skipped.
4. Session screen shows who has submitted, who's still pending.
5. Next track selection happens when the chooser/host triggers it (depending on ruleset). The chooser can pick while others still have pending races — this doesn't block.
6. If someone has pending races from earlier, they see those when they go to submit. Pending races shown in order, submit or skip each. Max 3 pending in UI (oldest expire first). Schema supports unlimited for future flexibility.
7. Choosing and submitting are independent actions — the chooser can pick the next track even if they haven't submitted their own time yet.
8. Repeat until the group decides to stop.

Earmarked: the track selection sub-workflow (how the chooser browses/searches for a track within a session) will be specified as part of Phase 3 detailed design. Starting point: browse by cup or search by name, consistent with the existing track browser concept.

### Workflow 5: Leaving a Session / Session End

1. Player taps "Leave Session."
2. If they have pending races: warning that pending times will be forfeited after a 5-minute grace period. If they rejoin within the grace period, pending races are preserved.
3. If the leaving player is the host: host role transfers to the earliest-joined remaining participant.
4. Session ends when all participants have left.
5. If no activity for 1 hour, session auto-closes and no further run submissions are accepted.

### Workflow 6: Checking Personal Stats

1. Opens profile.
2. Sees overall stats: total runs, most-played track, best track (highest leaderboard position), overall rank.
3. Sees session history: list of sessions participated in (date, participants, race count, personal W-L for that session). Tap into a session for race-by-race breakdown.
4. Sees full run history (all runs, newest first) — tappable to view details, flag, or delete.
5. Can drill into a specific track — time chart over time, PB, average.
6. Sees "players you've competed with" list (derived from shared session races) — tap one to see H2H record.

### Workflow 7: Tracks & Leaderboards

1. Opens "Tracks & Leaderboards."
2. Sees global leaderboard — most track records held per player, your rank pinned at bottom if not in top N.
3. Alcoholic/non-alcoholic/combined toggle (defaults to match user's preferred drink category).
4. Below or alongside: cups listed in game order (by ID).
5. Taps a cup — cup-level leaderboard + its 4 tracks in position order.
6. Taps a track — your PB, time history chart, run history on this track, track leaderboard.
7. Taps a player on any leaderboard — their stats at that level (track/cup/global).
8. Taps that player again — full profile.

Note: earmarked for later discussion — potential shared leaderboard component across global/cup/track levels with consistent visual style but different data.

### Workflow 8: Flagging a Run

1. User views one of their own runs (from run history in profile).
2. Run has a photo attached.
3. Taps "Flag for Review."
4. Selects a reason from preset list: "Time is incorrect", "Wrong track", "Wrong race setup", "Wrong drink type", "Other."
5. Optionally adds a short note for context.
6. Chooses visibility: keep visible (default) or hide until reviewed.
7. Run marked as flagged, appears in admin queue.

### Workflow 9: Admin Reviews Flagged Runs

1. Brendan opens admin page (accessible only if user ID matches env variable).
2. Sees list of unresolved flags: player name, track, entered time, flag reason, note, visibility status, whether auto-generated.
3. Taps a flag — run details alongside photo.
4. Actions:
   - **Resolve:** Run is correct as-is. Sets `resolved_at`.
   - **Edit and resolve:** Correct the time/track/setup/etc, then resolve. (Admin-only exception to immutability.)
   - **Delete:** Data is unsalvageable. Run removed, user can re-enter.

## API Surface

All endpoints prefixed with `/api/v1`.

### What the API provides

The API is the contract between the frontend and backend. The frontend never touches the database directly — it makes HTTP requests to the Rust server, which validates input, runs business logic, and returns JSON. This follows REST conventions: resources (runs, tracks, users) are nouns in the URL, HTTP methods (GET, POST, PUT, DELETE) are the verbs.

For future flexibility (querying data in ways not yet enumerated), the runs endpoint supports generous query parameters for filtering, sorting, and pagination. If this becomes insufficient, a GraphQL layer (`async-graphql` crate) can be added alongside REST later.

### Auth

Uses established Rust crates — not rolling crypto from scratch. `argon2` for password hashing, `jsonwebtoken` for JWT tokens. ~150 lines of code wrapping audited libraries. Sufficient for a self-hosted friends-and-game-night app. Account recovery is admin-reset for now.

```
POST   /auth/register              Create account (username, password), returns access token + sets refresh cookie
POST   /auth/login                 Returns access token + sets refresh cookie
POST   /auth/refresh               Exchange refresh cookie for new access token
POST   /auth/logout                Clears refresh cookie, increments refresh_token_version
PUT    /auth/password              Change own password (requires current password)
```

### Users

```
GET    /users                      List all users (public profiles)
GET    /users/:id                  Get user profile + preferred race setup
PUT    /users/:id                  Update profile / preferred race setup / preferred drink (self only)
```

### Pre-seeded Data (read-only)

```
GET    /characters                 List all characters
GET    /bodies                     List all vehicle bodies
GET    /wheels                     List all wheel sets
GET    /gliders                    List all gliders
GET    /cups                       List all cups
GET    /cups/:id                   Get cup with its tracks
GET    /tracks                     List all tracks (optional filter: cup_id)
GET    /tracks/:id                 Get track details
```

### Drink Types

```
POST   /drink-types                Create a new drink type (returns existing on UUID collision)
GET    /drink-types                List all drink types (optional filter: alcoholic)
GET    /drink-types/:id            Get drink type details
```

### Sessions

```
POST   /sessions                   Create a new session (choose ruleset)
GET    /sessions                   List active sessions (sorted by most recent activity)
GET    /sessions/:id               Get session details (participants, current race, state)
POST   /sessions/:id/join          Join a session (dedicated endpoint — designed for future password support)
POST   /sessions/:id/leave         Leave a session (triggers grace period for pending races)
POST   /sessions/:id/next-track    Trigger next track selection (host or chooser, depending on ruleset)
POST   /sessions/:id/choose-track  Choose a specific track (for rulesets where a player picks)
POST   /sessions/:id/skip-turn     Pass the chooser's turn to the next person (any participant can trigger)
GET    /sessions/:id/races         List all races in a session (with submission status per participant)
```

Note: Session state is consumed via polling — clients call `GET /sessions/:id` every 2-3 seconds to pick up joins, leaves, new races, and submissions. For a turn-based game where events happen every few minutes, polling latency is imperceptible. WebSockets can be added later as an optimization if polling ever feels sluggish.

### Runs

```
POST   /runs                       Record a new run (requires session_race_id; auto-flags if record-breaking without photo)
GET    /runs                       Query runs (filters: user_id, track_id, session_race_id, drink_type_id,
                                               alcoholic, disqualified, after, before, sort, limit, cursor)
GET    /runs/:id                   Get a specific run
DELETE /runs/:id                   Delete a run (owner or admin)
PUT    /runs/:id                   Edit a run (admin only, 403 for regular users)
POST   /runs/:id/photo             Upload photo for a run (auto-resolves record flag if present)
POST   /runs/:id/flag              Flag a run for review (owner only, requires photo on run)
```

Note: `GET /runs/suggest-track` has been removed — track coordination is now handled by sessions.

### Stats

```
GET    /stats/personal/:user_id                    Personal summary (total runs, most-played, best track, rank)
GET    /stats/personal/:user_id/track/:track_id    Per-track breakdown (PB, average, time history)
GET    /stats/personal/:user_id/sessions           Session history (date, participants, race count, personal W-L)
GET    /stats/leaderboard/global                   Global leaderboard (most track records held)
GET    /stats/leaderboard/cup/:cup_id              Cup-level leaderboard
GET    /stats/leaderboard/track/:track_id          Track leaderboard (best time per user)
GET    /stats/rivals/:user_id                      Players you've competed with (derived from shared session races)
GET    /stats/head-to-head/:user_id_1/:user_id_2   H2H record between two players (derived from session races)
```

All leaderboard endpoints accept `?alcoholic=true|false|all` to filter by drink category. Default matches the requesting user's preferred drink category. DQ'd runs are excluded from leaderboard calculations.

### Admin

```
GET    /admin/flags                List unresolved flags (admin only)
PUT    /admin/flags/:id            Resolve a flag (admin only)
```

## UI Screens (Mobile-First)

### 1. Login / Register
Simple form. Username + password. No email required for v1.

### 2. Home / Dashboard
- Active sessions list (sorted by most recent activity; each shows host, participants, race number, ruleset).
- "Start a Session" button (primary action).
- Recent runs (your last 5).
- Your overall rank (most track records held).
- Preferred Race Setup (character + kart displayed).

### 3. Session Screen
The main play screen. Shows:
- Current track being raced.
- Participant list with submission status (submitted / pending / DQ'd).
- Pending race indicator (who has unsubmitted races from earlier).
- "Submit Time" action — opens the run entry form for the current (or pending) race.
- Next track controls (host/chooser triggers, depending on ruleset).
- "Skip Turn" option (any participant can pass the chooser's turn).
- "Leave Session" button.
- Session history (tracks raced so far, results).

### 4. Run Entry (within session)
Streamlined compared to standalone entry — the track is already known from the session.
1. Enter time (M:SS.mmm — single digit minutes, no leading zero, manual entry for v1, camera/OCR later). Auto-advance moves focus forward through all 12 fields (total → L1 → L2 → L3); backspace on an empty field moves backward. Lap times must sum exactly to total time.
2. Drink defaults to previous, fallback to preferred. Can change or add new inline.
3. Race setup defaults to previous, fallback to preferred. Can change.
4. Option to mark run as DQ'd (didn't finish drink before finishing race).
5. Optional photo upload.
6. If record-breaking without photo: prompt, then auto-flag if skipped.
7. If pending races exist: shown in order, submit or skip each before current race.

### 5. Tracks & Leaderboards
- Global leaderboard: most track records held, your rank pinned at bottom.
- Alcoholic/non-alcoholic/combined toggle (defaults to user's preferred drink category).
- Cups listed in game order, each showing its 4 tracks.
- Drill into cup: cup-level leaderboard + tracks.
- Drill into track: your PB, time chart, run history on this track, track leaderboard.
- Tap a player: their stats at that level. Tap again: full profile.

### 6. Profile / Personal Stats
- Overall stats: total runs, most-played track, best track, overall rank.
- Session history: list of sessions (date, participants, race count, W-L). Tap for race-by-race breakdown.
- Full run history (newest first) — tappable for details, flag, or delete.
- Drill into a track for personal breakdown.
- "Players you've competed with" (derived from shared session races) — tap for H2H.

### 7. Admin (Brendan only)
- List of unresolved flags with run details, photos, reasons, notes.
- Actions: resolve, edit and resolve, or delete run.

### Shared UI Components (earmarked for discussion)
- **Drink type selector**: reusable wherever a drink is chosen (run entry, onboarding, profile). Includes "not listed? add new" inline form.
- **Leaderboard component**: potential shared component for global/cup/track levels with consistent visual style, different data.

## Project Structure

```
beerio-kart/
├── .claude/
│   └── CLAUDE.md                # AI assistant context (checked into repo)
│
├── DESIGN.md                    # Architecture design document (single source of truth)
├── compose.yaml                 # Docker compose
├── justfile                     # Developer workflow commands (just)
│
├── backend/
│   ├── Cargo.toml
│   ├── Dockerfile
│   └── src/
│       ├── main.rs              # Axum server setup, routing
│       ├── config.rs            # Environment/config management
│       ├── db/
│       │   ├── mod.rs
│       │   ├── migrations/      # SQL migration files
│       │   └── entities/        # SeaORM generated entity files
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── auth.rs
│       │   ├── sessions.rs
│       │   ├── runs.rs
│       │   ├── tracks.rs
│       │   ├── stats.rs
│       │   ├── users.rs
│       │   └── admin.rs
│       ├── services/            # Business logic layer
│       │   ├── mod.rs
│       │   ├── auth.rs
│       │   ├── sessions.rs      # Session lifecycle, rulesets, track selection
│       │   └── stats.rs
│       └── middleware/
│           ├── mod.rs
│           └── auth.rs          # JWT/session validation + admin check
│
├── frontend/
│   ├── package.json
│   ├── Dockerfile
│   ├── vite.config.ts
│   ├── tailwind.config.js
│   ├── index.html
│   └── src/
│       ├── main.tsx
│       ├── App.tsx
│       ├── api/                 # API client functions
│       ├── components/          # Reusable UI components
│       │   ├── DrinkTypeSelector.tsx
│       │   └── LeaderboardTable.tsx
│       ├── pages/               # Screen-level components
│       │   ├── Home.tsx
│       │   ├── Login.tsx
│       │   ├── Session.tsx
│       │   ├── RunEntry.tsx
│       │   ├── TracksAndLeaderboards.tsx
│       │   ├── TrackDetail.tsx
│       │   ├── CupDetail.tsx
│       │   ├── Profile.tsx
│       │   └── Admin.tsx
│       ├── hooks/               # Custom React hooks
│       └── types/               # TypeScript type definitions
│
├── static/
│   └── images/                  # Pre-seeded asset images
│       ├── characters/
│       ├── bodies/
│       ├── wheels/
│       ├── gliders/
│       ├── tracks/
│       └── cups/
│
├── reviews/
│   ├── pr/                      # Claude Code-generated PR review explanations
│   └── design/                  # Design session records (Cowork-generated, checkbox format)
│
└── data/
    ├── tracks.json              # MK8D track seed data
    ├── characters.json          # MK8D character seed data
    ├── bodies.json              # MK8D vehicle body seed data
    ├── wheels.json              # MK8D wheel set seed data
    ├── gliders.json             # MK8D glider seed data
    ├── cups.json                # MK8D cup seed data
    ├── db/
    │   └── beerio-kart.db       # SQLite database file (gitignored)
    └── uploads/                  # User-uploaded run photos (gitignored)
```

## Build Plan (Phases)

### Phase 1: Foundation
- [x] Initialize Rust project with Axum
- [x] Initialize React project with Vite + Bun + Tailwind
- [x] Set up SeaORM with SQLite and migrations (all tables including run_flags)
- [x] Seed MK8 Deluxe data (tracks, cups, characters, bodies, wheels, gliders)
- [x] Basic auth (register/login with argon2 + JWT)
- [x] Dockerfiles + compose.yaml

### Phase 2: Deployment
- [x] Validate single-container Dockerfile on Unraid (multi-stage build already exists from Phase 1)
- [x] Validate compose.yaml on Unraid (already exists from Phase 1, single service + volumes)
- [x] Configure Cloudflare tunnel to route domain to the app on Unraid
- [x] Set Cloudflare encryption mode to **Full (strict)** — Flexible encrypts browser-to-Cloudflare but forwards plaintext to the origin server, which means passwords travel unencrypted on the last hop
- [x] Verify HTTPS works end-to-end through Cloudflare
- [x] Test basic auth flow from phone over real network
- [x] Add .env / config for production vs development settings
- [x] Upgrade auth to refresh token flow (short-lived access token + HttpOnly refresh cookie + `refresh_token_version` on users)

Note: Deploying early (before core features) keeps the deployment simple and catches infrastructure issues before application complexity grows. The Dockerfile and compose.yaml were created in Phase 1 for local development — Phase 2 validates they work on the actual Unraid server behind Cloudflare.

### Phase 3: Sessions & Run Recording
- [ ] Session schema: sessions, session_participants, session_races tables + migrations
- [ ] Add `session_race_id` and `disqualified` columns to runs table (migration)
- [ ] Session polling endpoint (`GET /sessions/:id` returns full session state; clients poll every 2-3 seconds)
- [ ] Session lifecycle: create, join, leave, auto-close on inactivity (1 hour)
- [ ] Host transfer on leave (earliest-joined remaining participant)
- [ ] Random ruleset (first ruleset — track chosen at random without replacement)
- [ ] Run entry within session context (time, drink, race setup, DQ option)
- [ ] Pending race tracking (max 3 in UI, submit in order, skip option)
- [ ] 5-minute grace period for disconnects before pending races expire
- [ ] "Skip turn" — any participant can pass the chooser's turn
- [ ] Drink type selector with inline creation
- [ ] Previous/preferred defaulting for drink and race setup
- [ ] Photo upload (separate endpoint)
- [ ] Auto-flagging for record-breaking runs without photos (DQ'd runs excluded)
- [ ] Home screen: active sessions list + "Start a Session" primary action + recent runs
- [ ] Background task: Tokio task to close stale sessions (no activity for 1 hour, check every ~5 min)
- [ ] User profile endpoints (GET /users, GET /users/:id, PUT /users/:id for preferred setup and drink type)
- [ ] Drink types API (create, list, get)
- [ ] Sessions API (create, join, leave, next-track, choose-track, skip-turn, list races)
- [ ] Pre-seeded data read endpoints (characters, bodies, wheels, gliders, cups, tracks)
- [ ] Runs API (create within session, list, delete, photo upload)
- [ ] Password change endpoint (`PUT /auth/password`)
- [ ] justfile with recipes: `dev`, `test`, `entities`, `build`

Note: Solo racing uses a one-person session. Future enhancement: streamline the solo experience by making `session_race_id` nullable on runs and offering a lightweight entry flow without session overhead.

### Phase 4: Session Rulesets
- [ ] Default ruleset (least leaderboard points chooses; recusal; fallback to random)
- [ ] Least Played ruleset (track with fewest runs chosen; drink category config at session creation)
- [ ] Round-robin ruleset ("Can Choose" / "Can't Choose" groups; recusal; reset when empty)
- [ ] Ruleset selection UI at session creation (brief inline explanations)

Future consideration: allow ruleset changes mid-session (deferred post-MVP).

Required test scenarios per ruleset: normal flow, recusal by one player, recusal by all players, player joins mid-session, player leaves mid-session, host leaves mid-session. Each ruleset needs all six scenarios covered.

### Phase 5: Stats & Leaderboards
- [ ] Personal stats page (PBs, averages, run count, most-played track, best track)
- [ ] Session history in profile (date, participants, race count, personal W-L per session)
- [ ] Full run history with detail view
- [ ] Per-track time history with chart
- [ ] Track leaderboard (alcoholic / non-alcoholic / combined toggle; DQ'd runs excluded)
- [ ] Cup-level leaderboard
- [ ] Global leaderboard (most track records held)
- [ ] User rank pinned at bottom of leaderboards

### Phase 6: Social & Head-to-Head
- [ ] "Players you've competed with" (derived from shared session races)
- [ ] Head-to-head comparison view (wins/losses derived from session race data; DQ'd runs excluded; ties = 0-0)
- [ ] Win/loss records (H2H does not distinguish alcoholic vs non-alcoholic — drink category only matters for leaderboards)
- [ ] Profile page with improvement trends
- [ ] Flagging a run (user-initiated, with preset reasons + notes + visibility choice)
- [ ] Admin page (lightweight, env-variable-gated)
- [ ] Admin: review flagged runs, resolve, edit, or delete

### Phase 7: Camera/OCR (Future)
- [ ] Photo upload with each run (verification + training data)
- [ ] Use phone camera to photograph TV screen showing race time
- [ ] Extract time using OCR (likely browser-side Tesseract.js or similar)
- [ ] Auto-populate time field from photo
- [ ] Extract race setup from end-of-race screen
- [ ] Retire preferred race setup from user profiles once OCR is reliable

## Resolved Decisions

- **SQLite STRICT mode on lookup / static tables; DATETIME columns on timestamped tables.** STRICT kept on static game data tables for type safety at insert time. Dropped on timestamped tables so columns use `DATETIME` type, giving Rust `DateTime<Utc>` via SeaORM codegen instead of stringly-typed timestamps.
- **Global leaderboard ranking:** Most track records held.
- **Account recovery:** Admin reset for now.
- **Time entry validation:** No validation against plausible track times. Rely on photos and eventual OCR.
- **Beer vs water:** Separate leaderboards by default, with combined view. Default toggle matches user's preferred drink category.
- **Track variants:** 150cc only.
- **Admin model:** Lightweight admin page gated by user ID in env variable. No formal role system for MVP.
- **Run immutability:** Users cannot edit runs after creation. Admin can edit (for correcting OCR errors, etc.).
- **Head-to-head tracking:** Derived from session races. Two players have a H2H record when they both submitted non-DQ'd runs for the same session race. Replaces the earlier timestamp-clustering approach.
- **Sessions replace standalone run recording.** All runs are recorded within session context. Solo racing uses a one-person session (MVP). Future enhancement: nullable `session_race_id` for lightweight standalone runs.
- **H2H ties:** Identical times = 0-0 (draw). Neither player gets a win or loss.
- **H2H drink category:** H2H does not distinguish alcoholic vs non-alcoholic. A drinker and non-drinker in the same session race have their result counted. Drink category only matters for leaderboards.
- **DQ'd runs:** Recorded but excluded from H2H tallies and leaderboard positions. Self-reported at submission time (honor system). DQ = didn't finish drink before finishing race.
- **Pending race cap:** UI shows max 3 pending races (oldest expire first). Schema supports unlimited — cap is a UX guardrail, adjustable later.
- **Session host transfer:** When host leaves, earliest-joined remaining participant becomes new host.
- **Session timeout handling (MVP):** "Skip turn" allows any participant to pass the chooser's turn. Vote-to-kick deferred. Leave-and-restart is the fallback for stuck sessions.
- **Session passwords:** Deferred. The `POST /sessions/:id/join` endpoint is a dedicated action so password checking can be added later without restructuring the join flow.
- **Ruleset changes mid-session:** Deferred post-MVP.
- **Real-time updates via polling, not WebSockets.** Clients poll `GET /sessions/:id` every 2-3 seconds. For a turn-based game where events happen every few minutes, polling latency is imperceptible. WebSockets can be added later as an optimization. Polling is stateless, testable with standard HTTP tools, and avoids connection state management, reconnection logic, and heartbeat complexity.
- **Admin defense in depth.** Admin-only operations (editing runs, resolving flags) are checked in both the route middleware (AdminUser extractor) and the service layer independently. Two independent checks — if either the middleware or the service rejects the request, it fails. Prevents a middleware bug from exposing admin operations.
- **Photo upload validation.** Validate server-side by checking magic bytes (not just Content-Type header). Accept JPEG, PNG, HEIC/HEIF. Max file size: 10MB. Generate filenames from run ID (`{run_id}.{ext}`) — never use user-provided filenames. Optionally strip EXIF data (GPS, device info) for privacy in a future pass.
- **Upload path isolation.** User uploads are served from a separate URL prefix and filesystem directory from static assets. Static assets at `/static/...` (from `STATIC_DIR`), uploads at `/uploads/...` (from `UPLOAD_DIR`). Different prefixes and different directories prevent path traversal across boundaries.
- **Rulesets implemented as a Rust trait.** Each ruleset (Random, Default, Least Played, Round-robin) is a separate module implementing a `Ruleset` trait, not conditionals in the session service. Adding a fifth ruleset later means adding one module, not modifying existing code.
- **Entity regeneration via justfile recipe.** After adding or modifying a migration, run `just entities` (wraps `sea-orm-cli generate entity`) to regenerate SeaORM entity files. Standard SeaORM workflow — no custom tooling needed.
- **just (not Make) for developer commands.** Cargo handles Rust build dependencies, Bun handles frontend dependencies, Docker handles container caching. Developer workflow commands (`just dev`, `just test`, `just entities`) don't need file-level dependency tracking — just a named command runner.
- **Photo enforcement for records:** Runs are auto-flagged and hidden if record-breaking without a photo. Photo upload auto-resolves the flag.
- **Lap time column naming:** `lap1_time`, `lap2_time`, `lap3_time` (no underscore before digit). Matches SeaORM's `DeriveIden` macro output for variants like `Lap1Time`, avoiding unnecessary custom naming.
- **UUID storage in SQLite:** UUIDs stored as TEXT (not BLOB) for human readability when debugging with CLI tools. PostgreSQL migration will map to native UUID type. SeaORM maps both to `String` in Rust, so application code won't change.
- **Timestamp storage in SQLite:** Timestamps stored as TEXT in ISO 8601 format. SQLite has no native timestamp type. PostgreSQL migration will map to `TIMESTAMPTZ`.
- **run_flags audit trail:** `run_id` is not unique — a run can have multiple flags (different reasons tracked separately, resolved independently). Resolved flags are kept as history. Only duplicate flags (same run + same reason while unresolved) are prevented in application code.
- **Frontend serving strategy:** Axum serves everything in a single container. The Vite build produces static files that Axum serves via `tower-http::ServeDir`, with SPA fallback to `index.html`. No nginx or separate frontend container. Rationale: simpler deployment for a small-scale app, no CORS (same origin), one container to manage. If static asset performance ever matters (it won't at this scale), a CDN or nginx can be added in front later.
- **Auth token strategy:** Short-lived access token (15-30 min, Authorization header) + long-lived refresh token (7-30 days, HttpOnly/Secure/SameSite=Lax cookie scoped to `/api/v1/auth/refresh`). Lax (not Strict) because Strict blocks the cookie when navigating from an external link (e.g., a friend texts you the URL), which would force a re-login. Lax still protects against cross-origin POST attacks. A `refresh_token_version` column on `users` enables server-side revocation checked only on the refresh path, not every request. Frontend intercepts 401s, silently refreshes, and retries. Replaces the original 24-hour single JWT approach.
- **Pagination:** Cursor-based (keyset) pagination using `created_at` + `id` for list endpoints, particularly `GET /runs` and run history views. Preferred over offset-based to avoid duplicate/skipped entries when new data is inserted during browsing. If implementation proves too complex relative to offset-based, revisit.
- **Password change:** `PUT /auth/password` endpoint for users to change their own password. Implemented in Phase 2 alongside refresh token auth.
- **Refresh token format:** JWT (not opaque). Contains `sub`, `refresh_token_version`, `exp`, `iat`, `token_type`. Same signing key as access token. Simplest approach — no new table. Per-device revocation deferred; version-bump covers logout and password change.
- **Refresh token rotation:** On each refresh, a new refresh JWT is issued with a fresh expiry. Does not bump `refresh_token_version` — rotation is about extending the session window, not revocation.
- **Cloudflare Tunnel (not port forwarding)** for exposing the app. Outbound-only connection from Unraid to Cloudflare edge. No open ports on the home network.
- **Docker Compose Manager plugin** on Unraid for container management. compose.yaml as the single source of truth for the deployment.

## Backlog


Random ideas that may or may not be pursued.

- Turn list of previous players into invite emails to join.
- Ability to Change Username.
- Ability to send emails.
   - Account recovery.
- Handle concurrent `next_track` calls gracefully. Currently a double-tap can hit the `UNIQUE(session_id, race_number)` constraint and return a 500. Options: retry-on-conflict or optimistic locking. Low priority — single host makes this very unlikely.
