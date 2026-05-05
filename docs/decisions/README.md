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

## Adding a new ADR

1. Copy `template.md` to `NNNN-your-title.md` (next available number).
2. Fill in context, decision drivers, considered options, decision outcome, consequences.
3. Add a row to the index above (number, title, status, date).
4. If the ADR comes from a design review, set `source` to the design record path.
5. Mark the design record's sign-off summary with the new ADR number ("ADRs produced: NNNN, ...").
