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

## Repo Copies
- **WSL (source of truth):** `/home/bbyrne/projects/beerio-kart`
- **Windows (read-only mirror):** `/mnt/c/Users/obiva/beerio-kart`
- Both point to the same GitHub remote. To sync Windows: `git -C /mnt/c/Users/obiva/beerio-kart pull --ff-only`. If branches diverge, reset Windows to remote: `git -C /mnt/c/Users/obiva/beerio-kart reset --hard origin/main`.
- **Auto-sync:** After any commit is pushed from WSL, sync the Windows copy without being asked.
- **Cowork handoff:** Cowork may leave uncommitted changes on the Windows copy. To process: (1) read the diff/untracked files from Windows, (2) apply the changes in WSL, (3) reset Windows to clean (`git -C ... checkout .` + `git -C ... clean -fd`).

## Conventions
- Use LF (`\n`) line endings, not CRLF (`\r\n`).
- Keep `.gitattributes` in the repo root. Only add nested ones if a subdirectory needs genuinely distinct Git behavior (e.g., LFS for large assets).
