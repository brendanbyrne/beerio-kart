# beerio-kart

[![codecov](https://codecov.io/gh/brendanbyrne/beerio-kart/graph/badge.svg)](https://codecov.io/gh/brendanbyrne/beerio-kart)

We're really doing this thing.  There are two goals with this project.
* A mobile-first web app for tracking times and stats for the Mario Kart 8 Deluxe drinking game. Players race time trials, optionally drink, and the app tracks personal bests, leaderboards, and run history.
* Experimenting with using LLMs, with the intent to never write a line of actual code.  A test to see how far I can push a purely vibe coded project.

## Project layout

| Path | What's there |
|------|--------------|
| [`backend/`](./backend) | Rust API server (Axum, SeaORM, SQLite). See [`backend/README.md`](./backend/README.md). |
| [`frontend/`](./frontend) | React + TypeScript + Vite mobile-first web app. See [`frontend/README.md`](./frontend/README.md). |
| [`docs/`](./docs) | Project documentation — see below. |
| [`reviews/`](./reviews) | Design and PR review records (`reviews/design/`, `reviews/pr/`). |
| [`data/`](./data) | Seed data (tracks, characters, etc.) and the gitignored SQLite DB / uploads. |
| [`compose.yaml`](./compose.yaml), [`Dockerfile`](./Dockerfile) | Single-container deployment. |
| [`justfile`](./justfile) | Developer workflow recipes. |

## Documentation

Everything lives in [`docs/`](./docs):

- **[`docs/design.md`](./docs/design.md)** — Architecture design document. Single source of truth for the project's design decisions and data model. Read this first.
- **[`docs/api-contract.md`](./docs/api-contract.md)** — Wire-format conventions between backend and frontend (error codes, ETag polling, idempotency keys, time format).
- **[`docs/coding-standards/`](./docs/coding-standards)** — Backend coding standards split by area: general Rust (`rust.md`), SeaORM (`seaorm.md`), async/Tokio (`tokio.md`), plus a `README.md` index.
- **[`docs/compliance-plan.md`](./docs/compliance-plan.md)** — Sequenced PRs to bring the existing code into compliance with the coding standards.

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
