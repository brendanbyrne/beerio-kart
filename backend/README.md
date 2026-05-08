# Backend

Rust web server for Beerio Kart, built with [Axum](https://github.com/tokio-rs/axum).

## Tech

- **Framework:** [Axum](https://github.com/tokio-rs/axum) (async web framework)
- **Runtime:** [Tokio](https://tokio.rs) (multi-thread async runtime)
- **ORM:** [SeaORM](https://www.sea-ql.org/SeaORM/) backed by [SQLx](https://docs.rs/sqlx)
- **Database:** SQLite (with a path to PostgreSQL later — see `../docs/design.md`)
- **Auth:** [argon2](https://docs.rs/argon2) (password hashing) + [jsonwebtoken](https://docs.rs/jsonwebtoken) (JWT access + refresh tokens)
- **Logging:** [tracing](https://docs.rs/tracing) + `tower-http::TraceLayer`
- **Edition:** Rust 2024 (1.85+)

## Running

```sh
cargo run
```

Server starts on `http://localhost:3000`. The API is mounted at `/api/v1/*`; everything else serves the React frontend (built from `../frontend`).

## Tests

```sh
cargo test
```

Integration tests live in [`tests/`](./tests); unit tests live inline as `#[cfg(test)] mod tests` blocks in the file they cover.

## Layout

| Path | Contents |
|------|----------|
| [`src/main.rs`](./src/main.rs) | Server bootstrap, routing, middleware, shutdown |
| [`src/config.rs`](./src/config.rs) | Environment / config loading |
| [`src/error.rs`](./src/error.rs) | Unified `AppError` and `IntoResponse` |
| [`src/routes/`](./src/routes) | HTTP handlers (one file per resource) |
| [`src/services/`](./src/services) | Business logic (called from routes) |
| [`src/middleware/`](./src/middleware) | Auth, admin checks |
| [`src/entities/`](./src/entities) | Hand-written SeaORM entity types (per ADR 0023) |
| [`src/domain/`](./src/domain) | Hand-written domain types (IDs, enums, validated newtypes) |
| [`migration/`](./migration) | Schema migrations (workspace member crate) |

## Required reading

Before opening a PR that touches the backend, skim these:

- **[`../docs/design.md`](../docs/design.md)** — Architecture, data model, API surface, design decisions. Single source of truth.
- **[`../docs/coding-standards/`](../docs/coding-standards)** — Coding rules, split by area:
  - **[`rust.md`](../docs/coding-standards/rust.md)** — Errors, types, modules, testing, lints, docs, formatting, deps, config.
  - **[`seaorm.md`](../docs/coding-standards/seaorm.md)** — `ActiveModel` vs `Model`, queries, transactions, migrations, error handling, pool tuning.
  - **[`tokio.md`](../docs/coding-standards/tokio.md)** — Runtime, blocking, locks across `.await`, channels, cancellation, shutdown.
- **[`../docs/api-contract.md`](../docs/api-contract.md)** — Wire-format decisions (error codes, ETag polling, idempotency keys, time format).
- **[`../docs/compliance-plan.md`](../docs/compliance-plan.md)** — Sequenced PRs to bring the existing code to the standard. If you're picking work, look here.

If a PR introduces a pattern not covered by the standards, propose an addition in a design record under [`../docs/designs/`](../docs/designs) before merging.

## Justfile recipes

The repo's [`justfile`](../justfile) defines common workflow commands. From the repo root:

```sh
just dev         # run dev server (frontend + backend)
just test        # run cargo test
just entities-bootstrap  # one-shot scaffold for a new table — hand-edit afterward (see seaorm.md § 6)
just coverage    # generate HTML coverage report
```
