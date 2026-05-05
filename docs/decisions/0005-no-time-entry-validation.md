---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0005 — Time entry: no validation against plausible track times

## Context and problem statement

When a player submits a race time, the system could validate it against known physics constraints (fastest real 150cc times, lap speed bounds) to catch typos or fraud. Adding that validation complicates the ingestion path and requires maintaining a reference database of plausibility bounds.

## Decision drivers

- Simplify time-entry logic in MVP.
- Rely on downstream controls (photos, eventual OCR) to catch errors and fraud.
- Players are incentivized to be honest (leaderboard reputation) in small, social groups.

## Considered options

- **Option A:** Validate entered times against a plausibility range. Catches typos, but requires maintaining bounds.
- **Option B:** Accept times as-is; rely on photos and later OCR to verify.
- **Option C:** Require photo upload before accepting any time. More friction, but guaranteed evidence.

## Decision outcome

Chosen: **Option B** — No server-side validation of time plausibility. Photos are the audit trail; OCR will eventually automate verification.

### Positive consequences

- Minimal ingestion logic; times accepted immediately.
- Photo is the source of truth; typos are caught when photos are reviewed.
- No need to maintain a database of reference bounds.

### Negative consequences / trade-offs

- Obviously-wrong times (e.g., 1:05 for 150cc) can linger until photos are reviewed. Acceptable: photos are fast turnaround, and social pressure is strong in small groups.

## Links

- Source: `ad-hoc`
