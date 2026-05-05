---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0006 — Beer vs water: separate leaderboards by default, with combined view

## Context and problem statement

Some players drink beer, others drink water or skip the game entirely. Comparing times across drink categories isn't fair — alcohol affects performance. The leaderboard must separate them while letting players see how they compare across both categories if they choose.

## Decision drivers

- Fair competition within each drink category.
- Inclusive by default — non-drinkers have their own valid leaderboard.
- Flexible exploration of cross-category comparisons.

## Considered options

- **Option A:** Single unified leaderboard with a drink filter. Requires scrolling past non-preferred drinks.
- **Option B:** Separate leaderboards (beer, water) with a combined-view toggle. More work, better defaults.
- **Option C:** Per-session drink rules; leaderboards vary per ruleset. Overcomplicates early design.

## Decision outcome

Chosen: **Option B** — Separate leaderboards by default (beer vs water). A "combined view" toggle shows all categories ranked together, with drink category labeled per entry.

### Positive consequences

- Users see their category's leaderboard by default — no scroll-past-others friction.
- Default toggle setting matches the user's preferred drink category (stored in profile).
- Combined view is available for curiosity without making it the default.

### Negative consequences / trade-offs

- Slightly higher schema/query complexity (per-category indexes on leaderboards). Negligible: only two categories.

## Links

- Source: `ad-hoc`
