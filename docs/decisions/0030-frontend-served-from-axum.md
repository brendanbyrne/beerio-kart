---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0030 — Frontend serving strategy: Axum serves static files with SPA fallback

## Context and problem statement

The frontend is built by Vite and produces static files (HTML, CSS, JavaScript). These files need to be served to browsers. Options include Axum serving them directly, a separate nginx container, or Vite's dev server in production. The choice affects deployment complexity and operational overhead.

## Decision drivers

- Simple deployment: one container, one process.
- No CORS complexity; frontend and API are the same origin.
- Vite build is a standard artifact; no dev-server production hack.
- Scalability: if static performance ever matters (it won't at this scale), a CDN or reverse proxy can be added in front.

## Considered options

- **Option A:** Separate nginx container for static files; Axum for API only. Classic, adds complexity.
- **Option B:** Axum serves everything via `tower-http::ServeDir` with SPA fallback to `index.html`. Simple, one container.
- **Option C:** Vite dev server in production. No. Adds startup overhead and development tools to production.

## Decision outcome

Chosen: **Option B** — Axum serves the Vite build's static files via `tower-http::ServeDir` with SPA fallback to `index.html`. One container, no separate frontend infrastructure.

### Positive consequences

- Single deployment unit; no nginx config or container orchestration.
- No CORS headers needed (same origin).
- Standard Vite build; no production-specific tooling.

### Negative consequences / trade-offs

- Cache headers and asset versioning are less flexible than a dedicated CDN. Acceptable: can be optimized post-MVP if performance metrics demand it.

## Links

- Source: `ad-hoc`
