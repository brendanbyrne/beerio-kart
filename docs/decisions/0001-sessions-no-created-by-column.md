---
status: accepted
date: 2026-05-02
deciders: [Brendan]
source: docs/designs/archive/2026-05-02-sessions-created-by-removal.md
---

# 0001 — Sessions: no `created_by` column

## Context and problem statement

The `sessions` table originally had two FKs to `users`: `created_by` (the original creator) and `host_id` (the current host, which transfers on leave). While drafting the multi-FK relations rule (`coding-standards/seaorm.md` § 11), the redundancy surfaced: does any product feature actually use the original-creator fact?

Inspection found nothing. The UI surfaces only the current host (🏠 icon). Host-transfer logic reads `host_id` and the participant list — never `created_by`. No leaderboard, stat, or session detail screen exposes the original creator.

## Decision drivers

- Avoid a live instance of the multi-FK ambiguity case (sea-orm #405) when no feature needs the data.
- Keep the schema minimal during prelaunch; reversibility is cheap (one append-only migration if a future feature needs it).
- Eliminate an unused FK and the associated index/constraint surface.

## Considered options

- **Option A:** Keep both `created_by` and `host_id`. Status quo.
- **Option B:** Drop `created_by`; `host_id` starts as the creator and transfers normally.

## Decision outcome

Chosen: **Option B** — `host_id` carries the creator identity until host transfers on leave; no separate column.

### Positive consequences

- Schema is one column and one FK lighter; no live multi-FK case in current tables.
- Session-creation logic simplifies (sets `host_id` only).
- No external API impact — `created_by` was never exposed.

### Negative consequences / trade-offs

- After the first host transfer, the original creator's identity is lost. Acceptable: no feature needs it; reversible if requirements change.

## Links

- Source: [`docs/designs/archive/2026-05-02-sessions-created-by-removal.md`](../designs/archive/2026-05-02-sessions-created-by-removal.md)
- Implementing PRs: PR-E3 (`docs/designs/archive/compliance-plan.md` Stream E)
