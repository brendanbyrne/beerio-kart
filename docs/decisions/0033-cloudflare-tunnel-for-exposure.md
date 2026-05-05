---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0033 — Cloudflare Tunnel for exposure, not port forwarding

## Context and problem statement

The app runs on Brendan's Unraid server at home. To make it accessible from outside the home network (e.g., testing from a phone on mobile data), the server needs to be exposed to the internet. Options include opening a port on the home network's firewall (port forwarding) or using a service like Cloudflare Tunnel that doesn't require opening ports.

## Decision drivers

- No open ports on the home network — eliminates direct attack surface.
- Outbound-only connection from the server to Cloudflare's edge — no inbound listening.
- Cloudflare provides DDoS protection and SSL termination as a bonus.

## Considered options

- **Option A:** Port forwarding on the router. Opens the home network to direct attacks.
- **Option B:** Cloudflare Tunnel (outbound-only connection to the Cloudflare edge). Secure, no open ports.

## Decision outcome

Chosen: **Option B** — Cloudflare Tunnel. The server makes an outbound-only connection to Cloudflare's edge; requests are routed back to the server through the tunnel. No open ports on the home network.

### Positive consequences

- No open ports; reduced attack surface.
- Outbound-only connection is hard to exploit from outside.
- Bonus: DDoS protection and SSL from Cloudflare.

### Negative consequences / trade-offs

- Dependency on Cloudflare's uptime and reliability. Acceptable: small-scale hobby app; standard SaaS risk.

## Links

- Source: `ad-hoc`
