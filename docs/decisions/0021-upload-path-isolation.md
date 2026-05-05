---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0021 — Upload path isolation: separate URL prefix and filesystem directory

## Context and problem statement

The server serves both static assets (CSS, JavaScript, vendor libraries) and user uploads (photos). A path-traversal bug could allow attackers to escape the uploads directory and read/write static files or other sensitive data. Isolation prevents that.

## Decision drivers

- Prevent path-traversal attacks crossing asset and upload boundaries.
- Different directories on disk are a natural isolation boundary.
- Different URL prefixes make the separation explicit in routing.

## Considered options

- **Option A:** Serve uploads and static assets from the same directory. Simple, but path-traversal can cross boundaries.
- **Option B:** Same directory but different subdirs (e.g., `/static/` vs `/static/uploads/`). Still the same filesystem root.
- **Option C:** Different URL prefixes and different filesystem directories. Clear, secure isolation.

## Decision outcome

Chosen: **Option C** — User uploads are served from `/uploads/...` (from `UPLOAD_DIR` env var). Static assets are served from `/static/...` (from `STATIC_DIR`). Different prefixes and different directories prevent path-traversal across boundaries.

### Positive consequences

- Path-traversal bugs in one handler can't escape to the other directory.
- Different ACLs or security policies can be applied to each directory.
- Clear separation in code and on disk.

### Negative consequences / trade-offs

- Slightly more configuration (two env vars). Negligible: standard practice.

## Links

- Source: `ad-hoc`
