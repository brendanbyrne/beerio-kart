# Beerio Kart

## Start of Session
Read `docs/design.md` at the start of every session. It is the single source of truth for the project's design and reflects the full history of design decisions.

### Handoff files

Async task-passing between the two assistants lives in `.agents/handoffs/`, named for the **recipient** (Cowork writes the file Claude Code reads, and vice versa); the file's existence is the signal, and the reader deletes it when done. Format, lifecycle, and dated-variant naming: [`.agents/handoffs/README.md`](../.agents/handoffs/README.md). The two inboxes:

- **`.agents/handoffs/claude-code.md`** — Claude Code's inbox (Cowork writes). **Check it before starting work.**
- **`.agents/handoffs/cowork.md`** — Cowork's inbox (Claude Code writes). Cowork can't delete files (sandbox blocks `unlink()`), so Brendan or the next Claude Code session clears it after Cowork acks in chat.

**Anything the other assistant must act on — task specs, review findings, bug reports, design decisions, answers — MUST go in the handoff file, not just chat.** If you're composing a substantive response the other assistant needs, stop and write it there. **Don't use handoffs for your own session notes** — they're one-way action channels (file exists ⇒ work to do); Cowork keeps self-notes in `.agents/memory/cowork.md`, Claude Code uses its native cross-session memory.

**Why `.agents/` and not `.claude/`:** `.claude/` is low-churn, every-session conventions (this file, skills, settings); `.agents/` is high-churn per-agent state and comms. Splitting by lifecycle keeps the always-read surface small. Cowork's `Write`/`Edit` also refuse `.claude/` writes (anti-self-modification guard), so handoffs/memory must live outside it.

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
- **Subagent capability default.** Unless a task explicitly specifies a different level, launch all subagents at the **Opus** level of capability.

## Repo Location
- **Single checkout:** `C:\Users\obiva\beerio-kart` (Windows), accessible from WSL2 at `/mnt/c/Users/obiva/beerio-kart`.
- Both Cowork (Claude Desktop) and Claude Code (WSL2 CLI) work on this same checkout. No syncing needed.

## Conventions (cross-cutting)
- Use LF (`\n`) line endings, not CRLF (`\r\n`).
- Keep `.gitattributes` in the repo root. Only add nested ones if a subdirectory needs genuinely distinct Git behavior (e.g., LFS for large assets).
- Drafts in `docs/drafts/` are gitignored except for `WIP_*.md` files. Aggressive cleanup commands (`git clean -fdx`) will wipe them — check `docs/drafts/` before running them.

Backend-specific conventions (database naming, Rust style, testing) live in [`backend/CLAUDE.md`](../backend/CLAUDE.md). Frontend-specific conventions (stack, UI reference device, browser support) live in [`frontend/CLAUDE.md`](../frontend/CLAUDE.md). Doc-area conventions live in [`docs/CLAUDE.md`](../docs/CLAUDE.md), which also owns the Document history rule.

## Development Workflow

Two assistants share this checkout:

- **Cowork (Claude Desktop)** — design, architecture, docs, research, review. Edits files only; **cannot run git** (its sandbox blocks `unlink()`, which git needs). Reads/writes GitHub via the Composio MCP, as `brendanbyrne`.
- **Claude Code (WSL2 CLI)** — coding, building, testing, and all git operations. Reads/writes GitHub via `gh`.

The full operational guide — the two environments and their constraints, the GitHub-MCP capability matrix, the who-does-what table, Issue lifecycle, branch/commit conventions, milestones, triage, and handoff patterns — is [`docs/project-workflow.md`](../docs/project-workflow.md). Project-board field IDs are cached in [`docs/project-field-ids.md`](../docs/project-field-ids.md).

**Load-bearing git rules** (canonical detail in `project-workflow.md` § PR conventions → Review and merge):

- **Never push directly to `main`.** All code changes require a PR.
- **Never merge your own PR.** Only Brendan merges.
- Documentation-only changes (CLAUDE.md files, `docs/` content) can be committed to `main` directly — no code review needed.
- Claude Code **pushes** after making changes so the remote stays current; both assistants **check `git status` before starting**, since the shared checkout may carry the other's uncommitted work.
