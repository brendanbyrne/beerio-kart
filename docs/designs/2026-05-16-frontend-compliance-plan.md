# Frontend Compliance Plan

> **Purpose.** A sequenced list of PRs that brings the existing `beerio-kart` frontend into compliance with the coding standards in [`../coding-standards/typescript.md`](../coding-standards/typescript.md), [`react.md`](../coding-standards/react.md), and [`tailwind.md`](../coding-standards/tailwind.md). Each PR has a scope, a list of standards rules it satisfies, an effort estimate, dependencies, a risk note, and a sign-off checkbox.
> **Status.** Initial draft. Driven by [`./2026-05-16-frontend-audit.md`](./2026-05-16-frontend-audit.md) — see that file for per-file findings, line-numbered citations, and migration-cost ratings.
> **Sign-off.** Brendan signs off each PR once the change lands and is verified. Unfinished items roll into the next session.

## How this doc is used

A reviewer or assistant picks the next un-signed-off PR (respecting dependencies), opens it, and works through the scope. When merged, Brendan checks the box. The plan is living — re-order or split PRs as new findings emerge.

PRs are grouped into streams by theme. Streams are loosely ordered by what unlocks what: tooling and tsconfig first, then types and runtime validation (foundation for the rest), then data-layer migration, then mechanical cleanups, then UX polish. Within a stream, smaller PRs precede larger ones where possible.

**Effort scale:** S = up to a few hours; M = a half-day to a day; L = multi-day.

## Audit summary

Headline gaps surfaced by [`./2026-05-16-frontend-audit.md`](./2026-05-16-frontend-audit.md). See the audit for per-file findings, line numbers, and migration-cost ratings.

- **Default exports:** 12 sites (11 declarations + `main.tsx`'s import).
- **`interface` over `type`:** ~25 declarations across 6 files.
- **No branded IDs:** All API DTOs cross every boundary as raw `string` / `number`.
- **`useEffect` for data fetching/derivation:** 7 hooks/components.
- **`any`-typed `await res.json()`:** 9 files.
- **Non-null `!` assertions:** 3 files (5 sites in `Session.tsx` alone — `useParams<{id: string}>()` actually returns `Partial`).
- **`as Foo` casts and annotation-only validation:** 2 files.
- **Controlled form inputs (no `useActionState`):** 5 files.
- **Template-literal class concat:** 8 files.
- **Touch targets under 44 px:** 4 files (cancel/confirm buttons, step pills, password change/cancel).
- **`tsconfig.app.json` missing:** `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, explicit `isolatedModules`.
- **`eslint.config.js` missing:** `eslint-plugin-jsx-a11y`, `import/no-default-export`, `consistent-type-definitions`, `no-non-null-assertion`, `no-explicit-any` enforcement, `ban-ts-comment`, `eslint-config-prettier`.
- **No `@theme` design tokens** in `src/index.css` — file is `@import 'tailwindcss';` plus one custom rule.
- **No `react-error-boundary`**, no app-level or route-level boundaries.
- **`react-router-dom` 7 used in legacy `BrowserRouter` mode**, not `createBrowserRouter`.
- **No `AbortController` plumbing**, no TanStack Query, no `clsx`/`cva`, no `zod`.
- **No tests** (`vitest` not installed, no `*.test.ts(x)` files). Not in this plan's scope but flagged in the audit.

The PRs below address every line item.

---

## Stream A — Tooling and tsconfig foundations

### PR-A1: ESLint plugins, Prettier integration, package additions

- **Scope:**
  - Add `eslint-plugin-jsx-a11y`, `eslint-plugin-import`, `eslint-config-prettier` to devDependencies.
  - Add `@tanstack/react-query`, `@tanstack/react-query-devtools`, `clsx`, `react-error-boundary`, `zod` to dependencies.
  - Extend `eslint.config.js` with:
    - `tseslint.configs.strictTypeChecked` + `stylisticTypeChecked` (replaces bare `recommended`).
    - `jsx-a11y` flat-recommended.
    - `import` flat-recommended.
    - `eslint-config-prettier` last.
    - Rules: `consistent-type-definitions: ["error", "type"]`, `consistent-type-imports: "error"`, `import/no-default-export: "error"`, `no-restricted-syntax` ban on `TSEnumDeclaration`, `ban-ts-comment` requiring `@ts-expect-error`. Start `no-explicit-any` and `no-non-null-assertion` at **warn** so existing files don't block CI; flip to error in PR-F1.
  - Bump `ecmaVersion` to match the `target` in `tsconfig.app.json` (ES2023+).
  - Add `parserOptions.projectService: true`.
  - Add a `.prettierrc` if not present: `{ "singleQuote": true, "semi": true, "trailingComma": "all" }`.
- **Standards refs:** `typescript.md` § 9, `react.md` § 10, `tailwind.md` § 4.
- **Effort:** M.
- **Dependencies:** None.
- **Risk:** Low. The warn-level start keeps CI green; existing violations are documented, not enforced yet.
- **Verification:** `bun run lint` reports warnings (not errors) for the known violations from the audit; `bun run typecheck` passes; pre-commit hook fires both.
- **Sign-off:** [ ]

### PR-A2: Strict tsconfig flags

- **Scope:**
  - In `frontend/tsconfig.app.json` and `frontend/tsconfig.node.json`, add: `noUncheckedIndexedAccess: true`, `exactOptionalPropertyTypes: true`, `isolatedModules: true` (explicit).
  - Fix the type errors that surface. Expected hot spots: `RaceSetupPicker.tsx` (`STEPS[i+1]` and `STEPS[i-1]` access become `Step | undefined`), `RunEntrySheet.tsx` (`e.touches[0]` becomes `Touch | undefined`), a handful of optional-prop spreads that violate `exactOptionalPropertyTypes`.
  - Where the fix is a real bug (the array-access cases above), fix it properly with a narrowing check. Where the fix is just type-noise (e.g., spreads of well-typed objects), use a small helper.
- **Standards refs:** `typescript.md` § 1.
- **Effort:** M (1–2 days of cleanup expected).
- **Dependencies:** PR-A1 (so lints can guide).
- **Risk:** Medium. Surfaces latent bugs by design. Each fix should be small and reviewable, but the PR overall touches several files.
- **Verification:** `bun run typecheck` and `bun run lint` both pass. Manual smoke test of `RaceSetupPicker` step navigation and `RunEntrySheet` time-input touch handling.
- **Sign-off:** [ ]

---

## Stream B — Type system and runtime validation

### PR-B1: Branded ID types + `type` over `interface` in `api/types.ts`

- **Scope:**
  - Create `src/api/brand.ts` with the `Brand<T, B>` helper and exported brand types: `UserId`, `SessionId`, `RunId`, `RaceId`, `DrinkTypeId` (string brands); `CharacterId`, `BodyId`, `WheelId`, `GliderId`, `TrackId`, `CupId` (number brands).
  - Convert every `interface` in `api/types.ts` to `type`.
  - Replace raw `string` / `number` ID fields with the branded types. ~25 field changes.
  - Convert `SessionDetail.status: string` to `'open' | 'in_progress' | 'closed'` (or whatever the backend's actual values are — verify against `api-contract.md`). Same for `SessionSummary.ruleset`.
  - Update `api/sessions.ts` and `api/runs.ts` to accept and pass branded IDs.
  - Mint brands at the API-helper boundary (cast or trivial constructor — until PR-B2 replaces this with zod parsing). The mint sites are explicit and centralized.
- **Standards refs:** `typescript.md` § 2, § 3, § 4.
- **Effort:** L. Branded IDs ripple into every consumer; the API helpers and `useSession` / `useSessions` / `useUserProfile` / `useGameData` all need touch-ups.
- **Dependencies:** PR-A2.
- **Risk:** Medium-high. Mechanical but wide. Recommend doing in one PR rather than splitting — half-branded IDs are worse than fully-branded or fully-raw.
- **Verification:** `bun run typecheck` passes. Manual test of one happy-path flow (login → create session → submit run) confirms no regressions.
- **Sign-off:** [ ]

### PR-B2: Runtime-validated API responses (Zod)

- **Scope:**
  - Add Zod schemas for every API response shape in `src/api/types.ts`. Infer the TS types from the schemas (delete the hand-written types in favor of `z.infer<typeof ...>`).
  - Brand-mint inside the schema via `.transform((s) => s as UserId)` etc., replacing the at-the-call-site brand casts from PR-B1.
  - Replace `await res.json() as Foo` with `Schema.parse(await res.json())` in `api/sessions.ts`, `api/runs.ts`, `api/client.ts`, and the direct `apiFetch` calls in `useAuth.tsx`, `useGameData.ts`, `useUserProfile.ts`, `DrinkTypeSelector.tsx`, `Onboarding.tsx`, `Profile.tsx`.
  - Introduce a `Result<T, ApiError>` type in `src/api/result.ts` (per `typescript.md` § 6).
  - Define `ApiError` as a discriminated union on `code` values from `api-contract.md` § 8.
  - Update `getRunDefaults` to return `Result` instead of a silent-fallback hardcoded value.
  - Add `AbortSignal` parameter to every API helper and thread it into `fetch`.
- **Standards refs:** `typescript.md` § 6, § 7, § 8.
- **Effort:** L.
- **Dependencies:** PR-B1 (brand types in place).
- **Risk:** Medium. Parse failures are now loud — first run after merging may reveal previously-tolerated contract drift. That's the point. Fix it forward.
- **Verification:** `bun run typecheck` passes. Every happy-path flow (login, session list, session detail, run submit, onboarding, profile edit) works. Forcing a backend response shape mismatch (e.g., temporarily rename a field) produces a clear Zod error, not a silent bug.
- **Sign-off:** [ ]

---

## Stream C — Data-fetching migration

### PR-C1: TanStack Query setup + migration of static-data hooks

- **Scope:**
  - Wrap `App.tsx` in `QueryClientProvider`. Use the recommended default config (`refetchOnWindowFocus: true`, `retry: 1`, `staleTime: 30_000` for most queries).
  - Add React Query Devtools in dev only.
  - Migrate `useGameData.ts` (the five `useSimpleList` hooks + `useDrinkTypes`) to `useQuery`. Static data: long `staleTime`, no polling. Drink-types' `refresh()` becomes `queryClient.invalidateQueries({ queryKey: ['drink-types'] })`.
  - Migrate `useUserProfile.ts` to `useQuery`. `refresh()` becomes invalidation.
  - Keep the legacy hook names exported as thin wrappers if it shrinks the diff for call sites.
- **Standards refs:** `react.md` § 4, § 6.
- **Effort:** M.
- **Dependencies:** PR-B2 (typed responses).
- **Risk:** Low. Drop-in for hooks whose external contract stays the same.
- **Verification:** Every place that calls `useDrinkTypes`/`useUserProfile`/etc. still works. Devtools panel shows the queries.
- **Sign-off:** [ ]

### PR-C2: TanStack Query migration of polling hooks

- **Scope:**
  - Migrate `useSession.ts`: replace the polling/visibility-API/`endedRef` logic with `useQuery({ refetchInterval: (q) => (q.state.data?.ended_at ? false : 2500), refetchIntervalInBackground: false })`.
  - Migrate `useSessions.ts`: same pattern with a 5000 ms interval.
  - Replace `useAuth.tsx`'s `login`/`register`/`logout`/`changePassword` direct `fetch` calls with `useMutation` (or keep as plain async functions called from the auth context if mutation state isn't needed).
  - Replace `BottomNav.tsx`'s `useEffect`-on-pathname for `getMySession()` with `useQuery({ queryKey: ['my-session'] })` invalidated by the relevant session mutations.
  - Replace `RunEntrySheet.tsx`'s defaults-loading `useEffect` with `useQuery({ queryKey: ['run-defaults'] })`.
- **Standards refs:** `react.md` § 4, § 6.
- **Effort:** M-L.
- **Dependencies:** PR-C1.
- **Risk:** Medium. Polling behavior change should be transparent if `refetchInterval` is tuned correctly; verify against a long-running session that you don't pile up requests in background tabs.
- **Verification:** Open a session, switch tabs for 30 seconds, return — polling resumes without backfill spike. Submit a run on one device, confirm it appears on another within ~5 seconds. No `setState on unmounted component` warnings.
- **Sign-off:** [ ]

---

## Stream D — Mechanical refactors

### PR-D1: Named exports everywhere

- **Scope:**
  - Convert all 12 default exports to named exports. Files: `main.tsx` (import), `App.tsx`, `BottomNav.tsx`, `DrinkTypeSelector.tsx`, `RaceSetupPicker.tsx`, `RunEntrySheet.tsx`, `Login.tsx`, `Register.tsx`, `Onboarding.tsx`, `Profile.tsx`, `Home.tsx`, `Session.tsx`.
  - Update every importer. Mechanical sweep.
  - Flip `import/no-default-export` from warn to error.
- **Standards refs:** `typescript.md` § 5.
- **Effort:** S–M (mostly mechanical; the editor's "rename symbol" is your friend).
- **Dependencies:** None hard, but ordering after PR-B1/B2 means the type churn is already done.
- **Risk:** Low.
- **Verification:** `bun run lint` and `bun run typecheck` pass with the rule at error level. App boots.
- **Sign-off:** [ ]

### PR-D2: Remove non-null `!` assertions

- **Scope:**
  - `main.tsx:6` — replace `document.getElementById('root')!` with `if (!rootEl) throw new Error('Root element missing')`.
  - `Session.tsx:13, 29, 40, 51, 63` — the `useParams<{id: string}>()` calls. Add a single param-check at the top of the component: `if (!id) return <Navigate to="/" replace />` (or a 404 page). Downstream uses become unconditional `id: SessionId`.
  - `RaceSetupPicker.tsx:173-176` — replace `characterId!`, `bodyId!`, `wheelId!`, `gliderId!` with a narrowing function `function fullSetup(s: RaceSetupState): FullRaceSetup | null` that returns either all four IDs or `null`. The "Continue" button is disabled when `null`.
- **Standards refs:** `typescript.md` § 2 (assertions).
- **Effort:** S.
- **Dependencies:** None (could land alongside D1).
- **Risk:** Low.
- **Verification:** ESLint `no-non-null-assertion` (still at warn level until PR-F1 flips it) reports zero new warnings; no `!` assertions remain in `git grep`. The three flows (root mount, session deep-link without ID, RaceSetupPicker completion) still work.
- **Sign-off:** [ ]

### PR-D3: Remove `as Foo` casts and unsafe response annotations

- **Scope:**
  - `RunEntrySheet.tsx:176` — replace `(e.target as HTMLImageElement)` with `e.currentTarget`. Safe here specifically because the handler is attached directly to the `<img>` and `Event.target` / `currentTarget` are the same element in a non-bubbling `onError`. Do not blindly apply this substitution in delegated handlers where `target` and `currentTarget` differ.
  - `DrinkTypeSelector.tsx:43` — the `const created: DrinkType = await res.json()` line is a type-annotation lie (not an `as` cast, but the same anti-pattern: the runtime value isn't validated). Wrap the response in the Zod parse for `DrinkType` introduced in PR-B2.
  - Audit for any other `as` casts or "type-annotation-as-validation" patterns introduced after PR-B1, including `e.target as ...` in event handlers more generally.
- **Standards refs:** `typescript.md` § 2, § 8.
- **Effort:** S.
- **Dependencies:** PR-B2 (Zod schemas in place for the DrinkTypeSelector fix).
- **Risk:** Low.
- **Sign-off:** [ ]

---

## Stream E — React 19 forms and primitives

### PR-E1: Form migration to `useActionState` + `useFormStatus`

- **Scope:**
  - Convert `Login.tsx` from controlled inputs + manual `submitting` flag to uncontrolled inputs + `useActionState`. Add a shared `SubmitButton` component using `useFormStatus`.
  - Convert `Register.tsx` (twin of Login).
  - Convert `Profile.tsx` password change form.
  - Convert `DrinkTypeSelector.tsx` add-drink-type form.
  - Convert `Onboarding.tsx` two-phase form.
  - Native validation (`required`, `minLength`, `maxLength`, `pattern`) on every input; Zod schema at submit time for each form.
- **Standards refs:** `react.md` § 8.
- **Effort:** M-L.
- **Dependencies:** PR-B2 (Zod schemas).
- **Risk:** Medium. Form behavior change is user-visible. Test each form's happy path and error path explicitly.
- **Sign-off:** [ ]

### PR-E2: Ref-as-prop, Document Metadata, React Compiler

- **Scope:**
  - Audit any `forwardRef` usage (none expected in current code; the audit didn't surface any) and convert if found.
  - Add `<title>` / `<meta>` via React 19's Document Metadata where appropriate (each `pages/` component sets a per-page title).
  - Install `babel-plugin-react-compiler` in `vite.config.ts`. Enable compiler in dev and prod.
  - Verify `eslint-plugin-react-hooks` recommended preset includes the Compiler-specific rules.
  - Remove now-redundant `useMemo`/`useCallback` calls where the Compiler can handle them. Keep them only at sites with a measured reason (carve-outs in `react.md` § 7).
- **Standards refs:** `react.md` § 2, § 5, § 7.
- **Effort:** M.
- **Dependencies:** PR-A1 (lints), PR-C2 (data layer settled — Compiler interacts with re-render shape).
- **Risk:** Low-medium. Compiler is v1.0 stable; treated by the React team as production-ready. Monitor first few PRs after merge for any unexpected re-render behavior.
- **Verification:** Build succeeds; dev mode works; no Compiler errors in console. Profile a representative page (Session.tsx with active polling) before and after — re-render count should be equal or lower.
- **Sign-off:** [ ]

---

## Stream F — Routing, error boundaries, lazy loading

### PR-F1: Router upgrade + error boundaries + lazy routes

- **Scope:**
  - Migrate `App.tsx` from `BrowserRouter` + `<Routes>` to `createBrowserRouter` + `RouterProvider`.
  - Add an app-level `<ErrorBoundary>` from `react-error-boundary` with a generic fallback ("Something went wrong. Reload?").
  - Add per-route `errorElement` for each top-level route.
  - `lazy()`-load the six page imports; wrap each in `<Suspense fallback={<PageSkeleton />}>`.
  - Replace `BottomNav.tsx`'s `<button onClick={navigate}>` with `<NavLink>`. Set `aria-current="page"` via NavLink's className function.
  - Flip `no-explicit-any` and `no-non-null-assertion` from warn to error.
- **Standards refs:** `react.md` § 9, § 11.
- **Effort:** M.
- **Dependencies:** PR-D1 (named exports).
- **Risk:** Medium. `createBrowserRouter` is structurally different from the old `<Routes>` form. The error-boundary placement requires thought (per-route vs route-element-with-its-own-boundary).
- **Verification:** Every existing route navigates correctly. Triggering an error in one route (temporarily `throw new Error('test')` in a page) shows the route-scoped fallback, not the global one. Bundle analyzer shows separate chunks per route.
- **Sign-off:** [ ]

---

## Stream G — Styling, design tokens, accessibility

### PR-G1: Tailwind `@theme` tokens + `clsx` adoption

- **Scope:**
  - Add a `@theme` block to `frontend/src/index.css` defining brand colors (currently scattered as `blue-600` etc.), `--spacing-touch: 2.75rem`, semantic colors (`--color-success`, `--color-danger`).
  - Introduce `clsx` (already added in PR-A1). Refactor template-literal class concat in `BottomNav.tsx`, `DrinkTypeSelector.tsx`, `RaceSetupPicker.tsx`, `RunEntrySheet.tsx`, `Profile.tsx`, `Home.tsx`, `Session.tsx`, `App.tsx`.
  - Replace `style={{ maxHeight: '92%' }}` (`RunEntrySheet.tsx:161`) with `max-h-[92%]`. Leave the genuinely-dynamic styles (slide-to-confirm position) as inline styles.
- **Standards refs:** `tailwind.md` § 3, § 4, § 8.
- **Effort:** M.
- **Dependencies:** None hard.
- **Risk:** Low. Pure styling.
- **Verification:** Visual diff vs main looks identical. Brand color change (try changing `--color-brand-primary`) propagates everywhere.
- **Sign-off:** [ ]

### PR-G2: Accessibility sweep — touch targets, focus, modals

- **Scope:**
  - Bump every <44 px button to `min-h-touch` (or `min-h-[44px]` for one-offs). Hit list from audit: `DrinkTypeSelector.tsx` skip/cancel/add, `RaceSetupPicker.tsx` step pills, `Profile.tsx` password cancel/save, `Home.tsx` modal cancel, `Login.tsx` and `Register.tsx` submit buttons (borderline at `py-2.5`).
  - Remove `autoFocus` from `DrinkTypeSelector.tsx:100`. Use a `useEffect`-driven `inputRef.current?.focus()` if focus-on-mount is genuinely needed, or — better — accept the user's natural focus order.
  - Add `role="dialog"`, `aria-modal="true"`, focus trap, and Escape handling to:
    - `Home.tsx` create-session modal
    - `RunEntrySheet.tsx` bottom sheet
  - Consider native `<dialog>` for these — gets focus trap and Escape for free.
  - Verify `eslint-plugin-jsx-a11y` recommended produces zero warnings.
  - Add `@axe-core/react` in dev to catch run-time issues.
- **Standards refs:** `react.md` § 10, `tailwind.md` § 5.
- **Effort:** M.
- **Dependencies:** PR-G1 (the `--spacing-touch` token).
- **Risk:** Low-medium. Focus trap is the trickiest part; native `<dialog>` is the recommended escape hatch.
- **Verification:** Tab through every screen with the keyboard only. Open each modal and confirm Escape closes it, focus returns to the trigger. axe shows zero violations on the main flows.
- **Sign-off:** [ ]

---

## Stream H — Final consolidation

### PR-H1: Lint cleanup, discriminated-union audit, drift-check CI

- **Scope:**
  - Final pass converting any remaining `string` status fields to literal unions.
  - Confirm no TS `enum` usage (audit confirmed none today; the `no-restricted-syntax` rule keeps it that way).
  - Add a CI check (GitHub Actions) that fails the PR if `backend/src/dtos/` (or wherever DTOs live — verify path) changes without `frontend/src/api/types.ts` also changing. False positives are acceptable; one-line update.
  - Document the drift check in `typescript.md` § 11 and link from the type-sync research doc.
- **Standards refs:** `typescript.md` § 4, § 11.
- **Effort:** S.
- **Dependencies:** Everything else.
- **Sign-off:** [ ]

### PR-H2: Vitest scaffolding (optional, out of standards scope but flagged)

- **Scope:**
  - Add `vitest`, `@testing-library/react`, `@testing-library/jest-dom`, `jsdom` to devDependencies.
  - Add `vitest.config.ts`.
  - Add one example test per kind: a unit test for `utils/time.ts`, a hook test for one of the post-migration TanStack Query hooks, a component test for `Login.tsx`.
  - Add a CI job to run tests.
- **Standards refs:** Not currently covered by the standards; should be addressed in a follow-up update to `react.md` or a new `testing.md`.
- **Effort:** M.
- **Dependencies:** PRs C1, C2, E1 (so the example tests cover the post-migration shape).
- **Risk:** Low. Out-of-scope for compliance per se; included here because the audit flagged it as a notable gap.
- **Sign-off:** [ ]

---

## Sign-off summary

| PR | Title | Stream | Dep | Status |
|---|---|---|---|---|
| A1 | Lints + Prettier + packages | A | — | [ ] |
| A2 | Strict tsconfig flags | A | A1 | [ ] |
| B1 | Branded IDs + type-over-interface | B | A2 | [ ] |
| B2 | Zod runtime validation + Result | B | B1 | [ ] |
| C1 | TanStack Query setup + static hooks | C | B2 | [ ] |
| C2 | TanStack Query polling hooks | C | C1 | [ ] |
| D1 | Named exports everywhere | D | (B1) | [ ] |
| D2 | Remove `!` assertions | D | — | [ ] |
| D3 | Remove `as` casts | D | B2 | [ ] |
| E1 | Form migration to useActionState | E | B2 | [ ] |
| E2 | Ref-as-prop, Doc Metadata, Compiler | E | A1, C2 | [ ] |
| F1 | Router upgrade + boundaries + lazy | F | D1 | [ ] |
| G1 | Tailwind `@theme` + `clsx` | G | — | [ ] |
| G2 | Accessibility sweep | G | G1 | [ ] |
| H1 | Lint cleanup + drift-check CI | H | All | [ ] |
| H2 | Vitest scaffolding (optional) | H | C1, C2, E1 | [ ] |

ADRs produced: TBD (none anticipated unless a decision lands during the rollout that warrants one — most rules trace to the standards docs, which are the authority).

## Document history

- 2026-05-16 — Initial creation. Driven by the audit conducted same day against the new coding standards (`typescript.md`, `react.md`, `tailwind.md`, all created 2026-05-16). 16 PRs across 8 streams. No work has shipped against this plan yet; all checkboxes are open. Companion to the standards rollup in `../coding-standards/README.md` § History (2026-05-16 entry).
- 2026-05-16 — Pointed at the now-separate audit doc (`./2026-05-16-frontend-audit.md`) instead of carrying the audit findings inline. The audit was extracted into its own design record matching the backend pattern (`archive/2026-04-15-rust-audit.md` + `archive/compliance-plan.md`) so per-file context is durable when PRs are picked up individually.
