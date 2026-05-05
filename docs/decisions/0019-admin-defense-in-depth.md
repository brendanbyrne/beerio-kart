---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0019 — Admin defense in depth: two independent checks

## Context and problem statement

Admin operations (editing runs, resolving flags) are sensitive — a single missed permission check could expose them. A layered approach, where both the route middleware and the service layer check admin status independently, catches bugs in either layer.

## Decision drivers

- Defense in depth: if the middleware has a bug, the service still blocks unauthorized access.
- If the service has a bug, the middleware is the backstop.
- Two independent checks force intentional permission design, not careless shortcuts.

## Considered options

- **Option A:** Check admin status in middleware only. Simpler, but one bug exposes all admin ops.
- **Option B:** Check admin status in the service only. Relies on consistent implementation.
- **Option C:** Check in both layers independently. Slightly more code, but bulletproof.

## Decision outcome

Chosen: **Option C** — Admin-only operations (editing runs, resolving flags) are checked in both the route middleware (AdminUser extractor) and the service layer independently. Both must pass; failure in either blocks the request.

### Positive consequences

- A middleware bug doesn't accidentally expose admin operations.
- A service-layer bug doesn't bypass the middleware wall.
- Clear separation of concerns: middleware gates the route; service enforces the rule again.

### Negative consequences / trade-offs

- Slightly more code; feels repetitive. Acceptable: the cost is small, and the security benefit is worth it.

## Links

- Source: `ad-hoc`
