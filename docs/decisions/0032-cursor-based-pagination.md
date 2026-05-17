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

## Implementation status

Not yet implemented in any list endpoint as of 2026-05-17. Both `GET /runs` ([`services/runs/read.rs`](../../backend/src/services/runs/read.rs)) and `GET /me/notifications` ([`services/notifications.rs`](../../backend/src/services/notifications.rs)) currently use a flat `LIMIT 100` cap — the ADR-sanctioned substitute (see Negative consequences). Future history endpoints under `/stats/personal/...` should be built keyset-native rather than retrofitted.

Project-wide rollout is tracked in [#166](https://github.com/brendanbyrne/beerio-kart/issues/166).

## Links

- Source: `ad-hoc`
