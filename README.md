# beerio-kart

[![codecov](https://codecov.io/gh/brendanbyrne/beerio-kart/graph/badge.svg)](https://codecov.io/gh/brendanbyrne/beerio-kart)

We're really doing this thing.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Bun](https://bun.sh/) (v1+)
- For Docker: [Docker](https://docs.docker.com/get-docker/) or [Podman](https://podman.io/)

## Setup

```sh
cp .env.example .env
# Edit .env and set JWT_SECRET to a random string
bun install      # installs all dependencies (root + frontend workspace)
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
| Format | `backend/*.rs` | `cargo fmt --check` |
| Lint | `backend/*.rs` | `cargo clippy` |
| Lint | `frontend/*.{ts,tsx}` | ESLint |
| Format | `frontend/*.{ts,tsx,css,json}` | Prettier |

To run manually:

```sh
# Backend
cd backend && cargo fmt --check && cargo clippy -- -D warnings

# Frontend
cd frontend && bunx eslint src/ && bunx prettier --check "src/**/*.{ts,tsx,css,json}"
```
