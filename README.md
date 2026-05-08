# beerio-kart

[![codecov](https://codecov.io/gh/brendanbyrne/beerio-kart/graph/badge.svg)](https://codecov.io/gh/brendanbyrne/beerio-kart)

There are two goals with this project.
* A mobile-first web app for tracking times and stats for the Mario Kart 8 Deluxe drinking game. Players race time trials, drink something bubbly, and the app tracks personal bests, leaderboards, and run history.
* Experimenting with using LLMs, with the intent to never write a line of actual code.  A test to see how far I can push a purely vibe coded project.

## Project structure

```
beerio-kart/
├── .claude/                       # AI assistant context
│   ├── CLAUDE.md                  # Project conventions (every-session read)
│   ├── claude-code-notes.md       # Claude Code self-notes across sessions
│   └── cowork-notes.md            # Cowork self-notes across sessions
│
├── .github/
│   ├── ISSUE_TEMPLATE/            # bug.md, feature.md
│   ├── pull_request_template.md
│   └── workflows/                 # link-check.yml (lychee), coverage.yml
│
├── backend/                       # Rust + Axum API server
│   ├── Cargo.toml
│   ├── README.md
│   ├── migration/                 # SeaORM migrations (single consolidated file prelaunch)
│   ├── src/
│   │   ├── main.rs                # Axum server setup, routing
│   │   ├── lib.rs
│   │   ├── config.rs              # Environment/config management
│   │   ├── db.rs                  # DB connection + migration runner
│   │   ├── error.rs               # AppError unified error type
│   │   ├── seed.rs                # Pre-seeded data loader
│   │   ├── test_helpers.rs        # Shared test utilities
│   │   ├── domain/                # Domain primitives (enums, IDs, race setup)
│   │   ├── entities/              # Hand-written SeaORM entities (per ADR 0023)
│   │   ├── middleware/            # JWT extractors
│   │   ├── routes/                # HTTP handlers per resource
│   │   └── services/              # Business logic layer
│   └── tests/                     # Integration + verification tests
│
├── frontend/                      # React + Vite + TypeScript + Tailwind
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.*.json
│   ├── eslint.config.js
│   ├── index.html
│   ├── public/                    # favicon.svg
│   └── src/
│       ├── main.tsx
│       ├── App.tsx
│       ├── api/                   # API client functions
│       ├── components/            # Reusable UI components
│       ├── hooks/                 # Custom React hooks
│       ├── pages/                 # Screen-level components
│       └── utils/                 # time.ts, etc.
│
├── docs/                          # Project documentation — see Documentation below
│
├── tools/                         # Developer CLI utilities (login, register, change-password, ...)
│
├── data/
│   ├── *.json                     # MK8D seed data (tracks, characters, bodies, wheels, gliders, cups, drink_types)
│   ├── db/                        # SQLite database file (gitignored)
│   └── uploads/                   # User-uploaded run photos (gitignored)
│
├── compose.yaml, Dockerfile       # Single-container deployment (Axum serves API + frontend bundle)
├── justfile                       # Developer workflow recipes (`just dev`, `just test`, ...)
├── package.json, bun.lock         # Bun root workspace + dev tooling (lefthook hooks)
├── lefthook.yml                   # Pre-commit hook config
├── codecov.yml                    # Codecov upload config
├── rustfmt.toml                   # Rustfmt rules (uses nightly-only options)
└── .env.example                   # Copy to `.env`; set JWT_SECRET to a random string
```

## Documentation

Everything lives in [`docs/`](./docs). Start with [`docs/README.md`](./docs/README.md) — it's a short index that routes you to the right file based on what you're doing.

The shape:

- **[`docs/design.md`](./docs/design.md)** — Architectural overview: rules of the game, principles, tech stack, observability, naming. Read this first.
- **[`docs/data-model.md`](./docs/data-model.md)** — Database schema, table definitions, schema-design decisions.
- **[`docs/api-contract.md`](./docs/api-contract.md)** — Endpoint catalog plus wire-format conventions (error codes, ETag polling, idempotency, time format).
- **[`docs/user-workflows.md`](./docs/user-workflows.md)** — End-user flows (registration → racing → stats → admin) and screen-by-screen UI breakdown.
- **[`docs/roadmap.md`](./docs/roadmap.md)** — Cup-by-cup narrative of where the project is going (Mushroom, Flower, Star, Special, ...).
- **[`docs/project-workflow.md`](./docs/project-workflow.md)** — Operational guide: Issue lifecycle, milestone conventions, PR conventions, triage, multi-assistant coordination.
- **[`docs/compliance-plan.md`](./docs/compliance-plan.md)** — Sequenced PRs to bring the existing code into compliance with the coding standards.
- **[`docs/decisions/`](./docs/decisions)** — Architecture Decision Records in MADR format. Searchable index in [`docs/decisions/README.md`](./docs/decisions/README.md).
- **[`docs/designs/`](./docs/designs)** — Design records: durable narratives of design sessions that produced one or more ADRs.
- **[`docs/coding-standards/`](./docs/coding-standards)** — Backend coding standards split by area: general Rust, SeaORM, Tokio.
- **[`docs/research/`](./docs/research)** — Long-form technical investigations that inform designs but don't propose decisions themselves.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable, plus `nightly` for `rustfmt` only — see Setup)
- [Bun](https://bun.sh/) (v1+)
- For Docker: [Docker](https://docs.docker.com/get-docker/) or [Podman](https://podman.io/)

## Setup

```sh
cp .env.example .env
# Edit .env and set JWT_SECRET to a random string
bun install      # installs all dependencies (root + frontend workspace)

# Install nightly rustfmt (used only for `cargo fmt`; builds and tests
# stay on stable). Required because rustfmt.toml uses imports_granularity
# and group_imports, both nightly-only as of Rust 1.94.
rustup toolchain install nightly --component rustfmt
```

## Running (local dev)

```sh
bun run dev      # starts backend (port 3000) and frontend (port 5173) together
```

Open `http://localhost:5173` in your browser. The Vite dev server proxies API calls to the backend.

To run them separately:

```sh
cd backend && cargo run    # API server on port 3000
cd frontend && bun dev     # Dev server on port 5173
```

## Running (Docker)

Build and start the app in a single container. Axum serves both the API and the frontend.

```sh
# Set JWT_SECRET in your .env file first (see .env.example)

# With Docker:
docker compose up --build

# With Podman:
podman compose up --build
# Or without compose:
podman build -t beerio-kart .
podman run --rm -p 3000:3000 -e JWT_SECRET=your-secret-here beerio-kart
```

Open `http://localhost:3000`. The API is at `/api/v1/*`, everything else serves the React frontend.

Data is persisted in Docker volumes (`db-data` for the SQLite database, `uploads` for user photos). To reset:

```sh
docker compose down -v   # removes volumes (deletes all data)
```

## Running tests

```sh
cd backend && cargo test
```

## Linting & Formatting

Pre-commit hooks run automatically via [lefthook](https://github.com/evilmartians/lefthook). After cloning:

```sh
bun install          # also installs lefthook as a devDependency
bunx lefthook install
```

The pre-commit hook runs these checks in parallel:

| Check | Scope | Tool |
|-------|-------|------|
| Format | `backend/*.rs` | `cargo +nightly fmt --check` |
| Lint | `backend/*.rs` | `cargo clippy` |
| Lint | `frontend/*.{ts,tsx}` | ESLint |
| Format | `frontend/*.{ts,tsx,css,json}` | Prettier |

To run manually:

```sh
# Backend (or `just fmt` for the format step)
cd backend && cargo +nightly fmt --check && cargo clippy -- -D warnings

# Frontend
cd frontend && bunx eslint src/ && bunx prettier --check "src/**/*.{ts,tsx,css,json}"
```
