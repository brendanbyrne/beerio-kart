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

- **Option A:** Port forwarding on the router. Opens an inbound port on the home network. Direct attack surface; needs DDNS for a stable hostname; needs a separate TLS solution (Let's Encrypt + cert renewal).
- **Option B:** Cloudflare Tunnel (chosen). Outbound-only daemon on the Unraid server connects to the Cloudflare edge; Cloudflare proxies traffic back through the tunnel. No inbound port; TLS, DDoS protection, and a stable hostname are bundled.
- **Option C:** Tailscale Funnel or ngrok. Same outbound-tunnel shape as Cloudflare; bundles TLS and a stable hostname. Tailscale is good for "private mesh + selective public exposure" but the public-tunnel feature is comparatively new; ngrok's free tier has rate limits and a rotating hostname unless paid.
- **Option D:** Small VPS (e.g., $5/mo Hetzner / Fly.io) running a reverse proxy back to home via WireGuard or SSH tunnel. Most flexible, full control over TLS and routing, but adds an always-on box to operate.

## Decision outcome

Chosen: **Option B** — Cloudflare Tunnel. The server makes an outbound-only connection to Cloudflare's edge; requests are routed back to the server through the tunnel. No open ports on the home network.

### Positive consequences

- No open ports; reduced attack surface.
- Outbound-only connection is hard to exploit from outside.
- Bonus: DDoS protection and SSL from Cloudflare.

### Negative consequences / trade-offs

- Dependency on Cloudflare's uptime and reliability. Acceptable: small-scale hobby app; standard SaaS risk.
- Lock-in to Cloudflare's specific tunnel daemon (`cloudflared`) and DNS / hostname configuration. Migrating to Tailscale Funnel, ngrok, or a self-hosted reverse proxy later would touch the deploy config but not application code — cost is bounded.

## Links

- Source: `ad-hoc`
