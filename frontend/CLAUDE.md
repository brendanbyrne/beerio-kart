# Beerio Kart — Frontend

Loaded automatically when Claude works in `frontend/`. Captures conventions specific to the React + TypeScript + Vite + Tailwind layer. For project-wide conventions, the root [`.claude/CLAUDE.md`](../.claude/CLAUDE.md) still applies — this file adds frontend-specific layers.

## Stack

- **React + TypeScript** for components.
- **Vite** for the dev server and bundler.
- **Tailwind CSS** for styling — utility-first, mobile-first by convention.
- **Bun** for package management and script running (not npm).
- **lucide-react** for icons.

## UI reference device

Use the **Pixel 9 Pro** as the reference phone for all UI mockups and layout work. Physical resolution: 1280 × 2856 pixels at 495 ppi. Logical (CSS) resolution: ~427 × 952 px at 3× device pixel ratio.

Layout assumes a thumb-reachable bottom-nav region; the design principles in [`docs/design.md`](../docs/design.md) require single-handed usability.

## Browser support

**Firefox is a required target alongside Chrome/Safari mobile.** Avoid Chrome-only APIs or `-webkit-` prefixes without Firefox equivalents. Test on Firefox before shipping any UI change. (See [`docs/design.md`](../docs/design.md) § Technical Constraints.)

## Style

- TypeScript everywhere; no plain `.js`.
- Functional React components with hooks. No class components.
- Tailwind utility classes for styling; avoid bespoke CSS unless a Tailwind primitive doesn't exist.
- Prettier handles formatting; ESLint catches problems. Both run pre-commit via lefthook (see root README § Linting & Formatting).

## Key reading

- [`docs/user-workflows.md`](../docs/user-workflows.md) — end-user flows and screen-by-screen UI breakdown. Match what's specified there before introducing new UI patterns.
- [`docs/api-contract.md`](../docs/api-contract.md) — endpoint catalog (§ 1) and wire-format conventions. The frontend consumes a typed client; never hand-roll fetches per endpoint.
- [`docs/design.md`](../docs/design.md) — high-level principles (single-handed, never rushed, inclusive by default) and design goals (minimize choices, prefer simple interactions, sensible defaults).

## Stub note

This file is intentionally light. Frontend conventions haven't been documented in depth yet — the project is still in the early "build the app" stage of frontend work. Expect this to grow as more patterns settle (state management, form patterns, error UX, etc.).

## Document history

- 2026-05-08 — Initial creation as part of PR 6 / Issue [#79](https://github.com/brendanbyrne/beerio-kart/issues/79). UI reference device sentence sourced from root `.claude/CLAUDE.md`; Firefox-first note sourced from `docs/design.md` § Technical Constraints. Style and stack notes synthesized from existing project state. Filed as a stub per Issue #38 AC ("frontend stub may stay light").
