---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0014 — Session host transfer: earliest-joined remaining participant becomes new host

## Context and problem statement

The session host chooses tracks for the next race. If the host leaves the session (closes the app, navigates away), someone needs to take over hosting duties. The handoff should be automatic and predictable.

## Decision drivers

- Host duties need to transfer immediately when the host leaves.
- Deterministic rule so all clients agree without a broadcast.
- Fair: earliest-joined (longest commitment to the group) gets the role.

## Considered options

- **Option A:** Newest participant becomes host. Favors latecomers, feels arbitrary.
- **Option B:** Earliest-joined remaining participant becomes new host. Rewards continuity; deterministic.
- **Option C:** Session ends if host leaves. Harsh; forces session recreation.

## Decision outcome

Chosen: **Option B** — When the host leaves, the earliest-joined remaining participant automatically becomes the new host.

### Positive consequences

- Transfer is automatic; no one needs to nominate or agree.
- Deterministic: both client and server compute the same answer (timestamps are canonical).
- Feels fair in small groups.

### Negative consequences / trade-offs

- If the longest-committed participant doesn't want hosting duties, they can't decline gracefully. Acceptable: hosting is lightweight (choose a track), and in small groups, rotation happens naturally.

## Links

- Source: `ad-hoc`
