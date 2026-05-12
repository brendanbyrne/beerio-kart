# Architecture Decision Records

Decisions about Beerio Kart's architecture, captured as MADR records.

## Format

See `template.md` for the starting point. Files are named `NNNN-kebab-case-title.md` with a four-digit zero-padded sequence (repo-global, not per-area). Each ADR includes a `source` field pointing to the parent design record (or `ad-hoc` for informal-conversation decisions).

## Status legend

- **proposed** — under discussion, not yet adopted.
- **accepted** — current standard.
- **superseded** — replaced by a newer ADR (see the ADR's frontmatter for the link).
- **deprecated** — no longer in force, no replacement.

## Index

| # | Title | Status | Date |
|---|---|---|---|
| [0001](0001-sessions-no-created-by-column.md) | Sessions: no `created_by` column | accepted | 2026-05-02 |
| [0002](0002-sqlite-strict-mode-on-static-tables.md) | SQLite STRICT mode on lookup tables; DATETIME on timestamped tables | accepted | 2026-05-05 |
| [0003](0003-global-leaderboard-most-track-records.md) | Global leaderboard: most track records held | accepted | 2026-05-05 |
| [0004](0004-account-recovery-admin-reset.md) | Account recovery: admin reset for now | accepted | 2026-05-05 |
| [0005](0005-no-time-entry-validation.md) | Time entry: no validation against plausible track times | accepted | 2026-05-05 |
| [0006](0006-beer-vs-water-separate-leaderboards.md) | Beer vs water: separate leaderboards by default, with combined view | accepted | 2026-05-05 |
| [0007](0007-track-variants-150cc-only.md) | Track variants: 150cc only | accepted | 2026-05-05 |
| [0008](0008-admin-model-env-var-gated.md) | Admin model: lightweight admin page gated by user ID in env variable | accepted | 2026-05-05 |
| [0009](0009-run-immutability-admin-can-edit.md) | Run immutability: users cannot edit runs after creation; admin can | accepted | 2026-05-05 |
| [0010](0010-h2h-derived-from-session-races.md) | Head-to-head: derived from session races, not stored | accepted | 2026-05-05 |
| [0011](0011-sessions-replace-standalone-runs.md) | Sessions replace standalone run recording | accepted | 2026-05-05 |
| [0012](0012-dqd-runs-recorded-but-excluded.md) | DQ'd runs: recorded but excluded from leaderboards and H2H | accepted | 2026-05-05 |
| [0013](0013-pending-race-cap.md) | Pending race cap: UI shows max 3 pending races | accepted | 2026-05-05 |
| [0014](0014-session-host-transfer.md) | Session host transfer: earliest-joined remaining participant becomes new host | accepted | 2026-05-05 |
| [0015](0015-session-timeout-skip-turn.md) | Session timeout handling (MVP): "skip turn" allows recovery without restart | accepted | 2026-05-05 |
| [0016](0016-session-passwords-deferred.md) | Session passwords: deferred post-MVP | accepted | 2026-05-05 |
| [0017](0017-ruleset-changes-mid-session-deferred.md) | Ruleset changes mid-session: deferred post-MVP | accepted | 2026-05-05 |
| [0018](0018-realtime-via-polling-not-websockets.md) | Real-time updates: polling, not WebSockets | accepted | 2026-05-05 |
| [0019](0019-admin-defense-in-depth.md) | Admin defense in depth: two independent checks | accepted | 2026-05-05 |
| [0020](0020-photo-upload-validation.md) | Photo upload: magic-byte validation, format whitelist, size cap | accepted | 2026-05-05 |
| [0021](0021-upload-path-isolation.md) | Upload path isolation: separate URL prefix and filesystem directory | accepted | 2026-05-05 |
| [0022](0022-rulesets-as-rust-trait.md) | Rulesets: implemented as Rust trait with one module per ruleset | accepted | 2026-05-05 |
| [0023](0023-hand-written-seaorm-entities.md) | Hand-written SeaORM entities: committed source code | accepted | 2026-05-05 |
| [0024](0024-just-not-make-for-dev-commands.md) | just (not Make) for developer commands | accepted | 2026-05-05 |
| [0025](0025-photo-enforcement-for-records.md) | Photo enforcement for record-breaking runs (auto-flag + auto-resolve) | accepted | 2026-05-05 |
| [0026](0026-lap-time-column-naming.md) | Lap time column naming: `lap1_time`, `lap2_time`, `lap3_time` | accepted | 2026-05-05 |
| [0027](0027-uuid-storage-as-text-in-sqlite.md) | UUID storage in SQLite: TEXT, not BLOB | accepted | 2026-05-05 |
| [0028](0028-timestamp-storage-as-iso8601-text.md) | Timestamp storage in SQLite: TEXT in ISO 8601 format | accepted | 2026-05-05 |
| [0029](0029-run-flags-audit-trail.md) | run_flags: audit trail, no unique constraint on (run_id, reason) | accepted | 2026-05-05 |
| [0030](0030-frontend-served-from-axum.md) | Frontend serving strategy: Axum serves static files with SPA fallback | accepted | 2026-05-05 |
| [0031](0031-auth-token-strategy.md) | Auth: short access token + long refresh token (HttpOnly cookie) | accepted | 2026-05-05 |
| [0032](0032-cursor-based-pagination.md) | Pagination: cursor-based (keyset) on `created_at` + `id` | accepted | 2026-05-05 |
| [0033](0033-cloudflare-tunnel-for-exposure.md) | Cloudflare Tunnel for exposure, not port forwarding | accepted | 2026-05-05 |
| [0034](0034-docker-compose-manager-for-deployment.md) | Docker Compose Manager plugin on Unraid for container management | accepted | 2026-05-05 |
| [0035](0035-race-anchored-session-lifetime.md) | Race-anchored session lifetime: replace idle-session sweeper with race-derived sweeper | accepted | 2026-05-11 |

## Adding a new ADR

1. Copy `template.md` to `NNNN-your-title.md` (next available number).
2. Fill in context, decision drivers, considered options, decision outcome, consequences.
3. Add a row to the index above (number, title, status, date).
4. If the ADR comes from a design review, set `source` to the design record path.
5. Mark the design record's sign-off summary with the new ADR number ("ADRs produced: NNNN, ...").
