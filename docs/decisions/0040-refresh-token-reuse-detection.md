---
status: accepted
date: 2026-05-31
deciders: [Brendan]
source: ad-hoc
---

# 0040 — Refresh-token rotation with reuse detection (token families + `jti`)

## Context and problem statement

ADR [0031](0031-auth-token-strategy.md) established the access + refresh token
model: a short-lived access JWT plus a long-lived refresh JWT in an `HttpOnly`,
`Secure`, `SameSite=Lax` cookie scoped to `/api/v1/auth/refresh`. Its rotation
sub-decision was "new token on every refresh, **no** version bump," and it
explicitly **deferred** per-token / per-device revocation as a trade-off.

Reviewing PR #225 surfaced a consequence of that design (issue #226):
rotation re-issues a refresh JWT but doesn't change the token **value**.
`RefreshClaims` is `{ sub, refresh_token_version, exp, iat, token_type }` — there
is **no `jti`** (unique token id), the refresh path reuses the same
`refresh_token_version`, and `exp`/`iat` are second-granular. So two refresh
tokens minted in the same wall-clock second are **byte-identical**. There is no
per-token identity, which means:

- a stolen refresh token is **not invalidated** when the legitimate client
  refreshes (the old value can still be byte-identical to a valid one), and
- **reuse of a superseded token can't be detected**.

[RFC 9700](https://datatracker.ietf.org/doc/rfc9700/) (OAuth 2.0 Security Best
Current Practice, Jan 2025) § 4.14 requires that refresh tokens for this client
class be either **sender-constrained** or use **rotation with reuse detection**.
We adopt rotation with reuse detection. That property is inherently **stateful** —
it requires per-token server-side state, which the single `refresh_token_version`
integer cannot express.

## Decision drivers

- **Theft mitigation.** A stolen refresh token must become unusable once the
  legitimate client refreshes, and reuse must be detectable.
- **RFC 9700 compliance** for the refresh flow.
- **Preserve the stateless access-token path** — no per-request DB hit.
- **Preserve multi-device sessions.** `refresh_token_version` is a single
  per-user integer; rotating it per refresh would log out a user's other devices.
- **No spurious logouts** from concurrent or retried refreshes (the classic
  reuse-detection false-positive).

## Considered options

- **Option A — add a `jti` only, no server store.** Makes tokens byte-distinct
  (so a value-inequality test passes) but provides **no** detection or
  invalidation. A security no-op; it only changes production code to satisfy a
  weaker assertion. Rejected.
- **Option B — bump `refresh_token_version` on every refresh.** Invalidates the
  prior token (good) but the integer is **global per user**, so a second device's
  token dies the moment the first device refreshes — breaks multi-device. It also
  can't *detect* reuse (only reject by version mismatch) and collides with 0031's
  use of the counter as the revoke-all primitive. Rejected.
- **Option C (chosen) — token families with per-token state + reuse detection.**
  Each login starts a *family*; each refresh mints a successor token (new `jti`)
  in that family and marks its predecessor used; reuse of a used token revokes the
  whole family.
- **Option D — sender-constrained tokens (DPoP / mTLS).** The other RFC 9700
  path. Heavier client + infra cost; rotation-with-detection is the pragmatic fit,
  and 0031's storage posture (`HttpOnly`, path-scoped) already covers most of the
  theft surface. Deferred — not chosen now.

## Decision outcome

Chosen: **Option C.** Sub-decisions follow.

### Token family + reuse detection

A new `refresh_tokens` table holds one row per issued refresh token:

- `id` — the `jti` (UUID as TEXT, per ADR [0027](0027-uuid-storage-as-text-in-sqlite.md)); primary key.
- `user_id` — FK to `users.id`.
- `family_id` — UUID (TEXT); identifies the chain descended from one login; indexed.
- `used_at` — TIMESTAMP, nullable. `NULL` = live (current tip of the family); set = rotated away from.
- `expires_at` — TIMESTAMP.
- `created_at` — TIMESTAMP.

It is timestamp-bearing, so it is a **non-STRICT** table (per the STRICT-mode rule
in `design.md` / ADR [0002](0002-sqlite-strict-mode-on-static-tables.md)), with
timestamps as ISO-8601 TEXT (ADR [0028](0028-timestamp-storage-as-iso8601-text.md)).

The refresh handler branches on the presented token's row (looked up by `jti`):

- **live** (`used_at IS NULL`, not expired, family not revoked) → mark `used_at`,
  insert a successor row in the **same** `family_id`, return the new token.
- **already used** (`used_at` set) → **reuse detected**: revoke the entire family,
  emit a security log event, return 401.
- **missing / expired / family revoked** → 401.

Revoking a family invalidates the stolen token and the legitimate descendant
alike, forcing re-auth — the RFC 9700 property.

### Keep JWT (not opaque); add `jti` + `family_id` claims

The refresh token stays a JWT signed with the same key (`validate_refresh_token`
already exists; the signature gives cheap tamper-evidence). It gains `jti` and
`family_id` claims; the refresh path now also does a DB lookup by `jti`. Opaque
random tokens (store a hash, look up the row) were the textbook alternative and
remain a clean future migration — but JWT-keep is lower-churn and the detection
logic is identical either way. (0031 already named opaque tokens as the migration
path for per-token state.)

### `refresh_token_version` retained as the global revoke-all

The existing integer stays as the per-user "nuke every session" primitive, bumped
on logout and password change. Family revocation handles a single compromised
chain; the version handles *all* sessions. The refresh path checks both.

### ~10-second grace window for concurrent / retried refreshes

Reuse detection's classic failure mode is false positives: the SPA double-fires a
refresh, or a dropped response triggers a retry with the now-superseded token —
either looks like reuse and would wrongly nuke the family. Mitigation: on reuse of
a token whose `used_at` is within the last **~10 s**, return its already-issued
successor instead of revoking the family. Beyond the window, treat as theft. This
is the server-side backstop; the primary fix is on the frontend.

### Frontend: single-flight silent refresh

The 401 → refresh → retry interceptor (0031 § Frontend behavior) becomes
**single-flight**: concurrent 401s queue behind one in-flight refresh, so the app
never fires parallel refreshes. The new reuse-detected error forces a hard logout.

### Row lifecycle

The table grows one row per refresh; prune used / expired rows past the refresh
TTL (a startup sweep or periodic task, mirroring the race-anchored sweeper
precedent of ADR [0035](0035-race-anchored-session-lifetime.md)).

### Positive consequences

- A rotated refresh token is rejected on reuse, and reuse revokes the family →
  RFC 9700-compliant refresh flow; a stolen token has a bounded useful life.
- Per-family granularity is the substrate for a future "log out this device" UI
  with no further redesign.
- The access-token path is unchanged (still stateless, no per-request DB hit). The
  refresh path already hit the DB for the user / version check, so detection adds
  one indexed lookup — not a new cost category.
- Multi-device sessions keep working: each login is its own family.

### Negative consequences / trade-offs

- Refresh tokens are now **stateful** — a new table, hand-written entity (ADR
  [0023](0023-hand-written-seaorm-entities.md)), migration, and a pruning job.
  This is exactly the per-token state 0031 deferred; we take it on deliberately.
- **Concurrency handling is mandatory.** The grace window + single-flight client
  are required to avoid spurious logouts; this is the subtle part of the work.
- Logout / password-change now also clear the user's families alongside the
  version bump — slightly more to reason about on those paths.

## Links

- Source: `ad-hoc` (PR #225 review → issue [#226](https://github.com/brendanbyrne/beerio-kart/issues/226))
- **Supersedes (in part):** ADR [0031](0031-auth-token-strategy.md) — its
  "Rotation: new token on every refresh, no version bump" sub-decision and its
  "per-device revocation deferred" trade-off. The rest of 0031 (storage pattern,
  `SameSite=Lax`, JWT format, access-token shape, `refresh_token_version` as
  revoke-all) still stands.
- Related ADRs: [0019](0019-admin-defense-in-depth.md) (defense in depth),
  [0023](0023-hand-written-seaorm-entities.md) (hand-written entities),
  [0035](0035-race-anchored-session-lifetime.md) (sweeper precedent for pruning),
  [0036](0036-error-code-rollout.md) (error codes).
- Reference: [RFC 9700](https://datatracker.ietf.org/doc/rfc9700/) § 4.14.
- Implementing PRs: backend [#230](https://github.com/brendanbyrne/beerio-kart/pull/230) (table, `jti`/`family_id` claims, rotation + reuse detection, grace window, prune); frontend (single-flight refresh + `token_reuse_detected` handling) TBD. Issue [#226](https://github.com/brendanbyrne/beerio-kart/issues/226).
