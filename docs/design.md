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

## Architecture

Beerio Kart is a split web app: a **React + TypeScript** single-page frontend and a **Rust + Axum** backend, talking over a versioned JSON API (`/api/v1`). In production the backend is the only process — a single Axum container serves the API *and* the built frontend static files (via `tower-http::ServeDir`), so there's no separate nginx or frontend container. State lives in **SQLite**, with a deliberate path to PostgreSQL later (the ORM choice keeps that migration cheap). Containerized with a `Dockerfile` + `compose.yaml` that run under Docker or Podman.

Per-side stack and the rationale for each choice live with the code:

- **Backend** (Axum, Tokio, SeaORM/SQLx, SQLite, argon2+JWT, tracing) → [`backend/README.md`](../backend/README.md) § Tech
- **Frontend** (React/TypeScript, Vite, Tailwind, Bun) → [`frontend/README.md`](../frontend/README.md) § Tech

The two sides share two load-bearing design surfaces — the contracts between them:

- **API contract** — the wire seam (endpoints, error codes, polling, idempotency, time format) → [`api-contract.md`](./api-contract.md)
- **Data model** — the database schema both sides reason about (incl. naming conventions) → [`data-model.md`](./data-model.md)

## Errors & Observability

Errors use a single `error::Error` type that implements Axum's `IntoResponse`: every handler returns `Result<impl IntoResponse, error::Error>`, every error response is `{ "error": <human-readable>, "code": <stable machine string> }`, and internal (5xx-class) detail is logged but never returned to the client. Structured logging is via `tracing` + `tower-http::TraceLayer`; verbosity is controlled by `RUST_LOG` (defaults to `info`).

That's the cross-cutting shape; the detail lives in its canonical homes:

- **Wire contract** — response envelope, the stable `code` registry, frontend-facing semantics → [`api-contract.md`](./api-contract.md) §§ 2, 7.
- **Rust error handling** — the `error::Error` enum, `thiserror`, `#[non_exhaustive]`, context attachment, the hybrid helper API, `IntoResponse` boundary logging, the `Path`/`Json` extractors → [`coding-standards/rust.md`](./coding-standards/rust.md) § 1.
- **`From<sea_orm::DbErr>` mapping** — variant-aware DbErr translation, the `client`/`detail` split → [`coding-standards/seaorm.md`](./coding-standards/seaorm.md) § 7.
- **Timeout budgets** — the `Timeout` variant, `timeout::db_query` / `timeout::db_txn` → [`coding-standards/tokio.md`](./coding-standards/tokio.md) § 12.
- **Logging conventions & config** — log-level meanings, `RUST_LOG`, the observability stack → [`coding-standards/rust.md`](./coding-standards/rust.md) § 10.

Wire-format design rationale: [ADR 0036](./decisions/0036-error-code-rollout.md).

## Coverage & CI

- **Local:** backend — `just coverage` generates an HTML report, `just coverage-summary` prints a text summary; frontend — `bun run test:coverage` (Vitest + istanbul; HTML report under `frontend/coverage/`).
- **CI:** GitHub Actions. The `Backend` workflow runs `cargo-llvm-cov` on backend changes; the `Frontend` workflow runs `bun run test:coverage` on frontend changes. Both upload to Codecov — backend under the `backend` flag, frontend under `frontend`. Each workflow is path-filtered to its own subtree, so a PR touching only one side skips the other job (Codecov carries the untouched flag forward).
- **Exclusions:** backend — `entities/` (SeaORM codegen), `migration/`, `main.rs` (wiring), `seed.rs` (startup); frontend — test files, `src/mocks/`, `src/setupTests.ts`, `src/main.tsx` (bootstrap). Only business logic counts.
- **Policy:** No regression from the base branch (`target: auto`, 0.5% threshold). New/changed code must be 80% covered (`patch: 80%`). Backend's project and patch gates block; the frontend *project* gate is informational until PR-H1's test-coverage backfill lands (then blocks), while the frontend *patch* gate blocks from PR-H2 on. As coverage rises from the audit, we'll lock in a hard floor.
- **Reports:** Codecov posts a PR comment with coverage delta, patch coverage, and per-file breakdown.

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
- 2026-05-31 — Slimmed per #220 (PR #223). (1) `## Observability` / Error response pattern block → a short `## Errors & Observability` note + pointers; the content was a third copy already canonical elsewhere (wire → api-contract.md §§ 2/7; Rust impl → rust.md § 1; `From<DbErr>` → seaorm.md § 7; timeouts → tokio.md § 12; logging/config → rust.md § 10). Variants table dropped — `error.rs` is the source of truth, api-contract.md § 7 the registry. Three facts that were *only* here — the `Token`/`Hash` typed `#[from]` variants, the project-local `extract::Path`/`Json` envelope convention, and the `RUST_LOG` / `tracing-subscriber` / `TraceLayer` observability stack — were first preserved into rust.md §§ 1 and 10. (2) `## Tech Stack` → `## Architecture`: topology stays; per-side stack + rationale delegated to backend/README.md and frontend/README.md § Tech; the shared contracts (api-contract.md, data-model.md) called out as the cross-side seam. (3) `### ORM Usage` → seaorm.md §§ 2/10 and `## Naming Conventions` → data-model.md (backend/CLAUDE.md copies reduced to pointers). (4) The seven pointer-only sections (Data Model, User Workflows, API Surface, UI Screens, Build Plan, Resolved Decisions, Related documents) removed; navigation lives in docs/README.md. ADR 0036's `design.md#error-response-pattern` anchor repointed to api-contract.md § 2; the `§ Tech Stack` / `§ Observability` citers in backend/CLAUDE.md and api-contract.md repointed.
