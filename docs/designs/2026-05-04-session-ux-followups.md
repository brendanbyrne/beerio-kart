# Session UX follow-ups — design record (2026-05-04)

Date: 2026-05-04
Author: Cowork
Source: distilled from `reviews/design/2026-04-04-session-screen-mockup-critique.md` per the migration triage of design record `archive/2026-05-04-design-doc-restructure.md` (archived 2026-05-15).

## Status and how to use this record

This consolidates UX critique items raised during the April 2026 mockup review of the session screen. Phase 3C-1 and 3C-2 are merged; some critique items may already be addressed in shipped code.

**Each item below needs verification against the current frontend implementation before sign-off.** For each section:

- If shipped code already meets the recommendation → check **Approved — already shipped** with a short note pointing at the relevant file(s) / commit(s).
- If the recommendation is genuine open work → check **Approved — open Issue(s)** so it gets tracked on GitHub.
- If discussion is needed first → check **Needs discussion**.

The original critique was structured as a side-by-side analysis of four mockup states (`reviews/design/session-screen-mockup.jsx`); the mockup itself migrates to `docs/designs/assets/session-screen-mockup/` per the same triage and remains usable as a reference.

## 1. Touch targets to 48px minimum

**Original critique:** Most interactive elements measured 28–40px tall in the mockup, below Material Design's 48px minimum. The app's "usable by only one hand, which could be wet" principle wants targets that are *generous*, not just adequate.

**Specific items from the critique:**


- Primary CTAs ("Submit Time", "Submit Run") → `py-4` (≈52px).
- Pending race Submit/Skip → 48px+, full-width Submit, secondary Skip beside it.
- Participant expand button → `py-2` minimum (≈40px).
- DQ toggle → make full row tappable; toggle track ≥48×28px.
- Race history toggle → `py-3.5` for 48px target.

**Verification:** inspect the shipped session screen and run-entry sheet code; for each item, confirm sizing meets or exceeds the recommendation.

- [ ] Approved — already shipped (note which items, with file references)
- [ ] Approved — open Issue(s) for missing items
- [ ] Needs discussion

## 2. Pending-race flow leads with the pending track

**Original critique:** When a player has pending races, the current track card visually dominates while the user's actual next action is the *pending* track. A player glancing at the screen sees the current track's image and might submit their pending time thinking it's for the wrong track. Violates "minimize choices in any moment."

**Recommendation:** When pending races exist, swap the track card to show the pending track with an amber "Pending from Race N" badge; push the current track below in a compact row. After submitting the pending, advance to the next pending or to the current track.

Specific sub-items from the critique:
- 2.1 Track identity confusion (above).
- 2.2 The RunEntrySheet should display the pending track's info in its header when opened from the pending state (not the current track's info).
- 2.3 Multiple pending races aren't visualized — show a "1 of N pending" indicator and auto-advance after submission.
- 2.4 "Skip" needs lightweight confirmation ("Skip Mario Circuit? No time will be recorded.") — accidental tap loses a race.

**Verification:** exercise the pending-races flow in the shipped app (will need a session with the user marked as pending on a previous race). Confirm: is the track shown the one the player is submitting for? Does the RunEntrySheet header match? Is there confirmation on Skip?

- [ ] Approved — already shipped
- [ ] Approved — open Issue(s) for missing parts
- [ ] Needs discussion

## 3. Host-action vs. player-action button differentiation

**Original critique:** "Next Track" affects everyone and is irreversible. "Submit Time" is personal and reversible (opens a sheet). The mockup gave them identical styling, suggesting identical consequence. A host tapping "Next Track" by accident advances the whole session.

**Recommendation:** Give host-action buttons a distinct style — outline variant, secondary color (indigo or other), or an icon-only floating action. Consider tap-and-confirm or long-press for "Next Track" specifically.

**Verification:** inspect the shipped session screen. Do "Next Track" and "Submit Time" look visually different? Is there a confirmation step on "Next Track"?

- [ ] Approved — already shipped
- [ ] Approved — open Issue
- [ ] Needs discussion

## 4. Tailwind arbitrary-value classes + safe-area padding

**Original critique:** `h-5.5` and `w-4.5` are not valid Tailwind classes — they silently fail. The bottom sheet had no safe-area padding, so the gesture bar overlapped the submit button on Pixel 9 Pro.

**Recommendation:**
- Replace `h-5.5` → `h-[22px]`, `w-4.5` → `w-[18px]` (or wherever similar invalid classes appear).
- Add `pb-[env(safe-area-inset-bottom)]` or a conservative `pb-8` to the bottom sheet interior.

**Verification:** `rg "h-5\\.5|w-4\\.5"` across `frontend/src/`. If matches: open Issue. Test on Pixel 9 Pro: does the bottom sheet's submit button clear the gesture bar?

- [ ] Approved — already shipped (or never had the bug)
- [ ] Approved — open Issue
- [ ] Needs discussion

## 5. Sticky session header

**Original critique:** Spec says sticky top, ~60px. In the mockup the header scrolled away, losing race-number and participant context.

**Recommendation:** `sticky top-0 z-10 bg-white` on the SessionHeader wrapper.

**Verification:** scroll a long session screen and confirm the header stays pinned.

- [ ] Approved — already shipped
- [ ] Approved — open Issue
- [ ] Needs discussion

## 6. Open questions from the original critique

Three smaller questions raised in the critique — design choices, not yet decisions:

- **6.1 Leave Session confirmation.** Should "Leave Session" require a confirmation bottom sheet ("Your pending races will be saved for 5 minutes")? The 5-minute grace period message in the spec implied a confirmation step. — *Note (2026-05-16):* The 5-minute grace concept was removed by [ADR-0035](../decisions/0035-race-anchored-session-lifetime.md); the underlying UX question (whether Leave Session needs a confirmation) is still live, but the proposed copy is stale — see `user-workflows.md` § 1.5 for the current "races expire 1 hour after they start" framing.
- **6.2 Non-host version of "Waiting for Track".** Frame 4 of the original mockup showed the host view (with "Pick Next Track" button). A non-host sees the dashed empty card with no button — worth a separate empty-state design?
- **6.3 Race history default-open after 3+ races.** Auto-expanding once history is useful might save a tap.

These are smaller and more design-question than implementation-task. Worth folding into Milestone Star follow-up Issues if relevant; otherwise deferrable.

- [ ] Approved — defer all three
- [ ] Approved — pull specific ones into 3D or 3E (note which)
- [ ] Needs discussion

## Sign-off summary

When all sections are signed off, the source critique `2026-04-04-session-screen-mockup-critique.md` is deleted (per triage). Issues created from this record are tracked separately on GitHub (link them here once known).

ADRs produced: TBD
Issues produced: TBD

- [ ] All sections approved — clear to delete the source critique

## Document history

- 2026-05-05 — Created in `docs/designs/` by distilling the 2026-05-04 `reviews/design/2026-04-04-session-screen-mockup-critique.md` per the migration triage in `docs/designs/archive/2026-05-04-design-doc-restructure.md` (archived 2026-05-15). PR #41.
- 2026-05-15 — Updated the Source line and the 2026-05-05 history entry's path reference for the design-doc-restructure record (now archived under `designs/archive/`). Companion to PR [#160](https://github.com/brendanbyrne/beerio-kart/pull/160) / Issue [#159](https://github.com/brendanbyrne/beerio-kart/issues/159).
