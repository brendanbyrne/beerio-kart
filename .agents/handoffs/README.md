# Handoff files

This directory holds short-lived task specs and questions that pass between Cowork (Claude Desktop, design/docs/research) and Claude Code (WSL2 CLI, code/build/test/git). It exists because the two assistants don't share a chat history — anything one needs the other to act on has to be written down somewhere they'll both look.

## The convention

Two channels. **Files are named for the recipient, not the writer** — `cowork.md` is the file Cowork reads; Claude Code writes it.

- **`claude-code.md`** — for Claude Code (Cowork writes). Cowork writes; Claude Code reads, acts on it, and deletes it.
- **`cowork.md`** — for Cowork (Claude Code writes). Claude Code writes; Cowork reads and acknowledges in chat. Brendan or the next Claude Code session deletes it (Cowork's sandbox blocks `unlink()` repo-wide, so Cowork can't clean up its own inbox).

**The writer creates the file; the file's existence is the signal that there is work to do.** No file means no work.

For task-specific handoffs that shouldn't collide with the canonical filenames (e.g. multiple parallel work items), use a dated slug:

- `claude-code-<YYYY-MM-DD>-<slug>.md`
- `cowork-<YYYY-MM-DD>-<slug>.md`

## What goes here

- **Task specs.** "Apply these three edits to file X, open a PR, and delete this handoff."
- **Questions and research requests.** "What should the API contract say about idempotency keys on `POST /runs`?"
- **Code review findings, bug reports, design decisions, answers to the other assistant's questions.**

If you find yourself composing a substantive response in chat that the other assistant needs to see in order to do their next piece of work, **stop and write it here instead**. Anything delivered only in chat is invisible to the other assistant.

## What does NOT go here

- **Self-notes / session state.** Those go in `.agents/memory/cowork.md` (Cowork) or `.agents/memory/claude-code.md` (Claude Code). Note that the basenames intentionally match the handoff filenames — both `.agents/memory/cowork.md` and `.agents/handoffs/cowork.md` are *for* Cowork, just one is from itself and the other is from Claude Code. Self-notes are persistent memory you keep for your own future sessions; handoffs are one-way coordination channels to the *other* assistant. Mixing them breaks the existence-as-signal rule.
- **Anything intended for git history.** This directory is gitignored except for this README — handoffs are by definition transient and shouldn't pollute the log.

## Don't prescribe branch names

Handoffs **must not** suggest branch names. Branch naming follows [`docs/project-workflow.md`](../../docs/project-workflow.md) § PR conventions: `<issue_number>/<short-slug>` for Issue-linked PRs, `<slug>` for chore PRs. Claude Code derives the branch name from the linked Issue automatically — handoffs only need to point at the Issue.

Why: early Cowork handoffs prescribed branch names that contradicted the convention (e.g., suggesting `docs/pr1-restructure-foundation` for an Issue-linked PR when convention says `33/restructure-foundation`). Claude Code reasonably treated the handoff suggestion as more specific than project-workflow.md's general rule and used the wrong name twice. Removing the suggestion entirely closes the failure mode.

If you genuinely need to constrain the slug (e.g., the same Issue is being split across two parallel branches), describe the constraint in prose — "use slug `pre-flight-checks` since `add-leaderboard` is the other split" — but never the full branch name.

## Lifecycle and constraints

- **Cowork → Claude Code** is the easy direction. Cowork writes the file (via the normal Write tool — `.agents/` is not in Cowork's protected zone). Claude Code reads it, applies the changes, commits, opens a PR, and `rm`s the handoff file as part of the PR. One round trip.
- **Claude Code → Cowork** is messier because Cowork can't delete files. The flow:
  1. Claude Code writes `cowork.md` and pushes (or just leaves it on the working tree, since it's gitignored).
  2. Cowork reads it on next session start and addresses it in chat.
  3. Cowork tells Brendan it's been addressed and asks for the file to be deleted, OR notes in `.agents/memory/cowork.md` that the next Claude Code session should `rm` it.
  4. Brendan or Claude Code deletes the file.

Yes, this is awkward — it's the cost of Cowork's sandbox blocking `unlink()` (which is otherwise the *right* default for an AI agent that shouldn't be deleting files unsupervised).

## Why `.agents/` and not `.claude/`

`.claude/` is reserved for low-churn project conventions read every session — `CLAUDE.md`, skills, `settings.local.json`. Mixing high-churn agent state (memory, transient handoffs) with stable conventions made `.claude/` harder to scan and conceptually muddy. `.agents/` is the deliberate home for high-churn state, separate from conventions.

There's also a Cowork tool-layer constraint that made the older `docs/handoffs/` location necessary: Cowork's `Write` and `Edit` tools refuse to write any file under `.claude/`. The error string is "resolves to a protected location or a path outside the connected folder," and the rule prevents Cowork from silently rewriting its own context (`.claude/CLAUDE.md`, skills, `settings.local.json`). That protection is healthy and we don't want to poke holes in it for handoffs. `.agents/` sits outside the protected zone, so the normal file tools work — and the conceptual separation is a happy bonus rather than just a workaround.

## Document history

- 2026-05-05 — Initial creation as part of the handoff-convention move from `.claude/` to `docs/handoffs/`.
- 2026-05-05 — Added "Don't prescribe branch names" subsection. Cowork's first two handoffs (CLAUDE.md milestone-naming, PR 1 foundation) suggested branch names that contradicted `docs/workflow.md` § PR conventions; Claude Code followed the (wrong) suggestions both times. Convention is now to omit branch suggestions from handoffs entirely.
- 2026-05-08 — Updated the "Don't prescribe branch names" subsection's `workflow.md` references → `project-workflow.md` (operational doc renamed for clarity).
- 2026-05-08 — Moved from `docs/handoffs/README.md` to `.agents/handoffs/README.md`. Path references throughout updated (`cowork-handoff.md` etc. now live under `.agents/handoffs/`). Self-notes pointer updated from `.claude/cowork-notes.md` → `.agents/memory/cowork.md`. "Why `docs/handoffs/` and not `.claude/`" section rewritten as "Why `.agents/` and not `.claude/`" — same fundamental constraint, but the framing is now "deliberate home for high-churn state" rather than "workaround for the tool-layer block." Closes part of [#79](https://github.com/brendanbyrne/beerio-kart/issues/79).
- 2026-05-09 — Renamed handoff files to be addressed to the recipient instead of the writer, and dropped the `-handoff` suffix: `cowork-handoff.md` → `claude-code.md`; `claude-code-handoff.md` → `cowork.md`. Dated variants follow the same pattern (`claude-code-<YYYY-MM-DD>-<slug>.md` / `cowork-<YYYY-MM-DD>-<slug>.md`). Side-effect: handoff basenames now match the corresponding `.agents/memory/` file's basename, since both are "for" the same recipient — the directory disambiguates writer (memory = self, handoff = other assistant).
