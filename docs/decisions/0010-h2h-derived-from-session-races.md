---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0010 — Head-to-head: derived from session races, not stored

## Context and problem statement

Beerio Kart wants to surface "Player A is 7-3 against Player B" head-to-head records. The natural design instinct is a `head_to_head_records` table, updated whenever a comparable race finishes. That instinct collides with two observations:

1. **What counts as a "comparable race" is fuzzy.** An earlier draft used timestamp clustering — runs submitted within ~10 minutes of each other on the same track were treated as the same race. That clustering is a heuristic; it both over-counts (two players who happened to race the same track around the same time but weren't in the same session) and under-counts (network blip pushes one submission past the window).
2. **Sessions already give us the exact-match anchor.** Once `session_races` exists, "two players ran the same race" is no longer fuzzy — they share a `session_race_id`. The H2H signal falls out of the session schema for free.

A second design question — what to do about ties, and whether drink category (alcoholic vs non-alcoholic) splits H2H counts — gets folded into the same ADR because the answers are tightly coupled to how H2H is computed.

## Decision drivers

- The schema already tells us "these two players ran the same race" via `session_race_id`. A separate H2H table would duplicate that signal.
- Avoid the timestamp-clustering heuristic and its false-positive / false-negative failure modes.
- Schema simplicity: no new table, no maintenance code to keep H2H counts consistent with the underlying runs.
- Honesty about edge cases: ties happen, and the drink-category split (which matters for leaderboards) doesn't naturally apply to H2H.

## Considered options

- **Option A:** `head_to_head_records` table updated on race completion. Read-fast, write-coupled, easy to drift from `runs` truth.
- **Option B:** Timestamp-clustering derivation — group runs within ~10 minutes on the same track as one "race." (Earlier draft.) Stateless, but heuristic-driven.
- **Option C:** Derive from `session_races` — two players have an H2H record on a given session race iff both submitted non-DQ'd runs for that race. (Chosen.) Exact-match anchor, no separate table.

## Decision outcome

Chosen: **Option C** — H2H is derived. For any two players, walk the `session_races` they both have non-DQ'd runs on; tally wins/losses/draws by comparing their times.

**Tie handling.** Identical times → 0-0 draw. Neither player gets a win or a loss. Ties are rare enough that a special "tie" counter isn't worth surfacing; the absence of a win/loss is itself the signal.

**Drink category.** H2H does not split by alcoholic vs non-alcoholic. A drinker and a non-drinker in the same session race have their result counted normally. Drink category matters for leaderboards (separate alcoholic / non-alcoholic / combined views) but not for H2H — H2H is "did you beat them in a real race," and the answer doesn't depend on what they were drinking.

### Positive consequences

- No `head_to_head_records` table, no maintenance code keeping it in sync.
- H2H counts can never drift from underlying run data — they are a SQL query over `runs` joined on `session_races`.
- Replaces the timestamp-clustering heuristic outright; the failure modes don't apply.
- Drink category doesn't combinatorially split H2H counts, keeping the UI surface small.

### Negative consequences / trade-offs

- Read-time computation. For players with many shared races, the H2H query touches more rows than a stored aggregate. Mitigation: `session_race_id` is indexed. Revisit if a real query plan shows the cost.
- DQ'd runs (per ADR 0012) are excluded from H2H — this is intentional but worth flagging: a DQ doesn't count as a loss.

## Links

- Source: `ad-hoc`
- Related ADRs: [0011 (sessions replace standalone runs)](0011-sessions-replace-standalone-runs.md), [0012 (DQ'd runs excluded)](0012-dqd-runs-recorded-but-excluded.md), [0006 (beer vs water leaderboards split — drink category context)](0006-beer-vs-water-separate-leaderboards.md)
