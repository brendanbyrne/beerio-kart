---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0018 — Real-time updates: polling, not WebSockets

## Context and problem statement

Session state (whose turn it is, pending races, participant list) changes as the game progresses. Clients need to stay in sync. Real-time updates can be delivered via WebSockets (push) or polling (pull). WebSockets are lower-latency but stateful; polling is simple and idempotent.

## Decision drivers

- Turn-based game: events happen every few minutes; latency is imperceptible with polling.
- Polling is stateless and testable with standard HTTP tools.
- Avoids WebSocket connection management, reconnection logic, and heartbeat complexity.
- Can upgrade to WebSockets later if latency becomes a real UX issue.

## Considered options

- **Option A:** Polling. Clients call `GET /sessions/:id` every 2–3 seconds. Simple, testable.
- **Option B:** WebSockets. Server pushes updates immediately. Lower latency, stateful.
- **Option C:** Hybrid. WebSockets for speed, fallback to polling. Overengineered for MVP.

## Decision outcome

Chosen: **Option A** — Clients poll `GET /sessions/:id` every 2–3 seconds. This is sufficient for a turn-based game; WebSockets can be added as an optimization if latency becomes a problem.

### Positive consequences

- No server connection state; trivial to scale and test.
- Standard HTTP tools can debug the sync (curl, replay, etc.).
- Clients are simple; no reconnect logic needed.

### Negative consequences / trade-offs

- Worst-case update latency is 3 seconds; imperceptible for turn-based gameplay, but might matter later if the game evolves toward real-time action.

## Links

- Source: `ad-hoc`
