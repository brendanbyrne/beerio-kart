# Beerio Kart — Backend

Loaded automatically when Claude works in `backend/`. Captures conventions specific to the Rust + Axum + SeaORM + SQLite layer. For project-wide conventions (handoffs, GitHub access, two-assistant coordination, etc.), the root [`.claude/CLAUDE.md`](../.claude/CLAUDE.md) still applies — this file adds backend-specific layers, doesn't replace them.

## Stack

- **Rust** (stable, plus nightly for `rustfmt` only — see root README Setup).
- **Axum** for HTTP routing and middleware.
- **SeaORM** (built on `sqlx`) for the ORM. Hand-written entities per ADR 0023.
- **SQLite** for storage. STRICT mode on lookup tables only — see ADR 0002.
- **`tracing` + `tracing-subscriber` + `tower-http::TraceLayer`** for observability — see [`docs/design.md`](../docs/design.md) § Observability.
- **`argon2` + `jsonwebtoken`** for auth — see [`docs/api-contract.md`](../docs/api-contract.md) § 1.1 and § 5, ADR 0031.

## Key reading

For deep conventions, read these once and refer back when relevant:

- [`docs/coding-standards/rust.md`](../docs/coding-standards/rust.md) — general Rust style: error handling, module structure, dependency rules.
- [`docs/coding-standards/seaorm.md`](../docs/coding-standards/seaorm.md) — SeaORM patterns: builder vs raw SQL, entity conventions, migration handling, error mapping.
- [`docs/coding-standards/tokio.md`](../docs/coding-standards/tokio.md) — async/Tokio: spawn vs blocking, cancellation, lifetime tips.
- [`docs/data-model.md`](../docs/data-model.md) — schema, table definitions, FK conventions.
- [`docs/api-contract.md`](../docs/api-contract.md) — endpoint catalog (§ 1) plus wire-format conventions (error codes, ETag polling, idempotency, time format).
- [`docs/compliance-plan.md`](../docs/compliance-plan.md) — sequenced PRs to bring existing code up to the coding standards.

## ORM usage

Use SeaORM's builder API for single-table reads and writes (`Entity::find()`, `Entity::find_by_id`, `ActiveModel::insert` / `update`). Drop to raw SQL via `find_by_statement` only for multi-table JOINs where the builder's JOIN ergonomics become clumsy. Avoid hand-rolling SQL for single-table ops — the builder gives you type safety and refactor-proofness for free.

## Naming

- Table names: plural, snake_case (`drink_types`, `characters`).
- Column names: snake_case (`track_time`, `created_at`).
- Foreign keys: `{referenced_table_singular}_id` (`character_id`, `cup_id`).
- Primary keys: `id`.

Rust style: standard `rustfmt` (nightly options) + `clippy` defaults. Lefthook runs both pre-commit; see root README § Linting & Formatting.

## Schema changes (prelaunch)

While the project is prelaunch, **all schema lives in a single consolidated migration file**. New schema work edits that file rather than appending a new one. Rationale: pre-launch we don't preserve dev data, so the append-only history that migrations normally provide isn't earning its keep — it's just N files where 1 would do.

Operating rules:

- **Edit, don't append.** Adding a table, column, index, or constraint means modifying the consolidated migration file (currently `backend/migration/src/m20260101_000001_initial_schema.rs`). Do not create a new migration file.
- **Reset the dev DB after schema edits.** Delete the local SQLite file (or run the project's `dev-reset` task if/when one exists) before booting. SeaORM will recreate the schema from the consolidated migration on next startup.
- **No data preservation between schema versions.** If you have meaningful local test data, recreate it via seed or test fixtures after the reset, not by hand.
- **Code that depends on schema must change in the same PR as the migration edit.** Entities, services, tests — all in one atomic commit.

When we exit prelaunch (decided when we have real user data we don't want to lose), this convention flips back to standard append-only migrations: every schema change becomes a new file, and the consolidated initial migration becomes the immutable starting point. This file will be updated at that time.

## Testing

**Tests are a deliverable, not optional.** Every PR that adds business logic must include tests. PRs should not be opened without them.

- **Unit tests:** Use `#[cfg(test)] mod tests { }` in the same file as the code being tested. Cover business logic: validation rules, service functions, data transformations, error cases.
- **Integration tests:** Use `tests/` directory or Axum's test utilities to test HTTP endpoints end-to-end. Cover the happy path and key error cases (bad input, auth failures, not found, conflicts).
- **Verification tests:** Drift checks that exercise structural invariants between layers, not feature behavior. They live in `tests/` like integration tests but their contract is "two layers must stay in sync," not "this endpoint returns the right value." First instance: [`tests/schema_drift.rs`](./tests/schema_drift.rs) — verifies every entity in `backend/src/entities/` can `SELECT` its declared columns from the freshly-migrated schema. Add a verification test whenever a class of cross-layer drift is hard to catch by review alone.
- **What doesn't need tests:** Hand-written entities (declarations of column shape — no testable logic to unit-test; the schema-drift verification test covers mismatches between migration and entity), `mod.rs` re-exports, one-time startup code (seeding, migration runner), and simple config loading. Use judgment — if it has logic, it needs tests.
- **Test naming:** Descriptive names that read as sentences: `test_login_with_wrong_password_returns_401`, not `test_login_2`.

## Errors

All route handlers return `Result<impl IntoResponse, error::Error>` where `error::Error` (`src/error.rs`) is a unified error type that implements Axum's `IntoResponse`. This enables `?` propagation. See [`docs/design.md`](../docs/design.md) § Observability → Error response pattern for the full table of variants and the `From<sea_orm::DbErr>` mapping.

Open follow-up: [#84](https://github.com/brendanbyrne/beerio-kart/issues/84) tracks sanitizing driver-string leakage from `From<DbErr>` (Conflict/BadRequest paths leak schema details). Worth reading before touching `error.rs`.

## WSL2 build performance

WSL2 accessing `/mnt/c/` is slower than the native Linux filesystem, especially for `cargo build`. If build times become painful, configure Cargo to put build artifacts on the Linux filesystem while keeping source on Windows:

```toml
# backend/.cargo/config.toml
[build]
target-dir = "/home/bbyrne/.cargo-target/beerio-kart"
```

## Document history

- 2026-05-08 — Initial creation as part of PR 6 / Issue [#79](https://github.com/brendanbyrne/beerio-kart/issues/79). Sourced from root `.claude/CLAUDE.md` § Schema changes (prelaunch), § Testing, § Conventions (Rust style + naming), and § Repo Location (WSL2 build tip). Errors section pointer added with reference to [#84](https://github.com/brendanbyrne/beerio-kart/issues/84). Pointers to coding-standards/, data-model.md, api-contract.md, compliance-plan.md added.
