---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0002 — SQLite STRICT mode on lookup tables; DATETIME on timestamped tables

## Context and problem statement

SQLite's `STRICT` table modifier rejects rows whose column values don't match the declared type. It's stricter and safer than the default (where SQLite happily stores any type in any column). The catch: SeaORM's codegen, when reading STRICT tables that include `TEXT NOT NULL` timestamp columns, produces stringly-typed Rust models — `String`-shaped timestamps everywhere, with manual parsing at every boundary.

We wanted the type safety of STRICT for the static game-data tables (where data shape is small, well-defined, and inserted at seed time) without paying the stringly-typed timestamp cost on the runtime tables (sessions, runs, etc., which have lots of `created_at` / `last_activity_at` columns).

## Decision drivers

- Type safety at insert time for the parts of the schema where shape is fixed (characters, tracks, cups, bodies, wheels, gliders).
- Ergonomic Rust types for timestamps on the runtime side — `DateTime<Utc>` rather than `String`-and-parse-on-every-read.
- One consistent SeaORM codegen flow rather than a manual override per timestamp column.

## Considered options

- **Option A:** STRICT everywhere. Maximum type safety, but every timestamp column becomes `String` in Rust.
- **Option B:** STRICT nowhere. Clean Rust types via DATETIME columns, but no insert-time guarantees on static-data shape.
- **Option C:** STRICT on lookup/static tables, DATETIME (no STRICT) on tables with timestamps.

## Decision outcome

Chosen: **Option C** — STRICT on the static lookup tables; drop STRICT on timestamped tables so columns can use the `DATETIME` type and SeaORM codegen produces `DateTime<Utc>`.

### Positive consequences

- Static-data tables retain insert-time type guarantees.
- Runtime tables get ergonomic timestamp handling — no manual `parse` / `to_string` at every boundary.
- Codegen produces the right Rust shape automatically.

### Negative consequences / trade-offs

- The schema's strictness rules are non-uniform — readers have to remember which tables are STRICT. Mitigated by the rule being predictable ("STRICT iff no timestamp columns").

## Links

- Source: `ad-hoc` (resolved during early schema work; captured in `docs/design.md`'s Resolved Decisions section pre-PR-2).
