---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0029 — run_flags: audit trail, no unique constraint on (run_id, reason)

## Context and problem statement

Runs can be flagged for investigation (suspicious time, missing photo, etc.). Each flag has a reason and a resolved status. A single run can have multiple flags if different issues are discovered. The question is whether to allow duplicate reasons on the same run.

## Decision drivers

- A run might be flagged multiple times for the same reason if the issue isn't fully resolved, or if different admins flag it independently.
- Audit trail: all flags (resolved and unresolved) are kept for history.
- Only duplicate flags (same run + same reason while unresolved) need to be prevented to avoid spam.

## Considered options

- **Option A:** Unique constraint on (run_id, reason). Prevents any duplicate; forces deletion of old flags.
- **Option B:** Allow multiple flags per (run_id, reason); no uniqueness constraint. Spam risk if not careful.
- **Option C:** No unique constraint; prevent duplicates in application code only. Simpler schema, explicit logic.

## Decision outcome

Chosen: **Option C** — No unique constraint on (run_id, reason). The application code prevents duplicate flags (same run + same reason while unresolved). Resolved flags are kept as audit history.

### Positive consequences

- Audit trail is complete; resolved flags are never deleted.
- Application code is explicit about which duplicates are forbidden.
- Schema is simpler; constraints don't lock behavior.

### Negative consequences / trade-offs

- Relies on application code to prevent spam. Acceptable: checks are simple and testable.

## Links

- Source: `ad-hoc`
