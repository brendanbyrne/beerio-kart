---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0010 — Head-to-head: derived from session races, not stored

## Context and problem statement

Beerio Kart wants to surface "Player A is 7-3 against Player B" head-to-head records. The natural design instinct is a `head_to_head_records` table updated on each comparable race. We chose against that — H2H is **derived** from the session schema instead.

The earlier draft used **timestamp clustering**: runs submitted within ~10 minutes of each other on the same track were treated as the same race. That heuristic has two failure modes — it over-counts (two players who happened to race the same track around the same time) and under-counts (a network blip pushes a submission past the window). Once `session_races` exists, "two players ran the same race" stops being fuzzy: they share a `session_race_id`. The H2H signal falls out of the session schema for free.

Two related questions get folded in here because they're tightly coupled to *how* H2H is computed: tie handling, and whether drink category splits H2H counts.

## Decision drivers

- The schema already encodes "these two players ran the same race" via `session_race_id`. A separate H2H table duplicates that signal.
- Avoid the timestamp-clustering heuristic and its false-positive / false-negative failure modes.
- Schema simplicity: no new table, no maintenance code keeping H2H counts consistent with the underlying runs.
- **Unbounded rivals.** H2H is computed against any other player; we don't pay a per-pair storage cost. A friend group of arbitrary size doesn't change the schema.

## Considered options

- **Option A:** A `head_to_head_records` table updated on race completion. Read-fast, write-coupled, easy to drift from `runs` truth.
- **Option B:** Timestamp-clustering derivation — group runs within ~10 minutes on the same track as one "race." Stateless, but heuristic-driven.
- **Option C:** Derive from `session_races` — two players have an H2H entry on a given session race iff both submitted non-DQ'd runs for that race. Exact-match anchor, no separate table.

## Decision outcome

Chosen: **Option C** — H2H is derived. For any two players, walk the `session_races` they both have non-DQ'd runs on; tally wins / losses / draws by comparing `track_time`.

**Tie handling.** Identical times → 0-0 draw. Neither player gets a win or a loss. Ties are rare enough that a dedicated "draws" counter isn't worth surfacing — the absence of a win or loss is itself the signal.

**Drink category.** H2H does *not* split by alcoholic vs non-alcoholic. A drinker and a non-drinker in the same session race have their result counted normally. Drink category matters for *leaderboards* (separate alcoholic / non-alcoholic / combined views per ADR 0006) but not H2H — H2H is "did you beat them in a real race," and the answer doesn't depend on what either player was drinking.

### Positive consequences

- No `head_to_head_records` table; no maintenance code keeping it synced.
- H2H counts can never drift from underlying run data — they are a SQL query over `runs` joined on `session_races`.
- Replaces the timestamp-clustering heuristic outright; the failure modes don't apply.
- Drink category doesn't combinatorially split H2H counts, keeping the UI surface small.
- Unbounded by the number of opponents — no schema impact when the player pool grows.

### Negative consequences / trade-offs

- **Read-time computation.** For players with many shared races, the H2H query touches more rows than a stored aggregate would. Mitigations available if/when needed: `session_race_id` is already indexed; a materialized cache (per-pair or per-player) can land later without changing the source of truth.
- **DQ'd runs are excluded** (per ADR 0012). Intentional — a DQ doesn't count as a loss — but worth flagging so reviewers don't get surprised by missing entries.

## Links

- Source: `ad-hoc`
- Related ADRs: [0006 — separate leaderboards by drink category](0006-beer-vs-water-separate-leaderboards.md), [0011 — sessions replace standalone runs](0011-sessions-replace-standalone-runs.md), [0012 — DQ'd runs recorded but excluded](0012-dqd-runs-recorded-but-excluded.md)
