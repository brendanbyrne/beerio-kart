# Coding Standards

This directory contains the project's coding standards, split by area so a reviewer or an AI subagent can load only what's relevant to a given diff.

**Backend (Rust + Axum + SeaORM)**

- [`rust.md`](./rust.md) — General Rust patterns: error handling, type design, modules, ownership, iterators, documentation, testing, lints, edition, tracing, idioms, anti-patterns, file length, serde, Cargo.toml hygiene, formatting, config/env, feature flags.
- [`seaorm.md`](./seaorm.md) — SeaORM-specific: `ActiveModel` vs `Model`, query patterns, transactions, `ConnectionTrait` abstraction, migrations, entity organization, error handling, connection pool & SQLite, testing, raw SQL, relations, pitfalls.
- [`tokio.md`](./tokio.md) — Async Rust and Tokio: runtime choice, blocking, sync primitives across `.await`, channels, structured concurrency, cancellation, `select!`, background tasks, `Send`/`Sync` and `'static`, async tracing, async pitfalls, backpressure, shutdown.

**Frontend (React + TypeScript + Vite + Tailwind)**

- [`typescript.md`](./typescript.md) — TypeScript language rules: tsconfig strictness, `type` vs `interface`, `satisfies`, branded types, discriminated unions, exports/modules, error handling, async/cancellation, runtime validation, lints, anti-patterns, backend interop, testing (umbrella policy + Vitest patterns).
- [`react.md`](./react.md) — React 19 patterns: component shape, hooks rules, state management, data fetching (TanStack Query), React 19 primitives (`useActionState`/`useOptimistic`/etc.), effects, memoization (with the React Compiler), forms, error boundaries, accessibility, react-router 7 idioms, file organization, testing (React Testing Library + MSW + hook tests).
- [`tailwind.md`](./tailwind.md) — Tailwind v4 styling: utility-first ethos, mobile-first breakpoints, CSS-first `@theme` config, `clsx` for conditional classes, touch targets, Firefox/Safari compatibility, when to escape Tailwind, anti-patterns.

**Cross-cutting**

- [`testing.md`](./testing.md) — test-assertion rules that apply to both stacks: assert the specific error / `code` (not just `is_err` / the HTTP status), assert the observable outcome (not an interaction spy), and pin the serialized wire format. The language-specific testing *patterns* live in `rust.md` § 7, `typescript.md` § 12, and `react.md` § 13; this file holds the rules common to both and the lints that enforce them.

Companion documents:

- [`../api-contract.md`](../api-contract.md) — wire-format conventions between backend and frontend.
- [`../designs/archive/compliance-plan.md`](../designs/archive/compliance-plan.md) — archived sequenced PR plan that brought the existing backend codebase to the standard (signed off 2026-05-15).
- [`../designs/2026-05-16-frontend-audit.md`](../designs/2026-05-16-frontend-audit.md) — per-file compliance baseline for `frontend/src/` against the new standards. Source of the findings in the compliance plan.
- [`../designs/2026-05-16-frontend-compliance-plan.md`](../designs/2026-05-16-frontend-compliance-plan.md) — active sequenced PR plan to bring the existing frontend codebase to the standard.
- [`../research/rust-to-ts-codegen.md`](../research/rust-to-ts-codegen.md) — evaluation of automated Rust→TypeScript type-sync tools (typeshare, ts-rs, specta, schemars). Informs `typescript.md` § 11.
- [`../research/unit-test-audit.md`](../research/unit-test-audit.md) — the backend+frontend test audit that produced `testing.md`'s rules: the anti-pattern taxonomy, per-file findings, and the cleared false-positives. Source of Issue #217.

## How to use these docs

**Reviewing a PR.** Identify which area files the diff touches, and read only those plus this README. A SeaORM-only change doesn't need the Tokio file loaded; a background-task change doesn't need the SeaORM file. The split exists to make focused review tractable.

**Writing new code.** Read the area file relevant to what you're writing, plus the compliance plan if that area is mid-migration. Where rules cross areas (e.g., `AppError` is defined in `rust.md` but referenced from `seaorm.md`), cross-references are explicit.

**Disagreements.** Every rule has a `Source:` link. Re-litigate against evidence, not opinion. If the evidence has shifted since a rule was written, propose an update in a `docs/designs/` checkbox session.

## Format

Every rule has the same shape:

- **Rule:** short imperative.
- **Why:** one to three sentences of rationale.
- **Example:** code snippet showing the rule (often a do/don't pair).
- **Source:** authoritative URL(s).

The rules are *not* ranked. If a rule is in the doc, follow it. The compliance plan ranks the *order of adoption* for unmet rules — that's where prioritization lives.

## Tooling vs. review

Some rules are checked by tooling on every CI run (`cargo fmt --check`, `cargo clippy`, `cargo test`). Others depend on human (or AI) review. Each rule's `Why:` blurb usually makes the enforcement mechanism obvious; where it isn't, the rule body says explicitly.

## History

- 2026-05-02 — Initial monolithic draft at `docs/rust-coding-standards.md`.
- 2026-05-02 — Split into per-area files (this directory). Added serde, rustfmt, Cargo, config, feature-flag sections to `rust.md`. Adopted the "launch = first deploy where data preservation matters" definition in `seaorm.md`. Adopted the strict newtype-with-`nutype` direction.
- 2026-05-15 — Updated the compliance-plan companion link to point at its archived location (`../designs/archive/compliance-plan.md`); plan signed off 2026-05-15. Companion to PR [#160](https://github.com/brendanbyrne/beerio-kart/pull/160) / Issue [#159](https://github.com/brendanbyrne/beerio-kart/issues/159).
- 2026-05-16 — Added frontend standards: `typescript.md`, `react.md`, `tailwind.md`. Sourced from deep research conducted same day (TypeScript 5.9, React 19.2, React Compiler v1.0, TanStack Query v5, Tailwind v4, react-router 7) and a per-file audit of `frontend/src/`. Companion artifacts: `../designs/2026-05-16-frontend-audit.md` (per-file compliance baseline), `../designs/2026-05-16-frontend-compliance-plan.md` (sequenced PRs driven by the audit), `../research/rust-to-ts-codegen.md` (evaluation of typeshare / ts-rs / specta / schemars for type-sync). `frontend/CLAUDE.md` updated to point at the new files.
- 2026-05-18 — Added Testing requirements: `typescript.md` § 12 (umbrella policy + Vitest patterns) and `react.md` § 13 (React Testing Library + MSW + hook tests). Policy mirrors `backend/CLAUDE.md` § Testing: tests are a deliverable, not optional; every requirement should be unit- or integration-testable, within reason. `frontend/CLAUDE.md` gained a § Testing section with the same policy and pointers. The compliance plan's PR-H2 (Vitest scaffolding) was promoted from optional to required and re-sequenced ahead of the runtime-behavior PRs; filed as Issue [#193](https://github.com/brendanbyrne/beerio-kart/issues/193).
- 2026-05-31 — Added `testing.md` (cross-cutting test-assertion rules: pin the specific error / `code` rather than `is_err` / the HTTP status, assert the observable outcome rather than an interaction spy, pin the serialized wire literal) plus the lints that hold them — `#![warn(clippy::assertions_on_result_states)]` with lefthook `cargo clippy --all-targets`, and the `@vitest/eslint-plugin` + `eslint-plugin-testing-library` presets. Companion to Issue [#217](https://github.com/brendanbyrne/beerio-kart/issues/217)'s suite-wide tightening pass (the dominant gap was rejection tests that asserted *that* an op failed, not *which* error).
