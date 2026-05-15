---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: docs/designs/archive/2026-04-19-pending-races-and-grace-period.md
---

# 0013 — Pending race cap: UI shows max 3 pending races

## Context and problem statement

When a session host chooses a track for a race, that race becomes "pending" — waiting for times to be entered. If the host is distracted or forgets, pending races accumulate indefinitely. The system needs to prevent logjam without being tyrannical about timing.

## Decision drivers

- Prevent accumulated dead races from cluttering sessions.
- Give players a grace period to enter times before expiring.
- UI should limit the visible backlog to encourage prompt completion.

## Considered options

- **Option A:** Unlimited pending races. No enforcement; host responsible.
- **Option B:** Limit pending races at UI level (cap display). Schema allows unlimited; guardrail is soft.
- **Option C:** Hard limit in schema; reject new races if limit is reached. Strict, but frustrating if someone falls behind.

## Decision outcome

Chosen: **Option B** — UI shows max 3 pending races (oldest expire first). Schema supports unlimited — the cap is a UX guardrail, adjustable later if experience suggests a different number.

### Positive consequences

- UX doesn't overwhelm; players see the oldest pending races, encouraging completion.
- Flexible tuning: adjusting "3" to "4" or "2" is a one-line change.
- Schema isn't restricted unnecessarily.

### Negative consequences / trade-offs

- If a player is far behind and there are 3+ pending races, they can't see the newest one in the current view. Workaround: host can clear old races or players can ask to see them.

## Links

- Source: [`docs/designs/archive/2026-04-19-pending-races-and-grace-period.md`](../designs/archive/2026-04-19-pending-races-and-grace-period.md)
