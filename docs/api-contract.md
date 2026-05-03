# Beerio Kart — API Contract Conventions

> **Scope.** Cross-cutting decisions that shape the contract between the Axum backend and the eventual React frontend. These are decided **now**, while the backend is still being built, because retrofitting any of them after 30 endpoints exist is expensive.
> **Format.** Each item: Decision / Why / Implementation / Source.
> **Status.** Living document. The broader frontend coding standard comes later — this is just the wire-format and protocol pieces that constrain backend work.

The companion documents are in [`coding-standards/`](./coding-standards/). Where this doc says "in service code, do X," that X is also expected to follow the rules there.

---

## 1. API client generation

- **Decision:** The backend emits an OpenAPI 3.x spec from `utoipa` annotations on every route handler. The frontend consumes a generated TypeScript client (`openapi-typescript-codegen` or `openapi-fetch`) — no hand-rolled `fetch` wrappers per endpoint.
- **Why:** design.md lists ~40 endpoints. Hand-rolling a typed client and keeping it in sync with `AppError`'s shape, request/response DTOs, and query-parameter quirks is a part-time job. With `utoipa`, the spec is derived from the same Rust types that handlers already use, so drift is impossible by construction. The cost is ~5 lines of `#[utoipa::path(...)]` per handler — cheap if added as routes are written, painful to retrofit.
- **Implementation:**
  - Add `utoipa = { version = "5", features = ["axum_extras", "uuid", "chrono"] }` and `utoipa-axum = "0.2"` to `backend/Cargo.toml`.
  - Each route handler gets a `#[utoipa::path(...)]` attribute describing method, path, request body, responses, and tags.
  - All request DTOs and response DTOs derive `utoipa::ToSchema`.
  - `main.rs` exposes the spec at `/api/v1/openapi.json` (and optionally Swagger UI at `/api/v1/docs` in dev mode only).
  - Frontend (when it lands) runs a Bun script in CI to regenerate the client from `/api/v1/openapi.json` and commits the diff.
- **Trade-offs considered:**
  - **`utoipa` vs. `apistos` vs. `aide`:** `utoipa` has the largest ecosystem and the cleanest Axum integration via `utoipa-axum`. `aide` is also good but its DSL is more invasive.
  - **`openapi-typescript-codegen` vs. `openapi-fetch`:** `openapi-fetch` is smaller (no class hierarchy) and uses TypeScript's type system for path parameters at the call site. Recommend `openapi-fetch`.
  - **Hand-rolled:** Considered and rejected — fine for 5 endpoints, untenable past 15.
- **Source:** <https://docs.rs/utoipa>, <https://docs.rs/utoipa-axum>, <https://github.com/openapi-ts/openapi-typescript>

---

## 2. Error response contract

- **Decision:** Error responses include both an HTTP status code and a stable machine-readable `code` field. Shape:
  ```json
  { "error": "Session is closed.", "code": "session_closed" }
  ```
  The `error` field is human-readable (may change wording without notice). The `code` field is a stable string the frontend matches on (changing a code is a breaking change).
- **Why:** Status code alone (`409 Conflict`) doesn't tell the frontend whether to render "session is closed, start a new one" or "username is taken, pick another" — both are 409. The current `AppError` variants carry a free-text message, which forces the frontend to either show raw backend text (bad UX) or pattern-match on substrings (brittle). A stable code lets the frontend pick the right copy and the right recovery action without coupling to backend wording.
- **Implementation:**
  - Add a `code: &'static str` to each `AppError` variant, or better — split `AppError` into a flat enum where each variant carries its own code:
    ```rust
    #[derive(thiserror::Error, Debug)]
    #[non_exhaustive]
    pub enum AppError {
        #[error("Session is closed.")]
        SessionClosed,
        #[error("Username already taken.")]
        UsernameTaken,
        #[error("{0}")]
        BadRequest(String),
        // ...
    }

    impl AppError {
        fn code(&self) -> &'static str {
            match self {
                Self::SessionClosed => "session_closed",
                Self::UsernameTaken => "username_taken",
                Self::BadRequest(_) => "bad_request",
                // ...
            }
        }
        fn status(&self) -> StatusCode { /* ... */ }
    }
    ```
  - The `IntoResponse` impl serializes both fields.
  - Catalog the codes in this doc as they're added (see § 7 below for the registry).
- **Trade-offs considered:**
  - **`code` as enum vs. `code` as string:** String is friendlier to the OpenAPI spec (easy to declare as `enum: [...]` in a schema) and to the frontend (no code-gen ceremony for a one-off compare).
  - **RFC 7807 (`application/problem+json`):** Considered. It's well-specified but adds `type` (URI), `title`, `status`, `detail`, `instance` fields most of which we don't need. For an internal app, the simpler `{ error, code }` is enough.
- **Source:** <https://datatracker.ietf.org/doc/html/rfc7807> (for context, even though we're not adopting it)

---

## 3. Polling & conditional GETs

- **Decision:** `GET /sessions/:id` (the polling endpoint, called every 2–3s per design.md) supports `ETag` / `If-None-Match`. The backend computes a strong ETag from session state; clients sending `If-None-Match: <etag>` get `304 Not Modified` with an empty body when nothing's changed.
- **Why:** Without conditional GETs, every poll transfers the full session state — participants, races, submission status, pending lists. With ~10 active users polling a multi-participant session every 2s, that's a lot of redundant JSON. A 304 response is dozens of bytes. On mobile, the bandwidth and battery savings are real; on the server, the CPU savings are smaller but real.
- **Implementation:**
  - Compute the ETag as a hash of `(session.last_activity_at, session.status, max(session_races.created_at), max(runs.created_at where session_race.session_id = :id))`. Anything that would change the response body changes one of those four timestamps.
  - Use a `BLAKE3` or `xxhash` hash of the four values; format as `W/"<hex>"` (weak ETag, since two semantically-equivalent responses might serialize differently).
  - In the handler: compute the ETag *before* loading the full state. If `If-None-Match` matches, return 304 immediately. Otherwise load and return 200 with the new ETag in the response header.
  - Apply the same pattern only to `GET /sessions/:id` for now — it's the only high-frequency endpoint. Other GETs don't need it.
- **Trade-offs considered:**
  - **WebSockets / SSE:** design.md already decided polling. ETags make polling cheap enough that we don't need to revisit.
  - **`Last-Modified` header alone:** Less robust because it's second-resolution (design.md mentions `last_activity_at` updates within seconds of joins/leaves; second-resolution can race).
- **Source:** <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag>, <https://developer.mozilla.org/en-US/docs/Web/HTTP/Conditional_requests>

---

## 4. Auth refresh flow

- **Decision:** Access tokens carry their expiry visibly so the frontend can refresh *proactively* (~30s before expiry) rather than *reactively* (after a 401). The decoded JWT already includes `exp`; the frontend reads it from the access token after login/refresh. No backend change needed beyond what already exists.
- **Why:** Reactive-only refresh produces user-visible jank: a request fails with 401, the interceptor refreshes, the request retries. For a polling app where multiple requests are in flight at all times, you can hit the 401 storm pattern (10 requests fail simultaneously, all trigger refresh, race conditions, retries multiply). Proactive refresh — single timer that fires before expiry — eliminates the storm.
- **Implementation:**
  - **Backend:** Confirm `exp` is set on the access token JWT (it is — design.md says short-lived 15–30 min). No change required.
  - **Backend:** Document the access token expiry window prominently — frontend authors need to know whether to schedule the refresh 30s or 60s before expiry.
  - **Frontend (deferred):** Decode the JWT (no signature verification — the frontend just reads `exp`). Schedule a `setTimeout` to call `POST /auth/refresh` ~30s before expiry. On refresh response, reset the timer.
  - **Frontend (deferred):** Keep the reactive 401-handler too as a fallback for clock skew or stalled timers.
- **Why no `Authorization` token expiry header:** Some APIs return `X-Token-Expires-At`. Unnecessary here — the JWT itself has `exp`, and parsing the access token client-side is one line of base64 + JSON.
- **Source:** design.md "Auth token strategy" decision; <https://datatracker.ietf.org/doc/html/rfc7519#section-4.1.4>

---

## 5. Idempotency keys

- **Decision:** Mutating endpoints that are vulnerable to retry storms accept an `Idempotency-Key` request header. The backend keys deduplication on `(user_id, endpoint, idempotency_key)` and returns the original response on a duplicate. Initial scope: `POST /runs`, `POST /sessions/:id/next-track`, `POST /sessions/:id/choose-track`.
- **Why:** design.md already calls out the double-tap on `next-track` as a known race that hits the `UNIQUE(session_id, race_number)` constraint and returns 500. That's the canonical case. On mobile networks, retries from the frontend (timeout, network drop) are routine; without idempotency keys, a successfully-processed request whose response was lost on the network gets duplicated when the client retries.
- **Implementation:**
  - Frontend generates a UUID v4 per logical action and sends it in `Idempotency-Key`. Same retry → same key. New action → new key.
  - Backend stores `(idempotency_key, user_id, endpoint, response_status, response_body, created_at)` in a small `idempotency_records` table. Index on `(idempotency_key, user_id, endpoint)`.
  - On request: if the key exists for this (user, endpoint), return the stored response verbatim. Otherwise process normally and store the result.
  - TTL: 24 hours. A nightly cleanup task drops expired records.
  - Scope: only the three POST endpoints listed above. Other endpoints (login, register) aren't worth the storage; reads (GETs) are naturally idempotent.
- **Trade-offs considered:**
  - **Skip and rely on database constraints:** That's what's happening today, and the 500 in design.md's backlog is the result. Idempotency keys turn the retry into a deterministic 200 (with the original response), not a 500.
  - **Stripe-style key uniqueness with conflict semantics:** Stripe rejects requests where the key matches but the request body differs. We don't need that complexity — first-write-wins is fine for our request shapes.
- **Source:** <https://stripe.com/docs/api/idempotent_requests>, <https://datatracker.ietf.org/doc/html/draft-ietf-httpapi-idempotency-key-header>

---

## 6. Time format

- **Decision:** All timestamps cross the wire as **ISO 8601 with explicit UTC offset** — `"2026-05-02T14:32:11.123Z"` (RFC 3339 subset, with the literal `Z`, including milliseconds). Frontend parses with `new Date(...)` (which handles ISO 8601 natively) and formats locally for display.
- **Why:** Storing as TEXT in SQLite (per design.md) means the on-the-wire format is a serialization choice, not a constraint. ISO 8601 / RFC 3339 is unambiguous, sortable as text, supported by every JSON library on both sides, and human-readable for debugging. Epoch seconds and epoch milliseconds are both popular but lose timezone information at the type level (you have to remember which one you're looking at) and aren't human-readable.
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
| 401 | `invalid_credentials` | Login failed. |
| 401 | `token_expired` | Access token expired (frontend should refresh). |
| 401 | `token_invalid` | Token malformed or signature mismatch. |
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

---

## 8. Versioning

- **Decision:** Path-based versioning (`/api/v1/...`) — already in design.md. A breaking change ships as `/api/v2/...`; the v1 endpoints continue to serve until all clients have migrated.
- **Why:** design.md already chose this; restating so it doesn't get lost. The cost of v2 alongside v1 is low for an internal app — the duplication is mostly a thin layer that translates to the v2-internal types.
- **What counts as breaking:** removing a field, renaming a field, changing a field's type, changing an error `code`, changing default behavior of a query parameter.
- **What does NOT count as breaking:** adding a new field (frontend tolerates extra fields), adding a new endpoint, loosening validation, fixing a bug where the response shape was wrong.

---

## 9. CORS

- **Decision:** Same-origin in production (Axum serves the frontend bundle, per design.md). No CORS middleware in the production binary. In dev mode (Vite dev server on a different port), enable a permissive CORS layer scoped to the dev origin.
- **Why:** Same-origin avoids the entire CORS surface area and removes preflight latency from every non-trivial request. The dev-only loosening is needed because Vite serves on `:5173` while the API runs on `:3000`.
- **Implementation:**
  - Wrap the dev-mode CORS layer in `if cfg!(debug_assertions) || env::var("DEV_CORS").is_ok() { ... }`.
  - Use `tower_http::cors::CorsLayer::new().allow_origin(...).allow_credentials(true)` with the explicit dev origin — never `Any`.
- **Source:** <https://docs.rs/tower-http/latest/tower_http/cors/index.html>

---

## 10. Document history

- 2026-05-02 — Initial draft. Sets API client generation, error code contract, polling/ETag, refresh flow, idempotency, time format, error code registry, versioning, CORS. Items 1–6 are the "decide before the backend gets much further" set; 7–9 are clarifications of decisions that were already made or implied. To be revisited when the frontend work starts.
