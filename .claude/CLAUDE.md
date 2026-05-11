# Beerio Kart

## Start of Session
Read `docs/design.md` at the start of every session. It is the single source of truth for the project's design and reflects the full history of design decisions.

### Handoff files

Two handoff channels enable async task passing between assistants. The writer creates the file; the reader deletes it when done — the file's existence is the signal.

Handoff files live under `.agents/handoffs/`. The directory is gitignored except for `.agents/handoffs/README.md`, which documents the format and lifecycle for humans browsing the repo.

Files are named for the **recipient**, not the writer. So Cowork writes the file Claude Code reads, and vice versa.

- **`.agents/handoffs/claude-code.md`** — for Claude Code (Cowork writes). Check before starting work. Contains task instructions from the architecture/design assistant. Claude Code deletes after completing the work.
- **`.agents/handoffs/cowork.md`** — for Cowork (Claude Code writes). Write this when you have questions, research requests, or design decisions for Cowork. Brendan or the next Claude Code session deletes after Cowork acknowledges in chat — Cowork's sandbox blocks `unlink()` repo-wide, so Cowork can't clean up its own inbox.

For task-specific handoffs that shouldn't collide with the canonical filenames, use a dated variant: `claude-code-<YYYY-MM-DD>-<slug>.md` or `cowork-<YYYY-MM-DD>-<slug>.md`.

**Anything intended for the other assistant to act on — task specs, code review findings, bug reports, design decisions, answers to their questions — MUST be written to the appropriate handoff file.** Delivering it only in chat means the other assistant won't see it. If you find yourself composing a substantive response that the other assistant needs, stop and write it to the handoff file instead.

**Do not use handoff files for your own session notes.** The handoff files are one-way channels between assistants — if the file exists, the recipient assumes there's work to do. For self-notes or session state you want to preserve across your own sessions, use `.agents/memory/cowork.md` (Cowork) or `.agents/memory/claude-code.md` (Claude Code). These memory files are gitignored — they're per-checkout state, not durable artifacts.

**Why `.agents/` and not `.claude/`:** `.claude/` is for low-churn project conventions read every session (this file, skills, settings). `.agents/` is for high-churn per-agent state and inter-agent comms. Splitting them by lifecycle keeps the every-session reading surface small. Mechanically, Cowork's `Write`/`Edit` tools refuse `.claude/` writes (a tool-layer protection against silent self-modification of context); `.agents/` is outside that protection, so handoffs and memory work with the normal file tools.

## Project Phase
Star — Sessions & Run Recording (core gameplay loop). See [`docs/roadmap.md`](../docs/roadmap.md) for the cup-by-cup narrative and the full list of milestones.

## Overview
Beerio Kart is a mobile-first web app for tracking times and stats for a Mario Kart 8 Deluxe drinking game. Players race time trials, optionally drink, and the app tracks personal bests, leaderboards, and run history. Non-drinkers are equally welcome — inclusive by default is a core design principle.

## Architecture at a Glance
React handles the UI, Vite serves it and proxies API calls, Axum handles the API, SeaORM (backed by sqlx) is the ORM, SQLite is the database (with a path to PostgreSQL later), and Tailwind handles the styling. Bun is used instead of npm for package management.

## Preferences
- Suggest better approaches when you see them, with reasoning and sources.
- Keep responses concise but explain the "why."
- Don't assume knowledge — Brendan has deep C++/Python experience but is new to web dev, databases, and Rust.
- If a command fails because it needs sudo or a password, **stop and ask Brendan** before trying alternative approaches. Don't attempt workarounds (e.g., installing via conda, pip, or other package managers) — just explain what's needed and let Brendan handle the installation.
- When introducing web/database concepts, explain them briefly.

## Repo Location
- **Single checkout:** `C:\Users\obiva\beerio-kart` (Windows), accessible from WSL2 at `/mnt/c/Users/obiva/beerio-kart`.
- Both Cowork (Claude Desktop) and Claude Code (WSL2 CLI) work on this same checkout. No syncing needed.
- Backend-specific WSL2 build-performance tip is in [`backend/CLAUDE.md`](../backend/CLAUDE.md).

## Conventions (cross-cutting)
- Use LF (`\n`) line endings, not CRLF (`\r\n`).
- Keep `.gitattributes` in the repo root. Only add nested ones if a subdirectory needs genuinely distinct Git behavior (e.g., LFS for large assets).
- Drafts in `docs/drafts/` are gitignored except for `WIP_*.md` files. Aggressive cleanup commands (`git clean -fdx`) will wipe them — check `docs/drafts/` before running them.

Backend-specific conventions (database naming, Rust style, schema-changes-prelaunch, testing) live in [`backend/CLAUDE.md`](../backend/CLAUDE.md). Frontend-specific conventions (stack, UI reference device, browser support) live in [`frontend/CLAUDE.md`](../frontend/CLAUDE.md). Doc-area conventions live in [`docs/CLAUDE.md`](../docs/CLAUDE.md), which also owns the Document history rule.

## Development Workflow

### Two-assistant setup

This project uses two AI environments:

- **Cowork (Claude Desktop):** Design, architecture, documentation, research, review. Accesses the repo via a Windows mount (`C:\Users\obiva\beerio-kart`). Cannot access WSL2 filesystem and **cannot run git commands** — the Cowork sandbox mounts the repo via virtiofs with `unlink()` blocked at the mount layer (every file delete returns `EPERM`, including files Cowork just created). Git relies on creating and removing `.git/index.lock` and temp objects, so any git invocation from Cowork either fails outright or leaves a stale lock that breaks the next attempt. Cowork edits files only. For GitHub API operations (issues, PRs, project board) see § GitHub access below.
- **Claude Code (WSL2 CLI):** Coding, building, testing, git operations. Accesses the same checkout via `/mnt/c/Users/obiva/beerio-kart/`. WSL2's `/mnt/c` (9P/DrvFs) supports unlink, so git works fine there.

### GitHub access

Cowork can read and write GitHub data — issues, pull requests, project board items, milestones — through Composio's GitHub MCP connector. The connection authenticates as the GitHub user `brendanbyrne`. Anything Cowork does via the MCP appears in the GitHub UI under that account.

**What Cowork can do via the MCP:**

- File, label, assign, triage, and close issues; add comments.
- Read PR diffs, conversation threads, and review state. (Creating commits or PRs still requires Claude Code — that's a git operation, not an API operation.)
- Add items to the project board, move them between Status columns, set custom field values, attach milestones.
- Create and manage milestones.
- Run arbitrary GraphQL queries against `api.github.com/graphql` when no purpose-built tool exists.

**What the MCP cannot do, regardless of which assistant calls it:**

- **Run `git`.** The MCP only talks to GitHub's API. Branch creation, commits, pushes, and merges remain Claude Code's job.
- **Create or modify the project's built-in workflows** (auto-add, auto-close, auto-archive). GitHub's API does not expose these — they are Settings-UI-only.
- Custom fields, single-select option lists, and views *can* be created/edited via API — see [`docs/project-field-ids.md`](../docs/project-field-ids.md) for which path (GraphQL vs. Composio REST shim) works for which field type, and known REST-shim 500s.
- **Set Assignees, Labels, Milestone, or Repository via the project field mutation.** Those are properties of the underlying issue/PR; use the issue/PR mutations instead.

**What Claude Code can do via `gh`:** see [`docs/project-workflow.md`](../docs/project-workflow.md) § Claude Code's autonomy in moving Issue status for the token scopes, the `updateProjectV2ItemFieldValue` mutation, and the `INSUFFICIENT_SCOPES` recovery.

**Field IDs reference:** [`docs/project-field-ids.md`](../docs/project-field-ids.md) caches the project's field IDs and Status option IDs. Consult that file before issuing project-board write calls — both Composio MCP and `gh api graphql` require IDs, not names. Update the file if anyone changes the project's fields in the GitHub UI.

**Milestone naming:** Two milestone types — *product cups* (Mario Kart 8 Deluxe cup names: Mushroom, Flower, Star, Special, Shell, Banana, Leaf, Lightning, etc.) for user-facing feature work-chunks, and *workstreams* (topical prefixes: `Hardening:`, `Docs:`, etc.) for cross-cutting infrastructure that runs concurrent with product cups. Product cups use title format `<CupName>: <Description>` (e.g., `Star: Sessions & Run Recording`); workstreams use `<Topic>: <Description>`. Full convention is in [`docs/project-workflow.md`](../docs/project-workflow.md) § Milestone lifecycle; current mapping lives in [`docs/roadmap.md`](../docs/roadmap.md).

**When to use Cowork vs. Claude Code for GitHub work:** anything that ends in a commit, push, or merge → Claude Code. Anything that stays inside GitHub's API surface (issue triage, project board updates, PR comments, milestone management) → either works; pick whichever assistant is already in the conversation. Cowork is often faster for chat-driven triage; Claude Code is the natural choice when the GitHub action is part of a code-bearing PR (e.g., move-Issue-on-pickup at the start of a branch).

### Git workflow

For Issue lifecycle, branch naming (`<issue_number>/<short-slug>`), commit message format (`<issue_number>: <summary>`), PR template, and triage cadence, see [`docs/project-workflow.md`](../docs/project-workflow.md). It's the canonical operational guide; this file's role is the high-level role split, not the workflow details.

**Rules summary** (cross-references to project-workflow.md for detail):

- **Never push directly to `main`.** All code changes require a PR.
- **Never merge your own PR.** Only Brendan merges.
- Documentation-only changes (this file, `docs/design.md`) can be committed to `main` directly — they don't need code review.

**Coordination between assistants:**

- Both assistants work on the same checkout — no push/pull needed to see each other's changes.
- **Cowork** cannot run git at all (its sandbox mount blocks `unlink`). When Cowork wants a change committed, it edits the working tree and notes the intended commit in `.agents/handoffs/claude-code.md` or chat; Brendan or Claude Code then stages, commits, and pushes.
- **Claude Code** must `git push` after making changes so the remote stays current.
- Both should check `git status` before starting work to avoid conflicts.
- If both need to edit the same file, coordinate through the user (Brendan).

### Who does what

| Task | Tool |
|------|------|
| Architecture & design docs | Cowork |
| Code implementation | Claude Code |
| Building & testing | Claude Code |
| Git commits | Claude Code or Brendan (Cowork cannot run git) |
| Git pushes | Claude Code (or Brendan) |
| Code review & research | Either |
| Project board / issue triage | Either (Cowork via MCP, Claude Code via `gh`) |
| Deployment config | Claude Code (with Cowork for planning) |
| Browser-based tasks | Cowork |
| Design records | Cowork (writes to `docs/designs/`) |
| PR reviews | Claude Code (posts as PR comment via `gh pr comment` or MCP) |
