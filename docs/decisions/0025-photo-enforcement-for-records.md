---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0025 — Photo enforcement for record-breaking runs (auto-flag + auto-resolve)

## Context and problem statement

Beerio Kart's leaderboards are honor-system: users self-report their times. That works fine for everyday runs — there's social pressure inside a session and no strong incentive to fudge a mediocre lap. The problem is at the top of the leaderboard. A fabricated record-breaking time has outsized impact: it bumps the legitimate holder, anchors comparisons going forward, and is hard to dislodge once stale.

A photo of the in-game results screen is the cheap proof. The question is *how* to enforce it: a hard block at submission, manual admin review, or a softer pattern that doesn't require admin intervention on every record while still keeping unverified records out of the leaderboard surface.

## Decision drivers

- **Asymmetry.** Records have outsized impact on the leaderboard; everyday runs do not. Enforcement should match the asymmetry.
- **Submission ergonomics.** A hard block at submission is punitive — the photo often arrives a minute late from someone else's phone. Forcing synchronous upload at the worst possible moment ruins the flow.
- **No human in the loop for the common case.** Manual admin review doesn't scale and grows a queue monotonically. The whole point of MVP automation is keeping admin out of the routine path.
- **Existing primitive.** The schema already has `run_flags` (ADR 0029) — auto-applied conditions that need resolution. This decision is exactly its shape.
- **OCR future.** Photos serve double duty: enforcement now, OCR training data later. Encouraging photos broadly (even when not strictly required) feeds that future.

## Considered options

- **Option A:** Trust everyone. No photo requirement.
- **Option B:** Hard block. Record-breaking submissions without a photo are rejected. Forces synchronous photo upload at the worst time.
- **Option C:** Manual admin review for all record-breaking runs. Creates a backlog; admin becomes a bottleneck.
- **Option D (chosen):** Auto-flag and hide record-breaking runs that lack a photo. Photo upload auto-resolves the flag and the run becomes visible. Soft enforcement; no admin in the loop unless the flag stays unresolved.

## Decision outcome

Chosen: **Option D** — automatic, asymmetric, self-resolving.

**Mechanism.** When a run is submitted that would beat any existing record (track, cup, or global), the run service checks for an attached photo. If none, a `run_flags` row is inserted with reason `record_without_photo`. While that flag is unresolved, the run is excluded from leaderboard queries — the previous record-holder remains the surfaced one.

**Resolution.** Uploading a photo against the same run auto-resolves the flag. The run becomes visible and the leaderboard updates on the next read. No admin action required for the common case.

**DQ'd runs are excluded** from record-breaking eligibility per ADR 0012. A DQ can't be a record, so there's nothing to flag.

**Why this works.** The submitter who actually beat the record has the strongest incentive to upload — they want the credit. The vast majority of submissions aren't record-breaking and pass through with no friction. The rare case that *is* record-breaking is precisely the case worth a photo.

### Positive consequences

- No friction on the common case (most submissions aren't records).
- Strong incentive on the rare case (the record-holder *wants* the leaderboard to update).
- No admin queue for routine records — admin only intervenes when a flag stays unresolved.
- Composes cleanly with the `run_flags` audit trail (ADR 0029); records stay visible in history after resolution.

### Negative consequences / trade-offs

- **Unrelated-photo loophole.** A user can upload any image — a screenshot of someone else's race, a stock photo, a JPEG of nothing — and the flag auto-resolves. Magic-byte validation (ADR 0020) checks file *type*, not *content*. The MVP mitigation is social: in a friend-group app, fakes get called out. The durable mitigation is OCR (planned post-MVP) — extract the time and player name from the photo and verify they match the submission. Until OCR ships, this is a real attack on leaderboard credibility, not a hypothetical one.
- **Slight delay** between record submission and leaderboard update if the photo lags. Acceptable: the run is preserved; the user just has to upload to claim the spot.
- **Implementation complexity** vs. trust-everyone. Acceptable: the leaderboard's value depends on records being credible.

## Links

- Source: `ad-hoc`
- Related ADRs: [0012 — DQ'd runs excluded from records and leaderboards](0012-dqd-runs-recorded-but-excluded.md), [0019 — admin defense in depth](0019-admin-defense-in-depth.md), [0020 — photo upload validation (server-side magic-byte checks)](0020-photo-upload-validation.md), [0029 — run_flags audit trail](0029-run-flags-audit-trail.md)
