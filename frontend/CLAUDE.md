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
- [`docs/api-contract.md`](../docs/api-contract.md) — endpoint catalog (§ 1) and wire-format conventions. The frontend uses per-endpoint hand-written `fetch` wrappers with Zod schemas at the boundary; see [`docs/decisions/0039-api-client-generation.md`](../docs/decisions/0039-api-client-generation.md) for the decision and adoption trigger.
- [`docs/design.md`](../docs/design.md) — high-level principles (single-handed, never rushed, inclusive by default) and design goals (minimize choices, prefer simple interactions, sensible defaults).
- [`docs/designs/2026-05-16-frontend-compliance-plan.md`](../docs/designs/2026-05-16-frontend-compliance-plan.md) — active sequenced PR plan bringing the existing frontend code up to the standards. Consult before starting cross-cutting refactors to see what's already planned.

## UI reference device

Use the **Pixel 9 Pro** as the reference phone for all UI mockups and layout work. Physical resolution: 1280 × 2856 pixels at 495 ppi. Logical (CSS) resolution: ~427 × 952 px at 3× device pixel ratio.

Layout assumes a thumb-reachable bottom-nav region; the design principles in [`docs/design.md`](../docs/design.md) require single-handed usability.

## Browser support

**Firefox is a required target alongside Chrome/Safari mobile.** Avoid Chrome-only APIs or `-webkit-` prefixes without Firefox equivalents. Test on Firefox before shipping any UI change. (See [`docs/design.md`](../docs/design.md) § Technical Constraints and [`docs/coding-standards/tailwind.md`](../docs/coding-standards/tailwind.md) § Firefox & Safari compatibility.)

## Testing

**Tests are a deliverable, not optional.** Every PR that adds logic must include tests. PRs should not be opened without them. The principle: **every requirement placed on the code should be unit- or integration-testable, within reason.** See [`docs/coding-standards/typescript.md`](../docs/coding-standards/typescript.md) § 12 for the umbrella policy and the language-level patterns (Vitest, schemas, parsers, branded constructors); see [`docs/coding-standards/react.md`](../docs/coding-standards/react.md) § 13 for the React-specific patterns (React Testing Library, MSW, hook tests with provider wrappers).

- **Unit tests:** Co-located `*.test.ts` (or `*.test.tsx`) files next to the code they test. Cover branded constructors, Zod schemas, parsers, formatters, hook behavior, component interactions.
- **Integration tests:** End-to-end flow tests under `frontend/src/__tests__/` using React Testing Library + MSW to mock the API. Cover the major user flows (login → create session → submit run, etc.).
- **What doesn't need tests:** Pure presentational components with no interactive behavior, thin layout wrappers, generated types, app shell code that just composes children, one-time bootstrap. **If the PR can't name a user-visible behavior that would silently break, the test is theater.**
- **Test naming:** Sentences. `it('rejects an empty username', ...)`, not `it('test1', ...)`.
- **CI:** `bun test` runs on every PR; `vitest run --coverage` uploads to Codecov, mirroring the backend setup in [`docs/design.md`](../docs/design.md) § Coverage & CI.

The infrastructure for this (Vitest, React Testing Library, MSW, the coverage CI job) is set up by PR-H2 in the frontend compliance plan. That PR is sequenced ahead of the runtime-behavior PRs (B2, C1, C2, E1, F1) so each of them lands with tests.

## Code review

PR reviews of frontend code consult `docs/coding-standards/typescript.md`, `react.md`, and `tailwind.md` as the rule set. A reviewer (human or AI) reading only one of these can do a focused review of a diff that touches only that area — the split is deliberate. The same applies to the `code-review` skill: when reviewing a frontend PR, it should pull in the relevant standards file(s) by topic.

## Document history

- 2026-05-08 — Initial creation as part of PR 6 / Issue [#79](https://github.com/brendanbyrne/beerio-kart/issues/79). UI reference device sentence sourced from root `.claude/CLAUDE.md`; Firefox-first note sourced from `docs/design.md` § Technical Constraints. Style and stack notes synthesized from existing project state. Filed as a stub per Issue #38 AC ("frontend stub may stay light").
- 2026-05-16 — Promoted from stub. Replaced the inline `## Style` and `## Stub note` sections with pointers to the new `docs/coding-standards/typescript.md`, `react.md`, and `tailwind.md`. Added a `## Code review` section explaining how the coding standards plug into PR review. Added a pointer to the frontend compliance plan. Stack section updated with explicit React 19, TS 5.9, react-router 7 versions and a declarative-mode note. Companion to the standards creation same day; see those files' history and `docs/coding-standards/README.md` for the full rollup.
- 2026-05-18 — Added `## Testing` section mirroring `backend/CLAUDE.md` § Testing. Policy: tests are a deliverable, not optional; every requirement should be unit- or integration-testable within reason. Points at `docs/coding-standards/typescript.md` § 12 for the umbrella policy and Vitest patterns, and `docs/coding-standards/react.md` § 13 for the React Testing Library + MSW patterns. Surfaced during compliance-Issue filing — the audit had flagged "no tests" as an out-of-scope gap; this update makes it in-scope and the compliance plan's PR-H2 (Vitest scaffolding) is correspondingly promoted from optional to required.
