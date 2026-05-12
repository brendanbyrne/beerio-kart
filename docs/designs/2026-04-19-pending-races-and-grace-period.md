# Design Review — Pending Races & Grace Period (Phase 3 PR 3D)

> **Superseded in part by [ADR-0035](../decisions/0035-race-anchored-session-lifetime.md) (2026-05-11).** § 3 (grace period semantics) and the supporting grace logic in § 1, § 2, and § 8 are no longer in force — race-anchored expiry replaces the 5-minute rejoin grace and the `joined_at`-reset semantics. § 4 (submit-in-order enforcement), § 5 (current-track override), § 6 (API surface), and § 9 (out-of-scope items) are not affected. This file is preserved as the point-in-time record of the original design discussion.

Date: 2026-04-19
Author: Cowork
Scope: detailed design for the "pending races" feature and the 5-minute grace period around session leaves, per DESIGN.md §"Pending Race Tracking" (line 406), §Session Participants (line 274), and Workflow 5 (line 479).

## Purpose

DESIGN.md already has the high-level intent documented. This review turns that intent into implementable decisions: what data is needed, how pending is computed at query time, how the grace period is enforced lazily, and what API/service changes are required. Sign off section-by-section; approved sections become the PR 3D handoff spec.

The implementation principles from DESIGN.md that constrain this design:

- **"Never feel rushed."** No background timers pressuring users; no forced submissions.
- **Lazy checks.** Grace period is computed at query time by comparing timestamps — no Tokio timer that fires at `left_at + 5min`.
- **Derive before storing.** Pending state is derived from `session_participants`, `session_races`, and `runs` wherever possible. Only persist what can't be derived.
- **Inclusive by default.** Non-drinkers must be first-class — no feature below excludes them.

---

## 1. Definition of "pending race"

A session race `SR` is **pending** for user `U` if all of the following hold:

1. **Presence at creation.** `U` was an active participant of `SR.session_id` at time `SR.created_at` — i.e. had a `session_participants` row with `joined_at ≤ SR.created_at AND (left_at IS NULL OR left_at ≥ SR.created_at)`.
2. **No run submitted.** No `runs` row exists with `session_race_id = SR.id AND user_id = U.id`.
3. **Not skipped.** No row in the new `pending_race_skips` table (section 2) for `(SR.id, U.id)`.
4. **Grace period not expired** (section 3).

Ordering: results are returned ordered by `SR.race_number ASC`. UI caps at 3; the API returns all pending races (per DESIGN.md — "UI cap, not schema cap").

### SQL sketch (for illustration; final implementation uses SeaORM builder where possible)

```sql
SELECT sr.*
FROM session_races sr
WHERE sr.session_id = :session_id
  AND EXISTS (
    SELECT 1 FROM session_participants sp
    WHERE sp.session_id = sr.session_id
      AND sp.user_id = :user_id
      AND sp.joined_at <= sr.created_at
      AND (sp.left_at IS NULL OR sp.left_at >= sr.created_at)
  )
  AND NOT EXISTS (
    SELECT 1 FROM runs r
    WHERE r.session_race_id = sr.id AND r.user_id = :user_id
  )
  AND NOT EXISTS (
    SELECT 1 FROM pending_race_skips s
    WHERE s.session_race_id = sr.id AND s.user_id = :user_id
  )
  AND [grace period check — see §3]
ORDER BY sr.race_number ASC;
```

Per DESIGN.md §5.1 ORM convention, this is a multi-table JOIN/EXISTS and belongs in `find_by_statement`. Add to `services/sessions.rs` or a new module — section 6.

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

## 2. Skip mechanism — APPROVED (revised design)

**Resolved 2026-05-01 via discussion.** The originally-proposed `pending_race_skips` table is replaced by a more thorough redesign that simultaneously addresses skip storage, presence-at-race-creation, and the participation history question.

### Approved schema

Two tables change:

```
session_participants — simplified, mutable, one row per (session, user)
├── id, session_id, user_id
├── joined_at: DATETIME — time of CURRENT presence segment
└── left_at: DATETIME (nullable — NULL while present)

session_race_participations — NEW, captures per-race presence + skip
├── session_race_id (FK → session_races, not null)
├── user_id (FK → users, not null)
├── created_at (not null)
└── skipped_at: DATETIME (nullable — NULL while still pending)

PRIMARY KEY (session_race_id, user_id)
Index: (user_id) for "what's pending for me" queries.
```

### Behavior

- **At race creation** (in the same transaction as the `session_races` INSERT): for every user with `session_participants.left_at IS NULL` in this session, INSERT one `session_race_participations` row.
- **At rejoin within 5 min**: UPDATE the user's `session_participants` row, setting `left_at = NULL`. Leave `joined_at` alone.
- **At rejoin after 5 min**: UPDATE the user's `session_participants` row, setting `joined_at = NOW(), left_at = NULL`. (Optional reset; see §3.)
- **At skip**: `UPDATE session_race_participations SET skipped_at = NOW() WHERE session_race_id=? AND user_id=? AND skipped_at IS NULL`. Idempotent via the PK.

### Why this design (vs. the original `pending_race_skips` proposal)

- **No participation history.** session_participants is one mutable row per (session, user). Replaces multi-row left/joined history. Audit history not used by anything in MVP.
- **Presence-at-race-creation is captured at write time**, not derived from history. The row in `session_race_participations` IS the proof that the user was present.
- **Skip lives where presence does** — single table, single row per (race, user). Idempotent via PK constraint.
- **Bounded by race count × users-present-at-creation** per session. Doesn't grow with leave/rejoin events.
- **No JSON columns.** Each fact is its own normal row.

### Tradeoffs accepted

- Lose the audit trail of every join/leave event. Not used in MVP.
- §3 (grace semantics) becomes simpler — see that section for the cascade.

### Migration

Per the new prelaunch schema convention (CLAUDE.md), this is a single edit to the consolidated initial migration: change the `session_participants` table to drop the multi-row design, add the `session_race_participations` table.

- [x] Approved (revised design) — minimal `session_participants` + new `session_race_participations` table

---

## 3. Grace period semantics — SUPERSEDED by §2 redesign

**Resolved 2026-05-01.** The original Option A (gap-based history walk) required participation history that no longer exists under the §2 redesign. The new semantics are simpler and more closely match the spirit of "currently within grace."

### Approved semantics

A pending race is **accessible** (still actionable by the user) iff:

```
session_participants.left_at IS NULL              -- currently in session
  OR
(NOW() - session_participants.left_at) <= 5 min   -- left within grace window
```

Otherwise the row in `session_race_participations` exists but the user has no UI affordance to act on it (it's effectively forfeited until/unless they rejoin within grace).

### What this means in practice

- **Currently in session, never left:** all pending races are accessible.
- **Just left (4 min ago):** pending races still accessible — the UI may even still show them if the leave/rejoin flow lets the user see their pending list before fully exiting (UI call).
- **Left 6 min ago:** pending races inaccessible. Records still exist in `session_race_participations` for history.
- **Left, rejoined within 5 min:** `left_at` set back to NULL, all pending races accessible again.
- **Left, rejoined after 5 min:** `left_at` set back to NULL, `joined_at` reset to NOW(). Pending records from before the long gap still exist in the table — the question is whether they should be accessible. **Recommendation: no.** When `joined_at` advances on a long-gap rejoin, the prior pending races are considered forfeited. Implementation: filter pending to `session_race_participations.created_at >= session_participants.joined_at`.

### Edge cases

- **Multiple short flaps in succession** (left, back, left, back): each flap individually within grace. Pending stays accessible throughout. Simple — no aggregate "total time away" tracking needed.
- **Long gap then short flap:** long-gap rejoin resets `joined_at`. Pre-gap pending is forfeited. Subsequent short flaps don't restore it (they don't reset `joined_at`).

### Lazy check stays intact

Per DESIGN.md "no background timer needed" — the predicate above is computed at query time (every read), not by a sweeper task. The `session_race_participations` row stays in the DB regardless of accessibility.

- [x] Approved — superseded by §2 redesign; new semantics as above

---

## 4. Submit-in-order enforcement in `create_run`

DESIGN.md: "Submitting out of order is not allowed — this prevents cherry-picking favorable tracks to game H2H records."

Add a check in `services/runs.rs::create_run`: before accepting a submission for `session_race_id = SR`, verify that `U` has no **older** pending race (i.e., no pending race with `race_number < SR.race_number` in the same session).

**Error shape:** `AppError::Conflict("Must submit or skip pending race #{n} first")`. Include the oldest pending race number in the message so the client can surface it.

**Question: does the "skip" endpoint bypass this check?**

- If user calls `POST .../races/:race_id/skip` for race 2, and has pending races 1 and 2, should the skip succeed?
- **Recommendation: no.** Skipping out of order is the same category of cherry-pick problem. Enforce ordering on both submit and skip. The UI presents pending races in order anyway; out-of-order is an API-level guarantee, not a UI convenience.

Alternative: allow out-of-order skip (skip anything at any time) but enforce ordering on submit. This is slightly more permissive and simpler to implement. I don't think it opens cherry-picking because skipping doesn't help your H2H — you just don't record a time. Lean in favor of **allowing out-of-order skip** for simplicity; reject only out-of-order submit.

- [x] Enforce ordering on submit only; skip is free

### 4.1 UI requirements (deferred to follow-on UI design)

These are explicit UI requirements raised by Brendan. Not blockers for the backend (API/schema) work — captured here so they're not lost when handing off to frontend design:

- The session screen must indicate that pending races exist (badge / count / inline text).
- Surface overall pending status — e.g. "2 of 3 pending races to resolve before submitting current."
- Per-race status visibility on the pending list: pending / submitting / skipped / submitted.
- Skip interaction should require **friction**, mirroring the disqualification slider. Brendan open to alternative friction mechanisms — to be decided in UI design.

Backend obligation: the `session_race_participations` table provides everything needed (existence = present, `skipped_at` = skip status, `runs` row existence = submitted status) so the UI can render any of these states from the data it gets via the session detail polling endpoint.

UI work to be specified in a separate design review once the backend is in place.

---

## 5. "Submit for original or current track" choice

DESIGN.md Workflow 4 step 6: "If someone has pending races from earlier, they see those when they go to submit. Pending races shown in order, submit or skip each." And line 412: "If the session advances while a participant hasn't submitted, they get a choice: submit for the original track (default) or the current one."

Interpretation: this is a **UI concern**, not an API concern. The API already accepts any `session_race_id` on `POST /sessions/:id/runs`. The UI lists the pending races (oldest first) and the current race, letting the user pick which to submit for. If they pick the current race, they implicitly skip or defer the pending ones (but per §4, they can't submit for the current race if older pending exists — they must explicitly skip or submit oldest first).

**Resolution:** no API change needed here. The "choice" surfaces as: the UI shows [pending race N] → [pending race N+1] → ... → [current race], with submit and skip buttons at each step. If the user wants to jump ahead to the current race, they skip the pending ones.

This may contradict "submit for the original track (default) or the current one" as a free choice. If Brendan wants that literal interpretation — tapping the current race while pending exists should let you submit for the current race, and mark the pending as skipped automatically — that's a more complex UX but doesn't need more API surface (two calls: skip oldest, submit current).

- [x] Approved — no new API; enforced via §4's ordering check
- [ ] Needs discussion — want the "auto-skip prior pending" shortcut
- [ ] Skip

---

## 6. API surface

**New endpoints:**

- `GET /sessions/:id/pending` — returns the current authenticated user's pending races for this session, oldest first. Cap in response to the UI limit? **Recommendation:** return all; UI slices. Consistent with DESIGN.md.
  - Response shape: `Vec<SessionRaceInfo>` (reuse the existing type from `get_session_detail`). Avoids a new DTO.
- `POST /sessions/:id/races/:race_id/skip` — marks a pending race as skipped for the current user. Inserts a `pending_race_skips` row. Idempotent (second call on same `(session_race_id, user_id)` returns the same 200 OK — honor unique constraint in service layer).

**Modified behavior:**

- `POST /sessions/:id/runs` (`create_run`) — adds the §4 ordering check.
- `GET /sessions/:id` (`get_session_detail`) — consider folding the current user's pending races into the response so the UI doesn't need a second poll. Recommendation: **yes, inline it**. Add `your_pending: Vec<SessionRaceInfo>` to the response. Saves a round-trip on the hot polling endpoint.
  - Downside: `get_session_detail` grows by one query. It's authenticated, so we know which user.

**Alternative: skip the separate `GET /pending` endpoint.** If pending rides on `get_session_detail`, we don't need a dedicated endpoint at all. **Recommendation: merge into `get_session_detail` and drop `GET /sessions/:id/pending`.** Less API surface, fewer requests.

- [x] Approved — pending folded into `get_session_detail`; only new endpoint is `POST .../skip`

---

## 7. Interaction with existing stale-session cleanup — APPROVED (Option A)

**Resolved 2026-05-01 via discussion.** The original analysis stands: there is no interaction in normal operation because the 1-hour stale-session cutoff is always longer than the 5-minute grace period.

### Approved decision

**Do nothing.** When a session is auto-closed by the stale-session task:

- `sessions.status` flips to `'closed'`. No other table is touched.
- `session_race_participations` rows for unresolved pending races stay in the DB as historical state.
- All API paths already filter by `session.status = 'active'` (via `load_active_session`), so the unresolved rows become naturally inaccessible.

Rationale: pending records aren't in-flight work that needs flushing — they're frozen historical state. `session.status='closed'` is the source of truth for "no longer actionable." Adding explicit cleanup would introduce code with no user-visible benefit and a small risk of destroying data we'd otherwise keep for history.

### Discarded alternatives

- **Add a `closure_reason` typed column** for hygiene/debugging — reasonable but deferred. Can be added later if "why did this session close" becomes a real debugging need.
- **Add a freeform `notes` column on `sessions`** — rejected on principle; freeform columns tend to become dumping grounds. If we ever need structured closure metadata, prefer a typed column.
- **Wait-for-pending in the stale-session task** — rejected as overkill (1 hour ≫ 5 min grace makes the wait pointless).

- [x] Approved — no changes to stale-session cleanup

---

## 8. Test plan — revised against §2 schema and mapped to design requirements

Each test below maps to an explicit DESIGN.md requirement. Goal: every design statement has at least one test that fails if you break it.

### Race-creation hook (new work introduced by §2 redesign)

**Requirement:** A user can only participate in a race if they were in the session when the race was created.

- `create_session_race_inserts_participations_for_currently_present_users` — confirms one `session_race_participations` row per user with `left_at IS NULL` at race creation.
- `create_session_race_does_not_insert_participations_for_left_users` — user who left before race creation gets no row.
- `create_session_race_atomic_with_race_insert` — if any participation insert fails, the entire transaction rolls back (race not created).

### Pending race query (`get_session_detail` integration)

**Requirements:** "Within a session, a participant may have pending races..." (DESIGN line 408); "no time submitted, not skipped, within grace."

- `pending_includes_unresolved_present_races` — happy path.
- `pending_excludes_submitted_races` — user has run row → not pending.
- `pending_excludes_skipped_races` — `skipped_at IS NOT NULL` → not pending.
- `pending_excludes_races_user_was_absent_for` — no `session_race_participations` row → not pending. (This implicitly covers the "joined after race N" case, since they wouldn't have a row.)
- `pending_returned_ordered_by_race_number_asc` — design says "shown in order, oldest first."
- `pending_returns_all_records_ui_caps` — backend returns all; UI applies the 3-cap. DESIGN says "UI cap, not schema cap."

### Grace period (revised semantics per §3)

**Requirements:** DESIGN line 290 "5-minute grace period after leave"; "grace checked lazily at submission time."

- `pending_accessible_when_currently_in_session` — `left_at IS NULL` → all pending accessible.
- `pending_accessible_when_within_grace` — `now - left_at <= 5min` → all pending accessible.
- `pending_inaccessible_when_grace_expired` — `now - left_at > 5min` → no pending returned (records still in DB).
- `rejoin_within_grace_preserves_pending` — leave at T, rejoin at T+3min → all pre-leave pending accessible again, `joined_at` unchanged.
- `rejoin_after_grace_resets_joined_at_and_forfeits_pending` — leave at T, rejoin at T+20min → `joined_at = T+20min`, pre-gap pending excluded by `created_at >= joined_at` filter.
- `multiple_short_flaps_within_grace_preserve_pending` — leave/rejoin/leave/rejoin all within grace → pending stays accessible throughout.
- `lazy_check_assertion` — comment-only assertion documenting that no background task touches `session_race_participations` (intent documented in code).

### Skip mechanism

**Requirements:** "they can submit a time or skip" (DESIGN Workflow 4 step 6); skip is idempotent.

- `skip_pending_race_sets_skipped_at` — happy path.
- `skip_pending_race_idempotent` — second skip on same `(race, user)` returns 200 OK, doesn't change `skipped_at`.
- `skip_unknown_race_returns_404` — race doesn't exist or user has no participation row → "Pending race not found."
- `skip_already_submitted_returns_conflict` — user has run row → 409 "Already submitted."
- `skip_advances_pending_list` — after skipping the oldest pending, the next-oldest becomes the new "must-submit-first" target.

### Ordered-submit enforcement in `create_run`

**Requirements:** "Submit in order, oldest first... submitting out of order is not allowed... prevents cherry-picking" (DESIGN line 410).

- `submit_oldest_pending_first_succeeds` — happy path.
- `submit_newer_pending_while_older_exists_returns_conflict` — `Conflict("Must submit or skip pending race #{n} first")`.
- `submit_current_race_with_no_pending_succeeds` — control case.
- `submit_current_race_with_older_pending_returns_conflict` — same as above; current race counts as "newer."
- `submit_current_race_after_skipping_all_pending_succeeds` — confirms skip clears the block correctly.

### Integration tests (HTTP layer)

Per PR D precedent — full request/response round trips for:

- `POST /sessions/:id/races/:race_id/skip` — happy + 404 + 409.
- `POST /sessions/:id/runs` with pending — 409 with the expected message.
- `GET /sessions/:id` — includes `your_pending` field, ordered correctly, reflects skip/submit changes.

### Test that's intentionally NOT included

- "Forfeited records are physically deleted from `session_race_participations`." — they aren't, by design (per §7). Records stay in the DB after grace expires; only their accessibility changes.

- [x] Approved — comprehensive coverage, mapped to DESIGN.md requirements

---

## 9. Out of scope / deferred

- **Background timer to actively expire pending races.** Lazy check is sufficient. No cleanup task — `session_race_participations` rows don't accumulate (bounded by race count × users-present-at-creation per session).
- **Per-user UI cap of 3.** UI concern; enforce in frontend. The API returns all.
- **Notification when pending is about to expire.** Deferred. Would require push / background computation.
- **"Remind me" for pending races across sessions.** Out of scope.
- **Graceful handling of clock skew between client and server.** All timestamps are server-side (`chrono::Utc::now().naive_utc()`) — no client-supplied times in the grace-period calculation.
- **`closure_reason` typed column on `sessions`.** Discussed in §7. Not needed now; revisit if "why did this session close" becomes a real debugging need.
- **Audit history of join/leave events.** Lost as part of the §2 redesign (single mutable `session_participants` row). No MVP feature requires it. If reintroduced, would be a separate `session_participation_events` audit log table.

- [x] Approved

---

## Implementation strategy

Suggested PR decomposition (revised against §2 schema):

**PR 3D-0 (prerequisite, mechanical):**
- Squash existing migrations 1–12 into a single consolidated initial migration per the new prelaunch convention (CLAUDE.md).
- Verify identical schema is produced on a fresh boot.
- This is a pure refactor — no behavior change.

**PR 3D-1 (foundation):**
- Edit consolidated migration: simplify `session_participants` (remove multi-row design, mutable `joined_at`/`left_at`), add `session_race_participations` table.
- Update entities accordingly.
- Update `create_session_race` (or wherever races are created) to INSERT participation rows for currently-present users in the same transaction.
- Update join/leave/rejoin logic for the new mutable single-row participation model — including the `joined_at` reset rule on long-gap rejoin (§3).
- Implement `pending_races` query function + unit tests (§8 race-creation hook + pending query + grace period sections).
- Inline `your_pending` into `get_session_detail` response (§6).

**PR 3D-2 (enforcement + skip):**
- `POST /sessions/:id/races/:race_id/skip` endpoint + service.
- Ordered-submit check in `create_run` (§4).
- Integration tests (§8 integration section).

Splitting this way keeps the schema migration and query logic (the hardest parts to get right) in a PR that's mostly additive — no user-visible behavior change until PR 3D-2 wires the enforcement in. If 3D-1 ships and the query turns out to have bugs, they're visible in `get_session_detail` but don't block submissions.

- [x] Approved — split into 3D-0 / 3D-1 / 3D-2

---

## Resolution log

All open questions resolved during the 2026-05-01 design discussion:

1. **§2 skip storage** — RESOLVED: replaced with broader redesign. New `session_race_participations` table + simplified `session_participants` (single mutable row).
2. **§3 grace semantics** — RESOLVED: superseded by §2 redesign. Semantics are now "currently in session OR within 5 min of last leave," with `joined_at` reset on long-gap rejoin to forfeit pre-gap pending.
3. **§4 skip ordering** — RESOLVED: free skip; enforce ordering on submit only. UI requirements (§4.1) deferred to follow-on UI design.
4. **§5 current-track override** — RESOLVED: no API change. Enforced via §4 ordering check.
5. **§6 endpoint shape** — RESOLVED: fold `your_pending` into `get_session_detail`. No separate endpoint.
6. **§7 stale-session interaction** — RESOLVED: do nothing. `session.status='closed'` is the source of truth.
7. **§8 test plan** — RESOLVED: rewritten against §2 schema, mapped to DESIGN.md requirements.
8. **§9 PR split** — RESOLVED: 3D-0 (squash) → 3D-1 (foundation) → 3D-2 (enforcement + skip).

## Downstream documentation work

This design review's decisions need to be reflected in:

- **DESIGN.md schema section** — `session_participants` description (single row, mutable) and new `session_race_participations` table.
- **DESIGN.md §"Pending Race Tracking"** — updated derivation rule referencing the new table.
- **DESIGN.md grace-period mention (line 290)** — updated semantics ("currently in OR within 5 min of last leave," `joined_at` reset on long-gap rejoin).
- **CLAUDE.md** — already updated with the prelaunch schema-change convention (commit pending due to file-lock issue at the time of writing).

These updates should land before the PR 3D-0 / 3D-1 handoffs so Claude Code is working from a coherent design source.
