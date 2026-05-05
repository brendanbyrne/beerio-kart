---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0003 — Global leaderboard: most track records held

## Context and problem statement

The global leaderboard needs a meaningful ranking signal that works across different track variants and ruleset preferences. A simple "fastest time" approach doesn't capture the breadth of a player's skill — someone with several track records is stronger than someone with one.

## Decision drivers

- A single ranking metric that's easy to understand and compare across all players.
- Encourages diversified skill development across multiple tracks.
- Reflects competitive achievement (holding records) rather than one-off lucky runs.

## Considered options

- **Option A:** Rank by fastest single lap time across any track. Simple, but doesn't reflect breadth.
- **Option B:** Rank by most track records held. Rewards consistent competence across the roster.
- **Option C:** Cumulative ranking system (e.g., points per place on each track). Complex to maintain and explain.

## Decision outcome

Chosen: **Option B** — Global leaderboard ranks players by number of track records held across all tracks.

### Positive consequences

- Clear, single-number ranking that's easy to explain and compete for.
- Naturally encourages players to practice multiple tracks.
- Stable metric — moving between positions requires flipping a record, not grinding one session.

### Negative consequences / trade-offs

- Players with one very-fast time but no breadth rank lower than someone with modest records on many tracks. Acceptable: the metric intentionally favors consistency.

## Links

- Source: `ad-hoc`
