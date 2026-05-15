# Coding Standards

This directory contains the project's coding standards, split by area so a reviewer or an AI subagent can load only what's relevant to a given diff.

- [`rust.md`](./rust.md) — General Rust patterns: error handling, type design, modules, ownership, iterators, documentation, testing, lints, edition, tracing, idioms, anti-patterns, file length, serde, Cargo.toml hygiene, formatting, config/env, feature flags.
- [`seaorm.md`](./seaorm.md) — SeaORM-specific: `ActiveModel` vs `Model`, query patterns, transactions, `ConnectionTrait` abstraction, migrations, entity organization, error handling, connection pool & SQLite, testing, raw SQL, relations, pitfalls.
- [`tokio.md`](./tokio.md) — Async Rust and Tokio: runtime choice, blocking, sync primitives across `.await`, channels, structured concurrency, cancellation, `select!`, background tasks, `Send`/`Sync` and `'static`, async tracing, async pitfalls, backpressure, shutdown.

Companion documents:

- [`../api-contract.md`](../api-contract.md) — wire-format conventions between backend and frontend.
- [`../designs/archive/compliance-plan.md`](../designs/archive/compliance-plan.md) — archived sequenced PR plan that brought the existing codebase to the standard (signed off 2026-05-15).

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
