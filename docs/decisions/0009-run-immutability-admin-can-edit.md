---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0009 — Run immutability: users cannot edit runs after creation; admin can

## Context and problem statement

Once a race time is submitted, leaderboards and H2H stats depend on its value. Allowing free edits creates audit nightmares (was this score changed after it affected rankings?) and incentive problems (edit after seeing the H2H result). But admins need to correct OCR errors and typos before photos are available.

## Decision drivers

- Preserve leaderboard integrity; no post-hoc edits that change rankings.
- Allow admins to fix obvious errors without deleting and re-creating.
- Clear audit boundary: user creates once, admin can repair.

## Considered options

- **Option A:** Runs are immutable; users delete and re-submit if they made a mistake. Friction, but honest.
- **Option B:** Users can edit anytime. Audit nightmare; unfair to competitors.
- **Option C:** Users can edit within a time window (e.g., 1 hour). Still creates ranking uncertainty.
- **Option D:** Users cannot edit; admins can. Admin-corrected runs are marked as such.

## Decision outcome

Chosen: **Option D** — After a user submits a run, they cannot edit it. Admins can edit runs to correct typos and OCR errors. Whether admin edits surface as a visible flag on the run is a follow-up question (see Negative consequences); the immutability rule itself doesn't depend on it.

### Positive consequences

- Leaderboard integrity: no player can sneak in edits after ranking changes.
- Users learn to double-check before submitting.
- Admins can fix real errors without deleting data.

### Negative consequences / trade-offs

- Users who spot a typo immediately after submission have to ask an admin to fix it. Acceptable: creates accountability and keeps the flow simple.
- Admin edits aren't currently distinguishable from user-created runs in the schema. If "this score was changed by an admin" needs to be visible to users (or auditable post-hoc), `run_flags` (ADR 0029) would gain an `admin_edit` reason or `runs` would gain an `admin_modified_at` column. Open follow-up; not blocking the immutability rule itself.

## Links

- Source: `ad-hoc`
