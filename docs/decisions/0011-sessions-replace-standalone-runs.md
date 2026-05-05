---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0011 — Sessions replace standalone run recording

## Context and problem statement

Early designs allowed players to log individual race times outside of a session context — "I ran a solo time trial, here's my split." This adds two separate code paths (solo runs vs. session runs) and complicates leaderboards and H2H tracking. A unified model where all runs happen within sessions is simpler to reason about.

## Decision drivers

- One run-ingestion path: all runs are children of a session.
- Simpler service logic — no branching for solo vs. multiplayer runs.
- All runs have a session context for potential future social features (replay, commentary).

## Considered options

- **Option A:** Standalone runs are their own entity; sessions are separate. Flexibility, complexity.
- **Option B:** All runs must belong to a session; solo running uses a one-person session. Simpler model.
- **Option C:** Nullable `session_race_id` allows both; app prefers sessions. Future-compatible but MVP doesn't use it.

## Decision outcome

Chosen: **Option B** — All runs are recorded within session context. Solo racing uses a one-person session. Option C (nullable session_race_id) is deferred post-MVP for lightweight standalone runs if that use case emerges.

### Positive consequences

- Single run-ingestion path; no solo/multiplayer branching.
- Runs naturally have context (who else was racing, when, on what track).
- Simpler leaderboard and H2H queries.

### Negative consequences / trade-offs

- Creating a session for a solo time trial has ceremony (create session, add self, start race). Acceptable for MVP; single-button "quick run" mode can be added if friction is real.

## Links

- Source: `ad-hoc`
