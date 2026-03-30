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
| Database    | SQLite (via sqlx)             | File-based; no separate server; sufficient for this scale     |
| Package mgr | Bun                           | Drop-in npm replacement; faster installs and script running   |
| Containers  | Dockerfile + compose.yaml    | Works with Docker or Podman                                  |

## Naming Conventions

- Table names: plural, snake_case (`drink_types`, `characters`)
- Column names: snake_case (`track_time`, `created_at`)
- Foreign keys: `{referenced_table_singular}_id` (`character_id`, `cup_id`)
- Primary keys: `id`

## Design Principles

- **Minimize manual input.** Every design decision should favor automatically deducing information over requiring users to enter it explicitly.
- **Don't overengineer before OCR.** Many corner cases (time validation, race setup entry, session tracking) will be solved by OCR. Design the MVP for manual entry with hooks for OCR to slot in later.

## Data Model

### Design Decisions

- **UUID vs INTEGER primary keys.** INTEGER for pre-seeded static data (characters, tracks, cups, bodies, wheels, gliders) — stable, small, human-readable. UUID for user-generated runtime data (users, runs, drink_types) — globally unique, can be generated client-side without a database round trip (important for future offline support).
- **RaceSetup stored inline, not normalized.** Character, body, wheels, and glider IDs are stored directly on the `runs` and `users` tables rather than in separate junction tables. With ~3 million possible combinations (most never used), a reference table is wasteful. Inline storage costs 4 integer columns (16 bytes) — negligible. Migration to a normalized form later is straightforward if needed.
- **Images stored on disk, paths in the database.** Pre-seeded assets (characters, tracks, kart parts) ship as static files. User-uploaded photos (run verification) are saved to a configurable uploads directory. Database stores relative paths (e.g., `images/characters/mario.png`).
- **Fixed-size arrays use separate columns or relational joins.** Lap times (always 3) become `lap_1_time`, `lap_2_time`, `lap_3_time` — simple to query. Cup-to-track relationships use the `cup_id` foreign key on the `tracks` table, not an array on `cups`.
- **Leaderboards separate alcoholic and non-alcoholic runs by default**, with a combined view available.
- **Nullability defaults to NOT NULL** unless there is a clear reason for the data to be optional. Nullable columns map to `Option<T>` in Rust, adding handling overhead.
- **Database encryption** via SQLCipher is possible but deferred past v1.

### Users

User-modifiable: yes (own profile, default race setup).

```
users
├── id: UUID (primary key)
├── username: TEXT (unique, not null, 1-30 characters)
├── email: TEXT (unique, nullable — for account recovery)
├── password_hash: TEXT (not null)
├── default_character_id: INTEGER (foreign key -> characters, nullable)
├── default_body_id: INTEGER (foreign key -> bodies, nullable)
├── default_wheels_id: INTEGER (foreign key -> wheels, nullable)
├── default_glider_id: INTEGER (foreign key -> gliders, nullable)
├── created_at: TIMESTAMP (not null)
└── updated_at: TIMESTAMP (not null)
```

Notes:
- Default race setup columns are nullable (new user hasn't picked yet). All-or-nothing: either all four are set or none are. Enforced in application code, not the database.
- SQLite allows multiple NULLs in a UNIQUE column (email), which is the desired behavior.
- `email` validated as valid format in application code if provided.
- Default race setup will eventually be retired once OCR extracts setup from TV screen photos.

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

Note: Cup-to-track mapping is handled by the `cup_id` foreign key on the `tracks` table. Application-level validation ensures each cup has exactly 4 tracks after seeding.

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

User-created. Specific beverages used during runs (e.g., "Molson Canadian", "LaCroix Pamplemousse"). Users can submit new drink types. Deduplication is handled via deterministic UUID.

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

The core table. One row per player per race attempt. User-created, immutable (times cannot be edited after creation), deletable by owner only.

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
├── lap_1_time: INTEGER (milliseconds, not null, must be positive and non-zero)
├── lap_2_time: INTEGER (milliseconds, not null, must be positive and non-zero)
├── lap_3_time: INTEGER (milliseconds, not null, must be positive and non-zero)
├── drink_type_id: UUID (foreign key -> drink_types, not null)
├── photo_path: TEXT (nullable — optional for regular runs, required for record-breaking runs)
├── flagged_for_review: BOOLEAN (not null, default false)
├── created_at: TIMESTAMP (not null, defaults to current time, optionally user-provided)
└── notes: TEXT (nullable — freeform; may be mined for future structured columns)
```

Validation (application-level):
- `track_time` must be positive.
- All three lap times must be positive and non-zero.
- Lap times should roughly sum to `track_time` (with tolerance for game rounding).
- If this run would set a new track record, a photo is required. The frontend checks this before submission and prompts for a photo. The API enforces it as well.
- Race setup columns pre-fill from user defaults but are all required.

Flagging:
- Users can flag their own run for admin review (e.g., OCR extracted wrong data), but only if the run has a photo attached.
- Users cannot flag other users' runs.

Future (OCR):
- The end-of-race TV screen shows race setup, track, and all 3 lap times. OCR will eventually extract all of this automatically.
- Photos on all runs provide training data for OCR, even when not required.
- Once OCR is reliable, the `created_at` override becomes unnecessary (live capture only).

### Head-to-Head Context

Not an explicit feature. Runs played in the same round robin are loosely clustered by `created_at` timestamps. Head-to-head stats (Phase 5) are derived from timestamp proximity, not user-managed grouping. This avoids adding manual "session" bookkeeping — consistent with the design principle of minimizing manual input.

## API Surface

All endpoints prefixed with `/api/v1`.

### What the API provides

The API is the contract between the frontend and backend. The frontend never touches the database directly — it makes HTTP requests to the Rust server, which validates input, runs business logic, and returns JSON. This follows REST conventions: resources (runs, tracks, users) are nouns in the URL, HTTP methods (GET, POST, PUT, DELETE) are the verbs.

For future flexibility (querying data in ways not yet enumerated), the runs endpoint supports generous query parameters for filtering, sorting, and pagination. If this becomes insufficient, a GraphQL layer (`async-graphql` crate) can be added alongside REST later.

### Auth

Uses established Rust crates — not rolling crypto from scratch. `argon2` for password hashing, `jsonwebtoken` for JWT tokens. ~150 lines of code wrapping audited libraries. Sufficient for a self-hosted friends-and-game-night app. Account recovery is admin-reset for now.

```
POST   /auth/register          Create account (username, password)
POST   /auth/login             Returns JWT token
POST   /auth/logout            Invalidate token
```

### Runs

```
POST   /runs                   Record a new run (with optional photo upload)
GET    /runs                   Query runs (filters: user_id, track_id, drink_type_id,
                                           alcoholic, after, before, sort, limit, offset)
GET    /runs/:id               Get a specific run
DELETE /runs/:id               Delete a run (owner only)
```

### Tracks

```
GET    /tracks                 List all tracks (optional filter: cup_id)
GET    /tracks/:id             Get track details
```

### Cups

```
GET    /cups                   List all cups
GET    /cups/:id               Get cup with its tracks
```

### Drink Types

```
POST   /drink_types            Create a new drink type
GET    /drink_types             List all drink types (optional filter: alcoholic)
GET    /drink_types/:id        Get drink type details
```

### Stats

```
GET    /stats/personal/:user_id                  Personal summary (best times, averages, run count)
GET    /stats/personal/:user_id/track/:track_id  Per-track breakdown (PB, average, history)
GET    /stats/leaderboard/track/:track_id        Track leaderboard (best time per user)
GET    /stats/leaderboard/global                 Aggregate leaderboard (ranking method TBD)
GET    /stats/head-to-head/:user_id_1/:user_id_2 H2H record between two players
```

All leaderboard endpoints accept `?alcoholic=true|false|all` to filter by drink category (defaults to separating them).

### Users

```
GET    /users                  List all users (public profiles)
GET    /users/:id              Get user profile + default race setup
PUT    /users/:id              Update profile / default race setup (self only)
```

## UI Screens (Mobile-First)

### 1. Login / Register
Simple form. Username + password. No email required for v1.

### 2. Home / Dashboard
- Quick-action button: "Enter a Run"
- Recent runs (your last 5)
- Your overall rank (methodology TBD)
- Default Race Setup (character + kart displayed)

### 3. Record a Run
Core workflow:
1. Select track (searchable dropdown or grouped by cup)
2. Enter time (MM:SS.mmm format — manual entry for v1, camera/OCR later)
3. Select drink
4. Change race setup (defaults to user's default)
5. Optional: take/upload photo of TV screen

### 4. Track Browser
- List of all tracks grouped by cup
- Tap a track to see:
  - Your personal history (chart of times over time)
  - Your Personal Best
  - Track leaderboard

### 5. Leaderboard
- Per-track leaderboards (best time per player)
- Toggle: alcoholic / non-alcoholic / combined
- Overall ranking (methodology TBD — could be # of track PBs held, average percentile, etc.)

### 6. Profile / Personal Stats
- All-time stats: total runs, favorite track, best track (vs leaderboard)
- Per-track breakdown
- Head-to-head records against specific players
- Improvement trends

## Project Structure

```
beerio-kart/
├── .claude/
│   └── CLAUDE.md                # AI assistant context (checked into repo)
│
├── docs/
│   └── DESIGN.md                # This design document
│
├── compose.yaml                 # Docker/Podman compose
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
│       │   └── models.rs        # Rust structs matching DB tables
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── auth.rs
│       │   ├── runs.rs
│       │   ├── tracks.rs
│       │   ├── stats.rs
│       │   └── users.rs
│       ├── services/            # Business logic layer
│       │   ├── mod.rs
│       │   ├── auth.rs
│       │   └── stats.rs
│       └── middleware/
│           ├── mod.rs
│           └── auth.rs          # JWT/session validation
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
│       ├── pages/               # Screen-level components
│       │   ├── Home.tsx
│       │   ├── Login.tsx
│       │   ├── RecordRun.tsx
│       │   ├── TrackBrowser.tsx
│       │   ├── TrackDetail.tsx
│       │   ├── Leaderboard.tsx
│       │   └── Profile.tsx
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
- [ ] Set up SQLite with migrations (all tables)
- [ ] Seed MK8 Deluxe data (tracks, cups, characters, bodies, wheels, gliders)
- [ ] Basic auth (register/login with argon2 + JWT)
- [ ] Dockerfiles + compose.yaml

### Phase 2: Deployment
- [ ] Create Dockerfile for backend (multi-stage: build Rust binary, copy to slim runtime image)
- [ ] Create Dockerfile for frontend (build with Bun, serve with nginx or similar)
- [ ] Create compose.yaml (backend + frontend + shared volume for SQLite + uploads)
- [ ] Configure Cloudflare tunnel to route domain to the app on Unraid
- [ ] Verify HTTPS works end-to-end through Cloudflare
- [ ] Test basic auth flow from phone over real network
- [ ] Add .env / config for production vs development settings
- [ ] Set up sqlx offline mode (`cargo sqlx prepare`, check `.sqlx/` into git) for Docker builds

Note: Deploying early (before core features) keeps the deployment simple and catches infrastructure issues before application complexity grows. The Dockerfiles are listed in Phase 1 as well — Phase 1 creates them for local development, Phase 2 validates they work on the actual Unraid server behind Cloudflare.

### Phase 3: Core Feature — Recording Runs
- [ ] "Record a Run" form (track selection, manual time entry, drink type, race setup)
- [ ] Drink type management (create new drink types)
- [ ] Runs API (create, list, delete)
- [ ] Home screen showing recent runs

### Phase 4: Stats & Leaderboards
- [ ] Personal stats page (PBs, averages, run count per track)
- [ ] Per-track time history with chart
- [ ] Per-track leaderboard (alcoholic / non-alcoholic / combined toggle)
- [ ] Global leaderboard (ranking methodology TBD)

### Phase 5: Social & Head-to-Head
- [ ] Head-to-head comparison view (based on created_at clustering)
- [ ] Win/loss records
- [ ] Profile page with improvement trends

### Phase 6: Camera/OCR (Future)
- [ ] Photo upload with each run (verification + training data)
- [ ] Use phone camera to photograph TV screen showing race time
- [ ] Extract time using OCR (likely browser-side Tesseract.js or similar)
- [ ] Auto-populate time field from photo

## Open Questions

1. **Global leaderboard ranking method** — Number of track PBs held? Average percentile across tra