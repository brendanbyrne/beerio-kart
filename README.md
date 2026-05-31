# beerio-kart

[![backend coverage](https://img.shields.io/codecov/c/github/brendanbyrne/beerio-kart?flag=backend&label=backend%20coverage)](https://app.codecov.io/gh/brendanbyrne/beerio-kart?flags%5B0%5D=backend)
[![frontend coverage](https://img.shields.io/codecov/c/github/brendanbyrne/beerio-kart?flag=frontend&label=frontend%20coverage)](https://app.codecov.io/gh/brendanbyrne/beerio-kart?flags%5B0%5D=frontend)

There are two goals with this project.
* A mobile-first web app for tracking times and stats for the Mario Kart 8 Deluxe drinking game. Players race time trials, drink something bubbly, and the app tracks personal bests, leaderboards, and run history.
* Experimenting with using LLMs, with the intent to never write a line of actual code.  A test to see how far I can push a purely vibe coded project.

## Documentation

Project documentation lives in [`docs/`](./docs). Start with [`docs/README.md`](./docs/README.md) — a short index that routes you to the right file (architecture, data model, API contract, coding standards, roadmap, project workflow) based on what you're doing.

Run `tree` if you want a current map of the repo layout; directories whose purpose isn't self-evident carry their own `README.md`.

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

> **WSL2:** building under `/mnt/c` is slow. If `cargo build` drags, point Cargo's
> `target-dir` at the Linux filesystem via a repo-root `.cargo/config.toml`
> (`[build]` → `target-dir = "/home/<you>/.cargo-target/beerio-kart"`).

## Running (local dev)

```sh
bun run dev      # starts backend (port 3000) and frontend (port 5173) together
```

Open `http://localhost:5173` in your browser. The Vite dev server proxies API calls to the backend.

To run them separately:

```sh
cargo run                  # API server on port 3000
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
cargo test
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
cargo +nightly fmt --check && cargo clippy -- -D warnings

# Frontend
cd frontend && bunx eslint src/ && bunx prettier --check "src/**/*.{ts,tsx,css,json}"
```
