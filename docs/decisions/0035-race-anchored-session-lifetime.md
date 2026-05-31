---
status: accepted
date: 2026-05-11
deciders: [Brendan]
source: ad-hoc
---

# 0035 — Race-anchored session lifetime: replace idle-session sweeper with race-derived sweeper

## Context and problem statement

The session lifecycle currently uses two parallel timeout concepts:

1. **Idle-session timeout (1 hour).** A Tokio task (`services::sessions::close_stale_sessions`) runs every 5 minutes and closes any session whose `sessions.last_activity_at` is older than 1 hour. The `last_activity_at` column is maintained by seven `helpers::touch_session` calls scattered across `create_session`, `join_session`, `leave_session`, `next_track`, `skip_turn`, `create_run`, and `skip_pending_race`.
2. **Rejoin grace period (5 minutes).** A user who leaves and rejoins within 5 minutes keeps their pre-leave pending races; outside that window, `session_participants.joined_at` resets and pre-gap pending is forfeited. This is enforced by gap-comparison logic in `join_session` and two clauses in `get_pending_races`.

The two timeouts coexist because the 5-minute grace was designed to handle accidental disconnects inside the broader 1-hour idle window — the grace gives users a chance to recover before the sweeper closes their session out from under them.

Note that clean session closure does not depend on the sweeper. `leave_session` already closes the session inline (`transfer_host_or_close` returns `SessionClosed`, and the same transaction sets `sessions.status = 'closed'`) when the last active participant leaves. The sweeper's job is narrowly the *abandoned* case: a participant's row still reads `left_at IS NULL` but the user is no longer really there because their tab closed, their phone died, or they lost connection without invoking the leave endpoint. A browser-side `beforeunload`/`pagehide` signal is not a reliable substitute — those events are throttled on mobile, skipped when the OS kills a backgrounded app, and obviously can't fire on a dead battery.

The design works but it costs:

- A maintained `sessions.last_activity_at` column with seven `touch_session` call sites that future contributors must remember.
- Six clauses in the pending-races derivation, including a join on `session_participants` purely to evaluate the grace window.
- A mutable `joined_at` semantic (resets on long-gap rejoin) that complicates the "first-joined" host-succession ordering.
- A user-facing concept ("if you leave for 5+ minutes, your pending races are forfeit") that the leave-session UI is supposed to explain (workflow 1.5).

A simpler model: anchor the only timeout to the race rather than the participant. If submission for any session race is allowed up to 1 hour from `session_races.created_at`, the grace concept becomes redundant — a flaked-out user can rejoin any time within that hour and their pending is still pending, because the race itself hasn't expired.

This ADR proposes that change.

- [x] Approved
- [ ] Needs discussion

## Decision drivers

- **One timeout concept, not two.** Race-anchored deadlines collapse "session idle" and "rejoin grace" into a single number the user can see directly (the race they're looking at).
- **"Never feel rushed."** A 1-hour race window is strictly more forgiving than a 5-minute leave grace for the common phone-dies / network-drops case.
- **Derive before storing.** `sessions.last_activity_at` is maintained-by-convention state; `session_races.created_at` is already written for unrelated reasons.
- **Smaller surface to test and reason about.** Each branch of the rejoin gap-comparison, each `touch_session` call, and each clause of the pending query is a place a future contributor can get wrong.

- [x] Approved
- [ ] Needs discussion

## Considered options

- **Option A — Status quo.** Keep the idle-session sweeper and the 5-minute rejoin grace. Two timeouts, both well-tested. No code change.
- **Option B — Race-derived sweeper.** Replace the `last_activity_at < now-1h` predicate with `NOT EXISTS (race in this session created within the last hour)`. Drop `last_activity_at`, the `touch_session` calls, and the grace-period logic. Keep the periodic Tokio task and the `session.status='closed'` invariant.
- **Option C — Lazy-only, no sweeper.** Drop the Tokio task entirely. The session-listing endpoint filters out sessions whose newest race is more than an hour old. Cheapest code, but stale sessions linger in the DB with `status='active'` forever.
- **Option D — Per-participant heartbeats.** Each connected client updates a `last_seen_at` per participant via the existing 2.5s poll. Sessions close when all participants are stale. Significant new state to maintain and a DB write on every poll; overkill at this scale.

- [x] Approved
- [ ] Needs discussion

## Decision outcome

Chosen: **Option B — race-derived sweeper.**

The periodic Tokio task stays (same 5-minute cadence) but is repurposed: it is now solely the safety net for abandoned sessions (no one called `leave_session`). Clean exits continue to close their session inline via `leave_session`'s existing `transfer_host_or_close` path — unchanged by this ADR.

Its predicate changes from `last_activity_at < now - 1h` to:

```sql
WHERE sessions.status = 'active'
  AND sessions.created_at < NOW() - INTERVAL 1 HOUR
  AND NOT EXISTS (
    SELECT 1 FROM session_races sr
    WHERE sr.session_id = sessions.id
      AND sr.created_at >= NOW() - INTERVAL 1 HOUR
  )
```

The `sessions.created_at` clause handles the bootstrap case (session created, no race chosen yet): a brand-new session gets the same 1-hour window from its own creation timestamp.

The pending-races query (`get_pending_races`) drops:

- The grace clause `(sp.left_at IS NULL OR sp.left_at >= grace_cutoff)`.
- The `sr.created_at >= sp.joined_at` clause.
- The `session_participants` join (no longer needed for either of the above).

It gains one new clause:

- `sr.created_at >= NOW() - INTERVAL 1 HOUR` — the per-race expiry.

`join_session` drops the gap-comparison branch: rejoin always clears `left_at`, never resets `joined_at`. `joined_at` becomes monotonic — set on first join, never reset for the lifetime of the (session, user) row.

`REJOIN_GRACE_MINUTES` is deleted.

`check_not_in_any_session` and `get_active_session_id` — the queries that enforce "one active session per user" and that power `/sessions/mine` — gain the same race-derived liveness clause used by the sweeper. A user is treated as "in" a session only if `sessions.status = 'active'` AND the session has a race within the hour (or was itself created within the hour). This decouples user lockout from sweep timing: without it, a user whose abandoned session went stale could be told "you're already in session X" for up to one sweep interval after the staleness took effect.

With that decoupling in place, the sweeper becomes purely DB hygiene: it keeps `sessions.status` eventually consistent with the derived "alive" truth, but no user-facing read path depends on it having run recently. The sweep cadence is set to **15 minutes** (changed from the legacy 5-minute cadence inherited from the idle-timeout system). Under race-derived liveness no read path requires sub-quarter-hour freshness, and `status = 'closed'` lags derived reality by no more than a quarter of the race window — a defensible compromise between DB churn and observability.

- [x] Approved
- [ ] Needs discussion

## Schema and code changes

**Schema** (single edit to the consolidated initial migration, per `docs/coding-standards/seaorm.md` § 5 (Migrations)):

- `sessions` table: drop the `last_activity_at` column.

**Backend service code:**

- Delete `helpers::touch_session` and remove all seven call sites.
- `close_stale_sessions`: rewrite predicate as above. The transaction body (mark remaining participants left, set status closed) is unchanged.
- `get_pending_races`: rewrite SQL per the clauses above.
- `join_session`: drop the gap-comparison branch and the `REJOIN_GRACE_MINUTES` comparison. Rejoin clears `left_at` unconditionally; `joined_at` is set only when the row is first inserted.
- `check_not_in_any_session` and `get_active_session_id`: extend the SQL predicate with the race-derived liveness clause (session has a race within the last hour, OR was itself created within the last hour). Decouples user-lockout duration from sweep timing.
- `main.rs`: change the sweep interval from `Duration::from_secs(300)` (5 min) to `Duration::from_secs(900)` (15 min).
- Constants: remove `REJOIN_GRACE_MINUTES`.

**Entities:**

- `entities/sessions.rs`: drop the `last_activity_at` field.

**Tests:**

Specific tests change; the full list will live in the implementing PR description rather than here (to avoid stale references). Conceptually:

- Drop grace-period tests in `services/sessions.rs` (the five `pending_*_grace_*` and `rejoin_*_grace*` tests, plus `multiple_short_flaps_within_grace_preserve_pending`).
- Drop the `touch_session_updates_last_activity_at` helper test.
- Add race-expiry tests: pending excludes expired races, pending includes races within window, sweeper closes when all races expired, sweeper keeps session with a recent race, sweeper handles the "no races yet" bootstrap case.
- Update integration tests in `tests/session_integration.rs` that assert on `last_activity_at` movement.

**Frontend:**

- Update the workflow 1.5 leave-session warning copy from "pending races forfeited after 5-minute grace" to "races expire 1 hour after they start." (The frontend audit shows no implementation of the 5-min warning yet — this is a spec change, not a code change.)
- No visible per-race countdown timer. Honoring "never feel rushed" — races simply disappear from the pending list when they expire.

**Documentation:**

- `docs/data-model.md`: update the `sessions` table (drop `last_activity_at`), the `session_participants` notes (drop the grace and `joined_at`-reset rules), and the Pending Race Tracking derivation (replace clauses 4 and 5 with the per-race expiry clause).
- `docs/user-workflows.md` § 1.5: replace the 5-minute grace sentence with the race-expiry sentence.
- `docs/decisions/0013-pending-race-cap.md`: no change — the UI cap concept is orthogonal.
- `docs/decisions/0015-session-timeout-skip-turn.md`: no change — skip-turn semantics are independent of session/race lifetime.
- `docs/designs/archive/2026-04-19-pending-races-and-grace-period.md` § 3 (grace semantics) is superseded by this ADR. Per design-record convention the design record stays as-is, with a top-of-file note pointing here.

- [x] Approved
- [ ] Needs discussion

## Positive consequences

- Single timeout concept (1 hour, per race) replaces two (1 hour idle + 5 minute grace).
- `sessions.last_activity_at` column and seven `touch_session` call sites are deleted.
- `get_pending_races` shrinks from six clauses + a `session_participants` join to four clauses + no extra join.
- `join_session` becomes a "clear `left_at`" mutation with no gap math.
- `joined_at` becomes monotonic — set on first join, never reset. Host-succession ("earliest-joined remaining participant," ADR 0014) becomes a cleaner ordering.
- Rejoin behavior is strictly more forgiving (up to an hour from each race's start, not five minutes from your leave).
- The sweeper's responsibility narrows to one well-defined case: a session whose users left without notifying the server (tab closed, phone died). Clean exits are handled inline by `leave_session` (unchanged).
- All user-facing read paths (`get_pending_races`, `check_not_in_any_session`, `get_active_session_id`) derive session liveness from race timestamps directly. The sweeper is purely DB hygiene, and no user-facing read depends on it having run recently.

## Negative consequences and trade-offs

- **"Zombie active" lag between sweeps.** A session whose last race expires just after a sweep cycle stays `active` in the DB for up to ~15 minutes. With race-derived liveness pushed into every user-facing read path (`get_pending_races`, `check_not_in_any_session`, `get_active_session_id`), this is purely a DB-state-vs-derived-truth lag — no user-facing read returns a stale answer. Only admin queries and any future code path that reads `sessions.status` directly (without applying the liveness predicate) would observe it.
- **Late joiner to a near-stale session may have the session close underneath them.** If you join at minute 58 of a race and no one picks a new track, the session is hard-closed by the next sweep that catches it — somewhere in the minute 60 to minute 75 window. The joiner has had nothing to do in the meantime (they weren't present when the existing race was created and can't submit for it). Mitigation (accepted): the frontend session list filters out sessions whose newest race is more than ~30 minutes old, so users don't see near-stale sessions to join. A joiner who calls `next_track` immediately revives the session (the new race's `created_at` resets the liveness predicate). Mitigation (rejected): extend session life on participant join — brings back maintained state and partially defeats the simplification.
- **Loss of participation-history audit trail** is unchanged from today (`session_participants` is already a single mutable row). The change actually makes it slightly cleaner: `joined_at` no longer resets, so it now genuinely means "when did this user first join this session."

- [x] Approved
- [ ] Needs discussion

## Migration

Pre-launch schema change. Per `docs/coding-standards/seaorm.md` § 5 (Migrations), edit the consolidated initial migration (`backend/migration/src/m20260101_000001_initial_schema.rs`) directly. No data migration needed.

Suggested PR split:

1. **Schema + entity** — drop `last_activity_at` from migration and `entities/sessions.rs`.
2. **Service simplification** — rewrite `close_stale_sessions` and `get_pending_races`; simplify `join_session`; delete `touch_session` and `REJOIN_GRACE_MINUTES`; remove call sites.
3. **Test cleanup** — drop obsolete grace tests, add race-expiry tests.
4. **Docs** — update `data-model.md`, `user-workflows.md`, and add a supersession note to `2026-04-19-pending-races-and-grace-period.md`.

Steps 1–3 should land together (they're co-dependent — entity and migration must match, services must compile against the new entity, tests must match the new behavior). Step 4 can be a separate doc-only commit straight to main.

- [x] Approved
- [ ] Needs discussion

## Sign-off

Each section above carries its own approval checkbox. Once all are checked:

- Set `status` in this file's frontmatter to `accepted`.
- Update the row for 0035 in `docs/decisions/README.md` from `proposed` to `accepted`.
- Add a top-of-file "Superseded by ADR-0035 (§ 3 grace semantics)" note to `docs/designs/archive/2026-04-19-pending-races-and-grace-period.md`.
- File implementation issues per the PR split above under Milestone Star.

## Implementation notes (added 2026-05-16, post-merge)

One behavioral detail not covered in the ADR's literal text surfaced during implementation:

**Settle-on-new-join.** The schema carries a partial unique index `idx_session_participants_one_active_session` (`UNIQUE(user_id) WHERE left_at IS NULL`) that the ADR doesn't mention. Pushing the race-derived liveness clause into `check_not_in_any_session` and `get_active_session_id` decouples lockout from sweep timing **at the application level** — but the INSERT in `create_session` / `join_session` still collides with the partial unique index when a user holds a `left_at IS NULL` row in a now-stale session. To actually deliver the ADR's stated goal of decoupled lockout, the create/join transaction now also calls `settle_dangling_participation`, which sets `left_at = NOW()` on the user's pre-existing dangling row before the new INSERT. Semantically: starting or joining a new session is an implicit leave of any abandoned one. The stale session's `status` and its other participants' rows are left to the sweeper (eventual consistency).

This is captured in `docs/data-model.md` § Session Participants as the "Settle-on-new-join" bullet. The ADR's body retains its application-level framing for readability; this addendum is the schema-level wrinkle that makes the framing actually work.

## Links

- Source: ad-hoc (Cowork chat discussion, 2026-05-11)
- Narrowed by: [ADR-0037](./0037-pending-races-dropped-on-session-close.md) (2026-05-16) — the rejoin-anytime-within-the-hour promise becomes "rejoin while the session is still active." Per-race expiry clause removed from `get_pending_races`; sweeper predicate extended to honor all activity signals, not just race creation.
- Supersedes: `docs/designs/archive/2026-04-19-pending-races-and-grace-period.md` § 3 (grace period semantics)
- Related ADRs: 0013 (pending race cap — unchanged), 0014 (host transfer — cleaner under monotonic `joined_at`), 0015 (skip-turn semantics — unchanged), 0018 (polling — unchanged)
- Implementing PRs: <to fill in>
