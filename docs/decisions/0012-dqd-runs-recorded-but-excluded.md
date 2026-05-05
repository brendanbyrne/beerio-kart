---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0012 — DQ'd runs: recorded but excluded from leaderboards and H2H

## Context and problem statement

The Beerio Kart rules are simple: finish the race before or while finishing the drink (DQ = didn't finish drink before finishing race). Players submit their own DQ status at race time. These runs are real data (useful for history, personal stats) but shouldn't count toward leaderboard positions or H2H head-to-head comparisons.

## Decision drivers

- Preserve honest runs while allowing players to see DQ history for context.
- Prevent gaming of leaderboards (submit "fake" runs as DQ).
- Honor system works in small, social groups.

## Considered options

- **Option A:** DQ'd runs are deleted. No history, harsh.
- **Option B:** DQ'd runs are recorded but excluded from all rankings. Transparent history, fair competition.
- **Option C:** Admin must verify DQ status before recording. Adds bottleneck.

## Decision outcome

Chosen: **Option B** — DQ'd runs are recorded in the database. They're excluded from leaderboard positions and H2H tallies. Self-reported at submission time (honor system).

### Positive consequences

- Players can see their full race history, including DQ runs, without losing that data.
- Leaderboards and H2H remain fair — no artificially-inflated or deflated stats.
- No admin overhead verifying DQ claims.

### Negative consequences / trade-offs

- Relies on player honesty. Acceptable: social pressure and small-group accountability are strong, and the game is non-competitive (fun, not money).

## Links

- Source: `ad-hoc`
