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

All route handlers return `Result<impl IntoResponse, error::Error>` where `error::Error` (`src/error.rs`) is a unified error type that implements Axum's `IntoResponse` trait. This enables idiomatic `?` error propagation instead of verbose match arms.

**`error::Error` variants:**

| Variant | HTTP Status | User-facing message | `code` field (when variant-pinned) |
|---------|-------------|---------------------|-----------------------------------|
| `BadRequest { client, code, detail }` | 400 | The `client` field | carried in `code` — see registry |
| `Unauthorized { msg, code }` | 401 | The provided `msg` | carried in `code` — see registry |
| `Forbidden { msg, code }` | 403 | The provided `msg` | carried in `code` — see registry |
| `NotFound(msg)` | 404 | The provided `msg` | `not_found` |
| `Conflict { client, code, detail }` | 409 | The `client` field | carried in `code` — see registry |
| `Internal(anyhow_err)` | 500 | Generic `"Internal server error"` | `internal` |
| `Token(jwt_err)` | 500 | Generic `"Internal server error"` | `internal` |
| `Hash(hash_err)` | 500 | Generic `"Internal server error"` | `internal` |
| `Timeout { budget }` | 504 | Generic `"Request timed out"` | `gateway_timeout` |

The variants with multiple possible codes (`BadRequest`, `Unauthorized`, `Forbidden`, `Conflict`) carry the [`ErrorCode`](../backend/src/error.rs) explicitly in a `code` field. Variants pinned to a single code derive theirs from `Error::code()`.

**Key behaviors:**
- Every error response emits both an `error` (human-readable) and a `code` (stable machine-readable) field in the JSON body. The `code` strings mirror the [api-contract.md § 7 registry](./api-contract.md#7-error-code-registry) one-to-one; the [`ErrorCode`](../backend/src/error.rs) enum is the source of truth. Wire-format design rationale lives in [ADR 0036](./decisions/0036-error-code-rollout.md).
- The 500-class variants (`Internal`, `Token`, `Hash`) log the real error chain via `tracing::error!` (walking `error.source()`) but return a generic message to the client — internal details are never exposed.
- `Internal` wraps an `anyhow::Error` (via `#[from]`). Construct source-bearing internals as `anyhow::Error::new(e).context("Loading user")` and synthetic ones (invariant violations, missing seed data) as `anyhow::anyhow!("Cup not found for cup_id {id}")`. The `.context(...)` static-string layer answers *what we were doing*; the wrapped source's `Display` answers *what failed concretely*. The boundary log walks the full chain. Per `coding-standards/rust.md` § 1, error-message strings start with a capital letter and have no trailing punctuation.
- `Token` and `Hash` are typed wrappers (with `#[from]`) around `jsonwebtoken::errors::Error` and `argon2::password_hash::Error` respectively, so `?` works directly on token operations and password hashing. The wrapped error is reachable via `error.source()` so the boundary log captures the underlying detail.
- `From<sea_orm::DbErr>` is variant-aware: `RecordNotFound` → `NotFound` (404), `SqlErr::UniqueConstraintViolation` → `Conflict` (409, generic `ErrorCode::Conflict`), `SqlErr::ForeignKeyConstraintViolation` → `BadRequest` (400, generic `ErrorCode::BadRequest`), everything else → `Internal` (500) wrapped with the static context `"Database error"`. Specific codes like `username_taken` come from service-layer pre-checks, not the DbErr safety net.
- `Timeout { budget }` is the operational signal for a stuck call. Raised only by `timeout::db_query` (2 s) / `timeout::db_txn` (5 s) when the wrapped `tokio::time::timeout` budget elapses; maps to **504 Gateway Timeout** with the generic `"Request timed out"` body and `code: "gateway_timeout"`. The elapsed `budget` is logged via `tracing::warn!(budget_ms = …)` for operators but is never returned to the client. Distinct from `Internal` so a stuck query (operational pain) doesn't get bucketed with invariant violations or driver crashes (bug-class). See [`coding-standards/tokio.md`](./coding-standards/tokio.md) § 12 for the helpers and the budget defaults.
- `error::Error` is `#[non_exhaustive]`: the compiler enforces a wildcard arm in any external matcher so adding a future variant doesn't break callers. `ErrorCode` is also `#[non_exhaustive]` for the same reason.
- **Hybrid helper API.** Per-code helpers for the named domain codes (`Error::lap_times_mismatch(msg)`, `Error::username_taken(msg)`, `Error::session_closed(msg)`, etc. — see [`error.rs`](../backend/src/error.rs)) are the readable path for sites with a stable domain meaning. Generic helpers (`Error::bad_request(msg)`, `Error::conflict(msg)`, `Error::forbidden(msg)`) default to the generic codes (`bad_request`, `conflict`, `forbidden`) for bespoke long-tail errors. 401 has no generic — every `Unauthorized` picks one of `invalid_credentials()`, `token_expired()`, `token_invalid(msg)`.
- **Path/Json extractors.** Project-local [`extract::Path<T>` and `extract::Json<T>`](../backend/src/extract.rs) replace `axum::extract::Path` and `axum::Json` at every route. Rejection failures produce the standard envelope: invalid path segments → `invalid_path_param` (400), JSON parse / data validation failures → `invalid_request_body` (400). Axum's default plain-text rejection bodies never surface.
- `BadRequest` and `Conflict` are struct variants `{ client, code, detail }`. `client` is what the response body carries; `detail` (when `Some`) is logged via `tracing::warn!` at the `IntoResponse` boundary and never reaches the wire. The split exists so the `From<sea_orm::DbErr>` path can preserve the raw driver string (e.g., `"UNIQUE constraint failed: users.username"`) for operators without leaking schema details to clients. See Issue [#84](https://github.com/brendanbyrne/beerio-kart/issues/84) for the leak inventory and alternatives considered.

**Response format:** All errors return JSON: `{ "error": "<message>", "code": "<code>" }`

### Configuration

Log output is controlled via the `RUST_LOG` environment variable. Defaults to `info` if not set. Examples:
- `RUST_LOG=debug` — all debug-level output
- `RUST_LOG=beerio_kart=debug` — debug only for application code, info for dependencies

## Coverage & CI

- **Local:** backend — `just coverage` generates an HTML report, `just coverage-summary` prints a text summary; frontend — `bun run test:coverage` (Vitest + istanbul; HTML report under `frontend/coverage/`).
- **CI:** GitHub Actions. The `Backend` workflow runs `cargo-llvm-cov` on backend changes; the `Frontend` workflow runs `bun run test:coverage` on frontend changes. Both upload to Codecov — backend under the `backend` flag, frontend under `frontend`. Each workflow is path-filtered to its own subtree, so a PR touching only one side skips the other job (Codecov carries the untouched flag forward).
- **Exclusions:** backend — `entities/` (SeaORM codegen), `migration/`, `main.rs` (wiring), `seed.rs` (startup); frontend — test files, `src/mocks/`, `src/setupTests.ts`, `src/main.tsx` (bootstrap). Only business logic counts.
- **Policy:** No regression from the base branch (`target: auto`, 0.5% threshold). New/changed code must be 80% covered (`patch: 80%`). Backend's project and patch gates block; the frontend *project* gate is informational until PR-H1's test-coverage backfill lands (then blocks), while the frontend *patch* gate blocks from PR-H2 on. As coverage rises from the audit, we'll lock in a hard floor.
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

See [`api-contract.md`](./api-contract.md) § 1 (endpoint catalog) and §§ 2–9 (wire-format conventions).

## UI Screens (Mobile-First)

See [`user-workflows.md`](./user-workflows.md) § 2.

## Build Plan

See [`roadmap.md`](./roadmap.md) for the cup-by-cup narrative — goals, scope, deferred work, and success criteria per work-chunk. Status of individual work items lives on the [project board](https://github.com/users/brendanbyrne/projects/3); active-cup work is tracked there as Issues, future-cup work as scope bullets in roadmap.md until each cup goes active.

## Resolved Decisions

See [`decisions/`](./decisions/) — each prior bullet has been distilled into a MADR file under `docs/decisions/`. The index in [`decisions/README.md`](./decisions/README.md) lists every ADR with its title, status, and date.

## Related documents

- **`api-contract.md`** — Wire-format conventions between backend and frontend (error codes, ETag polling, idempotency keys, time format).
- **`coding-standards/`** — Backend coding standards (general Rust, SeaORM, Tokio).
- **`designs/archive/compliance-plan.md`** — Archived. Sequenced PRs that brought the existing code into compliance with the coding standards; all signed off 2026-05-15.
- **`research/`** — Long-form exploration of approaches not yet decided (e.g., OCR strategy, SeaORM 2.0 evaluation). Reference-only; not authoritative until promoted into this file or `coding-standards/`.
- **`data-model.md`** — Database schema, table definitions, and design decisions about them. (Extracted from this file in PR 1 of the docs restructure.)
- **`decisions/`** — Architecture Decision Records (MADR format). Searchable index in `decisions/README.md`.
- **`designs/`** — Design records (per-session sign-off-style narratives of how decisions were reached). PR review feedback now lives on GitHub (PR comments, line-anchored).

## Document history

- 2026-05-02 — Moved from repo root (`DESIGN.md`) to `docs/design.md`. Project structure section updated to reflect the move and the new `docs/` layout. The root `DESIGN.md` is kept as a redirect (Cowork sandbox cannot delete files); a Claude Code PR will remove it from the working tree.
- 2026-05-04 — Updated the AppError "Key behaviors" bullet to reflect the variant-aware `From<sea_orm::DbErr>` impl (NotFound / Conflict / BadRequest / Internal mapping). PR #25.
- 2026-05-04 — Added `docs/research/` to the project-structure tree and a corresponding entry in Related documents (long-form exploration not yet promoted to design or coding-standards).
- 2026-05-04 — Replaced the "Entity regeneration via justfile recipe" rule with "Hand-written SeaORM entities"; updated the `just (not Make)` example to use `just entities-bootstrap`. Closes the codegen-strategy decision recorded at [`docs/designs/archive/2026-05-02-entity-codegen-strategy.md`](./designs/archive/2026-05-02-entity-codegen-strategy.md). PR-X1.
- 2026-05-05 — Extracted Data Model section to `data-model.md`. PR 1 of the docs restructure.
- 2026-05-05 — Replaced the Resolved Decisions bullet list with a pointer to `docs/decisions/`. Each prior bullet distilled into a MADR file (0002–0034). PR 2 of the docs restructure.
- 2026-05-06 — Replaced the Build Plan section with a one-paragraph pointer to `roadmap.md` (created in this PR). Phase narratives moved to roadmap.md per cup; the 20 unchecked Phase 3 / Milestone Star bullets were filed as GitHub Issues (#46, #47, #49, #50, #51, #54, #56, #58, #59, #61, #62, #63, #64, #65, #66, #67, #70, #71, #72, #73) under Milestone Star. PR 3 of the docs restructure.
- 2026-05-06 — Removed the `## Backlog` section. The three random ideas (player invite emails, username change, send-emails / account recovery) moved to `docs/roadmap.md` § Random ideas. The fourth (concurrent `next_track` race condition) was filed as Issue #75 under Milestone Star with `enhancement` label.
- 2026-05-06 — Replaced § "API Surface" with a pointer to `api-contract.md` § 1. Replaced § "User Workflows" and § "UI Screens" with pointers to `user-workflows.md`. Two minor editorial changes carried in the moves (Workflow 1.4 "Phase 3" → "Milestone Star"; § 2 preamble adds Pixel 9 Pro reference). PR 4 of the docs restructure.
- 2026-05-08 — Removed the Project Structure section entirely (heading + body, ~99 lines of repo tree). The tree now lives only in the rebuilt repo-root `README.md`. Diverges from PR 4's stub-pointer pattern (User Workflows / API Surface / UI Screens kept their headings) — design.md is for architecture, repo tree is bootstrap content. PR 5 of the docs restructure.
- 2026-05-09 — Updated the AppError variants table and Key behaviors bullets to reflect the thiserror migration: added `Token` and `Hash` rows; clarified that the 500-class log path now walks `error.source()` for the full chain; noted `#[non_exhaustive]`. PR #105.
- 2026-05-09 — Reshaped `Internal` from `String` to `anyhow::Error` (PR-C2). Updated the variants table row and the Key behaviors bullet describing `Internal` construction patterns (`.context(...)` for source-bearing, `anyhow::anyhow!(...)` for synthetic). Noted that the `From<DbErr>` fallback uses a generic `"Database error"` context and that callers wanting richer context use `.map_err(...)` rather than `?`. PR #107.
- 2026-05-10 — Renamed `AppError` → `error::Error` (and `AppConfig` → `config::Config`, `AuthUser` → `auth::User`, `AuthResponse` → `auth::Response`, `RaceSetupUpdate` → `race_setup::Update`, plus the `list_<resource>` route/service functions → `list`) per the module-name-repetition cleanup in PR-H1+ (d). Live-prose references in this section updated; historical PR notes preserve the old names. PR #103 sequence.
- 2026-05-11 — Restructured `BadRequest` and `Conflict` from `String`-payload tuple variants to struct variants `{ client, detail }`. Added a Key behaviors bullet describing the client/detail split: `client` is the wire-facing sanitized message; `detail` (when `Some`) is logged via `tracing::warn!` at the `IntoResponse` boundary. The `From<sea_orm::DbErr>` mapping now stashes the raw driver string (e.g., `"UNIQUE constraint failed: users.username"`) in `detail` and returns a generic `client` text so schema details don't leak into the response body. Closes Issue [#84](https://github.com/brendanbyrne/beerio-kart/issues/84).
- 2026-05-15 — Added `Timeout { budget }` row to the `error::Error` variants table (504 Gateway Timeout, generic `"Request timed out"` body) and a Key behaviors bullet covering its semantics: raised only by `timeout::db_query` / `timeout::db_txn` when the per-call `tokio::time::timeout` budget elapses, `budget_ms` logged via `tracing::warn!` for operators, distinct from `Internal` so timeout rates can be charted independently of generic 500-class failures. Dropped `Timeout` from the `#[non_exhaustive]` parenthetical's hypothetical-variant list (no longer hypothetical) and added a one-line aside that the variant was added without breaking existing matchers. PR-F4 / PR [#155](https://github.com/brendanbyrne/beerio-kart/pull/155).
- 2026-05-15 — `code` field rollout (#157). Error response pattern section refreshed end-to-end. Variants table grew a `code` field column documenting how each variant carries its registry code (variant-pinned vs. carried in a `code` field). `BadRequest`, `Unauthorized`, `Forbidden`, `Conflict` reshaped to struct variants with a `code: ErrorCode` field; `Unauthorized` / `Forbidden` left tuple form behind in the process. Response format note updated from `{ error }` to `{ error, code }`. New Key behaviors bullets cover the hybrid helper API (per-code helpers for named domain codes, generic helpers for long-tail) and the project-local Path/Json extractors that emit `invalid_path_param` / `invalid_request_body`. Subsumes #146 (typed-Path-extractor 400s). PR [#158](https://github.com/brendanbyrne/beerio-kart/pull/158); ADR [0036](./decisions/0036-error-code-rollout.md).
- 2026-05-15 — Updated the Related documents pointer for `compliance-plan.md` (now archived at `designs/archive/`) and the 2026-05-04 history-entry link for the entity-codegen-strategy record (same archive move). Companion to PR [#160](https://github.com/brendanbyrne/beerio-kart/pull/160) / Issue [#159](https://github.com/brendanbyrne/beerio-kart/issues/159).
- 2026-05-18 — § Coverage & CI extended to cover the frontend. PR-H2 ([#193](https://github.com/brendanbyrne/beerio-kart/issues/193)) instruments the frontend with Vitest (istanbul coverage provider — the v8 provider reports 0% under Bun), adds a path-filtered `Frontend` GitHub Actions workflow uploading to Codecov under a `frontend` flag, and path-filters the existing backend coverage workflow (renamed `coverage.yml` → `backend.yml`, workflow name `Coverage` → `Backend`) to the backend subtree. Codecov restructured into per-flag project/patch statuses with carryforward; the frontend project gate is informational until PR-H1's backfill. Local/CI/Exclusions/Policy bullets rewritten to name both sides.
