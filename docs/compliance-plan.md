# Backend Compliance Plan

> **Purpose.** A sequenced list of PRs that bring the existing `beerio-kart` backend into compliance with the coding standards in [`coding-standards/`](./coding-standards/). Each PR has a scope, a list of standards rules it satisfies, an effort estimate, dependencies, a risk note, and a sign-off checkbox.
> **Status.** Initial draft. The "Current state" assessments below are based on file-listing and design.md inspection — they need verification by an actual code audit (see [PR-A2](#pr-a2-codebase-audit) below).
> **Sign-off.** Brendan signs off each PR once the change lands and is verified. Unfinished items roll into the next session.

## How this doc is used

A reviewer or assistant picks the next un-signed-off PR (respecting dependencies), opens it, and works through the scope. When merged, Brendan checks the box. The plan is living — re-order or split PRs as new findings emerge during the audit.

PRs are grouped into *phases* by theme. Phases are loosely ordered by what unlocks what: tooling and audit first, then bug fixes, then `AppError` (foundation for everything that returns errors), then type-driven design, then ergonomics, then infrastructure, then cleanup. Within a phase, smaller PRs precede larger ones where possible.

**Effort scale:** S = up to a few hours; M = a half-day to a day; L = multi-day.

---

## Phase A — Tooling and audit

### PR-A1: Workspace lint config, rustfmt, editorconfig

- **Scope:**
  - Add `[workspace.lints]` block to root `Cargo.toml` (see `rust.md` § 8 for full block).
  - Add `lints.workspace = true` to each crate's `Cargo.toml`.
  - Add `rustfmt.toml` with project settings (`max_width = 100`, `edition = "2024"`, `imports_granularity = "Crate"`, `group_imports = "StdExternalCrate"`).
  - Add `.editorconfig` covering Markdown, TOML, YAML.
  - For each lint that surfaces a warning on `cargo clippy --all-targets`, add a `#[allow(clippy::...)]` at the top of the relevant module (or a workspace-level `allow` if pervasive) so the build still passes.
- **Standards refs:** `rust.md` § 8, § 16.
- **Effort:** M.
- **Dependencies:** None.
- **Risk:** Low. The `#[allow]`s are temporary scaffolding; no behavioral change.
- **Verification:** `cargo build --all-targets` succeeds; `cargo clippy --all-targets` produces zero warnings; `cargo fmt --check` passes.
- **Sign-off:** [ ]

### PR-A2: Codebase audit

- **Scope:**
  - Walk every section of every standards doc (`rust.md`, `seaorm.md`, `tokio.md`) and for each rule, check whether the existing code conforms.
  - Output: a Cowork-generated review file at `reviews/design/standards-audit-001.md` using the project's checkbox format (one finding per non-conformance).
  - Each finding identifies the rule, the file/lines that violate it, and a recommended fix.
  - Re-order or split the remaining PRs in this plan based on what the audit finds (especially Phases C–E).
- **Standards refs:** All.
- **Effort:** L.
- **Dependencies:** PR-A1 (so lints can guide the audit).
- **Risk:** Low — read-only.
- **Verification:** Audit file exists; every standards-doc section has at least one matching audit entry (or "all conforming"); plan is updated.
- **Sign-off:** [ ]

---

## Phase B — Correctness bug fixes

These are real bugs the standard would prevent in new code, surfaced during research. Land before the larger refactors so they don't get lost.

### PR-B1: Fix `From<DbErr>` to preserve error semantics

- **Scope:**
  - In `backend/src/error.rs`, replace the blanket `From<DbErr> for AppError::Internal` impl with the variant-aware version in `seaorm.md` § 7.
  - Map `DbErr::RecordNotFound` → `AppError::NotFound`.
  - Map `SqlErr::UniqueConstraintViolation` → `AppError::Conflict`.
  - Map `SqlErr::ForeignKeyConstraintViolation` → `AppError::BadRequest`.
  - Other `DbErr` variants → `AppError::Internal`.
  - Add tests for each mapping using `MockDatabase` or in-memory SQLite with intentionally-failing queries.
- **Standards refs:** `seaorm.md` § 7, `rust.md` § 1.
- **Effort:** S.
- **Dependencies:** None (could land before A1; no harm in interleaving).
- **Risk:** Low. The current behavior produces 500s where 404/409/400 belong; this fix changes a small set of response codes that no client should be depending on.
- **Verification:** New tests pass; manually trigger a `find_by_id` for a missing record and confirm the response is 404 not 500.
- **Sign-off:** [ ]

### PR-B2: Fix SQLite PRAGMA scope by switching to SqliteConnectOptions

- **Scope:**
  - In `backend/src/main.rs`, replace `Database::connect(url)` + post-hoc `PRAGMA foreign_keys = ON` with a `SqliteConnectOptions`-based pool wrapped via `SqlxSqliteConnector::from_sqlx_sqlite_pool`.
  - Set: `journal_mode=Wal`, `synchronous=Normal`, `busy_timeout=5s`, `foreign_keys=true`, `create_if_missing=true`.
  - Set `SeaORM` `ConnectOptions` explicitly: `max_connections=5`, `min_connections=1`, `acquire_timeout=5s`, `idle_timeout=60s`, `sqlx_logging_level=Debug`.
- **Standards refs:** `seaorm.md` § 8.
- **Effort:** S–M (largely mechanical; the wrap-pool incantation is the trickiest part).
- **Dependencies:** None.
- **Risk:** Low–medium. Changes pool sizing and PRAGMA application. Run integration tests + smoke test on Unraid before merging.
- **Verification:** `PRAGMA foreign_keys` is on for *every* pool connection (test by acquiring multiple connections in parallel and inserting a row that violates an FK — should fail consistently); WAL mode confirmed via `PRAGMA journal_mode` query.
- **Sign-off:** [ ]

### PR-B3: Confirm/add Argon2 `spawn_blocking`

- **Scope:**
  - Audit `backend/src/services/auth.rs` for Argon2 hash and verify calls.
  - If not already in `spawn_blocking`, wrap them as in `tokio.md` § 2.
  - Add a `tokio::sync::Semaphore` to app state with ~16 permits; gate hash/verify behind it.
- **Standards refs:** `tokio.md` § 2, § 12.
- **Effort:** S.
- **Dependencies:** None.
- **Risk:** Low. Worst case the semaphore permit count is wrong; tunable.
- **Verification:** Load test login endpoint at modest concurrency (~50 concurrent logins). Confirm the server stays responsive on unrelated endpoints.
- **Sign-off:** [ ]

---

## Phase C — `AppError` foundation

### PR-C1: Migrate `AppError` to `thiserror`

- **Scope:**
  - Add `thiserror` to `[dependencies]`.
  - Rewrite `AppError` enum using `#[derive(thiserror::Error, Debug)]`. Mark `#[non_exhaustive]`.
  - Replace hand-rolled `Display` and `From` impls with `thiserror` attributes (`#[error("...")]`, `#[from]`).
  - Update `IntoResponse` impl to walk `error.source()` and log the full chain on `Internal` (per `rust.md` § 1).
  - Run all tests; no behavior change expected.
- **Standards refs:** `rust.md` § 1.
- **Effort:** M.
- **Dependencies:** PR-B1 (so the variant-aware `From<DbErr>` is in place before the refactor).
- **Risk:** Low. Mostly mechanical; the test suite catches regressions.
- **Verification:** All existing tests pass; error responses look identical to clients.
- **Sign-off:** [ ]

---

## Phase D — Type-driven design

The largest phase. Each PR is independently reviewable; sequence keeps blast radius bounded.

### PR-D1: Introduce `nutype`, define ID newtypes

- **Scope:**
  - Add `nutype` to `[dependencies]`.
  - Define ID newtypes in `backend/src/domain/ids.rs`: `UserId`, `RunId`, `SessionId`, `SessionRaceId`, `SessionParticipantId`, `RunFlagId`, `DrinkTypeId`, `TrackId`, `CharacterId`, `BodyId`, `WheelId`, `GliderId`, `CupId`. UUID-backed types wrap `uuid::Uuid`; INTEGER-backed wrap `i32`.
  - Migrate API request/response DTOs (in `routes/`) to use the newtypes via `serde(transparent)` / nutype's transparent derives.
  - Convert at the entity↔service boundary: where service code reads `model.id` (a `String` or `i32`), wrap in the newtype before passing further. Where service code writes back, unwrap to the primitive.
  - Update tests to use the newtype constructors.
- **Standards refs:** `rust.md` § 2, `seaorm.md` § 6.
- **Effort:** L. Touches every route, every service, every DTO.
- **Dependencies:** PR-C1 (`AppError::BadRequest` for parse failures).
- **Risk:** Medium. Long diff; each conversion site is a chance to break.
- **Verification:** All tests pass. API contract unchanged (verified by running through a few endpoints manually and confirming request/response shapes are identical).
- **Sign-off:** [ ]

### PR-D2: Validated string newtypes

- **Scope:**
  - Define `Username`, `EmailAddress`, `PasswordHash`, `DrinkTypeName`, `ImagePath`, `RunNotes` in `backend/src/domain/strings.rs` using `nutype` with appropriate validation.
  - Move existing validation logic out of services into newtype constructors. Where a service had `if username.len() > 30 { return Err(...) }`, replace with `Username::try_from(input)?`.
  - Update tests.
- **Standards refs:** `rust.md` § 2.
- **Effort:** M.
- **Dependencies:** PR-D1.
- **Risk:** Medium — moving validation from services to constructors means a few code paths change which type the parse error materializes from. Existing `AppError::BadRequest` text should be preserved or improved.
- **Verification:** Tests cover happy + invalid cases for each newtype.
- **Sign-off:** [ ]

### PR-D3: Convert string-typed enums to `DeriveActiveEnum`

- **Scope:**
  - Migrate the following from `String` to real enums with `DeriveActiveEnum`: `SessionStatus` (active/closed), `SessionRuleset` (random/default/least_played/round_robin), `DrinkCategory` (alcoholic/non_alcoholic), `RunFlagReason` (preset list).
  - Update entity definitions (regenerate via `just entities` after migration).
  - Update consolidated migration to specify the column type if needed (SeaORM's `EnumIter` / DeriveActiveEnum maps to TEXT with the variants enforced at the application level by default — fine for SQLite).
  - Reset dev DB after migration edit.
- **Standards refs:** `rust.md` § 2, `seaorm.md` § 5.
- **Effort:** M.
- **Dependencies:** PR-D1.
- **Risk:** Low. The DB still stores TEXT; the change is in Rust.
- **Verification:** Match arms over the new enums are exhaustive (compiler enforces); existing string-comparison code is gone.
- **Sign-off:** [ ]

### PR-D4: Numeric domain types

- **Scope:**
  - Define `RaceTimeMs(NonZeroI32)` and `LapTimeMs(NonZeroI32)` newtypes via `nutype`.
  - Migrate run-recording paths (`services/runs.rs`, run-creation routes) to use the newtypes for `track_time`, `lap1_time`, `lap2_time`, `lap3_time`.
  - The "lap times sum to total" invariant becomes a function over the typed values, not raw `i32`.
- **Standards refs:** `rust.md` § 2.
- **Effort:** S–M.
- **Dependencies:** PR-D1.
- **Risk:** Low.
- **Verification:** Existing run-time validation tests pass; the function `assert_lap_sum(laps: [LapTimeMs; 3], total: RaceTimeMs)` is the new invariant point.
- **Sign-off:** [ ]

---

## Phase E — SeaORM ergonomics

### PR-E1: `ActiveModelBehavior::before_save` for timestamps

- **Scope:**
  - For every entity with `created_at` and/or `updated_at`, implement `ActiveModelBehavior::before_save` in a sibling file `entities/{entity}_behavior.rs`, wired in via `entities/mod.rs`.
  - Remove all manual `created_at: Set(now)` / `updated_at: Set(now)` calls from service code.
- **Standards refs:** `seaorm.md` § 1, § 6.
- **Effort:** M.
- **Dependencies:** PR-A2 (audit identifies which entities need this).
- **Risk:** Low. Centralizing behavior reduces bugs, doesn't introduce them.
- **Verification:** Tests confirm `updated_at` advances on update; integration test inserts a row and reads back the timestamp.
- **Sign-off:** [ ]

### PR-E2: Audit `&impl ConnectionTrait` usage

- **Scope:**
  - Walk every function in `backend/src/services/`. Functions that take `&DatabaseConnection` and aren't top-level orchestration get migrated to `&impl ConnectionTrait`.
  - Top-level orchestration (called directly from a route handler that owns the `&DatabaseConnection`) stays.
- **Standards refs:** `seaorm.md` § 4.
- **Effort:** S–M.
- **Dependencies:** PR-A2 (audit identifies the offenders).
- **Risk:** Low. Generic bound is a strict superset of `&DatabaseConnection`.
- **Verification:** Compiler. After the change, every service can be called from inside a transaction.
- **Sign-off:** [ ]

### PR-E3: Drop `sessions.created_by`

- **Scope:**
  - Design decision in `reviews/design/sessions-created-by-removal.md` (one-line: "host_id is the only host indicator; created_by adds no current product value").
  - Update consolidated migration: remove `created_by` column and its FK.
  - Update entity (regenerate via `just entities`).
  - Update design.md: change "host_id starts as created_by" to "host_id starts as the user who created the session, transfers on leave."
  - Update services and routes that reference `created_by`.
  - Update CLAUDE.md if anything references `created_by` policy.
  - Reset dev DB.
- **Standards refs:** `seaorm.md` § 11 (multi-FK relations), design.md.
- **Effort:** S.
- **Dependencies:** None (independent of standards work).
- **Risk:** Low. Prelaunch — no data loss concern.
- **Verification:** Tests pass; `sessions` table no longer has a `created_by` column.
- **Sign-off:** [ ]

---

## Phase F — Tokio infrastructure

### PR-F1: TaskTracker + CancellationToken + graceful shutdown

- **Scope:**
  - Add `tokio-util` to `[dependencies]` (with `task` feature).
  - In `main.rs`: create a `CancellationToken` and `TaskTracker` at startup, store both in app state (or pass into background-task spawners).
  - Wire `axum::serve(...).with_graceful_shutdown(...)` to a `select!` over `tokio::signal::ctrl_c()` and (on Unix) `SIGTERM`.
  - On shutdown: cancel the token, then `tokio::time::timeout(20s, tracker.wait())`.
- **Standards refs:** `tokio.md` § 5, § 13.
- **Effort:** M.
- **Dependencies:** None.
- **Risk:** Medium — first time shutdown is exercised end-to-end. Test on Unraid before tagging release.
- **Verification:** Send SIGTERM during a long-running request; confirm in-flight requests complete (or the timeout triggers cleanly).
- **Sign-off:** [ ]

### PR-F2: Implement `session_cleanup_loop` as a tracked background task

- **Scope:**
  - Implement the 5-minute stale-session cleanup task per `tokio.md` § 8.
  - Spawn it via `TaskTracker::spawn` from PR-F1.
  - Integrate the `Entity::update_many()` set-based update for closing stale sessions (`seaorm.md` § 1).
- **Standards refs:** `tokio.md` § 8, `seaorm.md` § 1.
- **Effort:** S.
- **Dependencies:** PR-F1, PR-D3 (`SessionStatus` enum).
- **Risk:** Low.
- **Verification:** Insert a session with `last_activity_at` 2h ago; let the cleanup tick fire; confirm status flipped to `closed`.
- **Sign-off:** [ ]

### PR-F3: Tower middleware — request limits

- **Scope:**
  - Add `TimeoutLayer` (e.g., 30s request-level), `ConcurrencyLimitLayer` (e.g., 100), `RequestBodyLimitLayer` (e.g., 10 MiB to match the upload size cap from design.md).
  - Add `tower-governor` for rate limiting (default 60 req/min/IP; tunable).
- **Standards refs:** `tokio.md` § 12.
- **Effort:** S.
- **Dependencies:** None.
- **Risk:** Medium — rate limiter could falsely throttle. Start permissive; tighten with metrics.
- **Verification:** `curl --data-binary @big-file` against an upload endpoint past the size limit returns 413; concurrent request flood returns 503/429.
- **Sign-off:** [ ]

### PR-F4: Per-call timeouts on DB and external calls

- **Scope:**
  - Wrap every `await` against SeaORM and any future outbound HTTP in `tokio::time::timeout`.
  - Introduce a `db_timeout!` macro (or wrapper helper) to keep call sites tidy. Default 2s for queries, 5s for transactions.
  - Map elapsed errors to `AppError::Internal` (or a new `AppError::Timeout` variant if we want a distinct status — discuss before landing).
- **Standards refs:** `tokio.md` § 12.
- **Effort:** L. Touches every service.
- **Dependencies:** PR-C1 (AppError shape stable).
- **Risk:** Medium. Timeouts that fire under load are operational pain; need monitoring before tightening.
- **Verification:** Force a SQLite lock (long write transaction in another connection); confirm the blocked query times out cleanly with the configured budget.
- **Sign-off:** [ ]

---

## Phase G — Documentation, tests, formatting

### PR-G1: Test in-memory SQLite uses `?cache=shared`

- **Scope:**
  - Audit all test setups in `tests/` and `#[cfg(test)] mod tests` blocks for `sqlite::memory:` URLs.
  - Replace with `sqlite::memory:?cache=shared` (or unique cache names per test if cross-test isolation is needed).
- **Standards refs:** `seaorm.md` § 9.
- **Effort:** S.
- **Dependencies:** None.
- **Risk:** Low.
- **Verification:** Existing tests pass. Add one test that exercises pool size > 1 to confirm tables remain visible across connections.
- **Sign-off:** [ ]

### PR-G2: Add `rstest`, `proptest`, `insta` as dev-dependencies

- **Scope:**
  - Add the three crates as `[dev-dependencies]`.
  - Migrate one or two existing repetitive test files to `rstest` as a demo.
  - Add `insta` for one HTTP integration test as a demo.
  - Don't migrate everything in this PR — leave that for organic adoption.
- **Standards refs:** `rust.md` § 7.
- **Effort:** S.
- **Dependencies:** None.
- **Risk:** Low.
- **Verification:** Demo tests pass. `insta accept` workflow works.
- **Sign-off:** [ ]

### PR-G3: Doc-comment audit

- **Scope:**
  - Walk every `pub` and cross-module `pub(crate)` item.
  - Add `///` doc with one-sentence summary.
  - Add `# Errors` section to every fallible function (handlers and service layer).
  - Add crate-level `//!` doc to `lib.rs` and `migration/lib.rs`.
- **Standards refs:** `rust.md` § 6.
- **Effort:** L. Lots of mechanical work.
- **Dependencies:** PR-D1, PR-D2 (so docs reference newtypes, not primitives).
- **Risk:** Low.
- **Verification:** `cargo doc --no-deps` produces clean output; spot-check that summary tables are readable.
- **Sign-off:** [ ]

### PR-G4: File-length splits

- **Scope:**
  - Split `services/runs.rs` and `services/sessions.rs` (and any other >500-line non-test files identified in PR-A2) by concern.
  - Tests stay where they are — splitting tests is a separate question.
- **Standards refs:** `rust.md` § 13.
- **Effort:** M.
- **Dependencies:** PR-A2 (audit confirms which files need it).
- **Risk:** Medium — lots of `pub(crate)` boundary changes; the compiler will catch most issues but ergonomic shifts may surface.
- **Verification:** Tests pass; `git mv` history is preserved where possible.
- **Sign-off:** [ ]

---

## Phase H — Lint cleanup

### PR-H1+: Clear pedantic warnings, one or two lints per PR

- **Scope:**
  - For each `#[allow(clippy::...)]` added in PR-A1, fix the underlying issue and remove the allow.
  - One PR per lint (or per small group of related lints).
  - Order roughly by signal-to-noise: address lints that catch real bugs first (`clippy::needless_pass_by_value`, `clippy::large_types_passed_by_value`), style-only lints last.
- **Standards refs:** `rust.md` § 8.
- **Effort:** Variable per PR; total is L over many PRs.
- **Dependencies:** PR-A1.
- **Risk:** Low per PR.
- **Verification:** Each PR removes a specific `#[allow]` and the build still passes.
- **Sign-off:** Created lazily; one row per PR as we go.

---

## Phase I — Workflow

### PR-I1: Update code review skill

- **Scope:**
  - Update `.claude/` (or wherever the code-review skill is configured) to read `docs/coding-standards/README.md` first when starting a review.
  - The skill should identify which area files (`rust.md`, `seaorm.md`, `tokio.md`) the PR diff touches and load only those.
  - Findings get written to `reviews/pr/` (existing convention).
- **Standards refs:** Workflow improvement; not a code rule.
- **Effort:** S–M.
- **Dependencies:** Doc split lands (already done).
- **Risk:** Low.
- **Verification:** Run the skill on a sample PR; confirm it reads the right files.
- **Sign-off:** [ ]

---

## Order summary (TL;DR)

If you want to look at one ordered list:

1. **A1** — lints, rustfmt, editorconfig (with allows)
2. **A2** — codebase audit
3. **B1** — fix `From<DbErr>`
4. **B2** — fix SQLite PRAGMA scope
5. **B3** — Argon2 spawn_blocking confirmation
6. **C1** — `AppError` → thiserror
7. **D1** — ID newtypes
8. **D2** — validated string newtypes
9. **D3** — string enums → DeriveActiveEnum
10. **D4** — numeric domain types
11. **E1** — `before_save` for timestamps
12. **E2** — `&impl ConnectionTrait` audit
13. **E3** — drop `sessions.created_by`
14. **F1** — shutdown infrastructure
15. **F2** — session cleanup task
16. **F3** — Tower middleware
17. **F4** — per-call timeouts
18. **G1** — test SQLite `cache=shared`
19. **G2** — rstest / proptest / insta
20. **G3** — doc-comment audit
21. **G4** — file-length splits
22. **H1+** — lint cleanup PRs (many)
23. **I1** — code review skill update

Some PRs (B1, B3, E3) have no dependencies and can land in parallel with A1/A2.

---

## Document history

- 2026-05-02 — Initial draft. PRs identified by reading the standards docs against design.md and the repo file listing. Some PRs (D1, E2) presume audit findings (PR-A2) — concrete scopes will tighten once the audit lands.
