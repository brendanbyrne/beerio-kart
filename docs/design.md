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
- `From` impls for `jsonwebtoken::errors::Error` and `argon2::password_hash::Error` map library errors to `Internal`, so `?` works directly on token operations and password hashing.
- `From<sea_orm::DbErr>` is variant-aware: `RecordNotFound` → `NotFound` (404), `SqlErr::UniqueConstraintViolation` → `Conflict` (409), `SqlErr::ForeignKeyConstraintViolation` → `BadRequest` (400), everything else → `Internal` (500). This preserves error semantics that a blanket-Internal mapping would otherwise hide.
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

See [`data-model.md`](./data-model.md) for the full database schema, table definitions, and design decisions about them.

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
POST   /sessions/:id/races/:race_id/skip   Mark a pending race as skipped for the requesting user (idempotent)
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
├── compose.yaml                 # Docker compose
├── justfile                     # Developer workflow commands (just)
├── README.md                    # Project entry point — points at docs/, backend/, frontend/
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
│       │   ├── drink_types.rs
│       │   ├── game_data.rs
│       │   ├── runs.rs
│       │   ├── sessions.rs
│       │   └── users.rs
│       ├── services/            # Business logic layer
│       │   ├── mod.rs
│       │   ├── auth.rs
│       │   ├── helpers.rs       # Reusable service-layer primitives
│       │   ├── runs.rs
│       │   ├── session_context.rs
│       │   ├── sessions.rs      # Session lifecycle, rulesets, track selection
│       │   └── users.rs
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
├── docs/                        # Project documentation
│   ├── design.md                # This file — architecture design (single source of truth)
│   ├── api-contract.md          # Wire-format conventions between backend and frontend
│   ├── compliance-plan.md       # Sequenced PRs to bring code to the coding standard
│   ├── coding-standards/        # Backend coding standards (rust.md, seaorm.md, tokio.md)
│   └── research/                # Long-form research (no decisions made; reference-only)
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
- [x] Pre-seeded data read endpoints (characters, bodies, wheels, gliders, cups, tracks)
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

See [`decisions/`](./decisions/) — each prior bullet has been distilled into a MADR file under `docs/decisions/`. The index in [`decisions/README.md`](./decisions/README.md) lists every ADR with its title, status, and date.

## Backlog


Random ideas that may or may not be pursued.

- Turn list of previous players into invite emails to join.
- Ability to Change Username.
- Ability to send emails.
   - Account recovery.
- Handle concurrent `next_track` calls gracefully. Currently a double-tap can hit the `UNIQUE(session_id, race_number)` constraint and return a 500. Options: retry-on-conflict or optimistic locking. Low priority — single host makes this very unlikely.

## Related documents

- **`api-contract.md`** — Wire-format conventions between backend and frontend (error codes, ETag polling, idempotency keys, time format).
- **`coding-standards/`** — Backend coding standards (general Rust, SeaORM, Tokio).
- **`compliance-plan.md`** — Sequenced PRs to bring the existing code into compliance with the coding standards.
- **`research/`** — Long-form exploration of approaches not yet decided (e.g., OCR strategy, SeaORM 2.0 evaluation). Reference-only; not authoritative until promoted into this file or `coding-standards/`.
- **`data-model.md`** — Database schema, table definitions, and design decisions about them. (Extracted from this file in PR 1 of the docs restructure.)
- **`decisions/`** — Architecture Decision Records (MADR format). Searchable index in `decisions/README.md`.
- **`designs/`** — Design records (per-session sign-off-style narratives of how decisions were reached). PR review feedback now lives on GitHub (PR comments, line-anchored).

## Document history

- 2026-05-02 — Moved from repo root (`DESIGN.md`) to `docs/design.md`. Project structure section updated to reflect the move and the new `docs/` layout. The root `DESIGN.md` is kept as a redirect (Cowork sandbox cannot delete files); a Claude Code PR will remove it from the working tree.
- 2026-05-04 — Updated the AppError "Key behaviors" bullet to reflect the variant-aware `From<sea_orm::DbErr>` impl (NotFound / Conflict / BadRequest / Internal mapping). PR #25.
- 2026-05-04 — Added `docs/research/` to the project-structure tree and a corresponding entry in Related documents (long-form exploration not yet promoted to design or coding-standards).
- 2026-05-04 — Replaced the "Entity regeneration via justfile recipe" rule with "Hand-written SeaORM entities"; updated the `just (not Make)` example to use `just entities-bootstrap`. Closes the codegen-strategy decision recorded at [`reviews/design/2026-05-02-entity-codegen-strategy.md`](./reviews/design/2026-05-02-entity-codegen-strategy.md). PR-X1.
- 2026-05-05 — Extracted Data Model section to `data-model.md`. PR 1 of the docs restructure.
- 2026-05-05 — Replaced the Resolved Decisions bullet list with a pointer to `docs/decisions/`. Each prior bullet distilled into a MADR file (0002–0034). PR 2 of the docs restructure.
