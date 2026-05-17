# Beerio Kart — Frontend

Loaded automatically when Claude works in `frontend/`. Captures conventions specific to the React + TypeScript + Vite + Tailwind layer. For project-wide conventions, the root [`.claude/CLAUDE.md`](../.claude/CLAUDE.md) still applies — this file adds frontend-specific layers.

## Stack

- **React 19 + TypeScript 5.9** for components.
- **Vite** for the dev server and bundler.
- **Tailwind v4** for styling — utility-first, mobile-first by convention. Note v4's CSS-first config (`@theme` blocks, `@import "tailwindcss"`); v3 patterns like `tailwind.config.js` and `@tailwind base/components/utilities` directives don't apply.
- **Bun** for package management and script running (not npm).
- **react-router-dom 7** for client-side routing (declarative mode — see [`docs/coding-standards/react.md`](../docs/coding-standards/react.md) § Routing).

## Key reading

For deep conventions, read these once and refer back when relevant:

- [`docs/coding-standards/typescript.md`](../docs/coding-standards/typescript.md) — TS language rules: tsconfig strictness, `type` vs `interface`, branded types, discriminated unions, error handling, runtime validation, lints, anti-patterns, backend interop.
- [`docs/coding-standards/react.md`](../docs/coding-standards/react.md) — React 19 patterns: component shape, hooks, data fetching (TanStack Query), forms (`useActionState`), error boundaries, accessibility, routing, file organization.
- [`docs/coding-standards/tailwind.md`](../docs/coding-standards/tailwind.md) — Tailwind v4 styling: utility-first, mobile-first, CSS-first `@theme`, `clsx` for conditional classes, touch targets, Firefox/Safari quirks.
- [`docs/user-workflows.md`](../docs/user-workflows.md) — end-user flows and screen-by-screen UI breakdown. Match what's specified there before introducing new UI patterns.
- [`docs/api-contract.md`](../docs/api-contract.md) — endpoint catalog (§ 1) and wire-format conventions. The frontend consumes a typed client; never hand-roll fetches per endpoint.
- [`docs/design.md`](../docs/design.md) — high-level principles (single-handed, never rushed, inclusive by default) and design goals (minimize choices, prefer simple interactions, sensible defaults).
- [`docs/designs/2026-05-16-frontend-compliance-plan.md`](../docs/designs/2026-05-16-frontend-compliance-plan.md) — active sequenced PR plan bringing the existing frontend code up to the standards. Consult before starting cross-cutting refactors to see what's already planned.

## UI reference device

Use the **Pixel 9 Pro** as the reference phone for all UI mockups and layout work. Physical resolution: 1280 × 2856 pixels at 495 ppi. Logical (CSS) resolution: ~427 × 952 px at 3× device pixel ratio.

Layout assumes a thumb-reachable bottom-nav region; the design principles in [`docs/design.md`](../docs/design.md) require single-handed usability.

## Browser support

**Firefox is a required target alongside Chrome/Safari mobile.** Avoid Chrome-only APIs or `-webkit-` prefixes without Firefox equivalents. Test on Firefox before shipping any UI change. (See [`docs/design.md`](../docs/design.md) § Technical Constraints and [`docs/coding-standards/tailwind.md`](../docs/coding-standards/tailwind.md) § Firefox & Safari compatibility.)

## Code review

PR reviews of frontend code consult `docs/coding-standards/typescript.md`, `react.md`, and `tailwind.md` as the rule set. A reviewer (human or AI) reading only one of these can do a focused review of a diff that touches only that area — the split is deliberate. The same applies to the `code-review` skill: when reviewing a frontend PR, it should pull in the relevant standards file(s) by topic.

## Document history

- 2026-05-08 — Initial creation as part of PR 6 / Issue [#79](https://github.com/brendanbyrne/beerio-kart/issues/79). UI reference device sentence sourced from root `.claude/CLAUDE.md`; Firefox-first note sourced from `docs/design.md` § Technical Constraints. Style and stack notes synthesized from existing project state. Filed as a stub per Issue #38 AC ("frontend stub may stay light").
- 2026-05-16 — Promoted from stub. Replaced the inline `## Style` and `## Stub note` sections with pointers to the new `docs/coding-standards/typescript.md`, `react.md`, and `tailwind.md`. Added a `## Code review` section explaining how the coding standards plug into PR review. Added a pointer to the frontend compliance plan. Stack section updated with explicit React 19, TS 5.9, react-router 7 versions and a declarative-mode note. Companion to the standards creation same day; see those files' history and `docs/coding-standards/README.md` for the full rollup.
