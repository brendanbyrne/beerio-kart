---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0004 — Account recovery: admin reset for now

## Context and problem statement

Players need a way to regain access to their accounts if they forget their password. A full self-service recovery flow (email tokens, recovery codes) adds complexity to MVP. For a small, trusted user base in early phases, a simpler path works.

## Decision drivers

- Minimize account-recovery infrastructure for MVP.
- Support real-world password-loss scenarios without friction.
- Revisit later when player base scales beyond manual admin intervention.

## Considered options

- **Option A:** Full self-service password reset via email tokens. Secure, scalable, but requires email infrastructure.
- **Option B:** Admin-initiated password reset. Simple to implement, manual, works for MVP.
- **Option C:** No recovery path; user creates a new account. Poor UX, loses history.

## Decision outcome

Chosen: **Option B** — Admin can reset a player's password directly. Player contacts host/admin (out-of-band), admin updates their password to a temporary value via the admin page.

### Positive consequences

- No email infrastructure, token lifecycle, or replay-attack surface.
- Straightforward admin UX — one button.
- Low implementation cost.

### Negative consequences / trade-offs

- Requires admin involvement for every reset; doesn't scale past small groups. Acceptable for MVP; email-based self-service can be added when player count justifies it.

## Links

- Source: `ad-hoc`
