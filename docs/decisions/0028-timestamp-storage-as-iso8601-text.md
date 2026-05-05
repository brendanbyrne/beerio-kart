---
status: accepted
date: 2026-05-05
deciders: [Brendan]
source: ad-hoc
---

# 0028 — Timestamp storage in SQLite: TEXT in ISO 8601 format

## Context and problem statement

The app tracks timestamps: when sessions were created, when races were submitted, when flags were resolved. SQLite has no native timestamp type. Timestamps must be stored as TEXT, BLOB, or numeric (seconds since epoch). The format must be debuggable and migrate cleanly to PostgreSQL.

## Decision drivers

- Human-readable format for SQLite CLI debugging.
- ISO 8601 is a standard; no ambiguity about timezone or format.
- SeaORM codegen produces `DateTime<Utc>` for DATETIME columns, enabling ergonomic Rust handling.
- PostgreSQL migration later uses TIMESTAMPTZ (compatible with ISO 8601).

## Considered options

- **Option A:** Numeric (seconds since epoch). Compact, but opaque in CLI.
- **Option B:** TEXT in ISO 8601 format. Human-readable, standard, debuggable.
- **Option C:** SQLite DATETIME type (text format with custom parsing). Works, but SeaORM doesn't recognize it as `DateTime<Utc>`.

## Decision outcome

Chosen: **Option B** — Timestamps stored as TEXT in ISO 8601 format (e.g., `'2026-05-05T14:30:00Z'`). SQLite has no native timestamp type. PostgreSQL migration will map to `TIMESTAMPTZ`.

### Positive consequences

- Human-readable in CLI tools and logs.
- Standard format; no ambiguity.
- ISO 8601 is timezone-aware; no surprises.
- Clean migration to PostgreSQL (both understand ISO 8601).

### Negative consequences / trade-offs

- TEXT is slightly larger than numeric. Negligible: timestamp strings are ~20 bytes; at friend-group scale, storage is not a concern.

## Links

- Source: `ad-hoc`
