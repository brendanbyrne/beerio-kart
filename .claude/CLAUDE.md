# Beerio Kart

## Project Phase
Phase 1 — Foundation (backend + frontend scaffolded, hello world working).

## Overview
Beerio Kart is a drinking game variant of Mario Kart. Details TBD as design evolves.

## Architecture at a Glance
React handles the UI, Vite serves it and proxies API calls, Axum handles the API, Tailwind handles the styling.

## Preferences
- Suggest better approaches when you see them, with reasoning and sources.
- Keep responses concise but explain the "why."
- Don't assume knowledge — Brendan has deep C++/Python experience but is new to web dev, databases, and Rust.
- When introducing web/database concepts, explain them briefly.

## Repo Copies
- **WSL (source of truth):** `/home/bbyrne/projects/beerio-kart`
- **Windows (read-only mirror):** `/mnt/c/Users/obiva/beerio-kart`
- Both point to the same GitHub remote. To sync Windows: `git -C /mnt/c/Users/obiva/beerio-kart pull --ff-only`. If branches diverge, reset Windows to remote: `git -C /mnt/c/Users/obiva/beerio-kart reset --hard origin/main`.
- **Auto-sync:** After any commit is pushed from WSL, sync the Windows copy without being asked.
- **Cowork handoff:** Cowork may leave uncommitted changes on the Windows copy. To process: (1) read the diff/untracked files from Windows, (2) apply the changes in WSL, (3) reset Windows to clean (`git -C ... checkout .` + `git -C ... clean -fd`).

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
- **Claude Code (WSL2 CLI):** Coding, building, testing, git operations. Works directly in the WSL2-native checkout (`/home/bbyrne/projects/beerio-kart/`).

### Git workflow

**Branching:** Simple feature branches. Work on a branch (e.g., `phase-1/foundation`, `feature/run-entry`), merge to `main` when complete. No PRs required for now — merge directly.

**Branch naming:** `phase-N/description` for phase work, `feature/description` for standalone features, `fix/description` for bug fixes.

**Coordination between assistants:**

- **Claude Code** must `git push` after making changes so Cowork can see them.
- **Cowork** cannot push to GitHub. After Cowork edits files, Brendan or Claude Code must commit and push.
- Both should check `git status` before starting work to avoid conflicts.
- If both need to edit the same file, coordinate through the user (Brendan).

### Who does what

| Task | Tool |
|------|------|
| Architecture & design docs | Cowork |
| Code implementation | Claude Code |
| Building & testing | Claude Code |
| Git commits & pushes | Claude Code (or Brendan) |
| Code review & research | Either |
| Deployment config | Claude Code (with Cowork for planning) |
| Browser-based tasks | Cowork |
