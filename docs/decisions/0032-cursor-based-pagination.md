---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0032 — Pagination: cursor-based (keyset) on `created_at` + `id`

## Context and problem statement

List endpoints like `GET /runs` and run history views need pagination to avoid loading all data at once. Two strategies exist: offset-based (skip N, take M) and cursor-based (keyset pagination using a position marker). Cursor-based avoids duplicate/skipped entries when data is inserted during browsing.

## Decision drivers

- Avoid duplicates or skipped entries if new data is inserted while a user is paginating.
- Cursor-based is stable across inserts; offset-based is not.
- Standard pattern in modern APIs.

## Considered options

- **Option A:** Offset-based pagination. Simple to implement; buggy if new data arrives mid-browse.
- **Option B:** Cursor-based keyset pagination. Stable across inserts; slightly more complex.
- **Option C:** No pagination; load all data. Scalability nightmare.

## Decision outcome

Chosen: **Option B** — Cursor-based pagination using `created_at` + `id` (compound key ensures uniqueness). Pagination is implemented for `GET /runs` and run history views. If complexity proves unmanageable relative to offset-based, can revisit post-MVP.

### Positive consequences

- Stable browsing experience; no skipped or duplicate entries if new data arrives.
- Works at scale without offset overhead.

### Negative consequences / trade-offs

- Slightly more complex to implement and explain. Acceptable: if the implementation burden is real, offset-based can be substituted later with a note about the tradeoff.

## Links

- Source: `ad-hoc`
