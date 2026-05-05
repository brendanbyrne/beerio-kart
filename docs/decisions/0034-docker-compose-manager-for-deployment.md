---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0034 — Docker Compose Manager plugin on Unraid for container management

## Context and problem statement

The app and its services run in Docker containers on an Unraid server. Unraid has multiple container management options: the built-in UI, manual `docker-compose` commands, or dedicated plugins. The choice affects how easy it is to deploy, monitor, and update the app.

## Decision drivers

- Unraid is a home-lab platform designed for Docker; its plugins are native to the ecosystem.
- Docker Compose is a standard abstraction; using it as the config language ensures portability.
- The Compose Manager plugin gives Unraid's UI a view into the compose file without manual CLI work.

## Considered options

- **Option A:** Manual `docker-compose` commands; manage containers via CLI. Works, but less integrated with Unraid.
- **Option B:** Unraid's built-in Docker UI (separate from Compose). Limited Compose visibility.
- **Option C:** Docker Compose Manager plugin on Unraid. Unraid's UI knows about Compose; single source of truth in `compose.yaml`.

## Decision outcome

Chosen: **Option C** — Docker Compose Manager plugin on Unraid. `compose.yaml` is the single source of truth for the deployment. Unraid's plugin reads and manages the Compose file through the UI.

### Positive consequences

- `compose.yaml` is portable; the app can run on any system with Docker and Compose.
- Unraid's UI is integrated; no manual CLI.
- Easy to version control and review `compose.yaml` changes.

### Negative consequences / trade-offs

- Plugin dependency; if the plugin is discontinued, manual `docker-compose` commands remain as a fallback.

## Links

- Source: `ad-hoc`
