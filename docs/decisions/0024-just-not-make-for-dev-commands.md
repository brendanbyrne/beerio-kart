---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0024 — just (not Make) for developer commands

## Context and problem statement

Developers need a convenient way to run common tasks: start the dev server, run tests, bootstrap entities, etc. Build-system options include Make (traditional, complex for non-C projects) and just (task runner, simpler syntax). The choice should complement existing dependency management, not duplicate it.

## Decision drivers

- Cargo handles Rust build dependencies and incremental compilation.
- Bun handles frontend dependencies and bundling.
- Docker handles container caching.
- The remaining need is a task runner for orchestrating these, not file-level dependency tracking.

## Considered options

- **Option A:** Use Make. Powerful, but syntax is geared toward C workflows; Cargo and Bun already handle their domains.
- **Option B:** Use just. Simple task syntax; no redundant dependency tracking; good at orchestration.
- **Option C:** No task runner; document shell commands. Error-prone; not convenient.

## Decision outcome

Chosen: **Option B** — Use just for developer commands (`just dev`, `just test`, `just entities-bootstrap`). Justfile is at the repo root. Cargo, Bun, and Docker handle their respective dependency layers; just orchestrates them.

### Positive consequences

- Simple, readable syntax; developers understand what each task does at a glance.
- Minimal abstraction; just wraps existing tools without pretending to manage their dependencies.
- Easy to add new tasks; no make-specific learning curve.

### Negative consequences / trade-offs

- One more language/tool to learn. Acceptable: just is simpler than Make, and developers are already using Cargo and Bun.

## Links

- Source: `ad-hoc`
