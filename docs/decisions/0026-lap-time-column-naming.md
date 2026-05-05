---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0026 — Lap time column naming: `lap1_time`, `lap2_time`, `lap3_time`

## Context and problem statement

The `runs` table stores the time for each lap: lap 1, lap 2, lap 3. Column names need to be unambiguous and map cleanly to Rust identifiers. The question is whether to use underscores before digits (e.g., `lap_1_time`) or not (e.g., `lap1_time`).

## Decision drivers

- Column names map to Rust via SeaORM's `DeriveIden` macro, which converts snake_case to PascalCase.
- `lap1_time` → `Lap1Time` is the natural SeaORM output.
- `lap_1_time` → `Lap1Time` requires a custom rename, adding noise.
- Consistency with Rust identifier conventions (no underscore before digit).

## Considered options

- **Option A:** `lap_1_time`, `lap_2_time`, `lap_3_time` — explicit word boundaries. Requires custom naming in SeaORM.
- **Option B:** `lap1_time`, `lap2_time`, `lap3_time` — matches SeaORM's natural codegen output.

## Decision outcome

Chosen: **Option B** — `lap1_time`, `lap2_time`, `lap3_time`. No underscore before digit. Matches SeaORM's `DeriveIden` macro output naturally.

### Positive consequences

- Codegen produces the right Rust names without custom overrides.
- Fewer surprises in entity definitions.

### Negative consequences / trade-offs

- Negligible: SQL dialect supports both; readability is the same.

## Links

- Source: `ad-hoc`
