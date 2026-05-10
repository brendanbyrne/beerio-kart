# Beerio Kart - Architecture Design Document

## Overview

Beerio Kart is a mobile-first web app for tracking times and stats for the Mario Kart 8 Deluxe drinking game. Players race one at a time in Time Trial mode (150cc only). You can't touch the controller while touching your drink (one 12oz beer or sparkling water). The app tracks personal times per track, head-to-head records, and leaderboards.

## Rules of the Game

1. Players race one at a time using Time Trial mode in Mario Kart 8 Deluxe.
2. You cannot touch the controller while touching your drink.
3. The drink is one 12oz beer or one 12oz sparkling water, poured into a cup.
4. You may restart the race if it is before the end of the first lap AND you haven't had any of your drink yet.
5. If you finish the race before you finish your drink, your run is disqualified (DQ). This is self-reported on the honor system.
6. Played round robin — all players race the same track.
7. Fastest non-DQ'd time wins.

## High Level Principles

Consideration of these principles should go into every design decision made. If a decision can't tie itself back to at least one of these principles, then it should be questioned.

- **First class experience even if you don't drink.** Tailor each player's experience to their preference to drink or not.
- **Never a burden to use.** What's the point if it's not making things easier.
- **You should never feel rushed, unless absolutely necessary.** Enjoying each others' company is the point.
- **You can play in the same room as much as across the world.** Nothing should require that you're using different TVs or the same TV.
- **Should be usable by only one hand.** The other one could be wet.

## Design Goals

- **Minimize number of choices in any moment.** It's hard to use wrong, if you can only do what you need to.
- **Prefer simple interactions over complex ones.** Doing nothing > "single use button" > swipe screen > togglable button > swipe specific objects > multiple buttons > typing.
- **Provide sensible defaults whenever possible.** You can usually assume what most likely decision a person will make.

## Technical Constraints

- **Don't overengineer before OCR.** Many corner cases (time validation, race setup entry, session tracking) will be solved by OCR. Design the MVP for manual entry with hooks for OCR to slot in later.
- **SQLite STRICT mode on lookup / static tables.** Enforces type checking at the DB level where it adds real safety (static game data tables: `characters`, `bodies`, `wheels`, `gliders`, `cups`, `tracks`). Dropped on tables with timestamp columns (`users`, `sessions`, `session_participants`, `session_races`, `runs`, `drink_types`, `run_flags`) because SQLite STRICT does not accept the `DATETIME` type, and proper Rust-side `DateTime<Utc>` typing outweighs the duplicative safety (Rust's type system already enforces column types for this Rust-only codebase). Requires SQLite 3.37+ (2021).
- **Must work on Firefox.** Firefox is a target browser alongside Chrome/Safari mobile. Avoid Chrome-only APIs or `-webkit-` prefixes without Firefox equivalents. Test on Firefox before shipping UI changes.

## Tech Stack

| Layer       | Technology                    | Rationale                                                    |
|-------------|-------------------------------|--------------------------------------------------------------|
| Backend     | Rust + Axum                   | Learning opportunity; strong async/WebSocket support          |
| Frontend    | React + Vite                  | Largest ecosystem for mobile-web; camera API support          |
| Styling     | Tailwind CSS                  | Utility-first; fast iteration; mobile-first by convention     |
| ORM         | SeaORM (backed by sqlx)       | Rust-native query API; database-agnostic; easier SQLite->PostgreSQL migration |
| Database    | SQLite                        | File-based; no separate server; sufficient for this scale     |
| Package mgr | Bun                           | Drop-in npm replacement; faster installs and script running   |
| Containers  | Dockerfile + compose.yaml    | Works with Docker or Podman                                  |
| Serving     | Axum (single container)       | Axum serves both the API and the frontend static files via `tower-http::ServeDir`. No separate nginx or frontend container. |

### ORM Usage

Use SeaORM's builder API for single-table reads and writes (`Entity::find()`, `Entity::find_by_id`, `ActiveModel::insert` / `update`). Drop to raw SQL via `find_by_statement` only for multi-table JOINs where the builder's JOIN ergonomics become clumsy (most of `get_session_detail`'s helper queries, `list_runs`'s dynamic filters). Avoid hand-rolling SQL for single-table ops — the builder gives you type safety and refactor-proofness for free.

## Observability

### Crates

- **`tracing`** — structured, leveled logging facade (used throughout application code)
- **`tracing-subscriber`** — formats and emits log output (with `fmt` and `env-filter` features)
- **`tower-http`** — provides `TraceLayer` middleware for automatic HTTP request/response logging (method, path, status, duration)

### Log level conventions

- `error` — unexpected failures (DB errors, hashing failures, token creation failures)
- `warn` — suspicious but recoverable (e.g., rate-limit warnings in the future)
- `info` — request lifecycle, startup, seeding complete
- `debug` — detailed diagnostics during development

### Error response pattern

All route handlers return `Result<impl IntoResponse, AppError>` where `AppError` (`src/error.rs`) is a unified error type that implements Axum's `IntoResponse` trait. This enables idiomatic `?` error propagation instead of verbose match arms.

**`AppError` variants:**

| Variant | HTTP Status | User-facing message |
|---------|-------------|---------------------|
| `BadRequest(msg)` | 400 | The provided `msg` |
| `Unauthorized(msg)` | 401 | The provided `msg` |
| `Forbidden(msg)` | 403 | The provided `msg` |
| `NotFound(msg)` | 404 | The provided `msg` |
| `Conflict(msg)` | 409 | The provided `msg` |
| `Internal(log_msg)` | 500 | Generic `"Internal server error"` |
| `Token(jwt_err)` | 500 | Generic `"Internal server error"` |
| `Hash(hash_err)` | 500 | Generic `"Internal server error"` |

**Key behaviors:**
- The 500-class variants (`Internal`, `Token`, `Hash`) log the real error chain via `tracing::error!` (walking `error.source()`) but return a generic message to the client — internal details are never exposed.
- `Token` and `Hash` are typed wrappers (with `#[from]`) around `jsonwebtoken::errors::Error` and `argon2::password_hash::Error` respectively, so `?` works directly on token operations and password hashing. The wrapped error is reachable via `error.source()` so the boundary log captures the underlying detail.
- `From<sea_orm::DbErr>` is variant-aware: `RecordNotFound` → `NotFound` (404), `SqlErr::UniqueConstraintViolation` → `Conflict` (409), `SqlErr::ForeignKeyConstraintViolation` → `BadRequest` (400), everything else → `Internal` (500). This preserves error semantics that a blanket-Internal mapping would otherwise hide.
- `AppError` is `#[non_exhaustive]`: the compiler enforces a wildcard arm in any external matcher so adding a future variant (e.g., `Timeout`, `RateLimited`) doesn't break callers.
- Client-facing errors (`BadRequest`, `Unauthorized`, etc.) are always constructed explicitly — they require human judgment about the appropriate status code and message.

**Response format:** All errors return JSON: `{ "error": "<message>" }`

### Configuration

Log output is controlled via the `RUST_LOG` environment variable. Defaults to `info` if not set. Examples:
- `RUST_LOG=debug` — all debug-level output
- `RUST_LOG=beerio_kart=debug` — debug only for application code, info for dependencies

## Coverage & CI

- **Local:** `just coverage` generates an HTML report; `just coverage-summary` prints a text summary.
- **CI:** GitHub Actions runs `cargo-llvm-cov` on every PR and push to main. Results upload to Codecov.
- **Exclusions:** `entities/` (SeaORM codegen), `migration/`, `main.rs` (wiring), `seed.rs` (startup), `frontend/` (not yet instrumented). Only business logic counts.
- **Policy:** No regression from the base branch (`target: auto`, 0.5% threshold). New/changed code must be 80% covered (`patch: 80%`). As coverage rises from the audit, we'll lock in a hard floor.
- **Reports:** Codecov posts a PR comment with coverage delta, patch coverage, and per-file breakdown.

## Naming Conventions

- Table names: plural, snake_case (`drink_types`, `characters`)
- Column names: snake_case (`track_time`, `created_at`)
- Foreign keys: `{referenced_table_singular}_id` (`character_id`, `cup_id`)
- Primary keys: `id`

## Data Model

See [`data-model.md`](./data-model.md) for the full database schema, table definitions, and design decisions about them.

## User Workflows

See [`user-workflows.md`](./user-workflows.md) § 1.

## API Surface

See [`api-contract.md`](./api-contract.md) § 1 (endpoint catalog) and §§ 2–10 (wire-format conventions).

## UI Screens (Mobile-First)

See [`user-workflows.md`](./user-workflows.md) § 2.

## Build Plan

See [`roadmap.md`](./roadmap.md) for the cup-by-cup narrative — goals, scope, deferred work, and success criteria per work-chunk. Status of individual work items lives on the [project board](https://github.com/users/brendanbyrne/projects/3); active-cup work is tracked there as Issues, future-cup work as scope bullets in roadmap.md until each cup goes active.

## Resolved Decisions

See [`decisions/`](./decisions/) — each prior bullet has been distilled into a MADR file under `docs/decisions/`. The index in [`decisions/README.md`](./decisions/README.md) lists every ADR with its title, status, and date.

## Related documents

- **`api-contract.md`** — Wire-format conventions between backend and frontend (error codes, ETag polling, idempotency keys, time format).
- **`coding-standards/`** — Backend coding standards (general Rust, SeaORM, Tokio).
- **`compliance-plan.md`** — Sequenced PRs to bring the existing code into compliance with the coding standards.
- **`research/`** — Long-form exploration of approaches not yet decided (e.g., OCR strategy, SeaORM 2.0 evaluation). Reference-only; not authoritative until promoted into this file or `coding-standards/`.
- **`data-model.md`** — Database schema, table definitions, and design decisions about them. (Extracted from this file in PR 1 of the docs restructure.)
- **`decisions/`** — Architecture Decision Records (MADR format). Searchable index in `decisions/README.md`.
- **`designs/`** — Design records (per-session sign-off-style narratives of how decisions were reached). PR review feedback now lives on GitHub (PR comments, line-anchored).

## Document history

- 2026-05-02 — Moved from repo root (`DESIGN.md`) to `docs/design.md`. Project structure section updated to reflect the move and the new `docs/` layout. The root `DESIGN.md` is kept as a redirect (Cowork sandbox cannot delete files); a Claude Code PR will remove it from the working tree.
- 2026-05-04 — Updated the AppError "Key behaviors" bullet to reflect the variant-aware `From<sea_orm::DbErr>` impl (NotFound / Conflict / BadRequest / Internal mapping). PR #25.
- 2026-05-04 — Added `docs/research/` to the project-structure tree and a corresponding entry in Related documents (long-form exploration not yet promoted to design or coding-standards).
- 2026-05-04 — Replaced the "Entity regeneration via justfile recipe" rule with "Hand-written SeaORM entities"; updated the `just (not Make)` example to use `just entities-bootstrap`. Closes the codegen-strategy decision recorded at [`docs/designs/2026-05-02-entity-codegen-strategy.md`](./designs/2026-05-02-entity-codegen-strategy.md). PR-X1.
- 2026-05-05 — Extracted Data Model section to `data-model.md`. PR 1 of the docs restructure.
- 2026-05-05 — Replaced the Resolved Decisions bullet list with a pointer to `docs/decisions/`. Each prior bullet distilled into a MADR file (0002–0034). PR 2 of the docs restructure.
- 2026-05-06 — Replaced the Build Plan section with a one-paragraph pointer to `roadmap.md` (created in this PR). Phase narratives moved to roadmap.md per cup; the 20 unchecked Phase 3 / Milestone Star bullets were filed as GitHub Issues (#46, #47, #49, #50, #51, #54, #56, #58, #59, #61, #62, #63, #64, #65, #66, #67, #70, #71, #72, #73) under Milestone Star. PR 3 of the docs restructure.
- 2026-05-09 — Updated the AppError variants table and Key behaviors bullets to reflect the thiserror migration: added `Token` and `Hash` rows; clarified that the 500-class log path now walks `error.source()` for the full chain; noted `#[non_exhaustive]`. PR #105.
- 2026-05-06 — Removed the `## Backlog` section. The three random ideas (player invite emails, username change, send-emails / account recovery) moved to `docs/roadmap.md` § Random ideas. The fourth (concurrent `next_track` race condition) was filed as Issue #75 under Milestone Star with `enhancement` label.
- 2026-05-06 — Replaced § "API Surface" with a pointer to `api-contract.md` § 1. Replaced § "User Workflows" and § "UI Screens" with pointers to `user-workflows.md`. Two minor editorial changes carried in the moves (Workflow 1.4 "Phase 3" → "Milestone Star"; § 2 preamble adds Pixel 9 Pro reference). PR 4 of the docs restructure.
- 2026-05-08 — Removed the Project Structure section entirely (heading + body, ~99 lines of repo tree). The tree now lives only in the rebuilt repo-root `README.md`. Diverges from PR 4's stub-pointer pattern (User Workflows / API Surface / UI Screens kept their headings) — design.md is for architecture, repo tree is bootstrap content. PR 5 of the docs restructure.
