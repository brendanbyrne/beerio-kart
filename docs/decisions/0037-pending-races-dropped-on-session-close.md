---
status: accepted
date: 2026-05-16
deciders: [Brendan]
source: ad-hoc
---

# 0037 — Pending races dropped on session close

## Context and problem statement

[ADR-0035](./0035-race-anchored-session-lifetime.md) replaced the 5-minute rejoin grace with a per-race 1-hour submission window. The intent was that a flaked-out user could rejoin any time within that hour and their pending races would still be pending — the race itself hadn't expired. Workflow 1.5 was updated to reflect this: *"races must be submitted within 1 hour of when they started; rejoining within that hour preserves the pending race."*

That promise is not fully honored by the code that ships with ADR-0035. `transfer_host_or_close` (in `services/sessions/lifecycle.rs`) was not in the ADR's scope and still closes the session inline whenever the last active participant leaves — *regardless* of whether any race in that session is still within its per-race window. As a result:

- **Clean leave path** (everyone taps "Leave" eventually). When the last leaver triggers close, the session status flips to `'closed'`. The pending-race derivation's `sessions.status = 'active'` filter immediately hides any pending races from those users. They get no signal; the races silently vanish from their UI on next poll.
- **Abandoned path** (phones die, tabs close without anyone tapping Leave). Participants' `left_at` stays `NULL`, the session stays `'active'`, and the sweeper takes 1 hour from the last race to close it. Pending races remain accessible during that window. Users can come back.

So a clean exit is *punished* relative to a sloppy exit — the opposite of the UX signal you usually want — and the per-race 1-hour promise only holds in the sloppy case.

The proper fix could be:

1. Make `leave_session` race-aware (defer close while pending-in-window exists for anyone).
2. Skip-on-leave (mark every leaver's pending as `skipped` in their leave transaction).
3. Drop-on-close (close still fires on last-leave; the close transaction marks all unresolved pending as a new `dropped` state, distinct from skipped; users get a notification next time they sign in).
4. Status quo (silent drop, no signal).

This ADR records the choice and the supporting design.

- [x] Approved
- [ ] Needs discussion

## Decision drivers

- **Honor a coherent principle.** "Session liveness anchors what users can act on" is simpler and easier to communicate than "per-race liveness anchors what users can act on, except when the session closes around you, in which case the per-race liveness still technically applies but the API filters it out anyway."
- **Don't punish clean exits.** Tapping Leave should not be a worse outcome than closing the tab.
- **Decisiveness when there's no one left.** When literally everyone has left, the session is over. The "still has time on its races" thread has nothing to attach to — there's no one to anchor expectations against.
- **Surface what happened to the affected user.** Silent forfeit is the current behavior and it's the failure mode we're explicitly fixing. The user has to know.

## Considered options

- **Option A — Status quo.** Pending races become inaccessible via the existing `sessions.status = 'active'` filter; users see them silently vanish. The behavior ADR-0035 effectively ships.
- **Option B — Race-aware leave.** Modify `transfer_host_or_close` to skip the close branch when any race in the session has an unresolved pending row within its 1-hour window. The session lingers `'active'` until the sweeper picks it up. Honors the per-race promise across all paths.
- **Option C — Skip-on-leave.** Inside `leave_session`'s transaction, set `skipped_at = NOW()` on every unresolved pending row owned by the leaving user. Closes the "I came back to a vanished race" loop, but punishes clean exits (skip-on-leave) relative to abandoned exits (no skip on tab-close). Conflates user-chose-to-skip with had-to-leave in the `skipped_at` audit signal.
- **Option D — Drop-on-close.** Leave path stays exactly as it is today (last leaver closes the session inline). The close transaction (in both `leave_session` and `close_stale_sessions`) marks every unresolved pending row in the closing session as `dropped` via a new nullable `dropped_at` column on `session_race_participations`. `dropped` is a distinct state from `skipped`. Users see a notification on their next sign-in summarizing what was dropped. **Retains the per-race 1-hour expiry from ADR-0035** as belt-and-suspenders alongside `dropped_at`.
- **Option E — Drop-on-close, no per-race timer.** Like Option D, but additionally removes the `sr.created_at >= NOW() - 1 hour` clause from `get_pending_races`. Pending races stay submittable for the entire lifetime of the session — the session, not the race, is the deadline. The per-race timer was always redundant with `dropped_at IS NULL` once a session closed (the close transaction stamps both together); this removes the redundancy. Pairs naturally with extending the sweeper's predicate to honor every meaningful activity signal, not just race creation, so the session-is-the-deadline rule is honest end-to-end.

- [x] Approved
- [ ] Needs discussion

## Decision outcome

Chosen: **Option E — drop-on-close, no per-race timer, with an extended sweeper predicate.**

The user-facing rule is: **the session is the deadline.** Pending races stay submittable for as long as the session is alive; the session is alive as long as any meaningful activity has happened within the last hour (race creation, run submission, join, leave, or skip). When the session closes — whether via clean last-leave or via the sweeper — every unresolved pending row in that session has `dropped_at` stamped on it, and the affected users see a notification (per ADR-0038) on next sign-in.

The principle limit is named explicitly: a session is "the room," and the room ends when everyone has been gone for an hour. If a player wants to leave and come back, they need to coordinate out-of-band with at least one other participant to keep the session alive — the app does not extend the room's existence on someone's intent alone, because intent is not visible to the server.

The narrowing surfaces a concrete UX consequence: **a solo player cannot leave-and-come-back at all** — their leave is automatically the last leave. That's accepted; solo racing inside a session is rare, and the workaround ("don't tap Leave, just close the tab — the abandoned-session path keeps your work alive while the sweeper runs") is honest and known. Documented in user-workflows.md § 1.5.

The notification surface that tells users about drops is captured separately in [ADR-0038](./0038-notifications-system.md), since it generalizes beyond this single use case.

- [x] Approved
- [ ] Needs discussion

## Schema and code changes

**Schema** (single edit to the consolidated initial migration, per `backend/CLAUDE.md` § schema-changes-prelaunch):

- `session_race_participations` table: add a nullable `dropped_at: TIMESTAMP` column. PK and existing indexes unchanged.

**Entities:** `entities/session_race_participations.rs` gains the new field.

**Backend service code:**

- `services::sessions::lifecycle::close_session_and_drop_pending` — new helper. Inside the close transaction, runs a single `UPDATE` setting `dropped_at = NOW()` on every `session_race_participations` row where `session_id = closing_session` AND `skipped_at IS NULL` AND `dropped_at IS NULL` AND no `runs` row exists for `(session_race_id, user_id)`.
- `transfer_host_or_close`: when the `SessionClosed` branch fires, calls `close_session_and_drop_pending` before flipping `sessions.status` (still in the same transaction).
- `close_stale_sessions`: same — calls `close_session_and_drop_pending` for each session it's about to close, inside the existing close transaction. Also picks up the wider activity predicate; see "Sweeper predicate extension" below.
- `services::notifications::record_pending_drops(txn, user_id, session_id, dropped_count)` — called once per affected user inside the close transaction. The notification subsystem is specified by ADR-0038; the consumer site is just the close path.

**`get_pending_races` simplification.** Loses two clauses: the per-race expiry (`sr.created_at >= NOW() - 1 hour`) and the session-status filter (`sessions.status = 'active'`). Both become fully redundant with `srp.dropped_at IS NULL` once `dropped_at` is stamped on every unresolved row at session close. Final clause set:

1. Participation row exists (the eligibility prerequisite — see the status enum below).
2. `skipped_at IS NULL`.
3. No `runs` row for `(race, user)`.
4. `dropped_at IS NULL`.

Four clauses, no per-race timer, no `sessions` join. The query gets meaningfully cheaper.

**Sweeper predicate extension.** Today's `close_stale_sessions` predicate uses "no race in this session created within the last hour" as the proxy for "session is abandoned." That misses every other meaningful activity signal — runs submitted, participants joining/leaving, pending races skipped. Under the simplified `get_pending_races` (no per-race timer), the sweeper is now the *only* gatekeeper for closing dormant sessions, so it should honor every activity signal the session actually has. The predicate becomes:

```sql
WHERE sessions.status = 'active'
  AND sessions.created_at < NOW() - INTERVAL 1 HOUR
  AND NOT EXISTS (SELECT 1 FROM session_races sr
                  WHERE sr.session_id = s.id
                    AND sr.created_at >= NOW() - INTERVAL 1 HOUR)
  AND NOT EXISTS (SELECT 1 FROM runs r
                  JOIN session_races sr ON r.session_race_id = sr.id
                  WHERE sr.session_id = s.id
                    AND r.created_at >= NOW() - INTERVAL 1 HOUR)
  AND NOT EXISTS (SELECT 1 FROM session_participants sp
                  WHERE sp.session_id = s.id
                    AND (sp.joined_at >= NOW() - INTERVAL 1 HOUR
                         OR sp.left_at  >= NOW() - INTERVAL 1 HOUR))
  AND NOT EXISTS (SELECT 1 FROM session_race_participations srp
                  JOIN session_races sr ON srp.session_race_id = sr.id
                  WHERE sr.session_id = s.id
                    AND srp.skipped_at >= NOW() - INTERVAL 1 HOUR)
```

Five `NOT EXISTS` clauses, one per meaningful activity table. Each hits an indexed column; performance is fine at our scale. The predicate enumerates the same activity signals the ETag formula in `api-contract.md` § 4 uses — "what counts as 'something happened in this session.'" The two formulas should be kept in lockstep going forward; adding a new activity-producing endpoint means adding inputs to both.

**Per-(race, user) status enum** (derived; not stored as such). These four states apply only to `(race, user)` pairs where a `session_race_participations` row exists — i.e., where the user was present at race creation. A user who joined the session *after* a race was created has no row for that race; none of the four states apply to them and the race simply doesn't appear in their experience.

| Status | Derivation |
|---|---|
| `unraced` | participation row exists, no `runs` row, `skipped_at IS NULL`, `dropped_at IS NULL` |
| `raced` | `runs` row exists for `(race, user)` |
| `skipped` | `skipped_at IS NOT NULL` |
| `dropped` | `dropped_at IS NOT NULL` |

Mutually exclusive in practice. `unraced` is the current "pending" state; renamed in the doc vocabulary to align with the four-state enum.

**Documentation:**

- `docs/data-model.md` — `session_race_participations` table gets the `dropped_at` column; the Pending Race Tracking derivation simplifies to four clauses (drops the per-race expiry and the session-status filter); the four-state status enum gets a brief explanation with the eligibility prerequisite called out above the table; the Session Participants notes get the narrower rejoin-while-session-alive rule; the Sessions auto-close note updates to reflect the wider sweeper predicate.
- `docs/user-workflows.md` § 1.5 — replace the rejoin-within-the-hour copy with "races stay submittable while the session is active. If you leave with pending races, you can rejoin as long as someone else is still in the session. If everyone leaves, the session ends and pending races are dropped — you'll see a notification next time you visit."
- `docs/api-contract.md` § 4 — drop the 60-second time bucket from the ETag formula. With the per-race expiry gone, no time-based predicate ages a row out of the pending list without a corresponding data change, so the bucket no longer earns its keep. Six derived inputs, no time bucket.
- `docs/decisions/0035-race-anchored-session-lifetime.md` — add a `Links` note pointing at 0037 as the narrowing decision. ADR-0035 stays accepted; this ADR is the refinement.

- [x] Approved
- [ ] Needs discussion

## Positive consequences

- One coherent rule: the session is the deadline. Pending races stay submittable for as long as the session is alive, full stop.
- Clean exits and abandoned exits converge — both eventually close the session and drop pending in the same shape (clean exit drops inline; abandoned exit drops via the sweeper). No path-dependent UX.
- `dropped` is a distinct state from `skipped`, preserving the audit signal of "user chose to forfeit" vs. "the session ended."
- `get_pending_races` shrinks from a five-clause query with two timer-style predicates to a four-clause query with no time math. Cheaper at every poll.
- The sweeper now honors every meaningful activity signal — runs, joins, leaves, skips — not just race creation. A session full of people submitting their pending races but with no new track picked in 65 minutes stays alive instead of getting swept out from under them.
- ETag formula simplifies — the 60-second time bucket comes out, since no time-based predicate silently invalidates a row anymore. Six derived inputs, no time fudge factor.
- Users learn what happened via the notification (ADR-0038), so silent forfeit is gone.

## Negative consequences and trade-offs

- **Solo-player limitation.** A player racing alone cannot leave-and-come-back; tapping Leave is automatically the last leave, which closes the session and drops their pending. Acceptable trade-off but worth flagging in the leave-confirmation copy if we ever notice solo-flow friction.
- **Out-of-band coordination required** for leave-and-come-back in multi-player sessions. "Hey, don't leave yet, I want to come back" is a real-world ask. Acceptable for a friend-group app in voice/chat/same-room context.
- **Submission window is no longer per-race.** A run submitted 4 hours after the race was created is fine, as long as the session has been continuously active in between. The "1-hour-per-race" framing goes away entirely in favor of "the session is the deadline." Easier to communicate; loses the (mostly cosmetic) per-race time pressure. Cherry-picking concern is still bounded by the ordered-submit guard in `create_run` and by the session's own lifetime.
- **Sweeper predicate is more SQL** — five `NOT EXISTS` clauses vs. one. Each clause hits an indexed column, so performance is fine. The complexity is honest: it enumerates every signal that means "something is happening in this session." Same shape as the ETag formula in `api-contract.md` § 4. The two formulas now share a maintenance contract — adding a new activity-producing endpoint means adding inputs to both.
- **Closing a session now does more work** (one extra UPDATE on `session_race_participations` + one INSERT per affected user into `notifications`). Still O(participants), bounded by the session size; not a perf concern at friend-group scale.

- [x] Approved
- [ ] Needs discussion

## Migration

Pre-launch schema change. Per `backend/CLAUDE.md` § schema-changes-prelaunch, edit the consolidated initial migration directly. No data migration needed.

Suggested PR split:

1. **Schema + entity** — add `dropped_at` to the migration and the entity. Cohabits cleanly with the existing nullable `skipped_at`.
2. **Service code + simplifications** — add `close_session_and_drop_pending` helper; wire into `transfer_host_or_close` and `close_stale_sessions`; simplify `get_pending_races` (drop the per-race expiry and session-status clauses); extend the sweeper predicate to the five-clause activity check.
3. **Notification consumer** — wire the `record_pending_drops` call into the close transaction. Depends on ADR-0038's notification surface being in place.
4. **Docs** — `data-model.md`, `user-workflows.md` § 1.5, `api-contract.md` § 4 (drop the ETag time bucket), ADR-0035 cross-link.

Steps 1–2 can land together. Step 3 ships once ADR-0038's notifications module exists; this ADR can technically land *without* step 3, in which case drops are silent — undesirable for UX but mechanically correct. The natural form is one combined PR for steps 1–3 once ADR-0038 is signed off and its skeleton is in place.

- [x] Approved
- [ ] Needs discussion

## Sign-off

Each section above carries its own approval checkbox. Once all are checked:

- Set `status` in this file's frontmatter to `accepted`.
- Update the row for 0037 in `docs/decisions/README.md` from `proposed` to `accepted`.
- Add a "Narrowed by ADR-0037" Links entry to `docs/decisions/0035-race-anchored-session-lifetime.md`.
- File implementation Issues under Milestone Star (likely sibling Issues to #47 / #51 / #58; can be a single combined Issue if preferred).

## Links

- Source: ad-hoc (Cowork chat discussion, 2026-05-16)
- Narrows: [ADR-0035](./0035-race-anchored-session-lifetime.md) — the rejoin-anytime-within-the-hour promise becomes "rejoin while the session is still alive."
- Companion: [ADR-0038](./0038-notifications-system.md) — the notification infrastructure used to surface drops to users.
- Related ADRs: 0013 (pending race cap — unchanged), 0014 (host transfer — unchanged), 0015 (skip-turn — unchanged), 0029 (run_flags audit trail — independent).
- Implementing PRs: <to fill in>
