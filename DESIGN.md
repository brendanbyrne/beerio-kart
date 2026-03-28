# Beerio Kart - Architecture Design Document

## Overview

Beerio Kart is a mobile-first web app for tracking times and stats for the Mario Kart 8 Deluxe drinking game. Players race one at a time in Time Trial mode. You can't touch the controller while touching your drink (one 12oz beer or sparkling water). The app tracks personal times per track, head-to-head records, and leaderboards.

## Rules of the Game

1. Players race one at a time using Time Trial mode in Mario Kart 8 Deluxe.
2. You cannot touch the controller while touching your drink.
3. The drink is one 12oz beer or one 12oz sparkling water, poured into a cup.
4. You may restart the race if it is before the end of the first lap AND you haven't had any of your drink yet.
5. Played round robin — all players race the same track.
6. Fastest time wins.

## Tech Stack

| Layer       | Technology          | Rationale                                                   |
|-------------|---------------------|-------------------------------------------------------------|
| Backend     | Rust + Axum         | Learning opportunity; strong async/WebSocket support         |
| Frontend    | React + Vite        | Largest ecosystem for mobile-web; camera API support         |
| Styling     | Tailwind CSS        | Utility-first; fast iteration; mobile-first by convention    |
| Database    | SQLite (via rusqlite or sqlx) | File-based; no separate server; sufficient for this scale |
| Package mgr | Bun                 | Drop-in npm replacement; faster installs and script running  |
| Containers  | Dockerfile + compose.yaml | Works with Docker or Podman                             |

## Data Model

### Users
```
users
├── id: UUID (primary key)
├── username: TEXT (unique, display name)
├── email: TEXT (unique, optional, for account recovery)
├── password_hash: TEXT
├── created_at: TIMESTAMP
├── updated_at: TIMESTAMP
└── default_race_setup: RACESETUP
```

### Race Setup
```
race_setup
├── character_id: INTEGER (foreign key -> character)
└── car_setup: CARSETUP
```

### Characters
Pre-seeded table of all MK8 Deluxe characters (including DLC).
```
characters
├── id: INTEGER (primary key)
├── name: TEXT
└── image: FILEPATH
```

### Car Setup
```
car_setup
├── body_id: INTEGER (foreign key -> body)
├── wheels_id: INTEGER (foreign key -> wheels)
└── glider_id: INTEGER (foreign key -> glider)
```

### Bodies
Pre-seeded table of all MK8 Deluxe vehicle bodies.
```
bodies
├── id: INTEGER (primary key)
├── name: TEXT
└── image: FILEPATH
```

### Wheels
Pre-seeded table of all MK8 Deluxe vehicle wheels.
```
wheels
├── id: INTEGER (primary key)
├── name: TEXT
└── image: FILEPATH
```

### Gliders
Pre-seeded table of all MK8 Deluxe vehicle glider attachments.
```
gliders
├── id: INTEGER (primary key)
├── name: TEXT
└── image: FILEPATH
```

### Tracks
Pre-seeded table of all MK8 Deluxe tracks (including DLC).
```
tracks
├── id: INTEGER (primary key)
├── name: TEXT (e.g., "Rainbow Road")
├── image: FILEPATH
└── cup_id: INTEGER (foreign key -> cups)
```

### Cups
Pre-seeded table of all MK8 Deluxe Cups (including DLC).
```
cups
├── id: INTEGER (primary key)
├── name: TEXT (e.g., "Cloud Cup")
├── image: FILEPATH
└── track_ids: INTEGER[4] (array of foreign keys -> track)
```

### Runs
The core table.
One row per player per race attempt.
```
runs
├── id: UUID (primary key)
├── user_id: UUID (foreign key -> user)
├── race_setup: RACESETUP (defaults to what's store in the users profile)
├── track_id: INTEGER (foreign key -> track)
├── track_time: INTEGER (completion time in milliseconds)
├── lap_times[3]: INTEGER (completion time in milliseconds)
├── drink_type_id: UUID (foreign key -> drink_type)
├── created_at: TIMESTAMP (when entered into the system)
└── notes: TEXT (optional, e.g., "new controller, felt weird")
```

Would it be crazy if I wanted to store a photo of the tv screen with every run?
It would:
a) provide a way to manually validate a run.
b) provide training data for any future OCR algorithms

### Drink Types
Contains information about a given type of drink had during a run.
Think like, "Molsen Canadian" or "LaCroix Pamplemousse".
The leaderboards will default to separating results by alcholic vs non-alcoholic
```
drink_types
├── id: UUID (primary key)
├── alcholic: BOOLEAN
├── brand: TEXT
├── flavor: TEXT
└── image: FILEPATH
```

### Head-to-Head Context (optional, v1 stretch)

Won't do this as an explicit feature.  Going to just use the time of entry as a way to loosely cluster the data.

### Open Questions

* The concept of a `RaceSetup` is fungiable.
  * Is it better to separately store unique configations and reference them?  Or store a deep copy of the setup where it's needed?
  * Technically, you don't have to store multiple copies of the same setup.  You could reference to them in some separate cache.
  * There are a fixed number of combinations, but that number is around 3 million.
  * It seems both simplier and more robust to store copies.
    * Like the utility is a function of the runs tracked.
    * How hard would it be to switch from copies to references in the future?
* Is there an existing industry standard pattern for associating image data with items in the database?
* Naming convention for properties vs keys vs ids etc...
* How to represent a fixed size array in a database field?
  * e.g. all courses have 3 laps, all cups have 4 tracks, etc...
* Does the length of a field's name impact how much system resources are used?
* Possible to encrypt the database itself?
* Possible to store an image with every run?

## API Surface

All endpoints prefixed with `/api/v1`.

I'm not very familiar with API surfaces.
I would hear more about what a good API surface should provide.
Eventually I'd like to be able to query the data in interesting ways that I can't enumerate right now.

### Auth
Does this imply that I'm rolling my own auth?  I don't want to make any more security critical code than I need to.
```
POST   /auth/register          Create account (username, password)
POST   /auth/login             Get auth token
POST   /auth/logout            Invalidate token
```

### Runs
```
POST   /runs                   Record a new run
GET    /runs?user_id=&track_id=&limit=&offset=   Query runs with filters
GET    /runs/:id               Get a specific run
DELETE /runs/:id               Delete a run (owner only)
```

### Tracks
```
GET    /tracks                 List all tracks (with optional cup filter)
GET    /tracks/:id             Get track details
```

### Stats
```
GET    /stats/personal/:user_id                  Personal summary (best times, avg, run count)
GET    /stats/personal/:user_id/track/:track_id  Per-track breakdown (PB, avg, history)
GET    /stats/leaderboard/track/:track_id        Track leaderboard (best time per user)
GET    /stats/leaderboard/global                 Aggregate leaderboard (ranking method TBD)
GET    /stats/head-to-head/:user_id_1/:user_id_2 H2H record between two players
```

### Users
```
GET    /users                  List all users (public profiles)
GET    /users/:id              Get user profile
PUT    /users/:id              Update profile (self only)
```

## UI Screens (Mobile-First)

### 1. Login / Register
Simple form. Username + password. No email required for v1.

### 2. Home / Dashboard
- Quick-action button: "Enter a Run"
- Recent runs (your last 5)
- Your overall rank (methodology TBD)
- Default Race Setup

### 3. Record a Run
Core workflow:
1. Select track (searchable dropdown or grouped by cup)
2. Enter time (MM:SS.mmm format — manual entry for v1, camera/OCR later)
3. Select drink
4. Change race setup (defaults to user's default)

### 4. Track Browser
- List of all tracks grouped by cup
- Tap a track to see:
  - Your personal history (chart of times over time)
  - Your Personal Best
  - Track leaderboard

### 5. Leaderboard
- Per-track leaderboards (best time per player)
- Overall ranking (methodology TBD — could be # of track PBs held, average percentile, etc.)

### 6. Profile / Personal Stats
- All-time stats: total runs, favorite track, best track (vs leaderboard)
- Per-track breakdown
- Head-to-head records against specific players
- Improvement trends

## Project Structure

```
beerio-kart/
├── compose.yaml                 # Docker/Podman compose
├── Makefile                     # Or justfile — common commands
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
└── data/
    ├── tracks.json              # MK8D track seed data
    └── beerio-kart.db           # SQLite database file (gitignored)
```

### Open Questions
* Directory for documention?
* Directory for and AI based documentation like skills?
 * Should this be handled outside of the repo?

## Build Plan (Phases)

### Phase 1: Foundation
- [ ] Initialize Rust project with Axum
- [ ] Initialize React project with Vite + Bun + Tailwind
- [ ] Set up SQLite with migrations (users, tracks, runs tables)
- [ ] Seed MK8 Deluxe track list
- [ ] Basic auth (register/login)
- [ ] Dockerfiles + compose.yaml

### Phase 2: Core Feature — Recording Runs
- [ ] "Record a Run" form (track selection, manual time entry, drink type, restart toggle)
- [ ] Runs API (create, list, delete)
- [ ] Home screen showing recent runs

### Phase 3: Stats & Leaderboards
- [ ] Personal stats page (PBs, averages, run count per track)
- [ ] Per-track time history with chart
- [ ] Per-track leaderboard
- [ ] Global leaderboard (ranking methodology TBD)

### Phase 4: Social & Head-to-Head
- [ ] Matchup tracking (group runs that happened together)
- [ ] Head-to-head comparison view
- [ ] Win/loss records

### Phase 5: Camera/OCR (Future)
- [ ] Use phone camera to photograph TV screen showing race time
- [ ] Extract time using OCR (likely via browser-side Tesseract.js or similar)
- [ ] Auto-populate time field from photo

## Open Questions

1. **Global leaderboard ranking method** — Number of track PBs held? Average percentile across tracks? ELO-style rating from matchups? Needs discussion.

2. **Account recovery** — Email-based? Or keep it simple with admin reset?
  Keep it simple for now.

3. **Time entry validation** — Should we validate that entered times are plausible for a given track? (e.g., reject a 30-second Rainbow Road time)
  No.  Maybe store images.  And eventually use OCR to extract the times itself.

4. **Beer vs water stats** — Should leaderboards separate beer and water runs, or combine them? Are water runs "official"?
  Separate them, but provide a way to view everything combined as well.  Water runs are official, but also officially separate...for now.

5. **Track variants** — MK8D has 150cc, 200cc, Mirror. Do we care? Time Trials are typically 150cc.
  150 cc only.
