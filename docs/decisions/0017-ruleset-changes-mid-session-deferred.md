---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0017 — Ruleset changes mid-session: deferred post-MVP

## Context and problem statement

A session's ruleset (which determines how the next track is chosen) is set at creation time. Players might want to switch rulesets mid-session (e.g., "let's try Round-robin instead of Random"). Supporting this adds state-synchronization complexity for MVP.

## Decision drivers

- Defer ruleset mutation until the product is stable and MVP ruleset logic is battle-tested.
- MVP sessions run with one ruleset start-to-finish; switching requires creating a new session.
- Future-compatible: the data model already supports it; enabling it is a service-layer change.

## Considered options

- **Option A:** Rulesets are immutable for the session's lifetime. Simple, clear.
- **Option B:** Rulesets can change mid-session if the host requests it. Flexible, but state-sync is complex.

## Decision outcome

Chosen: **Option A** — Rulesets are set at session creation and cannot change. Switching rulesets requires creating a new session. Post-MVP enhancement to allow mid-session changes.

### Positive consequences

- Eliminates state-sync complexity around ruleset changes.
- Clear mental model: one ruleset for the whole session.
- Creating a new session is fast (name it, choose ruleset, invite players).

### Negative consequences / trade-offs

- Users must restart the session to try a different ruleset. Minor friction; can be addressed post-MVP if common.

## Links

- Source: `ad-hoc`
