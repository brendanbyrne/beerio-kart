---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0027 — UUID storage in SQLite: TEXT, not BLOB

## Context and problem statement

The app uses UUIDs for entity IDs (runs, sessions, users, etc.). SQLite doesn't have a native UUID type. Two options: store as TEXT (human-readable) or BLOB (binary, compact). The choice affects debuggability and PostgreSQL migration path.

## Decision drivers

- Human readability when debugging with SQLite CLI tools.
- No performance penalty for this app's scale (friend groups, not millions of rows).
- PostgreSQL migration later can use its native UUID type; SeaORM maps both to `String` in Rust, so application code won't change.

## Considered options

- **Option A:** Store as TEXT (e.g., `'550e8400-e29b-41d4-a716-446655440000'`). Human-readable, slightly larger.
- **Option B:** Store as BLOB (binary 16 bytes). Compact, opaque, hard to debug.
- **Option C:** Hybrid: BLOB in SQLite, TEXT in PostgreSQL. Adds migration complexity.

## Decision outcome

Chosen: **Option A** — UUIDs stored as TEXT in SQLite for human readability. PostgreSQL migration will map to native UUID type. SeaORM maps both to `String` in Rust; application code doesn't change.

### Positive consequences

- Easy debugging with SQLite CLI (can read UUIDs directly).
- No application-code changes when migrating to PostgreSQL.
- No compression needed at this scale.

### Negative consequences / trade-offs

- TEXT is slightly larger on disk than BLOB. Negligible at this scale; the savings are sub-millisecond.

## Links

- Source: `ad-hoc`
