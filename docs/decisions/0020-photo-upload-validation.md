---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0020 — Photo upload: magic-byte validation, format whitelist, size cap

## Context and problem statement

Players upload photos as proof of their race times (for later OCR). The server needs to validate that uploaded files are actually images, not arbitrary content masquerading as photos (security: no executable uploads, no zip bombs).

## Decision drivers

- Prevent abuse (executable uploads, oversized files, malformed images).
- Check magic bytes, not just Content-Type header (headers are user-controlled).
- Accept the formats players use (JPEG, PNG, HEIC/HEIF from modern phones).
- Standardize filenames to prevent path-traversal attacks.

## Considered options

- **Option A:** Trust Content-Type header; accept anything. Simple, insecure.
- **Option B:** Validate magic bytes; accept JPEG, PNG; reject HEIC. Secure but excludes modern phone formats.
- **Option C:** Validate magic bytes; accept JPEG, PNG, HEIC/HEIF; cap size at 10MB; use run-ID-based filenames.

## Decision outcome

Chosen: **Option C** — Server-side magic-byte validation. Accept JPEG, PNG, HEIC/HEIF. Max 10MB per file. Generate filenames from run ID (`{run_id}.{ext}`) — never use user-provided names. Optionally strip EXIF data (GPS, device info) for privacy in a future pass.

### Positive consequences

- Immune to Content-Type spoofing; real image validation.
- Supports modern phone formats without friction.
- Standardized filenames prevent path-traversal and naming attacks.
- Size cap prevents disk abuse.

### Negative consequences / trade-offs

- EXIF stripping (privacy) is deferred; device info briefly exposed. Acceptable: low-risk in a friend-group game; EXIF removal is a post-MVP optimization.

## Links

- Source: `ad-hoc`
