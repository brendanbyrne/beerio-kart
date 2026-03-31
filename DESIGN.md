# Beerio Kart - Architecture Design Document

## Overview

Beerio Kart is a mobile-first web app for tracking times and stats for the Mario Kart 8 Deluxe drinking game. Players race one at a time in Time Trial mode (150cc only). You can't touch the controller while touching your drink (one 12oz beer or sparkling water). The app tracks personal times per track, head-to-head records, and leaderboards.

## Rules of the Game

1. Players race one at a time using Time Trial mode in Mario Kart 8 Deluxe.
2. You cannot touch the controller while touching your drink.
3. The drink is one 12oz beer or one 12oz sparkling water, poured into a cup.
4. You may restart the race if it is before the end of the first lap AND you haven't had any of your drink yet.
5. Played round robin — all players race the same track.
6. Fastest time wins.

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

Internal error details are never exposed to clients. When an unexpected error occurs:
1. Log the real error server-side with `tracing::error!` (including the error value for diagnostics)
2. Return a generic `"Internal server error"` message in the JSON response body
3. Keep the appropriate HTTP status code (e.g., 500)

### Configuration

Log output is controlled via the `RUST_LOG` environment variable. Defaults to `info` if not set. Examples:
- `RUST_LOG=debug` — all debug-level output
- `RUST_LOG=beerio_kart=debug` — debug only for application code, info for dependencies

## Naming Conventions

- Table names: plural, snake_case (`drink_types`, `characters`)
- Column names: snake_case (`track_time`, `created_at`)
- Foreign keys: `{referenced_table_singular}_id` (`character_id`, `cup_id`)
- Primary keys: `id`

## Design Principles

- **Minimize manual input.** Every design decision should favor automatically deducing information over requiring users to enter it explicitly.
- **Inclusive by default.** The game is historically a drinking game, but non-drinkers should never feel like second-class participants. The app treats alcoholic and non-alcoholic runs with equal prominence.
- **Don't overengineer before OCR.** Many corner cases (time validation, race setup entry, session tracking) will be solved by OCR. Design the MVP for manual entry with hooks for OCR to slot in later.

## Data Model

### Design Decisions

- **UUID vs INTEGER primary keys.** INTEGER for pre-seeded static data (characters, tracks, cups, bodies, wheels, gliders) — stable, small, human-readable. UUID for user-generated runtime data (users, runs, drink_types) — globally unique, can be generated client-side without a database round trip (important for future offline support).
- **RaceSetup stored inline, not normalized.** Character, body, wheels, and glider IDs are stored directly on the `runs` and `users` tables rather than in separate junction tables. With ~3 million possible combinations (most never used), a reference table is wasteful. Inline storage costs 4 integer columns (16 bytes) — negligible. Migration to a normalized form later is straightforward if needed.
- **Images stored on disk, paths in the database.** Pre-seeded assets (characters, tracks, kart parts) ship as static files. User-uploaded photos (run verification) are saved to a configurable uploads directory. Database stores relative paths (e.g., `images/characters/mario.png`).
- **Fixed-size arrays use separate columns or relational joins.** Lap times (always 3) become `lap_1_time`, `lap_2_time`, `lap_3_time` — simple to query. Cup-to-track relationships use the `cup_id` foreign key on the `tracks` table, not an array on `cups`.
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
├── preferred_wheels_id: INTEGER (foreign key -> wheels, nullable)
├── preferred_glider_id: INTEGER (foreign key -> gliders, nullable)
├── preferred_drink_type_id: UUID (foreign key -> drink_types, nullable)
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

### Runs

The core table. One row per player per race attempt. User-created, immutable for regular users (times cannot be edited after creation; admin can edit), deletable by owner or admin.

```
runs
├── id: UUID (primary key)
├── user_id: UUID (foreign key -> users, not null)
├── track_id: INTEGER (foreign key -> tracks, not null)
├── character_id: INTEGER (foreign key -> characters, not null)
├── body_id: INTEGER (foreign key -> bodies, not null)
├── wheels_id: INTEGER (foreign key -> wheels, not null)
├── glider_id: INTEGER (foreign key -> gliders, not null)
├── track_time: INTEGER (milliseconds, not null, must be positive)
├── lap1_time: INTEGER (milliseconds, not null, must be positive and non-zero)
├── lap2_time: INTEGER (milliseconds, not null, must be positive and non-zero)
├── lap3_time: INTEGER (milliseconds, not null, must be positive and non-zero)
├── drink_type_id: UUID (foreign key -> drink_types, not null)
├── photo_path: TEXT (nullable — optional but encouraged; required for record-breaking runs)
├── created_at: TIMESTAMP (not null, defaults to current time, optionally user-provided)
└── notes: TEXT (nullable — freeform; may be mined for future structured columns)
```

Validation (application-level):
- `track_time` must be positive.
- All three lap times must be positive and non-zero.
- Lap times should roughly sum to `track_time` (with tolerance for game rounding).
- Race setup columns pre-fill from previous run (or preferred from profile), but are all required.

Record-breaking run enforcement:
- When a run is created, the backend checks if the time is a new track record (per drink category).
- If it is a record and no photo is attached, the run is saved but auto-flagged with `hide_while_pending = true`.
- When a photo is uploaded via `POST /runs/:id/photo`, the auto-flag is resolved automatically.
- If the photo never arrives, the run remains flagged and hidden from leaderboards. Admin can see and act on it.

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

### Head-to-Head Context

Not an explicit feature. Runs played in the same round robin are loosely clustered by `created_at` timestamps. Head-to-head stats (Phase 5) are derived from timestamp proximity, not user-managed grouping. This avoids adding manual "session" bookkeeping — consistent with the design principle of minimizing manual input.

## User Workflows

### Workflow 1: New User Joins

1. Gets URL from a friend, opens on phone.
2. Registers (username + password), auto-logged-in.
3. Lands on home/dashboard — empty state.
4. Prompted to set up preferred race setup (character, body, wheels, glider) and preferred drink type. Drink type selector includes "not listed? add new" option.

### Workflow 2: Recording a Run

1. Opens app (already logged in).
2. Taps "Enter a Run."
3. Track suggestion: if another player entered a run on a different track within the last 15 minutes, suggest that track. Presented as a suggestion, not auto-selected.
4. Selects track (browse by cup or search by name) — or accepts the suggestion.
5. Enters time (MM:SS.mmm format — manual entry for v1, camera/OCR later).
6. Drink selector: defaults to previous (last run's drink), falls back to preferred (profile). Can change or add new inline.
7. Race setup: defaults to previous (last run's setup), falls back to preferred (profile). Changeable. *(Earmarked: should both previous and preferred be shown as options when they differ?)*
8. Optionally takes/uploads photo of TV screen.
9. If time would beat the track record and no photo is attached → app prompts for a photo. If skipped, run is auto-flagged and hidden.
10. Submits.
11. Sees confirmation, home screen updates.

### Workflow 3: Checking Personal Stats

1. Opens profile.
2. Sees overall stats: total runs, most-played track, best track (highest leaderboard position), overall rank.
3. Sees full run history (all runs, newest first) — tappable to view details, flag, or delete.
4. Can drill into a specific track — time chart over time, PB, average.
5. Sees "players you've competed with" list (derived from timestamp clustering) — tap one to see H2H record.

### Workflow 4: Tracks & Leaderboards

1. Opens "Tracks & Leaderboards."
2. Sees global leaderboard — most track records held per player, your rank pinned at bottom if not in top N.
3. Alcoholic/non-alcoholic/combined toggle (defaults to match user's preferred drink category).
4. Below or alongside: cups listed in game order (by ID).
5. Taps a cup — cup-level leaderboard + its 4 tracks in position order.
6. Taps a track — your PB, time history chart, run history on this track, track leaderboard.
7. Taps a player on any leaderboard — their stats at that level (track/cup/global).
8. Taps that player again — full profile.

Note: earmarked for later discussion — potential shared leaderboard component across global/cup/track levels with consistent visual style but different data.

### Workflow 5: Flagging a Run

1. User views one of their own runs (from run history in profile).
2. Run has a photo attached.
3. Taps "Flag for Review."
4. Selects a reason from preset list: "Time is incorrect", "Wrong track", "Wrong race setup", "Wrong drink type", "Other."
5. Optionally adds a short note for context.
6. Chooses visibility: keep visible (default) or hide until reviewed.
7. Run marked as flagged, appears in admin queue.

### Workflow 6: Admin Reviews Flagged Runs

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
POST   /auth/register              Create account (username, password), returns JWT
POST   /auth/login                 Returns JWT token
POST   /auth/logout                Invalidate token
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
POST   /drink_types                Create a new drink type (returns existing on UUID collision)
GET    /drink_types                List all drink types (optional filter: alcoholic)
GET    /drink_types/:id            Get drink type details
```

### Runs

```
POST   /runs                       Record a new run (auto-flags if record-breaking without photo)
GET    /runs                       Query runs (filters: user_id, track_id, drink_type_id,
                                               alcoholic, after, before, sort, limit, offset)
GET    /runs/:id                   Get a specific run
DELETE /runs/:id                   Delete a run (owner or admin)
PUT    /runs/:id                   Edit a run (admin only, 403 for regular users)
POST   /runs/:id/photo             Upload photo for a run (auto-resolves record flag if present)
POST   /runs/:id/flag              Flag a run for review (owner only, requires photo on run)
GET    /runs/suggest-track         Track suggestion heuristic (15-min window, server-side logic)
```

### Stats

```
GET    /stats/personal/:user_id                    Personal summary (total runs, most-played, best track, rank)
GET    /stats/personal/:user_id/track/:track_id    Per-track breakdown (PB, average, time history)
GET    /stats/leaderboard/global                   Global leaderboard (most track records held)
GET    /stats/leaderboard/cup/:cup_id              Cup-level leaderboard
GET    /stats/leaderboard/track/:track_id          Track leaderboard (best time per user)
GET    /stats/rivals/:user_id                      Players you've competed with (timestamp clustering)
GET    /stats/head-to-head/:user_id_1/:user_id_2   H2H record between two players
```

All leaderboard endpoints accept `?alcoholic=true|false|all` to filter by drink category. Default matches the requesting user's preferred drink category.

### Admin

```
GET    /admin/flags                List unresolved flags (admin only)
PUT    /admin/flags/:id            Resolve a flag (admin only)
```

## UI Screens (Mobile-First)

### 1. Login / Register
Simple form. Username + password. No email required for v1.

### 2. Home / Dashboard
- Quick-action button: "Enter a Run"
- Recent runs (your last 5)
- Your overall rank (most track records held)
- Preferred Race Setup (character + kart displayed)

### 3. Record a Run
1. Track suggestion shown if applicable (another player entered a run on a different track within 15 minutes).
2. Select track (searchable dropdown or grouped by cup) — or accept suggestion.
3. Enter time (MM:SS.mmm format — manual entry for v1, camera/OCR later).
4. Select drink (defaults to previous, falls back to preferred; "not listed? add new" inline).
5. Race setup (defaults to previous, falls back to preferred; changeable).
6. Optional: take/upload photo of TV screen.
7. If record-breaking without photo: prompt for photo. If skipped, run is auto-flagged and hidden.

### 4. Tracks & Leaderboards
- Global leaderboard: most track records held, your rank pinned at bottom.
- Alcoholic/non-alcoholic/combined toggle (defaults to user's preferred drink category).
- Cups listed in game order, each showing its 4 tracks.
- Drill into cup: cup-level leaderboard + tracks.
- Drill into track: your PB, time chart, run history on this track, track leaderboard.
- Tap a player: their stats at that level. Tap again: full profile.

### 5. Profile / Personal Stats
- Overall stats: total runs, most-played track, best track, overall rank.
- Full run history (newest first) — tappable for details, flag, or delete.
- Drill into a track for personal breakdown.
- "Players you've competed with" — tap for H2H.

### 6. Admin (Brendan only)
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
├── Makefile                     # Or justfile — common dev commands
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
│       │   ├── runs.rs
│       │   ├── tracks.rs
│       │   ├── stats.rs
│       │   ├── users.rs
│       │   └── admin.rs
│       ├── services/            # Business logic layer
│       │   ├── mod.rs
│       │   ├── auth.rs
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
│       │   ├── RecordRun.tsx
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
├── uploads/                     # User-uploaded run photos (gitignored)
│
└── data/
    ├── tracks.json              # MK8D track seed data
    ├── characters.json          # MK8D character seed data
    ├── bodies.json              # MK8D vehicle body seed data
    ├── wheels.json              # MK8D wheel set seed data
    ├── gliders.json             # MK8D glider seed data
    ├── cups.json                # MK8D cup seed data
    └── beerio-kart.db           # SQLite database file (gitignored)
```

## Build Plan (Phases)

### Phase 1: Foundation
- [x] Initialize Rust project with Axum
- [x] Initialize React project with Vite + Bun + Tailwind
- [x] Set up SeaORM with SQLite and migrations (all tables including run_flags)
- [x] Seed MK8 Deluxe data (tracks, cups, characters, bodies, wheels, gliders)
- [x] Basic auth (register/login with argon2 + JWT)
- [ ] Dockerfiles + compose.yaml

### Phase 2: Deployment
- [ ] Create Dockerfile for backend (multi-stage: build Rust binary, copy to slim runtime image)
- [ ] Create Dockerfile for frontend (build with Bun, serve with nginx or similar)
- [ ] Create compose.yaml (backend + frontend + shared volume for SQLite + uploads)
- [ ] Configure Cloudflare tunnel to route domain to the app on Unraid
- [ ] Set Cloudflare encryption mode to **Full (strict)** — Flexible encrypts browser-to-Cloudflare but forwards plaintext to the origin server, which means passwords travel unencrypted on the last hop
- [ ] Verify HTTPS works end-to-end through Cloudflare
- [ ] Test basic auth flow from phone over real network
- [ ] Add .env / config for production vs development settings

Note: Deploying early (before core features) keeps the deployment simple and catches infrastructure issues before application complexity grows. The Dockerfiles are listed in Phase 1 as well — Phase 1 creates them for local development, Phase 2 validates they work on the actual Unraid server behind Cloudflare.

### Phase 3: Core Feature — Recording Runs
- [ ] "Record a Run" form (track selection, manual time entry, drink type, race setup)
- [ ] Track suggestion heuristic (15-minute window)
- [ ] Drink type selector with inline creation
- [ ] Previous/preferred defaulting for drink and race setup
- [ ] Photo upload (separate endpoint)
- [ ] Auto-flagging for record-breaking runs without photos
- [ ] Runs API (create, list, delete, photo upload)
- [ ] Home screen showing recent runs

### Phase 4: Stats & Leaderboards
- [ ] Personal stats page (PBs, averages, run count, most-played track, best track)
- [ ] Full run history with detail view
- [ ] Per-track time history with chart
- [ ] Track leaderboard (alcoholic / non-alcoholic / combined toggle)
- [ ] Cup-level leaderboard
- [ ] Global leaderboard (most track records held)
- [ ] User rank pinned at bottom of leaderboards

### Phase 5: Social & Head-to-Head
- [ ] "Players you've competed with" (timestamp clustering)
- [ ] Head-to-head comparison view
- [ ] Win/loss records
- [ ] Profile page with improvement trends
- [ ] Flagging a run (user-initiated, with preset reasons + notes + visibility choice)
- [ ] Admin page (lightweight, env-variable-gated)
- [ ] Admin: review flagged runs, resolve, edit, or delete

### Phase 6: Camera/OCR (Future)
- [ ] Photo upload with each run (verification + training data)
- [ ] Use phone camera to photograph TV screen showing race time
- [ ] Extract time using OCR (likely browser-side Tesseract.js or similar)
- [ ] Auto-populate time field from photo
- [ ] Extract race setup from end-of-race screen
- [ ] Retire preferred race setup from user profiles once OCR is reliable

## Resolved Decisions

- **Global leaderboard ranking:** Most track records held.
- **Account recovery:** Admin reset for now.
- **Time entry validation:** No validation against plausible track times. Rely on photos and eventual OCR.
- **Beer vs water:** Separate leaderboards by default, with combined view. Default toggle matches user's preferred drink category.
- **Track variants:** 150cc only.
- **Admin model:** Lightweight admin page gated by user ID in env variable. No formal role system for MVP.
- **Run immutability:** Users cannot edit runs after creation. Admin can edit (for correcting OCR errors, etc.).
- **Head-to-head tracking:** No explicit sessions table. Derived from timestamp proximity.
- **Photo enforcement for records:** Runs are auto-flagged and hidden if record-breaking without a photo. Photo upload auto-resolves the flag.
- **Lap time column naming:** `lap1_time`, `lap2_time`, `lap3_time` (no underscore before digit). Matches SeaORM's `DeriveIden` macro output for variants like `Lap1Time`, avoiding unnecessary custom naming.
- **UUID storage in SQLite:** UUIDs stored as TEXT (not BLOB) for human readability when debugging with CLI tools. PostgreSQL migration will map to native UUID type. SeaORM maps both to `String` in Rust, so application code won't change.
- **Timestamp storage in SQLite:** Timestamps stored as TEXT in ISO 8601 format. SQLite has no native timestamp type. PostgreSQL migration will map to `TIMESTAMPTZ`.
- **run_flags audit trail:** `run_id` is not unique — a run can have multiple flags (different reasons tracked separately, resolved independently). Resolved flags are kept as history. Only duplicate flags (same run + same reason while unresolved) are prevented in application code.
## Future Ideas (Not Committed)

These are loose ideas that may or may not be pursued. No guarantees.

- Turn list of previous players into invite emails to join.
