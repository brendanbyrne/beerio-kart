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

## Coverage & CI

- **Local:** backend — `just coverage` generates an HTML report, `just coverage-summary` prints a text summary; frontend — `bun run test:coverage` (Vitest + istanbul; HTML report under `frontend/coverage/`).
- **CI:** GitHub Actions. The `Checks` workflow runs frontend `lint`/`typecheck` and backend `clippy`/`fmt --check` — a CI backstop for the lefthook pre-commit gates (which are local-only and `--no-verify`-bypassable). The `Backend` workflow runs `cargo-llvm-cov` and the `Frontend` workflow runs `bun run test:coverage`, both uploading to Codecov (`backend` / `frontend` flags). `Backend` is path-filtered to its subtree (Codecov carries the untouched flag forward when it skips); `Frontend` and `Checks` run on **every** PR — `Frontend` so Codecov always gets an upload and posts its checks, `Checks` so its required statuses always post. A `DTO Drift` workflow fails any PR that changes a backend wire-contract file without also updating `frontend/src/api/types.ts`, the hand-maintained Zod mirror (see [`coding-standards/typescript.md`](./coding-standards/typescript.md) § 11). Every merge-gating workflow that posts its *own* status uses an **always-run + internal-skip** shape (run on every PR, skip the work internally when nothing relevant changed, but still post a status) so it's safe to mark required; the one exception is backend coverage — `Backend` stays path-filtered and its `codecov/patch/backend` check posts via Codecov flag carryforward. See [`branch-protection.md`](./branch-protection.md).
- **Exclusions:** backend — `entities/` (SeaORM codegen), `migration/`, `main.rs` (wiring), `seed.rs` (startup); frontend — test files, `src/mocks/`, `src/setupTests.ts`, `src/main.tsx` (bootstrap). Only business logic counts.
- **Policy:** No regression from the base branch (`target: auto`, 0.5% threshold). New/changed code must be 80% covered (`patch: 80%`). The `codecov/patch/{frontend,backend}` checks report pass/fail, so a patch-coverage miss surfaces a red ✗. A **branch-protection ruleset** is now prepared ([`branch-protection.md`](./branch-protection.md)) and applied manually (a `gh api` POST, per that doc) once Issue #195's PR merges; it makes those two patch checks plus the `Checks` (lint/typecheck/clippy/fmt) and `DTO Drift` jobs **required**, so they block the merge rather than only flag it. (`codecov/project/*` is configured non-informational in `codecov.yml` but isn't currently emitted as a GitHub check, so it stays advisory and out of the required set — a documented follow-up in `branch-protection.md`.)
- **Reports:** Codecov posts a PR comment with coverage delta, patch coverage, and per-file breakdown.

## Document history

> This is a decision log, not a changelog (see [`docs/CLAUDE.md`](./CLAUDE.md) § Document history rule). Entries before 2026-05-31 were a per-PR changelog of the file's pre-restructure life; that detail lives in git. The three entries below record the decisions behind the file's current shape.

- 2026-05-05–08 — Docs restructure (PRs 1–6). design.md was reduced to the architecture overview: the Data Model, API Surface, User Workflows/UI Screens, Build Plan, and Resolved Decisions sections, plus the repo-tree, were extracted to `data-model.md`, `api-contract.md`, `user-workflows.md`, `roadmap.md`, `decisions/` (MADRs 0002–0034), and the root `README.md`. Full narrative and per-PR rationale: [`designs/archive/2026-05-04-design-doc-restructure.md`](./designs/archive/2026-05-04-design-doc-restructure.md).
- 2026-05-18 — § Coverage & CI extended to cover the frontend alongside the backend: per-flag Codecov project/patch gates with carryforward, path-filtered `Backend`/`Frontend` Actions workflows, Vitest with the istanbul provider (v8 reports 0% under Bun). PR-H2 ([#193](https://github.com/brendanbyrne/beerio-kart/issues/193)).
- 2026-06-15 — § Coverage & CI: frontend *project* status flipped from informational to pass/fail once PR-H1's ([#185](https://github.com/brendanbyrne/beerio-kart/issues/185)) test-coverage backfill landed — all four Codecov statuses now report a ✗ on regression (enforcement still pending a `main` branch-protection rule that lists them as required). Same PR added the `DTO Drift` CI workflow (a backend wire-contract change without a `frontend/src/api/types.ts` update fails the PR; rule in `coding-standards/typescript.md` § 11).
- 2026-06-15 — § Coverage & CI: added the `Checks` workflow (frontend lint/typecheck + backend clippy/fmt — a CI backstop for the `--no-verify`-bypassable lefthook hooks) and converted the merge-gating workflows to **always-run + internal-skip** so their statuses always post. Prepared the `main` branch-protection ruleset ([`branch-protection.md`](./branch-protection.md)) that makes `codecov/patch/{frontend,backend}` + the two `Checks` jobs + `DTO Drift` **required**; `codecov/project/*` is excluded because Codecov isn't emitting it as a check (follow-up). Pinned the stable Rust toolchain (`rust-toolchain.toml`) so the required clippy check can't surprise-break when a runner bumps stable — local lefthook and CI now resolve the same rustc. Cross-file consequence: the docs-only direct-to-`main` carve-out is retired — all changes now go through a PR (`.claude/CLAUDE.md`, `project-workflow.md`, post-merge skill updated). Issue [#195](https://github.com/brendanbyrne/beerio-kart/issues/195).
- 2026-05-31 — Slimmed to overview + § Architecture + § Coverage & CI (#220, PR #223). The `## Observability`/error-response block and the seven pointer-only sections were removed — their content was already canonical elsewhere (wire → `api-contract.md`; Rust impl → `rust.md`; ORM/timeouts → `seaorm.md`/`tokio.md`; naming → `data-model.md`) and navigation lives in `docs/README.md`. Three facts that lived *only* here were preserved into `rust.md` §§ 1 and 10 first: the `Token`/`Hash` typed `#[from]` variants, the project-local `extract::Path`/`Json` envelope, and the `RUST_LOG`/`tracing-subscriber`/`TraceLayer` stack. Per-side stack + rationale now lives in `backend/README.md` and `frontend/README.md` § Tech; the error registry in `api-contract.md` § 7 with `error.rs` as source of truth (see ADR [0036](./decisions/0036-error-code-rollout.md)).
