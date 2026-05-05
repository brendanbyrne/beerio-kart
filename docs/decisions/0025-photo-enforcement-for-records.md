---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0025 — Photo enforcement for record-breaking runs (auto-flag + auto-resolve)

## Context and problem statement

Beerio Kart's leaderboards are honor-system: users self-report their times. That works fine for everyday runs — there's social pressure within a session and no real incentive to fudge a mediocre lap. The problem starts at the top of the leaderboard: a fabricated record-breaking time has outsized impact (it bumps the legitimate holder, anchors comparisons, and is hard to dislodge once stale).

A photo of the in-game results screen is the cheap proof. The question is *how* to enforce it: hard block at submission, manual admin review, or some softer pattern that doesn't require admin intervention on every record but still keeps unverified records out of the leaderboard surface.

## Decision drivers

- Records have outsized impact on the leaderboard surface; everyday runs do not. Enforcement should match the asymmetry.
- Hard-block at submission feels punitive — players in the middle of a session shouldn't have to pull out their phone, take a photo, transfer it, and upload it just to record a time. Half the time the photo arrives a minute later from someone else.
- Admin manual review doesn't scale and creates a queue. The whole point of MVP automation is no human in the loop for the common case.
- The system already has a `run_flags` table (ADR 0029) that's designed exactly for this shape: auto-applied conditions that need resolution.

## Considered options

- **Option A:** Trust everyone. No photo requirement.
- **Option B:** Hard block — record-breaking submissions without photo are rejected. Forces synchronous photo upload at the worst time.
- **Option C:** Manual admin review for all record-breaking runs. Creates a backlog; admin becomes a bottleneck.
- **Option D (chosen):** Auto-flag and hide record-breaking runs that lack a photo. Photo upload auto-resolves the flag and the run becomes visible. Soft enforcement; no admin in the loop unless the flag stays unresolved.

## Decision outcome

Chosen: **Option D** — automatic, asymmetric, self-resolving.

**Mechanism.** When a run is submitted that would beat any existing record (track, cup, or global), the run service checks for an attached photo. If absent, a `run_flags` row is inserted with a `record_without_photo` reason. While that flag is unresolved, the run is excluded from leaderboard queries — the record holder is whoever was there before.

**Resolution.** Uploading a photo against the same run auto-resolves the flag. The run becomes visible and the leaderboard updates on the next read. No admin action required for the common case.

**Why this works.** The asymmetry is the win: 95% of submissions are not record-breaking and pass through with no friction. The 5% that are record-breaking are exactly the ones worth a photo. And the user who actually beat the record has the strongest incentive to upload the photo — they want the credit.

### Positive consequences

- No friction on the common case (most submissions aren't records).
- Strong incentive on the rare case (the record-holder *wants* the leaderboard to update).
- No admin queue for routine records — admin only intervenes on disputed flags.
- Composes cleanly with the `run_flags` audit trail (ADR 0029) — records stay visible after resolution.

### Negative consequences / trade-offs

- Slight delay between record submission and leaderboard update if the photo lags. Acceptable: the run is preserved; the user just has to upload to claim the spot.
- Honor-system loophole: someone could upload an unrelated photo to resolve the flag. OCR work in a future phase tightens this; for MVP, the photo is enough of a deterrent because users see each other's submissions.
- Adds complexity vs. trust-everyone. Acceptable: the leaderboard's value depends on records being credible.

## Links

- Source: `ad-hoc`
- Related ADRs: [0020 (photo upload validation — server-side magic-byte checks)](0020-photo-upload-validation.md), [0029 (run_flags audit trail)](0029-run-flags-audit-trail.md), [0019 (admin defense in depth)](0019-admin-defense-in-depth.md)
