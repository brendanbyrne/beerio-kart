# Beerio Kart — API Contract

> **Scope.** The wire contract between the Axum backend and the eventual React frontend. Two parts:
> - § 1 catalogs the endpoints (paths, methods, purpose, query parameters).
> - § 2 onward captures cross-cutting decisions that shape the contract — error format, polling, idempotency, time format, etc.
>
> The endpoint catalog is the source-of-truth listing — route handlers must implement what's here. The convention sections were decided early because retrofitting any of them after 30 endpoints exist is expensive.
> **Status.** Living document. Convention sections (§§ 2–10) constrain backend implementation; § 1 is updated when endpoints are added, removed, or renamed.

The companion documents are in [`coding-standards/`](./coding-standards/). Where this doc says "in service code, do X," that X is also expected to follow the rules there.

---

## 1. Endpoint catalog

All endpoints are prefixed with `/api/v1`.

The frontend never touches the database directly — it makes HTTP requests to the Rust server, which validates input, runs business logic, and returns JSON. This follows REST conventions: resources (runs, tracks, users) are nouns in the URL, HTTP methods (GET, POST, PUT, DELETE) are the verbs.

For future flexibility (querying data in ways not yet enumerated), the runs endpoint supports generous query parameters for filtering, sorting, and pagination. If this becomes insufficient, a GraphQL layer (`async-graphql` crate) can be added alongside REST later.

### 1.1 Auth

Uses established Rust crates — not rolling crypto from scratch. `argon2` for password hashing, `jsonwebtoken` for JWT tokens. ~150 lines of code wrapping audited libraries. Sufficient for a self-hosted friends-and-game-night app. Account recovery is admin-reset for now.

```
POST   /auth/register              Create account (username, password), returns access token + sets refresh cookie
POST   /auth/login                 Returns access token + sets refresh cookie
POST   /auth/refresh               Rotate refresh cookie for a new access token (reuse detection — § 4)
POST   /auth/logout                Clears refresh cookie, bumps refresh_token_version, clears token families
PUT    /auth/password              Change own password (requires current password)
```

### 1.2 Users

```
GET    /users                      List all users (public profiles)
GET    /users/:id                  Get user profile + preferred race setup
PUT    /users/:id                  Update profile / preferred race setup / preferred drink (self only)
```

### 1.3 Pre-seeded data (read-only)

```
GET    /characters                 List all characters
GET    /bodies                     List all vehicle bodies
GET    /wheels                     List all wheel sets
GET    /gliders                    List all gliders
GET    /cups                       List all cups
GET    /cups/:id                   Get cup with its tracks
GET    /tracks                     List all tracks (optional filter: cup_id)
GET    /tracks/:id                 Get track details
```

### 1.4 Drink types

```
POST   /drink-types                Create a new drink type (returns existing on (name, alcoholic) collision)
GET    /drink-types                List all drink types (optional filter: alcoholic)
GET    /drink-types/:id            Get drink type details
```

A drink type is identified by `(name, alcoholic)`, matched case-insensitively on the name. Re-submitting an existing `(name, alcoholic)` pair returns the original row with `200` (idempotent — no `409`). The same name with a *different* alcoholic flag is a distinct drink (e.g. alcoholic vs non-alcoholic "Punch") and creates a new row.

### 1.5 Sessions

```
POST   /sessions                   Create a new session (choose ruleset)
GET    /sessions                   List active sessions (sorted by most recent activity)
GET    /sessions/:id               Get session details (participants, current race, state)
POST   /sessions/:id/join          Join a session (dedicated endpoint — designed for future password support)
POST   /sessions/:id/leave         Leave a session (if you were the last to leave, the session closes and unresolved pending races are dropped)
POST   /sessions/:id/next-track    Trigger next track selection (host or chooser, depending on ruleset)
POST   /sessions/:id/choose-track  Choose a specific track (for rulesets where a player picks)
POST   /sessions/:id/skip-turn     Pass the chooser's turn to the next person (any participant can trigger)
GET    /sessions/:id/races         List all races in a session (with submission status per participant)
POST   /sessions/:id/races/:race_id/skip   Mark a pending race as skipped for the requesting user (idempotent)
```

Session state is consumed via polling — clients call `GET /sessions/:id` every 2-3 seconds to pick up joins, leaves, new races, and submissions. For a turn-based game where events happen every few minutes, polling latency is imperceptible. § 4 specifies the ETag protocol that keeps polling cheap. WebSockets can be added later as an optimization if polling ever feels sluggish.

### 1.6 Runs

```
POST   /runs                       Record a new run (requires session_race_id; auto-flags if record-breaking without photo)
GET    /runs                       Query runs (filters: user_id, track_id, session_race_id, drink_type_id,
                                               alcoholic, disqualified, after, before, sort, limit, cursor)
GET    /runs/:id                   Get a specific run
DELETE /runs/:id                   Delete a run (owner or admin)
PUT    /runs/:id                   Edit a run (admin only, 403 for regular users)
POST   /runs/:id/photo             Upload photo for a run (auto-resolves record flag if present)
POST   /runs/:id/flag              Flag a run for review (owner only, requires photo on run)
```

`GET /runs/suggest-track` was removed when track coordination became a session concern.

### 1.7 Stats

```
GET    /stats/personal/:user_id                    Personal summary (total runs, most-played, best track, rank)
GET    /stats/personal/:user_id/track/:track_id    Per-track breakdown (PB, average, time history)
GET    /stats/personal/:user_id/sessions           Session history (date, participants, race count, personal W-L)
GET    /stats/leaderboard/global                   Global leaderboard (most track records held)
GET    /stats/leaderboard/cup/:cup_id              Cup-level leaderboard
GET    /stats/leaderboard/track/:track_id          Track leaderboard (best time per user)
GET    /stats/rivals/:user_id                      Players you've competed with (derived from shared session races)
GET    /stats/head-to-head/:user_id_1/:user_id_2   H2H record between two players (derived from session races)
```

All leaderboard endpoints accept `?alcoholic=true|false|all` to filter by drink category. Default matches the requesting user's preferred drink category. DQ'd runs are excluded from leaderboard calculations.

### 1.8 Notifications

```
GET    /me/notifications               List the caller's notifications, newest first (unread only; ?include_read=true for all)
GET    /me/notifications/unread-count  Cheap unread tally for the home-screen badge — returns { "count": N }
POST   /me/notifications/read-all      Mark all of the caller's unread notifications as read (204 No Content)
```

Per-user inbox of asynchronous events ([ADR-0038](./decisions/0038-notifications-system.md)). All three endpoints scope to the authenticated user. The first (MVP) event kind is `pending_races_dropped` — emitted when a session closes around unresolved pending races ([ADR-0037](./decisions/0037-pending-races-dropped-on-session-close.md)). `GET /me/notifications` is capped at 100 rows; cursor pagination per [ADR-0032](./decisions/0032-cursor-based-pagination.md) is a project-wide follow-up (no list endpoint uses keyset pagination yet — `GET /runs` shares the flat cap).

### 1.9 Admin

```
GET    /admin/flags                List unresolved flags (admin only)
PUT    /admin/flags/:id            Resolve a flag (admin only)
```

---

## 2. Error response contract

- **Decision:** Error responses include both an HTTP status code and a stable machine-readable `code` field. Shape:
  ```json
  { "error": "Session is closed.", "code": "session_closed" }
  ```
  The `error` field is human-readable (may change wording without notice). The `code` field is a stable string the frontend matches on — once consumed by a client, **renaming a code is a breaking change**.
- **Why:** Status code alone (`409 Conflict`) doesn't tell the frontend whether to render "session is closed, start a new one" or "username is taken, pick another" — both are 409. Free-text messages force the frontend to either show raw backend text (bad UX) or pattern-match on substrings (brittle). A stable code lets the frontend pick the right copy and the right recovery action without coupling to backend wording.
- **Implementation:** Emitted by `IntoResponse` for [`error::Error`](../backend/src/error.rs). Codes come from the [`ErrorCode`](../backend/src/error.rs) enum (variant per registry row, `#[serde(rename_all = "snake_case")]` serialization). The full design — per-variant-enum vs. argument, hybrid helper API — is documented in [ADR 0036](./decisions/0036-error-code-rollout.md).
- **Trade-offs considered:**
  - **`code` as enum vs. `code` as string:** A wire-side string is friendlier to OpenAPI (easy to declare as `enum: [...]` in a schema) and to the frontend (no code-gen ceremony for a one-off compare). The backend stores it as a typed `ErrorCode` enum and serializes to snake_case strings — best of both.
  - **RFC 7807 (`application/problem+json`):** Considered. It's well-specified but adds `type` (URI), `title`, `status`, `detail`, `instance` fields most of which we don't need. For an internal app, the simpler `{ error, code }` is enough.
- **Source:** <https://datatracker.ietf.org/doc/html/rfc7807> (for context, even though we're not adopting it)

---

## 3. Polling & conditional GETs

- **Decision:** `GET /sessions/:id` (the polling endpoint, called every 2–3s per § 1.5) supports `ETag` / `If-None-Match`. The backend computes a strong ETag from session state; clients sending `If-None-Match: <etag>` get `304 Not Modified` with an empty body when nothing's changed.
- **Why:** Without conditional GETs, every poll transfers the full session state — participants, races, submission status, pending lists. With ~10 active users polling a multi-participant session every 2s, that's a lot of redundant JSON. A 304 response is dozens of bytes. On mobile, the bandwidth and battery savings are real; on the server, the CPU savings are smaller but real.
- **Implementation:**
  - Compute the ETag as a hash of six values:
    1. `session.status`
    2. `COALESCE(MAX(session_races.created_at), '1970-01-01T00:00:00Z')` for this session
    3. `COALESCE(MAX(runs.created_at), '1970-01-01T00:00:00Z')` where the run's `session_race.session_id = :id`
    4. `COALESCE(MAX(session_participants.joined_at), '1970-01-01T00:00:00Z')` for this session
    5. `COALESCE(MAX(session_participants.left_at), '1970-01-01T00:00:00Z')` for this session
    6. `COALESCE(MAX(session_race_participations.skipped_at), '1970-01-01T00:00:00Z')` for races in this session
  - **Why these six.** They are the derived-data signals for every state mutation: status change, new race, new run, join, leave/rejoin (rejoin clears the leaver's `left_at` to NULL, which changes the MAX-of-remaining), and skip-pending-race. ADR-0035 had also added a seventh input — a `FLOOR(NOW() / 60)` one-minute time bucket — because the per-race 1-hour expiry was a time-based invalidation no stored data captured. [ADR-0037](./decisions/0037-pending-races-dropped-on-session-close.md) removed that per-race timer: a pending race now drops out only when `dropped_at` is stamped at session close, which already moves either `session.status` (clean last-leave) or `session_participants.left_at` (the leave that triggered the close) — both already inputs. No time-based predicate silently ages a row out anymore, so the bucket no longer earns its keep and is dropped. The six inputs here are the same activity signals the stale-session sweeper's predicate enumerates (see `close_stale_sessions`); the two formulas share a maintenance contract — a new activity-producing endpoint means adding inputs to both.
  - Use `BLAKE3` (or `xxhash`) of the six values concatenated with a separator; format as `W/"<hex>"` (weak ETag, since two semantically-equivalent responses might serialize differently). `COALESCE` keeps NULL inputs from causing spurious churn.
  - In the handler: compute the six inputs *before* loading the full state (one query for the `MAX(...)` aggregates, one for the session row). If `If-None-Match` matches, return 304 immediately. Otherwise load and return 200 with the new ETag in the response header.
  - Apply the same pattern only to `GET /sessions/:id` for now — it's the only high-frequency endpoint. Other GETs don't need it.
- **Trade-offs considered:**
  - **WebSockets / SSE:** § 1.5 already decided polling. ETags make polling cheap enough that we don't need to revisit.
  - **`Last-Modified` header alone:** Less robust because it's second-resolution; multiple state mutations can land within the same wall-clock second and would collide.
  - **Hash the participant set directly** instead of MAX(joined_at) + MAX(left_at): strictly more precise but more expensive per poll (table scan + digest) without practical added coverage at this scale. MAX-of-`left_at` correctly captures re-leaves and rejoins because clearing one user's `left_at` to NULL changes the MAX-of-remaining.
  - **Reintroduce a maintained `sessions.updated_at`** column bumped on every membership / skip event: rejected as the same anti-pattern ADR-0035 removed (maintained state instead of derived).
  - **Defer the ETag spec until scale demands it.** Reasonable position — at single-friend-group scale the savings are theoretical. Kept the spec in place because the design cost is small and the implementation can ride a later perf-pass PR.
- **Source:** <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag>, <https://developer.mozilla.org/en-US/docs/Web/HTTP/Conditional_requests>

---

## 4. Auth refresh flow

- **Decision:** Access tokens carry their expiry visibly so the frontend can refresh *proactively* (~30s before expiry) rather than *reactively* (after a 401). The decoded JWT already includes `exp`; the frontend reads it from the access token after login/refresh. No backend change needed beyond what already exists.
- **Why:** Reactive-only refresh produces user-visible jank: a request fails with 401, the interceptor refreshes, the request retries. For a polling app where multiple requests are in flight at all times, you can hit the 401 storm pattern (10 requests fail simultaneously, all trigger refresh, race conditions, retries multiply). Proactive refresh — single timer that fires before expiry — eliminates the storm.
- **Implementation:**
  - **Backend:** Confirm `exp` is set on the access token JWT (it is — short-lived 15–30 min per ADR 0031). No change required.
  - **Backend:** Document the access token expiry window prominently — frontend authors need to know whether to schedule the refresh 30s or 60s before expiry.
  - **Frontend (deferred):** Decode the JWT (no signature verification — the frontend just reads `exp`). Schedule a `setTimeout` to call `POST /auth/refresh` ~30s before expiry. On refresh response, reset the timer.
  - **Frontend (deferred):** Keep the reactive 401-handler too as a fallback for clock skew or stalled timers.
- **Why no `Authorization` token expiry header:** Some APIs return `X-Token-Expires-At`. Unnecessary here — the JWT itself has `exp`, and parsing the access token client-side is one line of base64 + JSON.
- **Source:** ADR 0031 (refresh-token auth); <https://datatracker.ietf.org/doc/html/rfc7519#section-4.1.4>

#### Rotation with reuse detection (ADR-0040)

- **Decision:** `POST /auth/refresh` rotates the refresh token with **reuse detection**, per [ADR-0040](./decisions/0040-refresh-token-reuse-detection.md) ([RFC 9700](https://datatracker.ietf.org/doc/rfc9700/) § 4.14). Each login starts a *token family*; each refresh mints a successor token (new `jti`) in that family and marks its predecessor used. Replaying an already-used token past a grace window is treated as theft: the whole family is revoked and the call returns `401 token_reuse_detected`. Server-side state lives in the `refresh_tokens` table ([`data-model.md`](./data-model.md) § Refresh Tokens).
- **Why:** Plain re-issue can't tell a legitimate rotation from a stolen-token replay. Per-token state lets a stolen refresh token become unusable once the real client refreshes, and makes the replay detectable — the RFC 9700 property for this client class.
- **Behavior by case (the response a client sees):**
  - **Live token** → `200` with a new access token + a rotated refresh cookie (a different value every time — the `jti` makes tokens byte-distinct).
  - **Used token, within the grace window** (`refresh_grace_seconds`, default 10s) → `200`, reissuing the family's current live successor. This is the backstop for concurrent / retried refreshes — they don't trip detection.
  - **Used token, past the grace window** → `401 token_reuse_detected`; the family is revoked, so its current live token stops working too. The client must re-authenticate.
  - **Revoked / unknown / expired** → `401 token_invalid` or `token_expired`, as before. The global `refresh_token_version` (bumped on logout / password change) still revokes every family at once.
- **Frontend (deferred to the follow-up PR):** Make the silent-refresh interceptor **single-flight** — concurrent 401s queue behind one in-flight refresh so the app never fires parallel refreshes (the client-side half of the grace-window mitigation). Treat `token_reuse_detected` as a hard logout with a "signed out for security" message, distinct from an ordinary expiry re-login.
- **Source:** [ADR-0040](./decisions/0040-refresh-token-reuse-detection.md); [RFC 9700](https://datatracker.ietf.org/doc/rfc9700/) § 4.14.

---

## 5. Idempotency keys

- **Decision:** Mutating endpoints that are vulnerable to retry storms accept an `Idempotency-Key` request header. The backend keys deduplication on `(user_id, endpoint, idempotency_key)` and returns the original response on a duplicate. Initial scope: `POST /runs`, `POST /sessions/:id/next-track`, `POST /sessions/:id/choose-track`.
- **Why:** Issue [#75](https://github.com/brendanbyrne/beerio-kart/issues/75) tracks the canonical case — the double-tap on `next-track` hits the `UNIQUE(session_id, race_number)` constraint and returns 500. On mobile networks, retries from the frontend (timeout, network drop) are routine; without idempotency keys, a successfully-processed request whose response was lost on the network gets duplicated when the client retries.
- **Implementation:**
  - Frontend generates a UUID v4 per logical action and sends it in `Idempotency-Key`. Same retry → same key. New action → new key.
  - Backend stores `(idempotency_key, user_id, endpoint, response_status, response_body, created_at)` in a small `idempotency_records` table. Index on `(idempotency_key, user_id, endpoint)`.
  - On request: if the key exists for this (user, endpoint), return the stored response verbatim. Otherwise process normally and store the result.
  - TTL: 24 hours. A nightly cleanup task drops expired records.
  - Scope: only the three POST endpoints listed above. Other endpoints (login, register) aren't worth the storage; reads (GETs) are naturally idempotent.
- **Trade-offs considered:**
  - **Skip and rely on database constraints:** That's what's happening today, and the 500 in #75 is the result. Idempotency keys turn the retry into a deterministic 200 (with the original response), not a 500.
  - **Stripe-style key uniqueness with conflict semantics:** Stripe rejects requests where the key matches but the request body differs. We don't need that complexity — first-write-wins is fine for our request shapes.
- **Source:** <https://stripe.com/docs/api/idempotent_requests>, <https://datatracker.ietf.org/doc/html/draft-ietf-httpapi-idempotency-key-header>

---

## 6. Time format

- **Decision:** All timestamps cross the wire as **ISO 8601 with explicit UTC offset** — `"2026-05-02T14:32:11.123Z"` (RFC 3339 subset, with the literal `Z`, including milliseconds). Frontend parses with `new Date(...)` (which handles ISO 8601 natively) and formats locally for display.
- **Why:** Storing as TEXT in SQLite (per [`data-model.md`](./data-model.md)) means the on-the-wire format is a serialization choice, not a constraint. ISO 8601 / RFC 3339 is unambiguous, sortable as text, supported by every JSON library on both sides, and human-readable for debugging. Epoch seconds and epoch milliseconds are both popular but lose timezone information at the type level (you have to remember which one you're looking at) and aren't human-readable.
- **Implementation:**
  - Backend uses `chrono::DateTime<Utc>` everywhere. Serde via `chrono`'s default `Serialize` impl produces ISO 8601 with the `Z` suffix.
  - Be explicit about the precision: ms (3 digits). Configure if the default differs.
  - Frontend stores all times as JS `Date` objects internally; format at render time using the user's locale.
  - **Never** display raw ISO strings in the UI.
- **Trade-offs considered:**
  - **Epoch ms:** Smaller payload, but unreadable in logs and DB inspection. The savings (8 bytes vs. ~24 bytes per timestamp) don't matter at our scale.
  - **Database driver-native types:** SQLite has no native timestamp type, so we'd be inventing an encoding anyway. ISO 8601 is the obvious encoding.
- **Source:** <https://datatracker.ietf.org/doc/html/rfc3339>

---

## 7. Error code registry

The list of stable `code` values returned in error responses. Add to this list when a new error case is introduced.

| HTTP | Code | Meaning |
|------|------|---------|
| 400 | `bad_request` | Generic validation failure (free-text message). |
| 400 | `lap_times_mismatch` | Lap times don't sum to total time. |
| 400 | `track_id_mismatch` | Submitted `track_id` doesn't match the `session_race`'s track. |
| 400 | `invalid_path_param` | URL path segment failed to parse into a typed `Path<T>` extractor. |
| 400 | `invalid_request_body` | JSON body failed to parse or deserialize. |
| 401 | `invalid_credentials` | Login failed. |
| 401 | `token_expired` | Access token expired (frontend should refresh). |
| 401 | `token_invalid` | Token malformed or signature mismatch. |
| 401 | `token_reuse_detected` | A rotated (already-used) refresh token was replayed past the grace window; its family is revoked. Frontend shows "signed out for security" (§ 4, ADR-0040). |
| 403 | `forbidden` | Authenticated but not authorized for this action. |
| 403 | `admin_required` | Endpoint requires admin. |
| 404 | `not_found` | Generic "resource doesn't exist." |
| 409 | `username_taken` | Registration conflict. |
| 409 | `session_closed` | Submission to a closed session. |
| 409 | `pending_races_first` | Must resolve pending races before current race. |
| 409 | `out_of_order_submission` | Pending race must be submitted in order. |
| 409 | `race_number_conflict` | Concurrent `next-track` race lost (idempotency-key retry will return the winning response). |
| 422 | `unprocessable` | Body parsed but failed semantic validation (catch-all). |
| 500 | `internal` | Unexpected. Frontend shows generic message. |
| 504 | `gateway_timeout` | Per-call timeout budget elapsed; retry is safe. |

---

## 8. Versioning

- **Decision:** Path-based versioning (`/api/v1/...`) — already in § 1. A breaking change ships as `/api/v2/...`; the v1 endpoints continue to serve until all clients have migrated.
- **Why:** Restated here so it doesn't get lost. The cost of v2 alongside v1 is low for an internal app — the duplication is mostly a thin layer that translates to the v2-internal types.
- **What counts as breaking:** removing a field, renaming a field, changing a field's type, changing an error `code`, changing default behavior of a query parameter.
- **What does NOT count as breaking:** adding a new field (frontend tolerates extra fields), adding a new endpoint, loosening validation, fixing a bug where the response shape was wrong.
- **Prelaunch carve-out.** "Launch" mirrors the definition in [`seaorm.md` § 5](./coding-standards/seaorm.md#5-migrations): the first deploy where we have clients we owe backwards compatibility to. While prelaunch, breaking changes ship directly in `/api/v1` — we don't spin up `/api/v2` to preserve compatibility we don't owe anyone. As of May 2026 the frontend is still being built and there are no deployed external clients, so the project is prelaunch by this definition. Once a real client ships (production frontend, third-party integration), this carve-out closes and breaking changes go through the v2 path. CLAUDE.md will be updated at that time.

---

## 9. CORS

- **Decision:** Same-origin in production (Axum serves the frontend bundle, per [`design.md`](./design.md) § Architecture). No CORS middleware in the production binary. In dev mode (Vite dev server on a different port), enable a permissive CORS layer scoped to the dev origin.
- **Why:** Same-origin avoids the entire CORS surface area and removes preflight latency from every non-trivial request. The dev-only loosening is needed because Vite serves on `:5173` while the API runs on `:3000`.
- **Implementation:**
  - Wrap the dev-mode CORS layer in `if cfg!(debug_assertions) || env::var("DEV_CORS").is_ok() { ... }`.
  - Use `tower_http::cors::CorsLayer::new().allow_origin(...).allow_credentials(true)` with the explicit dev origin — never `Any`.
- **Source:** <https://docs.rs/tower-http/latest/tower_http/cors/index.html>

---

## 10. Document history

- 2026-05-02 — Initial draft. Sets API client generation, error code contract, polling/ETag, refresh flow, idempotency, time format, error code registry, versioning, CORS. The first six (API client generation through Time format) are the "decide before the backend gets much further" set; the rest (Error code registry, Versioning, CORS) are clarifications of decisions that were already made or implied. To be revisited when the frontend work starts.
- 2026-05-02 — Added prelaunch carve-out to the Versioning section: while prelaunch, breaking changes ship in `/api/v1` directly rather than spinning up a v2 path. Mirrors the "launch" definition in `seaorm.md` § 5.
- 2026-05-06 — Merged the API Surface section from `design.md` as new § 1 "Endpoint catalog". Convention sections renumbered: previous §§ 1–9 are now §§ 2–10; previous § 10 (history) is now § 11. Internal cross-references updated: § 3 (Error response contract) "see § 7 below for the registry" → "see § 8 below"; § 4 (Polling) and § 6 (Idempotency) updated to reference § 1.5 / Issue #75 instead of `design.md` callouts. Top-of-document scope statement expanded to cover both catalog and conventions. ADR 0031 reference added to § 5 to replace the `design.md` "Auth token strategy" pointer. PR 4 of the docs restructure.
- 2026-05-10 — Renamed `AppError` → `error::Error` in the § 3 (error response contract) prose and example. Companion to the module-name-repetition cleanup in PR-H1+ (d). PR #103 sequence.
- 2026-05-15 — § 8 error code registry: added `504 | gateway_timeout` for the per-call DB timeout path introduced in PR-F4. The `code` field is deferred per § 3, so this isn't a wire-contract change today — the registry already documents codes ahead of implementation (e.g., `lap_times_mismatch`), and adding this row avoids drift when the `code` field eventually lands. PR [#155](https://github.com/brendanbyrne/beerio-kart/pull/155).
- 2026-05-15 — `code` field rollout (#157). § 3 rewritten: dropped the speculative `Implementation:` block (the codebase shape is now real); references to "deferred" replaced with the actually-emitted shape; pointer to ADR 0036 added for the design rationale. § 8 grew two rows for the path/json extractor failures (`invalid_path_param`, `invalid_request_body`) added by the custom extractors that closed #146 as part of #157. The `code` field is now emitted on every error response.
- 2026-05-16 — ADR-0037 + ADR-0038. New § 1.8 Notifications endpoint group (`GET /me/notifications`, `GET /me/notifications/unread-count`, `POST /me/notifications/read-all`); Admin renumbered 1.8 → 1.9. § 4 ETag formula dropped from seven inputs to six — the `FLOOR(NOW() / 60)` time bucket is removed now that ADR-0037 deleted the per-race expiry timer (no time-based predicate silently ages a pending row out anymore). § 1.5 `POST /sessions/:id/leave` description updated: leaving as the last participant closes the session and drops unresolved pending races. Issues [#58](https://github.com/brendanbyrne/beerio-kart/issues/58), [#164](https://github.com/brendanbyrne/beerio-kart/issues/164).
- 2026-05-21 — Deleted § 2 "API client generation" (an aspirational `utoipa` + `openapi-fetch` plan from the 2026-05-02 initial draft, never implemented; the actual frontend is hand-rolled per-endpoint with Zod schemas at the boundary, per [`coding-standards/typescript.md`](./coding-standards/typescript.md) § 8 and PR-B2 / Issue [#191](https://github.com/brendanbyrne/beerio-kart/issues/191)). Renumbered the remaining sections: previous §§ 3–11 are now §§ 2–10 (Error response contract through Document history). The decision now lives in [ADR-0039](./decisions/0039-api-client-generation.md), which captures both the current hand-rolled state and the at-threshold codegen path (`schemars` + [`json-schema-to-zod`](https://www.npmjs.com/package/json-schema-to-zod) + brand-mint overlay). Cross-references swept across `design.md`, `coding-standards/typescript.md`, `research/rust-to-ts-codegen.md`, `decisions/0036-error-code-rollout.md`, `decisions/0037-pending-races-dropped-on-session-close.md`, `designs/2026-05-16-frontend-compliance-plan.md`, `backend/CLAUDE.md`, and `frontend/CLAUDE.md`. Code-file and `.claude/skills/` cross-references handed off to Claude Code via `.agents/handoffs/claude-code.md` (Cowork's sandbox blocks `.claude/` writes and is a docs-only assistant for `.rs`/`.ts` files).
- 2026-05-27 — § 1.4 drink-type identity changed from name-only to `(name, alcoholic)`. The deterministic UUID now derives from both fields, so the alcoholic and non-alcoholic forms of the same name are distinct drinks instead of colliding (the old name-only key silently returned the existing row and discarded the submitted flag). Dedup on a matching `(name, alcoholic)` pair still returns the existing row with 200; case-insensitive name matching preserved. Backend change in Issue [#212](https://github.com/brendanbyrne/beerio-kart/issues/212), surfaced during PR-E1 ([#211](https://github.com/brendanbyrne/beerio-kart/pull/211)) smoke testing.
- 2026-05-31 — § 9 CORS: repointed the `design.md` reference from § Tech Stack to § Architecture, following the design.md slim-down (#220/#223) that replaced the Tech Stack table with § Architecture.
- 2026-06-01 — § 4 gained a "Rotation with reuse detection (ADR-0040)" sub-section and § 7 gained the `401 token_reuse_detected` code; § 1.1 refresh/logout one-liners updated. Refresh now rotates with per-token reuse detection (token families + `jti`) per [ADR-0040](./decisions/0040-refresh-token-reuse-detection.md) / Issue [#226](https://github.com/brendanbyrne/beerio-kart/issues/226); the single-flight client + reuse-message handling are deferred to the follow-up frontend PR.
