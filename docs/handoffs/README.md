# Handoff files

This directory holds short-lived task specs and questions that pass between Cowork (Claude Desktop, design/docs/research) and Claude Code (WSL2 CLI, code/build/test/git). It exists because the two assistants don't share a chat history — anything one needs the other to act on has to be written down somewhere they'll both look.

## The convention

Two channels:

- **`cowork-handoff.md`** — Cowork → Claude Code. Cowork writes; Claude Code reads, acts on it, and deletes it.
- **`claude-code-handoff.md`** — Claude Code → Cowork. Claude Code writes; Cowork reads and acknowledges in chat. Brendan or the next Claude Code session deletes it (Cowork's sandbox blocks `unlink()` repo-wide, so Cowork can't clean up its own inbox).

**The writer creates the file; the file's existence is the signal that there is work to do.** No file means no work.

For task-specific handoffs that shouldn't collide with the canonical filenames (e.g. multiple parallel work items), use a dated slug:

- `cowork-handoff-<YYYY-MM-DD>-<slug>.md`
- `claude-code-handoff-<YYYY-MM-DD>-<slug>.md`

## What goes here

- **Task specs.** "Apply these three edits to file X, open a PR, and delete this handoff."
- **Questions and research requests.** "What should the API contract say about idempotency keys on `POST /runs`?"
- **Code review findings, bug reports, design decisions, answers to the other assistant's questions.**

If you find yourself composing a substantive response in chat that the other assistant needs to see in order to do their next piece of work, **stop and write it here instead**. Anything delivered only in chat is invisible to the other assistant.

## What does NOT go here

- **Self-notes / session state.** Those go in `.claude/cowork-notes.md` (Cowork) or `.claude/claude-code-notes.md` (Claude Code). Self-notes are persistent memory you keep for your own future sessions; handoffs are one-way coordination channels to the *other* assistant. Mixing them breaks the existence-as-signal rule.
- **Anything intended for git history.** This directory is gitignored except for this README — handoffs are by definition transient and shouldn't pollute the log.

## Don't prescribe branch names

Handoffs **must not** suggest branch names. Branch naming follows `docs/project-workflow.md` § PR conventions: `<issue_number>/<short-slug>` for Issue-linked PRs, `<slug>` for chore PRs. Claude Code derives the branch name from the linked Issue automatically — handoffs only need to point at the Issue.

Why: early Cowork handoffs prescribed branch names that contradicted the convention (e.g., suggesting `docs/pr1-restructure-foundation` for an Issue-linked PR when convention says `33/restructure-foundation`). Claude Code reasonably treated the handoff suggestion as more specific than project-workflow.md's general rule and used the wrong name twice. Removing the suggestion entirely closes the failure mode.

If you genuinely need to constrain the slug (e.g., the same Issue is being split across two parallel branches), describe the constraint in prose — "use slug `pre-flight-checks` since `add-leaderboard` is the other split" — but never the full branch name.

## Lifecycle and constraints

- **Cowork → Claude Code** is the easy direction. Cowork writes the file (via the normal Write tool — `docs/` is not in Cowork's protected zone). Claude Code reads it, applies the changes, commits, opens a PR, and `rm`s the handoff file as part of the PR. One round trip.
- **Claude Code → Cowork** is messier because Cowork can't delete files. The flow:
  1. Claude Code writes `claude-code-handoff.md` and pushes (or just leaves it on the working tree, since it's gitignored).
  2. Cowork reads it on next session start and addresses it in chat.
  3. Cowork tells Brendan it's been addressed and asks for the file to be deleted, OR notes in `.claude/cowork-notes.md` that the next Claude Code session should `rm` it.
  4. Brendan or Claude Code deletes the file.

Yes, this is awkward — it's the cost of Cowork's sandbox blocking `unlink()` (which is otherwise the *right* default for an AI agent that shouldn't be deleting files unsupervised).

## Why `docs/handoffs/` and not `.claude/`

Cowork's `Write` and `Edit` tools refuse to write any file under `.claude/`. The error string is "resolves to a protected location or a path outside the connected folder," and it's a Cowork tool-layer rule, not a filesystem permission — bash writes to `.claude/` succeed. The point of the rule is to prevent Cowork from silently rewriting its own context (`.claude/CLAUDE.md`, skills, `settings.local.json`), which is a healthy default for an AI agent. We don't want to poke holes in it just for handoffs.

`docs/handoffs/` sits outside the protected zone, supports the normal file tools, and reads naturally to humans browsing the repo.

## Document history

- 2026-05-05 — Initial creation as part of the handoff-convention move from `.claude/` to `docs/handoffs/`.
- 2026-05-05 — Added "Don't prescribe branch names" subsection. Cowork's first two handoffs (CLAUDE.md milestone-naming, PR 1 foundation) suggested branch names that contradicted `docs/workflow.md` § PR conventions; Claude Code followed the (wrong) suggestions both times. Convention is now to omit branch suggestions from handoffs entirely.
- 2026-05-08 — Updated the "Don't prescribe branch names" subsection's `workflow.md` references → `project-workflow.md` (operational doc renamed for clarity).
