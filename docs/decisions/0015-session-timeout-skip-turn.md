---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0015 — Session timeout handling (MVP): "skip turn" allows recovery without restart

## Context and problem statement

If the current track chooser leaves the session or is stuck, the whole session halts — no one can choose the next track. The group needs a way to recover without restarting the session and losing progress.

## Decision drivers

- Unblock stuck sessions without restarting.
- Minimize feature complexity for MVP (vote-to-kick deferred).
- Lightweight recovery for a small, trusted group.

## Considered options

- **Option A:** Vote-to-kick the chooser; majority rule takes over. Robust, but adds voting logic.
- **Option B:** "Skip turn" button: any participant can pass the chooser's turn to the next person. Simple, trusting.
- **Option C:** Session auto-expires if inactive for N minutes. Harsh; loses progress.

## Decision outcome

Chosen: **Option B** — MVP supports "Skip turn": any participant can pass the current chooser's turn to the next participant in rotation. Vote-to-kick is deferred post-MVP.

### Positive consequences

- Simple to implement and explain.
- Doesn't require consensus or voting machinery.
- Reasonable fallback for small, trusted groups.

### Negative consequences / trade-offs

- A malicious participant can spam "skip" to sabotage the game. Acceptable for MVP in a trusted context; moderation (vote-to-kick, temporary role revocation) can be added if needed.

## Links

- Source: `ad-hoc`
