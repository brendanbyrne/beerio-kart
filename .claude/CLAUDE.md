# Beerio Kart

## Start of Session
Read `DESIGN.md` (repo root) at the start of every session. It is the single source of truth for the project's design and reflects the full history of design decisions.

### Handoff from Cowork
Before starting work, check if `.claude/cowork-handoff.md` exists. If it does, read it — it contains task instructions written by Cowork (the architecture/design assistant). Follow those instructions for the current task. After completing the work, delete the file so Cowork knows the task is done.

## Project Phase
Phase 1 — Foundation (backend + frontend scaffolded, hello world working).

## Overview
Beerio Kart is a mobile-first web app for tracking times and stats for a Mario Kart 8 Deluxe drinking game. Players race time trials, optionally drink, and the app tracks personal bests, leaderboards, and run history. Non-drinkers are equally welcome — inclusive by default is a core design principle.

## Architecture at a Glance
React handles the UI, Vite serves it and proxies API calls, Axum handles the API, SeaORM (backed by sqlx) is the ORM, SQLite is the database (with a path to PostgreSQL later), and Tailwind handles the styling. Bun is used instead of npm for package management.

## Preferences
- Suggest better approaches when you see them, with reasoning and sources.
- Keep responses concise but explain the "why."
- Don't assume knowledge — Brendan has deep C++/Python experience but is new to web dev, databases, and Rust.
- When introducing web/database concepts, explain them briefly.

## Repo Location
- **Single checkout:** `C:\Users\obiva\beerio-kart` (Windows), accessible from WSL2 at `/mnt/c/Users/obiva/beerio-kart`
- Both Cowork (Claude Desktop) and Claude Code (WSL2 CLI) work on this same checkout. No syncing needed.
- **Performance note:** WSL2 accessing `/mnt/c/` is slower than the native Linux filesystem, especially for `cargo build`. If build times become painful, configure Cargo to put build artifacts on the Linux filesystem while keeping source on Windows:
  ```toml
  # backend/.cargo/config.toml
  [build]
  target-dir = "/home/bbyrne/.cargo-target/beerio-kart"
  ```

## Conventions
- Use LF (`\n`) line endings, not CRLF (`\r\n`).
- Keep `.gitattributes` in the repo root. Only add nested ones if a subdirectory needs genuinely distinct Git behavior (e.g., LFS for large assets).
- Database naming: Tables plural snake_case, columns snake_case, FKs `{singular}_id`, PKs `id`.
- Rust style: Follow standard `rustfmt` and `clippy` conventions.
- Frontend style: TypeScript, functional React components, Tailwind for styling.

## Development Workflow

### Two-assistant setup

This project uses two AI environments:

- **Cowork (Claude Desktop):** Design, architecture, documentation, research, review. Accesses the repo via a Windows mount (`C:\Users\obiva\beerio-kart`). Cannot access WSL2 filesystem. Cannot access GitHub directly.
- **Claude Code (WSL2 CLI):** Coding, building, testing, git operations. Accesses the same checkout via `/mnt/c/Users/obiva/beerio-kart/`.

### Git workflow

**Branching:** Simple feature branches. All code changes go through pull requests — never push directly to `main`.

**Branch naming:** `phase-N/description` for phase work, `feature/description` for standalone features, `fix/description` for bug fixes.

**Pull request workflow:**

1. Claude Code creates a feature branch, commits work, and pushes to GitHub.
2. Claude Code opens a PR via `gh pr create` with a description summarizing what changed and why.
3. Brendan reviews the diff on GitHub (or via `gh pr diff` / Cowork in Chrome).
4. Brendan approves and merges (GitHub UI or `gh pr merge`).

**Rules:**
- **Never push directly to `main`.** All code changes require a PR.
- **Never merge your own PR.** Only Brendan merges.
- PR descriptions should summarize the changes, call out anything non-obvious, and list any open questions.
- Documentation-only changes (CLAUDE.md, DESIGN.md) can be committed to `main` directly since Cowork can't push and these don't need code review.

**Coordination between assistants:**

- Both assistants work on the same checkout — no push/pull needed to see each other's changes.
- **Cowork** can commit locally but cannot push to GitHub (no DNS access to github.com). After Cowork commits, Brendan or Claude Code must `git push`.
- **Claude Code** must `git push` after making changes so the remote stays current.
- Both should check `git status` before starting work to avoid conflicts.
- If both need to edit the same file, coordinate through the user (Brendan).

### Who does what

| Task | Tool |
|------|------|
| Architecture & design docs | Cowork |
| Code implementation | Claude Code |
| Building & testing | Claude Code |
| Git commits (local) | Either (Cowork can commit but not push) |
| Git pushes | Claude Code (or Brendan) |
| Code review & research | Either |
| Deployment config | Claude Code (with Cowork for planning) |
| Browser-based tasks | Cowork |
