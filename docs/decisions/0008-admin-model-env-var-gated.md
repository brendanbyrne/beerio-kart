---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0008 — Admin model: lightweight admin page gated by user ID in env variable

## Context and problem statement

Some operations need admin privileges (resetting passwords, editing/deleting runs, resolving flags). A full role-based access control (RBAC) system is overengineered for MVP. A simpler gate — user ID checked against an env variable — provides the security we need without the infrastructure.

## Decision drivers

- MVP doesn't need dynamic role assignment; the admin is known and fixed.
- Env-var gate is trivial to implement and deploy.
- Upgrade path to RBAC is clear if roles later need more flexibility.

## Considered options

- **Option A:** Full RBAC with database roles, permissions, audit logs. Over-engineered for MVP.
- **Option B:** Lightweight gate: admin user ID in env variable. Simple, sufficient, upgradeable.
- **Option C:** No explicit admin at all; rely on Axum middleware checks everywhere. Fragile and error-prone.

## Decision outcome

Chosen: **Option B** — Admin operations are guarded by an `AdminUser` extractor that checks the authenticated user's ID against `ADMIN_USER_ID` from the environment.

### Positive consequences

- Minimal implementation — one extractor, one env var.
- Admin identity is easy to rotate (redeploy with new env var).
- All admin checks are explicit and discoverable by code grep.

### Negative consequences / trade-offs

- Only one admin user can be designated at a time. Acceptable for MVP; multi-admin support can be added when needed.

## Links

- Source: `ad-hoc`
