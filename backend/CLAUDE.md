# Beerio Kart — Backend

Loaded automatically when Claude works in `backend/`. Captures conventions specific to the Rust + Axum + SeaORM + SQLite layer. For project-wide conventions (handoffs, GitHub access, two-assistant coordination, etc.), the root [`.claude/CLAUDE.md`](../.claude/CLAUDE.md) still applies — this file adds backend-specific layers, doesn't replace them.

## Stack

Canonical stack + rationale: [`docs/design.md`](../docs/design.md) § Tech Stack. Backend-reader orientation: [`README.md`](./README.md) § Tech. Backend decisions worth keeping in view while working here:

- Hand-written SeaORM entities — ADR 0023.
- SQLite STRICT mode on lookup tables only — ADR 0002.
- `argon2` + JWT auth — ADR 0031, [`docs/api-contract.md`](../docs/api-contract.md) §§ 1.1, 4.
- `tracing` + `tower-http::TraceLayer` observability — [`docs/design.md`](../docs/design.md) § Observability.

## Key reading

The annotated required-reading list for backend work lives in [`README.md`](./README.md) § Required reading (design overview, coding standards, schema, API contract). Read it once and refer back when relevant.

## ORM usage

Use SeaORM's builder API for single-table reads and writes (`Entity::find()`, `Entity::find_by_id`, `ActiveModel::insert` / `update`). Drop to raw SQL via `find_by_statement` only for multi-table JOINs where the builder's JOIN ergonomics become clumsy. Avoid hand-rolling SQL for single-table ops — the builder gives you type safety and refactor-proofness for free.

## Naming

- Table names: plural, snake_case (`drink_types`, `characters`).
- Column names: snake_case (`track_time`, `created_at`).
- Foreign keys: `{referenced_table_singular}_id` (`character_id`, `cup_id`).
- Primary keys: `id`.

Rust style: standard `rustfmt` (nightly options) + `clippy` defaults. Lefthook runs both pre-commit; see root README § Linting & Formatting.

## Schema changes (prelaunch)

The prelaunch schema-change policy — single consolidated migration, edit-don't-append, reset the dev DB, no data preservation, schema-dependent code in the same PR, and the post-launch flip to append-only — lives in [`docs/coding-standards/seaorm.md`](../docs/coding-standards/seaorm.md) § 5 Migrations.

## Testing

The full testing policy — unit / integration / verification tests, the "what doesn't need a test" exemptions, and naming — lives in [`docs/coding-standards/rust.md`](../docs/coding-standards/rust.md) § 7. Tests are a required PR deliverable, enforced by the [PR template](../.github/pull_request_template.md) author checklist.

## Errors

All route handlers return `Result<impl IntoResponse, error::Error>` where `error::Error` (`src/error.rs`) is a unified error type that implements Axum's `IntoResponse`. This enables `?` propagation. See [`docs/design.md`](../docs/design.md) § Observability → Error response pattern for the full table of variants and the `From<sea_orm::DbErr>` mapping.

Open follow-up: [#84](https://github.com/brendanbyrne/beerio-kart/issues/84) tracks sanitizing driver-string leakage from `From<DbErr>` (Conflict/BadRequest paths leak schema details). Worth reading before touching `error.rs`.

## Document history

- 2026-05-08 — Initial creation as part of PR 6 / Issue [#79](https://github.com/brendanbyrne/beerio-kart/issues/79). Sourced from root `.claude/CLAUDE.md` § Schema changes (prelaunch), § Testing, § Conventions (Rust style + naming), and § Repo Location (WSL2 build tip). Errors section pointer added with reference to [#84](https://github.com/brendanbyrne/beerio-kart/issues/84). Pointers to coding-standards/, data-model.md, api-contract.md, compliance-plan.md added.
- 2026-05-17 — WSL2 perf snippet path updated for workspace-root move (Issue [#169](https://github.com/brendanbyrne/beerio-kart/issues/169)): `backend/.cargo/config.toml` → `.cargo/config.toml` at the repo root, with a note that Cargo discovers the config by walking up from the invocation directory.
- 2026-05-31 — § Testing reduced to a pointer at `docs/coding-standards/rust.md` § 7. The two items that were sole-owned here — verification tests and the "what doesn't need a test" exemption list — were promoted into rust.md § 7; the rest already duplicated it. Part of the "CLAUDE.md references standards, doesn't own them" cleanup.
- 2026-05-31 — §§ Stack and Key reading reduced to pointers (#220). The stack bullet list duplicated `docs/design.md` § Tech Stack and `README.md` § Tech, so § Stack now points there and keeps only the backend decision cross-refs (ADRs 0023/0002/0031, observability). § Key reading duplicated `README.md` § Required reading, so it now points there; the `data-model.md` entry it uniquely carried was added to that README list.
- 2026-05-31 — § Schema changes (prelaunch) reduced to a pointer at `docs/coding-standards/seaorm.md` § 5 Migrations, which absorbed this section's operating detail and is now the canonical home; the three ADRs that cited this section (0035, 0037, 0038) were repointed there too. Removed § WSL2 build performance — user-level machine config (a repo-root `.cargo/config.toml` Brendan maintains himself); a one-line advisory now lives in root README Setup and the root `.claude/CLAUDE.md` pointer to it was dropped. (#220)
