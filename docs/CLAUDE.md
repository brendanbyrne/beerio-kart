# Doc-area conventions for Beerio Kart

This file is loaded automatically when Claude works in `docs/`. It captures conventions that don't apply elsewhere in the codebase. Source design record: `docs/designs/2026-05-04-design-doc-restructure.md`.

## Where does this content go?

| If it is… | Put it in… |
|---|---|
| A non-obvious decision with tradeoffs that a future contributor will revisit | `decisions/NNNN-*.md` (an ADR) |
| The conversation that produced one or more decisions | `designs/YYYY-MM-DD-topic.md` |
| Work in flight that isn't signed off yet | `drafts/topic.md` (gitignored) or `drafts/WIP_topic.md` (checkpointed) |
| Long-form technical investigation that informs designs but doesn't propose a decision | `research/<topic>.md` |
| A phase's narrative goals and success criteria | `roadmap.md` |
| A unit of executable work for Claude Code | A GitHub Issue (not a file) |
| Non-task communication between Cowork and Claude Code | `docs/handoffs/cowork-handoff.md` or `docs/handoffs/claude-code-handoff.md` |
| PR review feedback | A GitHub PR comment, line-anchored where possible (not a file) |
| Project workflow convention (Issue lifecycle, branch naming, statuses, triage) | `workflow.md` |

If unsure between a design record and an ADR, default to design record. Distill ADRs from it once decisions are clear. If unsure between research and a design record, ask: "does this propose a decision and need sign-off?" Yes → design record. No → research.

## Drafts → designs lifecycle

Working drafts live in `drafts/`, which is gitignored by default with a `WIP_` prefix exception:

```
docs/drafts/*
!docs/drafts/WIP_*.md
!docs/drafts/.gitkeep
```

- **Default:** `drafts/topic.md` doesn't appear in `git status`. Persists on disk across sessions.
- **Checkpoint:** Copy `drafts/topic.md` to `drafts/WIP_topic.md` for save-state or cross-machine sync. `WIP_*.md` files are checked in via the gitignore exception.
- **WIP_ files stay static after commit.** Resume work by copying back to `drafts/topic.md`.
- **Promotion:** When sign-off completes, rename `drafts/topic.md` → `designs/YYYY-MM-DD-topic.md` and delete the matching `WIP_topic.md`.
- **Abandonment:** Delete from `drafts/`. Nothing in git history.

Warning: aggressive cleanup commands (`git clean -fdx`) wipe gitignored drafts. Don't run those without checking `drafts/` first.

## Design records (`designs/`)

Single artifact type — no formal "plan vs design" distinction.

- Naming: `YYYY-MM-DD-kebab-case-topic.md`.
- Format: numbered sections with checkbox sign-offs (`- [ ] Approved / - [ ] Needs discussion / - [ ] Skip`).
- Each design record's sign-off summary lists ADRs spawned ("ADRs produced: 0042, 0043"). Mark `TBD` until known.
- Implementation plans live inline under a `## Implementation plan` heading.

Optional autonomy framing for high-trust implementation plans:

> ## Implementation plan (high-autonomy)
>
> Claude Code: work through items 1–N in order. Don't check in between items unless blocked. Definition of blocked: <explicit list>.

## ADRs (`decisions/`) — MADR format

- Files named `NNNN-kebab-case-title.md` with four-digit zero-padded sequence.
- Sequence is repo-global, not per-area.
- Each ADR has a `Source:` line (frontmatter) pointing to its parent design record (or `ad-hoc` for informal-conversation ADRs).
- Use `template.md` as the starting point.

Status legend (in `decisions/README.md`): `proposed`, `accepted`, `superseded`, `deprecated`. Most existing decisions land as `accepted`. If a decision changes later, mark the original `superseded` and create a new ADR — never edit-and-append history.

## Cross-linking

- ADRs link back to their parent design record via the `source` field.
- Design records list ADRs spawned in their sign-off summary.
- Code references the ADR: `// See docs/decisions/0007-h2h-derivation.md`.
- Implementation PR descriptions reference the ADR, not the design.
- Design records cite research files via relative path: `[seaorm-2.0 evaluation](../research/seaorm_2_0_migration.md)`. Multiple designs can cite the same research.

## Research notes (`research/`)

Long-form technical investigations that inform designs but don't propose decisions themselves. Examples that already exist: `ocr-strategy.md`, `seaorm_2_0_migration.md`.

- Naming: `<topic>.md` — no date prefix, since research is per-topic, not per-session.
- Format: free-form prose. No required sign-off section.
- Lifecycle: created as needed; can stay indefinitely or be deleted when superseded. No archive convention.
- Promotion path: research that grows a "Decision:" section, recommended path, or sign-off-style conclusions is actually a design record — move it to `designs/` and update cross-references.
- Document history: required (these are durable narrative). Append a dated bullet on AI-authored changes.

When does new research warrant its own file (vs. being absorbed into a design's "Decision Drivers" section)?

- Multi-thousand words.
- Will inform multiple future designs.
- Has standalone informational value.

Default for shorter, single-design research: keep it in the design's `## Decision drivers` or `## Considered options` section, not a separate file.

## Model tiering for bulk doc work

When generating multiple structurally-similar docs (ADRs from a list of bullets, Issues from an implementation plan):

1. Draft one canonical example by hand.
2. Spawn a Haiku-tier subagent (`model: "haiku"` in the Agent tool) with the example, template, and remaining inputs.
3. Review drafts; rewrite the few that need richer treatment.

This applies to bulk operations only. One-off ADRs or Issues don't need the tier-down — just write them.

## Document history rule (carve-outs)

The root `CLAUDE.md` requires `docs/` files to maintain a `## Document history` section. Carve-outs:

- ADRs (`decisions/`) — frontmatter has `date`; the ADR is intrinsically historical.
- `roadmap.md` — task tracking; history lives in Issues / Project board.

The rule still applies to `design.md`, `data-model.md`, `workflows.md`, `api-contract.md`, `coding-standards/*`, `designs/*` records, and `research/*` files.
