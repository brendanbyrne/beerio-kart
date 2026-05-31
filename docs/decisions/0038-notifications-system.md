---
status: accepted
date: 2026-05-16
deciders: [Brendan]
source: ad-hoc
---

# 0038 — Notifications system

## Context and problem statement

[ADR-0037](./0037-pending-races-dropped-on-session-close.md) introduces the first user-facing event that has to reach a user *asynchronously* — when their session closes and pending races are dropped, the user is by definition not in the app at that moment, so the signal has to land somewhere they can pick up on their next sign-in.

Pending-drops isn't the only such case. Other near-term and plausible notification kinds:

- Head-to-head record changes (you started beating someone you weren't, or stopped beating someone you were).
- A track or lap record you held was beaten by another player.
- Your position on the global leaderboard moved.
- (Future, less concrete) a friend started a session you might want to join, your flagged run was reviewed by admin, your photo upload was accepted, etc.

The system needs an "inbox" abstraction: per-user persisted records of these events, with a read/unread state, deliverable via the existing polling pattern (per ADR-0018). Without it, each notification kind gets ad-hoc surfacing logic, which doesn't scale.

This ADR specifies the table shape, the API surface, the generation pattern, and the open-question defaults for MVP. The first consumer is ADR-0037's pending-drops; the design is shaped to accommodate the other kinds listed above without further structural change.

- [x] Approved
- [ ] Needs discussion

## Decision drivers

- **Type-driven design** — per `coding-standards/rust.md`. Notification kinds are a closed set known at compile time; modeling them as a Rust enum surfaces forgotten cases at the compiler.
- **One general system, many kinds.** Adding a new notification kind should be an enum variant + a trigger site, not a schema change.
- **Append-only mental model.** Notifications are events, not state. Same shape as structured logs: stable envelope, kind-specific structured body, query by `(user, kind, time)`.
- **Polling, not push.** ADR-0018 already committed to polling. The home screen pulls notifications on navigation / on a slow poll. WebSockets / SSE are out of scope.
- **No new operational concerns for MVP.** No worker process, no queue, no retention task — every notification INSERT lives in the same transaction as the triggering write.

## Considered options

- **Option A — Stateless / session-scoped.** No persistence. On home screen load, compute "you have N dropped session_race_participations rows newer than `users.last_seen_at`" on the fly. Dismissal is purely client-side (user closes the dropdown). Works *only* for kinds where the notification is fully derivable from existing data; doesn't generalize to kinds that report a transient signal (e.g., "your rank moved from 5 to 7" — by the time the user reads it, the rank may have moved again, and the original signal is gone). Cheap but a dead-end.
- **Option B — `users.notifications_seen_through: TIMESTAMP`.** Single column on the users table. When the user dismisses, server writes `now()` here. Notifications computed by "find data newer than `notifications_seen_through`" across whichever tables the kind reads from. Avoids a new table but still requires per-kind derivation logic, suffers the same transient-signal problem as Option A for non-derived kinds, and doesn't carry per-event payload (e.g., the rank-changed kind needs *what* the rank moved from and to, which isn't stored anywhere else).
- **Option C — Full notifications table.** Real persisted rows: `(id, user_id, kind, payload JSON, created_at, read_at)`. Each event materializes a row; dismissal flips `read_at`. Generalizes to every kind in the list. Slightly more infrastructure but proportional to the use case.

- [x] Approved
- [ ] Needs discussion

## Decision outcome

Chosen: **Option C — full notifications table.**

The notification surface is a first-class concept in the data model. JSON payload keeps the table shape stable across kinds; a Rust enum (`NotificationPayload`) carries the type discipline on the application side.

Four design defaults for MVP, signed off explicitly:

- **Append-only, no supersession.** Each event is its own row. "Three H2H flips" yields three rows. The audit-log shape; user sees the history.
- **Keep forever.** No cleanup task in MVP. At friend-group scale (~10 users × handful of notifications/day × indefinite retention), the table is bounded well below "concern." Future-Issue if it ever matters.
- **One row per user.** No multi-recipient batching. Every query is `WHERE user_id = me`. Junction tables stay out of the design.
- **No preferences / muting.** Defer. Adding it later is a `users.notification_preferences: JSON` column or a sibling table; nothing forecloses it.

- [x] Approved
- [ ] Needs discussion

## Schema

```
notifications
├── id: UUID (primary key)
├── user_id: UUID (FK -> users, not null) — recipient
├── kind: TEXT (not null) — discriminator, snake_case
├── payload: TEXT (JSON, not null) — type-specific structured data
├── created_at: TIMESTAMP (not null)
└── read_at: TIMESTAMP (nullable — set when user dismisses)
```

Indexes:

- `idx_notifications_user_unread` on `(user_id, created_at DESC) WHERE read_at IS NULL` — supports the "list my unread, newest first" hot path.
- `idx_notifications_user_created` on `(user_id, created_at DESC)` — supports paginated "show me everything, including read."

Constraints: none beyond the FK on `user_id`. Cascade delete on user deletion is the right default (the user is gone, their inbox should be too).

**On JSON storage.** SQLite has the JSON1 extension built into recent builds. Stored as TEXT, queried with `json_extract` if needed. We rarely need to query inside the payload — the kind discriminator is the primary index, payload is opaque to the database. Stable per ADR-0028 (timestamps as ISO 8601 TEXT) — same approach for any structured field.

- [x] Approved
- [ ] Needs discussion

## Rust modeling

Notification payloads are a closed enum with serde-tagged serialization:

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum NotificationPayload {
    PendingRacesDropped {
        session_id: SessionId,
        dropped_count: u32,
    },
    H2hLeadChanged {
        opponent_id: UserId,
        opponent_username: String,
        you_now_winning: bool,
        wins: u32,
        losses: u32,
    },
    TrackRecordLost {
        track_id: i32,
        track_name: String,
        previous_run_id: RunId,
        new_run_id: RunId,
    },
    LeaderboardRankChanged {
        previous_rank: u32,
        new_rank: u32,
        scope: LeaderboardScope, // global / cup / track
    },
}
```

`kind` is the serde tag (lifted into a column for indexing) and the rest of the variant body is the JSON payload. Adding a kind = adding a variant; the compiler surfaces every site that pattern-matches.

**Frontend type agreement.** The TypeScript counterpart of `NotificationPayload` lives in `frontend/src/api/types.ts` — hand-written, kept in sync via PR review. Same pattern the codebase already uses for `SessionDetail`, `RunDetail`, and every other API response shape. No new tooling for MVP.

The hand-written approach has a known shelf life. Trigger for revisiting: either the API type surface crosses roughly 10–15 hand-maintained interfaces *or* a drift bug lands (frontend deserializes against a stale shape and breaks at runtime). At that point, file a follow-up Issue to evaluate codegen tooling. The current leading candidates are `ts-rs` and `typeshare` (both derive-macro crates with first-class support for serde-tagged enums), with an OpenAPI-based path (`utoipa` + `openapi-typescript`) as the heavier alternative that would cover the entire API contract rather than just notifications. Deferred deliberately — at the MVP type-surface size, hand-writing is the lowest-cost option, and committing to a codegen tool too early locks us into one before we know which one fits best.

- [x] Approved
- [ ] Needs discussion

## API surface

Three endpoints, all under `/me`:

```
GET    /me/notifications                List unread, newest first; cursor pagination per ADR-0032.
                                        Optional `?include_read=true` to fetch the full history.
GET    /me/notifications/unread-count   Returns `{ "count": N }`. Cheap, polled by the home screen for the badge.
POST   /me/notifications/read-all       Marks all unread for the requesting user as read (sets `read_at`).
```

Deferred until UI demands it:

- `POST /me/notifications/:id/read` — single-item mark-as-read. Most flows want bulk-dismiss-on-view, so this isn't day-one critical.
- `DELETE /me/notifications/:id` — hard delete. Mark-as-read is sufficient; deletion adds undo complexity for no current need.

The `unread-count` endpoint exists separately from `GET /me/notifications` so the home-screen badge can poll cheaply (every 30s, say) without pulling full payloads. The list endpoint is fetched on-demand when the user opens the dropdown.

- [x] Approved
- [ ] Needs discussion

## Generation pattern

**Service-layer fan-out.** Wherever an event happens in service code, that service function writes the relevant notification rows inside the same transaction as the triggering write. No async dispatch, no worker.

A small `services::notifications` module exposes typed constructors:

```rust
pub async fn record(
    txn: &impl ConnectionTrait,
    user_id: &UserId,
    payload: NotificationPayload,
) -> Result<(), Error> {
    let kind = payload.kind_str();              // derived from the serde tag
    let payload_json = serde_json::to_string(&payload)?;
    notifications::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        user_id: Set(user_id.as_str().to_string()),
        kind: Set(kind.into()),
        payload: Set(payload_json),
        created_at: Set(Utc::now().naive_utc()),
        read_at: Set(None),
    }
    .insert(txn)
    .await?;
    Ok(())
}
```

Each event site:

- **ADR-0037 pending-drops** — `transfer_host_or_close` (close branch) and `close_stale_sessions`. One row per affected user with `PendingRacesDropped { session_id, dropped_count }`.
- **H2H lead changed** — `services::runs::create_run` (and `delete_run`). After the run write, recompute H2H against each opponent who shares any `session_race_id` with this user; for any pair that flipped winner, emit `H2hLeadChanged`. Bounded by the number of session participants.
- **Track record lost** — `services::runs::create_run`. If the new run beats the existing per-(track, drink-category) record, emit `TrackRecordLost` to the previous holder.
- **Leaderboard rank changed** — `services::runs::create_run` (and any other surface that can shift rank). Compute affected users via the leaderboard query before and after; emit one notification per moved user. Practical scope to bound the number of notifications: only emit when crossing into / out of the top N visible on the leaderboard (e.g., top 10), so a rank shift from 47 to 46 doesn't notify anyone.

**Atomicity.** Every notification INSERT runs inside the same transaction as the triggering write. If `create_run` fails, no spurious "you lost the record" notification. If the notification INSERT itself fails, the whole event rolls back. The notification system carries no async-failure modes — by design.

**Consolidation helper.** A single `notifications::handle_run_created(txn, &new_run)` is the convention for the run-create site, which generates all run-related kinds (H2H, track-record-lost, leaderboard-rank-changed) so the `create_run` service stays readable.

- [x] Approved
- [ ] Needs discussion

## MVP scope vs. future scope

**MVP (ships alongside ADR-0037's drop-on-close implementation):**

- `notifications` table + indexes.
- `NotificationPayload` enum with the `PendingRacesDropped` variant only.
- `services::notifications::record` helper.
- The three API endpoints (`GET /me/notifications`, `GET /me/notifications/unread-count`, `POST /me/notifications/read-all`).
- Frontend bell icon + dropdown + render for `PendingRacesDropped` notifications.
- Trigger sites in `transfer_host_or_close` (close branch) and `close_stale_sessions`.

**Future Issues (filed under future cups; the system accommodates them without schema change):**

- `H2hLeadChanged` variant + trigger + UI render (lands with Leaf — social/H2H).
- `TrackRecordLost` variant + trigger + UI render (lands with Banana — leaderboards).
- `LeaderboardRankChanged` variant + trigger + UI render (lands with Banana).

Each future kind is a localized addition: one enum variant, one trigger site, one UI render function. No coordination with the notification core needed.

- [x] Approved
- [ ] Needs discussion

## Positive consequences

- One general inbox abstraction covers all four planned notification kinds plus any future ones.
- Append-only, structured-log-style data model — easy to inspect, query, and reason about.
- Atomic with the triggering writes — no half-states, no async failure paths to handle.
- Future evolution (preferences, supersession, retention) doesn't break the core shape.
- The MVP scope is genuinely minimal — table + helper + three endpoints + one consumer.

## Negative consequences and trade-offs

- **Storage grows unboundedly** without a retention policy. Tolerable at our scale (~18k rows/year, bounded by user activity), but a real concern at any larger scale. Filing a follow-up Issue to add retention if/when needed.
- **No supersession** means active sessions with chatty events (e.g., H2H flipping back and forth) will produce repeated rows for the same `(kind, opponent)` pair. If real usage shows this is annoying, we'll migrate to per-`(kind, context_key)` supersession — the migration is `DELETE` of outdated rows on each new insert, doable without a schema change. Listed as a future-Issue trigger.
- **JSON payload is opaque to the DB query planner.** We can't easily say "give me all my track-record-lost notifications where the track was Rainbow Road." Not a need today. If it becomes one, materialize the relevant fields into columns at that time.
- **Service-layer fan-out couples each event site to notification generation.** The trade-off is no extra infrastructure (no worker, no queue, no event log). Could revisit if the event-trigger sites get noisy; for now the consolidation pattern (one `handle_run_created` per write site) keeps things readable.

- [x] Approved
- [ ] Needs discussion

## Migration

Pre-launch schema change. Per `docs/coding-standards/seaorm.md` § 5 (Migrations), edit the consolidated initial migration directly. No data migration.

Suggested PR shape:

1. **Schema + entity + service helper** — add the `notifications` table to the migration, the entity, and the `services::notifications::record` helper. Define the `NotificationPayload` enum with just `PendingRacesDropped` for MVP. No consumers yet, no endpoints yet.
2. **API endpoints** — `GET /me/notifications`, `GET /me/notifications/unread-count`, `POST /me/notifications/read-all`. Cursor pagination wired per ADR-0032.
3. **First consumer (ADR-0037 pending-drops trigger)** — call `notifications::record` from `transfer_host_or_close` and `close_stale_sessions`. Depends on ADR-0037's `dropped_at` column landing first; in practice will likely co-ship.
4. **Frontend** — bell icon, badge from `unread-count`, dropdown that lists `GET /me/notifications`, render function for `PendingRacesDropped`.

Steps 1–3 can ship as a single backend PR (or be split if smaller is easier). Step 4 is a separate frontend PR.

- [x] Approved
- [ ] Needs discussion

## Sign-off

Each section above carries its own approval checkbox. Once all are checked:

- Set `status` in this file's frontmatter to `accepted`.
- Update the row for 0038 in `docs/decisions/README.md` from `proposed` to `accepted`.
- File implementation Issues under Milestone Star for the MVP scope.
- Add forward-pointer bullets to `docs/roadmap.md`'s Banana and Leaf scope sections for the future notification variants (`TrackRecordLost` and `LeaderboardRankChanged` under Banana, `H2hLeadChanged` under Leaf). Per the roadmap convention, future-cup work lives as scope bullets, not Issues — those bullets become Issues only when the cup goes active.

## Links

- Source: ad-hoc (Cowork chat discussion, 2026-05-16)
- Companion: [ADR-0037](./0037-pending-races-dropped-on-session-close.md) — the first consumer of this system; ships in parallel.
- Related ADRs: 0018 (polling, not WebSockets — confirms the delivery pattern), 0028 (TEXT timestamps — JSON payload follows the same shape), 0031 (auth — `/me` scoping), 0032 (cursor pagination — applied to `GET /me/notifications`).
- Partially superseded by: [ADR-0039](./0039-api-client-generation.md) — the "Frontend type agreement" subsection above named `ts-rs`/`typeshare` as the leading codegen candidates and used "10–15 hand-maintained interfaces" as the revisit trigger. Both are superseded: the candidates failed on the `nutype` newtype convention (see ADR 0039's analysis), and the trigger has been reframed from interface count to "Zod-maintenance friction" with `schemars + json-schema-to-zod + brand-mint overlay` as the at-threshold path. The "hand-write notification types for MVP" decision itself stands.
- Implementing PRs: <to fill in>
