---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0007 — Track variants: 150cc only

## Context and problem statement

Mario Kart 8 Deluxe supports multiple engine classes (50cc, 100cc, 150cc, 200cc) and vehicle configurations. Tracking all of them in Beerio Kart's leaderboards and stats multiplies schema complexity. A focused scope on one variant captures the core gameplay without dilution.

## Decision drivers

- 150cc is the most popular competitive variant in Mario Kart communities.
- Simpler leaderboards, fewer per-track records to track, less UI clutter.
- Can expand to other variants later if demand emerges.

## Considered options

- **Option A:** Support all engine classes. Comprehensive, but 4× schema complexity.
- **Option B:** Support 150cc only. Focused, simple, MVP-appropriate.
- **Option C:** Make engine class configurable per-session, but only track 150cc on global leaderboards. Adds UI complexity for unclear benefit.

## Decision outcome

Chosen: **Option B** — 150cc only. The schema and leaderboards assume 150cc throughout.

### Positive consequences

- Schema is clean and uniform; no branching logic per engine class.
- Leaderboards are focused and easier to understand.
- MVP ships faster.

### Negative consequences / trade-offs

- Players who prefer other variants are out of scope until post-MVP expansion. Acceptable: 150cc is the standard competitive class.

## Links

- Source: `ad-hoc`
