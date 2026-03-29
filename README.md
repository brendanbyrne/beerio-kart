# beerio-kart

We're really doing this thing.

## Running

```sh
bun install      # install root + frontend dependencies
bun run dev      # starts backend (port 3000) and frontend (port 5173) together
```

Open `http://localhost:5173` in your browser.

To run them separately:

```sh
cd backend && cargo run    # API server on port 3000
cd frontend && bun dev     # Dev server on port 5173
```

## Linting & Formatting

Pre-commit hooks run automatically via [lefthook](https://github.com/evilmartians/lefthook). After cloning, install them:

```sh
bun install
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
