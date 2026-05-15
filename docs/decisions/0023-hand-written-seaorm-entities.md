---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: docs/designs/archive/2026-05-02-entity-codegen-strategy.md
---

# 0023 — Hand-written SeaORM entities: committed source code

## Context and problem statement

SeaORM's `sea-orm-cli generate entity` scaffolds Rust entities from the database schema. Regenerating those entities on every schema change can clobber hand-written corrections (partial-index attributes, relation cardinalities, custom derives). The question is: who owns the source truth — migration or codegen output?

## Decision drivers

- The migration is the schema source of truth; entities mirror that shape.
- Hand-edits (relation cardinalities, attributes) are authoritative and shouldn't be lost to codegen.
- Codegen is a one-shot scaffolding tool, not a continuous mirror.

## Considered options

- **Option A:** Regenerate entities from the schema on every migration. Loses hand-edits; forces custom overrides everywhere.
- **Option B:** Hand-write entities once; never regenerate. Entities must be kept in sync with migrations manually.
- **Option C:** Hand-write entities; regenerate only when adding a new table. Entities are committed source; partial edits are preserved.

## Decision outcome

Chosen: **Option C** — Entities under `backend/src/entities/` are committed source code, hand-edited as the schema evolves. The migration is the schema source of truth; entities mirror that shape. Codegen (`just entities-bootstrap`) is a one-shot scaffolding tool used only when adding a brand-new table — never run on existing entities, as it will clobber hand-corrections.

### Positive consequences

- Hand-edits (partial-index attributes, relation cardinalities, custom derives) are safe and durable.
- Entities and migrations can evolve together without clobbering.
- Clear responsibility: migrations define schema; entities define Rust shapes.

### Negative consequences / trade-offs

- Entities must be kept in sync with the schema manually. Acceptable: schema drift is caught by the `tests/schema_drift.rs` verification test, and changes are infrequent.

## Links

- Source: [`docs/designs/archive/2026-05-02-entity-codegen-strategy.md`](../designs/archive/2026-05-02-entity-codegen-strategy.md)
- Implementing PRs: PR-X1
