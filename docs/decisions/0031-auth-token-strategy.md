---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0031 — Auth: short access token + long refresh token (HttpOnly cookie)

## Context and problem statement

The original Phase 1 design had a single 24-hour JWT in the `Authorization` header. That's adequate for "you're logged in for a day," but it has three problems:

1. **No revocation.** A leaked or compromised token is valid for the full window with no recourse. Bumping a key revokes everyone, which is unacceptable.
2. **Long-lived bearer tokens in JS-accessible storage.** The 24-hour token sits wherever the frontend stores it (localStorage, in-memory). Either choice has tradeoffs around XSS exposure or page-reload state.
3. **No graceful re-auth.** When the 24-hour mark hits, the user is bounced to login mid-session.

Phase 2's auth refactor replaces the single-token model with the standard short-access + long-refresh pattern, with one notable refinement: the refresh token lives in an HttpOnly cookie scoped to the refresh endpoint, not in JS-accessible storage. This single ADR consolidates four bullets that were originally separate but are too tightly coupled to live apart.

## Decision drivers

- Need server-side revocation without bumping a global signing key.
- Want short bearer-token windows (minutes, not hours) to limit damage from theft.
- Want the refresh path to survive page reload without re-login.
- Want refresh tokens unreachable from JS (XSS-resilient).
- Want the cookie to survive following an external link to the app (a friend texts you the URL — re-login is a bad first impression).

## Considered options

- **Option A:** Single long-lived JWT. Original design. Simple but no revocation, no graceful re-auth, long bearer-token exposure.
- **Option B:** Access + refresh, both in JS-accessible storage. Standard pattern but refresh token is XSS-reachable.
- **Option C:** Access + refresh, refresh in HttpOnly cookie scoped to `/api/v1/auth/refresh`. (Chosen.) JS can't read the refresh token; cookie-scoped path means it isn't sent on most requests.

A second decision orthogonal to the storage choice: **`SameSite=Lax` vs `SameSite=Strict`** on the refresh cookie. Strict blocks the cookie when the request originates from another site — including the entirely benign case of clicking a link to the app from a text message or email. Lax still blocks the cross-site POST attacks the cookie attribute is meant to prevent (CSRF on refresh) while letting "user navigated here from outside" work.

A third decision: **refresh token format** — opaque random string with a server-side lookup table, or JWT signed with the same key as the access token. JWT wins on simplicity (no new table); per-device revocation is the only thing it gives up, and we don't need that for MVP.

A fourth decision: **rotation** — should each refresh issue a brand-new refresh token, or extend the existing one's expiry? New token. Simpler and matches the standard refresh-rotation pattern.

## Decision outcome

Chosen: **Option C** with the orthogonal pieces resolved as below.

**Tokens.**

- **Access token.** 15–30 minute lifetime. Sent in the `Authorization: Bearer <jwt>` header on every API call. JWT signed with the auth key.
- **Refresh token.** 7–30 day lifetime. Lives in an HttpOnly, Secure, `SameSite=Lax` cookie scoped to `Path=/api/v1/auth/refresh`. JWT signed with the same key as the access token. Claims: `sub`, `refresh_token_version`, `exp`, `iat`, `token_type: "refresh"`.

**Revocation: `refresh_token_version` column on `users`.** Bumped on logout and on password change. The refresh path is the only place the version is checked — the access-token path is unchanged (no DB hit per request). A bumped version invalidates every existing refresh token for that user; the next refresh attempt is rejected and the user is bounced to login. Per-device revocation is deferred — a single global counter is sufficient for MVP.

**Rotation.** Each successful refresh issues a brand-new refresh JWT with a fresh expiry. The version is *not* bumped on rotation — rotation is about extending the session window, not revoking. Bumping on rotation would invalidate the just-issued token immediately.

**Frontend behavior.** API responses with status 401 trigger a silent refresh: the frontend POSTs to `/api/v1/auth/refresh` (cookie attached automatically), receives a new access token + rotated refresh cookie, and retries the original request. User-visible re-login only happens when the refresh itself returns 401.

**Password change.** `PUT /auth/password` lives on the same route module as the refresh endpoint. It bumps `refresh_token_version` as part of the change, invalidating any existing refresh cookies. The user re-authenticates after a password change — that's the desired behavior and the version-bump is the mechanism.

### Positive consequences

- Refresh token is unreachable from JS (XSS-resilient).
- 401s trigger a silent refresh; users don't see the token expiry.
- Server-side revocation without a per-token table — one integer per user.
- Logout, password change, and "rotate everything" are all the same primitive (bump the counter).
- The cookie scope (`Path=/api/v1/auth/refresh`) means the refresh token isn't sent on every API request — narrower attack surface than a session cookie.
- `SameSite=Lax` keeps "follow a link to the app" working without sacrificing CSRF protection on the refresh path.

### Negative consequences / trade-offs

- Two-token complexity vs. one. Frontend has to handle 401-then-refresh-then-retry; backend has two token validators. Acceptable: this is the standard pattern and the security/UX gains pay for it.
- Per-device revocation isn't possible without a per-token table. Deferred until there's a feature that needs it (e.g., "log out other devices" UI).
- Cookies require HTTPS in production (Secure attribute). Cloudflare Tunnel handles this; not a constraint in practice.

## Links

- Source: `ad-hoc`
- Related ADRs: [0016 (session passwords deferred — re-uses the same `POST /sessions/:id/join` shape so password support is additive later)](0016-session-passwords-deferred.md)
- Implementing PRs: PR #7 (Phase 2: Production config + refresh token auth)
