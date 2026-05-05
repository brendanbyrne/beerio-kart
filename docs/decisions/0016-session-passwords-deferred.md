---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0016 — Session passwords: deferred post-MVP

## Context and problem statement

Early designs considered session passwords to keep random participants out of a group's session. This adds complexity to the join flow and session data. For MVP, sessions are created by people who know each other (in-person or close circles), so an open join is acceptable.

## Decision drivers

- Defer security features until the product scales beyond friend groups.
- Keep the join flow simple: `POST /sessions/:id/join` with no credential.
- Architectural preparedness: the endpoint is already a dedicated action, so passwords can be added later without restructuring.

## Considered options

- **Option A:** Sessions are open; anyone with the ID can join. Simple, no security.
- **Option B:** Password-protected sessions. Secure, but adds credential management.
- **Option C:** Invite-only sessions (admin must approve). Heavyweight governance.

## Decision outcome

Chosen: **Option A** — Sessions are open (no password) in MVP. The `POST /sessions/:id/join` endpoint is a dedicated action, so password checking can be added later without restructuring the join flow.

### Positive consequences

- Simple join experience; no credential friction.
- Endpoint is already isolated, so adding password validation is a one-line service change.
- Matches MVP's trust model (friend groups).

### Negative consequences / trade-offs

- If the app ever scales to public sessions, random people could infiltrate friend-group sessions. Acceptable: password support is deferred to that phase.

## Links

- Source: `ad-hoc`
