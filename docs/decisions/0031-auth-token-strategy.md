---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0031 — Auth: short access token + long refresh token (HttpOnly cookie)

## Context and problem statement

The original Phase 1 design used a single 24-hour JWT in the `Authorization` header. That's adequate for "you're logged in for a day," but three problems surface:

1. **No revocation.** A leaked token is valid for the full window with no recourse. Bumping the signing key revokes everyone, which is unacceptable.
2. **Long-lived bearer tokens in JS-accessible storage.** A 24-hour token sits wherever the frontend stores it — `localStorage` (XSS-readable) or in-memory (lost on reload). Both are bad.
3. **No graceful re-auth.** When the 24 hours hit, the user is bounced to login mid-session.

Phase 2's auth refactor replaces the single-token model. The decision splits into one core choice (storage pattern for the refresh token) and three coupled sub-decisions (cookie `SameSite`, refresh format, rotation policy) that are easier to reason about together than separately.

## Decision drivers

- **Server-side revocation** without bumping a global signing key.
- **Short bearer-token windows** (minutes, not hours) to limit damage from token theft.
- **Survives page reload** without re-login.
- **XSS-resilient.** The refresh token must be unreachable from JS.
- **Survives external links.** A friend texts you the URL → you click → still logged in. Re-login from an external nav is a bad first impression.

## Considered options

(Storage pattern only — sub-decisions are below in the outcome.)

- **Option A:** Single long-lived JWT. Original design. Simple but no revocation, no graceful re-auth, long bearer-token exposure.
- **Option B:** Access + refresh, both in JS-accessible storage. Standard pattern, but the refresh token is XSS-reachable.
- **Option C (chosen):** Access + refresh, refresh in HttpOnly cookie scoped to `/api/v1/auth/refresh`. JS can't read the refresh token; the cookie's path scope means it isn't sent on most requests.

## Decision outcome

Chosen: **Option C.** Sub-decisions follow.

### Tokens

- **Access token.** 15–30 minute lifetime. Sent in `Authorization: Bearer <jwt>` on every API call. JWT signed with the auth key.
- **Refresh token.** 7–30 day lifetime. HttpOnly, Secure, `SameSite=Lax` cookie scoped to `Path=/api/v1/auth/refresh`. JWT signed with the same key as the access token. Claims: `sub`, `refresh_token_version`, `exp`, `iat`, `token_type: "refresh"`.

### `SameSite=Lax`, not `Strict`

Strict blocks the cookie when the request originates from another site — including the entirely benign case of clicking a link to the app from a text message or email. The user follows the link, gets to the app, and is unexpectedly logged out. Lax still blocks the cross-site POST attacks the cookie attribute is meant to prevent (CSRF on refresh) while letting "user navigated here from outside" work. Lax wins on UX with no meaningful security loss.

### Refresh token format: JWT, not opaque

Opaque random tokens require a server-side lookup table per token. JWT signed with the access-token key requires no new table. The only thing JWT gives up is per-device revocation — and we don't need that for MVP. If "log out other devices" ever becomes a feature, opaque tokens with a per-token DB row are the migration path; the existing `refresh_token_version` plumbing carries forward.

### Rotation: new token on every refresh, no version bump

Each successful refresh issues a brand-new refresh JWT with a fresh expiry. The version is **not** bumped on rotation — rotation extends the session window, not revokes existing sessions. Bumping on rotation would invalidate the just-issued token immediately.

### Revocation: `refresh_token_version` column on `users`

Bumped on logout and on password change. Checked **only on the refresh path** — the access-token path is unchanged (no DB hit per request). A bumped version invalidates every existing refresh token for that user; the next refresh attempt is rejected and the user is bounced to login.

### Frontend behavior

API responses with status 401 trigger a silent refresh: the frontend POSTs to `/api/v1/auth/refresh` (cookie attached automatically), receives a new access token + rotated refresh cookie, and retries the original request. User-visible re-login only happens when the refresh itself returns 401.

### Password change

`PUT /auth/password` lives on the same route module as the refresh endpoint. It bumps `refresh_token_version` as part of the change. The user re-authenticates after a password change — that's the desired behavior, and the version-bump is the mechanism.

### Positive consequences

- Refresh token unreachable from JS (XSS-resilient).
- 401s trigger silent refresh; users don't see token expiry.
- Server-side revocation without a per-token table — one integer per user.
- Logout, password change, and "rotate everything" are the same primitive: bump the counter.
- Cookie scoped to `/api/v1/auth/refresh` means the refresh token isn't sent on every API request — narrower attack surface than a session cookie.
- `SameSite=Lax` keeps "follow a link to the app" working without sacrificing CSRF protection on the refresh path.

### Negative consequences / trade-offs

- **Two-token complexity vs. one.** The frontend handles 401 → refresh → retry; the backend has two token validators. Acceptable: this is the standard pattern, and the security/UX gains pay for it.
- **Per-device revocation isn't possible** without a per-token table. Deferred until there's a feature that needs it (e.g., a "log out other devices" UI).
- **HTTPS required in production** for the Secure attribute. Cloudflare Tunnel (ADR 0033) provides this; not a constraint in practice.

## Links

- Source: `ad-hoc`
- Related ADRs: [0016 — session passwords deferred (re-uses POST `/sessions/:id/join` shape)](0016-session-passwords-deferred.md), [0033 — Cloudflare Tunnel for exposure](0033-cloudflare-tunnel-for-exposure.md)
- Implementing PRs: PR #7 (Phase 2: Production config + refresh token auth)
