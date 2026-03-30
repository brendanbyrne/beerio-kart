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

**Branching:** Simple feature branches. Work on a branch (e.g., `phase-1/foundation`, `feature/run-entry`), merge to `main` when complete. No PRs required for now — merge directly.

**Branch naming:** `phase-N/description` for phase work, `feature/description` for standalone features, `fix/description` for bug fixes.

**Coordination between assistants:**

- Both assistants work on the same checkout — no push/pull needed to see each other's changes.
- **Cowork** cannot access GitHub directly. After Cowork edits files, Brendan or Claude Code must commit and push.
- **Claude Code** must `git push` after making changes so the remote stays current.
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
