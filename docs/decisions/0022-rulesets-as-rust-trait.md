---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0022 — Rulesets: implemented as Rust trait with one module per ruleset

## Context and problem statement

The app supports multiple rulesets for choosing the next track: Random, Default, Least Played, Round-robin. Each ruleset has different logic (some deterministic, some probabilistic). The code needs to evolve as new rulesets are added.

## Decision drivers

- Adding a new ruleset should require adding one module, not modifying existing code.
- Each ruleset's logic is isolated from the others; bugs in one don't spread.
- Clear architectural pattern; future developers know where to add new rulesets.

## Considered options

- **Option A:** Giant switch statement in session service. Simple, but grows unbounded as rulesets increase.
- **Option B:** Each ruleset is a separate module implementing a `Ruleset` trait. Extensible, testable per-ruleset.
- **Option C:** Rulesets are data-driven (lookup table, config file). Adds indirection without clear benefit at this scale.

## Decision outcome

Chosen: **Option B** — Each ruleset (Random, Default, Least Played, Round-robin) is a separate module implementing a `Ruleset` trait. The session service calls the trait method to get the next track.

### Positive consequences

- Adding a new ruleset is one module, no existing-code changes.
- Each ruleset can have its own tests; logic is isolated.
- Trait bounds make the interface explicit.

### Negative consequences / trade-offs

- Minimal indirection; negligible performance overhead.

## Links

- Source: `ad-hoc`
