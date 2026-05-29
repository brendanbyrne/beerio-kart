# Frontend Compliance Plan

> **Purpose.** A sequenced list of PRs that brings the existing `beerio-kart` frontend into compliance with the coding standards in [`../coding-standards/typescript.md`](../coding-standards/typescript.md), [`react.md`](../coding-standards/react.md), and [`tailwind.md`](../coding-standards/tailwind.md). Each PR has a scope, a list of standards rules it satisfies, an effort estimate, dependencies, a risk note, and a sign-off checkbox.
> **Status.** All 16 PRs filed as Issues under [Milestone 9 — `Hardening: Frontend standards compliance`](https://github.com/brendanbyrne/beerio-kart/milestone/9). All Ready / Medium priority. Driven by [`./2026-05-16-frontend-audit.md`](./2026-05-16-frontend-audit.md) — see that file for per-file findings, line-numbered citations, and migration-cost ratings.
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

**Issue:** [#187](https://github.com/brendanbyrne/beerio-kart/issues/187) · **Merged PR:** [#194](https://github.com/brendanbyrne/beerio-kart/pull/194)

- **Scope:**
  - Add `eslint-plugin-jsx-a11y`, `eslint-plugin-import`, `eslint-config-prettier` to devDependencies.
  - Add `@tanstack/react-query`, `@tanstack/react-query-devtools`, `clsx`, `react-error-boundary`, `zod` to dependencies.
  - Add a `"format": "prettier --write ."` script to `frontend/package.json`. Verify lefthook's pre-commit hook already runs Prettier (per `frontend/CLAUDE.md` it should); add it if not.
  - Extend `eslint.config.js` with:
    - `tseslint.configs.strictTypeChecked` + `stylisticTypeChecked` (replaces bare `recommended`).
    - `jsx-a11y` flat-recommended.
    - `import` flat-recommended.
    - `eslint-config-prettier` last.
    - Rules at `error` (no existing violations): `consistent-type-imports`, `no-restricted-syntax` ban on `TSEnumDeclaration`, `ban-ts-comment` requiring `@ts-expect-error`.
    - Rules started at **warn** because they fire on existing code — the warn-down principle below applies to these named rules too: `consistent-type-definitions: ["warn", "type"]` (~25 `interface` declarations), `import/no-default-export` (flipped to error in PR-D1), `no-explicit-any` / `no-non-null-assertion` (flipped in PR-F1). Also enabled `import/consistent-type-specifier-style` (`prefer-top-level`, `warn`) — `typescript.md` § 9 names it; added during PR-A1 review since the original AC omitted it.
    - **Warn-down (don't fix) any other rule from `strictTypeChecked` that fires on existing code.** Expected culprits: the `@typescript-eslint/no-unsafe-*` family (every `await res.json()` until PR-B2 lands Zod parses — 9 files), `no-floating-promises` / `no-misused-promises` (fire-and-forget fetches), `no-unnecessary-condition` (over-defensive null checks). The principle: PR-A1 is purely additive infrastructure — it must produce zero new errors in CI or the pre-commit hook. Later PRs (B2 especially) fix the underlying code and flip the rules back to error.
  - Bump `ecmaVersion` to match the `target` in `tsconfig.app.json` (ES2023+).
  - Add `parserOptions.projectService: true`.
  - Add a `.prettierrc` if not present: `{ "singleQuote": true, "semi": true, "trailingComma": "all" }`.
- **Standards refs:** `typescript.md` § 9, `react.md` § 10, `tailwind.md` § 4.
- **Effort:** M.
- **Dependencies:** None.
- **Risk:** Low. The warn-level start keeps CI green; existing violations are documented, not enforced yet.
- **Verification:** `bun run lint` reports warnings (not errors) for the known violations from the audit; `bun run typecheck` passes; pre-commit hook succeeds on a no-op commit.
- **Sign-off:** [x]

### PR-A2: Strict tsconfig flags

**Issue:** [#173](https://github.com/brendanbyrne/beerio-kart/issues/173) · **Merged PR:** [#196](https://github.com/brendanbyrne/beerio-kart/pull/196)

- **Scope:**
  - In `frontend/tsconfig.app.json` and `frontend/tsconfig.node.json`, add: `noUncheckedIndexedAccess: true`, `exactOptionalPropertyTypes: true`, `isolatedModules: true`, and `noImplicitOverride: true` — the fourth still-missing `typescript.md` § 1 flag (zero errors today; added so tsconfig is fully § 1-compliant, since PR-A2 is the only PR touching tsconfig).
  - Fix the `typecheck` script: `tsc --noEmit` runs against the solution-style root `tsconfig.json` (no `files`/`include`) and checks nothing — change it to `tsc -b` so it actually typechecks the referenced projects.
  - Fix the type errors that surface. Expected hot spots: `RaceSetupPicker.tsx` (`STEPS[i+1]` and `STEPS[i-1]` access become `Step | undefined`), `RunEntrySheet.tsx` (`e.touches[0]` becomes `Touch | undefined`), a handful of optional-prop spreads that violate `exactOptionalPropertyTypes`.
  - Where the fix is a real bug (the array-access cases above), fix it properly with a narrowing check. Where the fix is just type-noise (e.g., spreads of well-typed objects), use a small helper.
- **Standards refs:** `typescript.md` § 1.
- **Effort:** M (1–2 days of cleanup expected).
- **Dependencies:** PR-A1 (so lints can guide).
- **Risk:** Medium. Surfaces latent bugs by design. Each fix should be small and reviewable, but the PR overall touches several files.
- **Verification:** `bun run typecheck` and `bun run lint` both pass. Manual smoke test of `RaceSetupPicker` step navigation and `RunEntrySheet` time-input touch handling.
- **Sign-off:** [x]

---

## Stream B — Type system and runtime validation

### PR-B1: Branded ID types + `type` over `interface` in `api/types.ts`

**Issue:** [#171](https://github.com/brendanbyrne/beerio-kart/issues/171) · **Merged PR:** [#198](https://github.com/brendanbyrne/beerio-kart/pull/198)

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
- **Sign-off:** [x]

### PR-B2: Runtime-validated API responses (Zod)

**Issue:** [#191](https://github.com/brendanbyrne/beerio-kart/issues/191) · **Merged PR:** [#199](https://github.com/brendanbyrne/beerio-kart/pull/199)

- **Scope:**
  - Add Zod schemas for every API response shape in `src/api/types.ts`. Infer the TS types from the schemas (delete the hand-written types in favor of `z.infer<typeof ...>`).
  - Brand-mint inside the schema via `.transform((s) => s as UserId)` etc., replacing the at-the-call-site brand casts from PR-B1.
  - Replace `await res.json() as Foo` with `Schema.parse(await res.json())` in `api/sessions.ts`, `api/runs.ts`, `api/client.ts`, and the direct `apiFetch` calls in `useAuth.tsx`, `useGameData.ts`, `useUserProfile.ts`, `DrinkTypeSelector.tsx`, `Onboarding.tsx`, `Profile.tsx`.
  - Introduce a `Result<T, ApiError>` type in `src/api/result.ts` (per `typescript.md` § 6).
  - Define `ApiError` as a discriminated union on `code` values from `api-contract.md` § 7.
  - Update `getRunDefaults` to return `Result` instead of a silent-fallback hardcoded value.
  - Add `AbortSignal` parameter to every API helper and thread it into `fetch`.
- **Standards refs:** `typescript.md` § 6, § 7, § 8.
- **Effort:** L.
- **Dependencies:** PR-B1 (brand types in place).
- **Risk:** Medium. Parse failures are now loud — first run after merging may reveal previously-tolerated contract drift. That's the point. Fix it forward.
- **Verification:** `bun run typecheck` passes. Every happy-path flow (login, session list, session detail, run submit, onboarding, profile edit) works. Forcing a backend response shape mismatch (e.g., temporarily rename a field) produces a clear Zod error, not a silent bug.
- **Sign-off:** [x]

---

## Stream C — Data-fetching migration

### PR-C1: TanStack Query setup + migration of static-data hooks

**Issue:** [#176](https://github.com/brendanbyrne/beerio-kart/issues/176) · **Merged PR:** [#203](https://github.com/brendanbyrne/beerio-kart/pull/203)

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
- **Sign-off:** [x]

### PR-C2: TanStack Query migration of polling hooks

**Issue:** [#186](https://github.com/brendanbyrne/beerio-kart/issues/186) · **Merged PR:** [#204](https://github.com/brendanbyrne/beerio-kart/pull/204)

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
- **Sign-off:** [x]

---

## Stream D — Mechanical refactors

### PR-D1: Named exports everywhere

**Issue:** [#175](https://github.com/brendanbyrne/beerio-kart/issues/175) · **Merged PR:** [#205](https://github.com/brendanbyrne/beerio-kart/pull/205)

- **Scope:**
  - Convert all 12 default exports to named exports. Files: `main.tsx` (import), `App.tsx`, `BottomNav.tsx`, `DrinkTypeSelector.tsx`, `RaceSetupPicker.tsx`, `RunEntrySheet.tsx`, `Login.tsx`, `Register.tsx`, `Onboarding.tsx`, `Profile.tsx`, `Home.tsx`, `Session.tsx`.
  - Update every importer. Mechanical sweep.
  - Flip `import/no-default-export` from warn to error.
- **Standards refs:** `typescript.md` § 5.
- **Effort:** S–M (mostly mechanical; the editor's "rename symbol" is your friend).
- **Dependencies:** None hard, but ordering after PR-B1/B2 means the type churn is already done.
- **Risk:** Low.
- **Verification:** `bun run lint` and `bun run typecheck` pass with the rule at error level. App boots.
- **Sign-off:** [x]

### PR-D2: Remove non-null `!` assertions

**Issue:** [#192](https://github.com/brendanbyrne/beerio-kart/issues/192) · **Merged PR:** [#208](https://github.com/brendanbyrne/beerio-kart/pull/208)

- **Scope:**
  - `main.tsx:6` — replace `document.getElementById('root')!` with `if (!rootEl) throw new Error('Root element missing')`.
  - `Session.tsx:13, 29, 40, 51, 63` — the `useParams<{id: string}>()` calls. Add a single param-check at the top of the component: `if (!id) return <Navigate to="/" replace />` (or a 404 page). Downstream uses become unconditional `id: SessionId`.
  - `RaceSetupPicker.tsx:173-176` — replace `characterId!`, `bodyId!`, `wheelId!`, `gliderId!` with a narrowing function `function fullSetup(s: RaceSetupState): FullRaceSetup | null` that returns either all four IDs or `null`. The "Continue" button is disabled when `null`.
- **Standards refs:** `typescript.md` § 2 (assertions).
- **Effort:** S.
- **Dependencies:** None (could land alongside D1).
- **Risk:** Low.
- **Verification:** ESLint `no-non-null-assertion` (still at warn level until PR-F1 flips it) reports zero new warnings; no `!` assertions remain in `git grep`. The three flows (root mount, session deep-link without ID, RaceSetupPicker completion) still work.
- **Sign-off:** [x]

### PR-D3: Remove `as Foo` casts and unsafe response annotations

**Issue:** [#179](https://github.com/brendanbyrne/beerio-kart/issues/179) · **Merged PR:** [#210](https://github.com/brendanbyrne/beerio-kart/pull/210)

- **Scope:**
  - `RunEntrySheet.tsx:234` — replace `(e.target as HTMLImageElement)` with `e.currentTarget`. Safe here specifically because the handler is attached directly to the `<img>` and React types `currentTarget` to the element that owns the handler — no cast needed. Do not blindly apply this substitution in delegated handlers where `target` and `currentTarget` differ. (Line number drifted from `:176` since the plan was filed.)
  - ~~`DrinkTypeSelector.tsx:43` — the `const created: DrinkType = await res.json()` line is a type-annotation lie (not an `as` cast, but the same anti-pattern: the runtime value isn't validated). Wrap the response in the Zod parse for `DrinkType` introduced in PR-B2.~~ Already fixed in PR-B2 ([#199](https://github.com/brendanbyrne/beerio-kart/pull/199)): the call site now reads `const created = await parseBody(DrinkTypeSchema, res)`, which is the correct pattern. No D3 work needed at that site.
  - Audit for any other `as` casts or "type-annotation-as-validation" patterns introduced after PR-B1, including `e.target as ...` in event handlers more generally. Audit done 2026-05-26: only the RunEntrySheet cast above was an offender. The remaining `as` uses are all legitimate and stay: branded-type minting at the boundary in `api/brand.ts` (§ 2 carve-out), test-narrowing casts in `api/result.test.ts` (narrowing a known shape inside a test, not at a runtime boundary), and `api/result.ts:88`'s `(API_ERROR_CODES as readonly string[]).includes(code)` (widening a `readonly` literal tuple so `Array.includes` accepts an arbitrary `string` — § 2's "tell the compiler something it can't verify"; already documented inline at that call site).
- **Standards refs:** `typescript.md` § 2, § 8.
- **Effort:** S.
- **Dependencies:** PR-B2 (Zod schemas in place for the DrinkTypeSelector fix).
- **Risk:** Low.
- **Sign-off:** [x]

---

## Stream E — React 19 forms and primitives

### PR-E1: Form migration to `useActionState` + `useFormStatus`

**Issue:** [#182](https://github.com/brendanbyrne/beerio-kart/issues/182) · **Merged PR:** [#211](https://github.com/brendanbyrne/beerio-kart/pull/211)

- **Scope:**
  - Convert `Login.tsx` from controlled inputs + manual `submitting` flag to uncontrolled inputs + `useActionState`. Add a shared `SubmitButton` component using `useFormStatus`.
  - Convert `Register.tsx` (twin of Login).
  - Convert `Profile.tsx` password change form.
  - Convert `DrinkTypeSelector.tsx` add-drink-type form.
  - Convert `Onboarding.tsx` two-phase form. **Note:** `Onboarding` doesn't render its own `<form>` — both phases delegate to `RaceSetupPicker` / `DrinkTypeSelector`, which dispatch already-typed `Setup` / `DrinkType` payloads to `useActionState` via callback. There is no `FormData` parse step where a submit-time Zod schema would fit, so the Zod-at-submit rule below applies only to the four real forms (Login, Register, Profile password, DrinkTypeSelector add-form).
  - Native validation (`required`, `minLength`, `maxLength`, `pattern`) on every input; Zod schema at submit time for each form.
- **Standards refs:** `react.md` § 8.
- **Effort:** M-L.
- **Dependencies:** PR-B2 (Zod schemas).
- **Risk:** Medium. Form behavior change is user-visible. Test each form's happy path and error path explicitly.
- **Sign-off:** [x]

### PR-E2: Ref-as-prop, Document Metadata, React Compiler

**Issue:** [#180](https://github.com/brendanbyrne/beerio-kart/issues/180) · **Merged PR:** [#214](https://github.com/brendanbyrne/beerio-kart/pull/214)

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
- **Sign-off:** [x]

---

## Stream F — Routing, error boundaries, lazy loading

### PR-F1: Router upgrade + error boundaries + lazy routes

**Issue:** [#190](https://github.com/brendanbyrne/beerio-kart/issues/190)

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

**Issue:** [#183](https://github.com/brendanbyrne/beerio-kart/issues/183)

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

**Issue:** [#184](https://github.com/brendanbyrne/beerio-kart/issues/184)

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

### PR-H1: Lint cleanup, drift-check CI, test-coverage backfill

**Issue:** [#185](https://github.com/brendanbyrne/beerio-kart/issues/185)

- **Scope:**
  - Final pass converting any remaining `string` status fields to literal unions.
  - **Flip remaining lint rules from `warn` to `error`.** PR-A1 started several rules at `warn` so existing code wouldn't block CI; later PRs remove the offenders. By PR-H1, audit `eslint.config.js`'s warn-level blocks and flip every rule whose offenders are gone — `consistent-type-definitions`, `import/consistent-type-specifier-style`, and the `strictTypeChecked` family (`no-unsafe-*`, `no-floating-promises`, `no-misused-promises`, `no-unnecessary-condition`, `no-confusing-void-expression`, etc.). `import/no-default-export` and `no-explicit-any` / `no-non-null-assertion` are flipped earlier (PR-D1, PR-F1). Any rule that still can't be flipped means an earlier PR missed its scope.
  - Confirm no TS `enum` usage (audit confirmed none today; the `no-restricted-syntax` rule keeps it that way).
  - Add a CI check (GitHub Actions) that fails the PR if `backend/src/dtos/` (or wherever DTOs live — verify path) changes without `frontend/src/api/types.ts` also changing. False positives are acceptable; one-line update.
  - Document the drift check in `typescript.md` § 11 and link from the type-sync research doc.
  - **Test-coverage backfill.** By the time this PR runs, the earlier PRs in this milestone should have added tests for every file they touched. Verify with `bun run test:coverage` that no production code outside the standards' carve-outs (pure presentational components, app shell composition, generated types — see `typescript.md` § 12) is uncovered. Add tests for any straggler files. This should be a small final mop-up, not a major effort; if it turns into a multi-day task, the earlier PRs missed their AC.
  - **Flip the Codecov frontend project gate to blocking.** PR-H2 set `coverage.status.project.frontend.informational: true` in `codecov.yml` because the frontend was un-ignored before it had tests. Once the backfill above lands, change it to `informational: false` so frontend project coverage regressions block PRs the same way backend's does. (The frontend *patch* gate already blocks — PR-H2 set it.) Issue [#193](https://github.com/brendanbyrne/beerio-kart/issues/193)'s PR carries the matching note; Issue [#185](https://github.com/brendanbyrne/beerio-kart/issues/185) tracks this task.
- **Standards refs:** `typescript.md` § 4, § 11, § 12.
- **Effort:** S–M (depends on how well earlier PRs held to the standard).
- **Dependencies:** Everything else.
- **Verification:** All AC bullets pass. `bun test:coverage` shows ≥80% on production code (or a documented exemption per file).
- **Sign-off:** [ ]

### PR-H2: Vitest scaffolding (required — sequenced before B1)

> **Re-sequencing note (2026-05-18):** Originally optional and last. After `typescript.md` § 12 and `react.md` § 13 added a testing requirement to the standards, this PR is required infrastructure and moves to right after PR-A2 (before B1). Every subsequent PR in the plan now lands with tests for new/changed logic.

**Issue:** [#193](https://github.com/brendanbyrne/beerio-kart/issues/193) · **Merged PR:** [#197](https://github.com/brendanbyrne/beerio-kart/pull/197)

- **Scope:**
  - Add to devDependencies: `vitest`, `@vitest/coverage-v8`, `@testing-library/react`, `@testing-library/user-event`, `@testing-library/jest-dom`, `jsdom`, `msw`.
  - Add `frontend/vitest.config.ts` with `environment: 'jsdom'`, the `istanbul` coverage provider, and a single test-file glob `src/**/*.test.{ts,tsx}` (it already matches the integration tests in `src/__tests__/`; see the 2026-05-18 review-feedback history entry for why the AC's second `src/__tests__/**` glob was dropped). (The AC named the `v8` provider; it reports a flat 0% under Bun because Bun's test workers don't feed V8's coverage hooks, and the project has no Node toolchain. `istanbul` is the other first-class Vitest provider, instruments the source directly, and works under Bun — see the 2026-05-18 history entry.)
  - Add a `test` script (`"test": "vitest run"`) and a `test:watch` script (`"test:watch": "vitest"`) to `frontend/package.json`. Also add `"test:coverage": "vitest run --coverage"`.
  - Set up MSW: `frontend/src/mocks/handlers.ts` (initial empty handlers, populated as later PRs land), `frontend/src/mocks/server.ts` (Node setup), `frontend/src/setupTests.ts` (start/stop MSW + extend `expect` with `@testing-library/jest-dom`).
  - Wire `setupTests.ts` into `vitest.config.ts` via `setupFiles`.
  - Add a CI job to `.github/workflows/` that runs `bun test` and uploads `vitest run --coverage` output to Codecov, matching the backend's setup in `docs/design.md` § Coverage & CI.
  - Add lefthook integration so `bun test --changed` (or equivalent affected-file run) fires pre-push (not pre-commit — too slow).
  - Write one example test per kind to anchor the patterns:
    - **Unit:** `src/utils/time.test.ts` — formatter and parser.
    - **Schema:** placeholder for when PR-B2 lands the first Zod schema (leave a TODO comment; B2 fills in).
    - **Hook:** placeholder for when PR-C1 lands TanStack Query (leave a TODO; C1 fills in).
    - **Component:** `src/pages/Login.test.tsx` — RTL test against the current Login form (will be updated in PR-E1 when the form migrates to `useActionState`).
- **Standards refs:** `typescript.md` § 12 (umbrella policy + Vitest patterns), `react.md` § 13 (RTL + MSW + hook tests).
- **Effort:** M.
- **Dependencies:** PR-A1 (lint config in place), PR-A2 (strict tsconfig so test files inherit the rules).
- **Risk:** Low. New infrastructure only; no production code changes. The example tests are small enough to be self-documenting.
- **Verification:** `bun test` runs and the example tests pass. `bun test:coverage` produces an HTML coverage report. CI job runs and uploads to Codecov. Pre-push lefthook hook fires on `git push`.
- **Sign-off:** [x]

---

## Sign-off summary

All Issues live under [Milestone 9 — Hardening: Frontend standards compliance](https://github.com/brendanbyrne/beerio-kart/milestone/9). Each is on the project board at Ready / Medium priority. Rows are listed in **suggested pickup order** (not in PR-identifier alphabetical order — see the Re-sequencing note on PR-H2).

| Order | PR | Title | Dep | Issue | Status |
|---|---|---|---|---|---|
| 1 | A1 | Lints + Prettier + packages | — | [#187](https://github.com/brendanbyrne/beerio-kart/issues/187) | [x] |
| 2 | A2 | Strict tsconfig flags | A1 | [#173](https://github.com/brendanbyrne/beerio-kart/issues/173) | [x] |
| 3 | **H2** | **Vitest + RTL + MSW + coverage CI** (test infrastructure — re-sequenced 2026-05-18) | A1, A2 | [#193](https://github.com/brendanbyrne/beerio-kart/issues/193) | [x] |
| 4 | B1 | Branded IDs + type-over-interface | A2, H2 | [#171](https://github.com/brendanbyrne/beerio-kart/issues/171) | [x] |
| 5 | B2 | Zod runtime validation + Result | B1, H2 | [#191](https://github.com/brendanbyrne/beerio-kart/issues/191) | [x] |
| 6 | C1 | TanStack Query setup + static hooks | B2, H2 | [#176](https://github.com/brendanbyrne/beerio-kart/issues/176) | [x] |
| 7 | C2 | TanStack Query polling hooks | C1, H2 | [#186](https://github.com/brendanbyrne/beerio-kart/issues/186) | [x] |
| 8 | D1 | Named exports everywhere | (B1) | [#175](https://github.com/brendanbyrne/beerio-kart/issues/175) | [x] |
| 9 | D2 | Remove `!` assertions | — | [#192](https://github.com/brendanbyrne/beerio-kart/issues/192) | [x] |
| 10 | D3 | Remove `as` casts | B2 | [#179](https://github.com/brendanbyrne/beerio-kart/issues/179) | [x] |
| 11 | E1 | Form migration to useActionState | B2, H2 | [#182](https://github.com/brendanbyrne/beerio-kart/issues/182) | [x] |
| 12 | E2 | Ref-as-prop, Doc Metadata, Compiler | A1, C2 | [#180](https://github.com/brendanbyrne/beerio-kart/issues/180) | [x] |
| 13 | F1 | Router upgrade + boundaries + lazy | D1, H2 | [#190](https://github.com/brendanbyrne/beerio-kart/issues/190) | [ ] |
| 14 | G1 | Tailwind `@theme` + `clsx` | — | [#183](https://github.com/brendanbyrne/beerio-kart/issues/183) | [ ] |
| 15 | G2 | Accessibility sweep | G1 | [#184](https://github.com/brendanbyrne/beerio-kart/issues/184) | [ ] |
| 16 | H1 | Lint cleanup + drift-check CI + test-coverage backfill | All | [#185](https://github.com/brendanbyrne/beerio-kart/issues/185) | [ ] |

PR-H2 keeps its identifier (handoffs and Issue body already use it) but is now sequenced third, between A2 and B1. The label "Stream H" no longer fits — H2 is functionally early infrastructure, not final consolidation — but renaming the identifier mid-stream would invalidate cross-references. The Order column above is the source of truth for pickup sequence; the PR identifier is just a stable label.

The `Dep` column adds `H2` to every PR that introduces testable logic (B1's branded constructors, B2's schemas, C1/C2's hooks, E1's forms, F1's routes) — they all need the test infrastructure in place to satisfy the standards' "tests are a deliverable" rule.

ADRs produced: TBD (none anticipated unless a decision lands during the rollout that warrants one — most rules trace to the standards docs, which are the authority).

## Document history

- 2026-05-16 — Initial creation. Driven by the audit conducted same day against the new coding standards (`typescript.md`, `react.md`, `tailwind.md`, all created 2026-05-16). 16 PRs across 8 streams. No work has shipped against this plan yet; all checkboxes are open. Companion to the standards rollup in `../coding-standards/README.md` § History (2026-05-16 entry).
- 2026-05-16 — Pointed at the now-separate audit doc (`./2026-05-16-frontend-audit.md`) instead of carrying the audit findings inline. The audit was extracted into its own design record matching the backend pattern (`archive/2026-04-15-rust-audit.md` + `archive/compliance-plan.md`) so per-file context is durable when PRs are picked up individually.
- 2026-05-18 — Filed all 15 PRs as GitHub Issues under [Milestone 9 — Hardening: Frontend standards compliance](https://github.com/brendanbyrne/beerio-kart/milestone/9). All Issues set to Ready / Medium priority on the project board. PR-H2 (Vitest scaffolding) intentionally not filed — out of scope for standards compliance. Each PR section above now carries an `**Issue:** [#NN](...)` reference; sign-off summary table gained an Issue column. Issue numbers are non-contiguous (171, 173, 175, 176, 179, 180, 182, 183, 184, 185, 186, 187, 190, 191, 192) because the batched filing hit a few transient GitHub 500s that retried into new sequence positions.
- 2026-05-18 — Added testing requirement to the standards (`typescript.md` § 12, `react.md` § 13, `frontend/CLAUDE.md` § Testing). Reversed the previous decision to leave PR-H2 unfiled: it's now required infrastructure and filed as [#193](https://github.com/brendanbyrne/beerio-kart/issues/193), Ready/Medium. Re-sequenced H2 to third position (between A2 and B1) so each subsequent runtime-behavior PR can land with tests. Sign-off summary table reshaped: added an Order column, listed rows in pickup order (not PR-identifier order), added H2 as a dependency of B1/B2/C1/C2/E1/F1.
- 2026-05-18 — Applied PR [#197](https://github.com/brendanbyrne/beerio-kart/pull/197) (PR-H2) review feedback. Collapsed the Vitest `include` to a single `src/**/*.test.{ts,tsx}` glob: the AC's second `src/__tests__/**/*.{ts,tsx}` glob was redundant (the first already matches `*.test.` files anywhere under `src/`) and would make Vitest try to run plain helper/fixture files in `src/__tests__/` as test suites. Integration tests there use the `*.test.tsx` suffix like any other test. Also added an `eslint.config.js` override disabling `import/no-default-export` for `**/*.config.{ts,js}` — config files structurally require a default export, so they can never satisfy the rule; the override also clears the pre-existing `vite.config.ts` warning and prevents PR-D1's error-flip from failing the build on config files.
- 2026-05-18 — Reconciled the plan with PR-H2 ([#193](https://github.com/brendanbyrne/beerio-kart/issues/193)) as it was implemented. Two scope deltas from the AC, both decided with Brendan: (1) the coverage provider is `istanbul`, not `v8` — `v8` reports 0% under Bun, and the project has no Node toolchain (PR-H2 scope bullet updated). (2) The frontend CI job lives in a new path-filtered `.github/workflows/frontend.yml`; the backend coverage workflow gained a matching `backend/**` path filter and was renamed `coverage.yml` → `backend.yml` (workflow name `Coverage` → `Backend`) so each side's CI skips on the other's PRs and the two files are symmetric. `codecov.yml` was restructured into per-flag (`backend` / `frontend`) project + patch statuses with carryforward, and `frontend/**` was un-ignored; the frontend *project* status is `informational` until PR-H1's backfill — PR-H1 gained a scope bullet to flip it to blocking. There is no rust-lint CI job today (clippy/fmt run only in the lefthook pre-commit hook); creating one — with its own path filter — remains Issue [#195](https://github.com/brendanbyrne/beerio-kart/issues/195)'s scope.
- 2026-05-18 — Reconciled the plan with PR-A1 ([#194](https://github.com/brendanbyrne/beerio-kart/pull/194)) and PR-A2 ([#196](https://github.com/brendanbyrne/beerio-kart/pull/196)), both merged. Corrected PR-A1's rule list: `consistent-type-definitions` and `import/no-default-export` ship at `warn`, not `error` — they fire on existing code, and the warn-down principle applies to named rules too. Recorded `import/consistent-type-specifier-style` (added during PR-A1 review) and, in PR-A2, `noImplicitOverride` plus the `typecheck`-script fix (`tsc --noEmit` → `tsc -b`). Added a PR-H1 scope bullet making it the explicit owner of flipping every remaining `warn`-level lint rule back to `error` — previously no PR owned the `consistent-type-definitions` flip. Per-PR **Merged PR** links added to the A1 and A2 sections. Sign-off checkboxes left for Brendan.
- 2026-05-26 — Caught up sign-off bookkeeping for the first eight PRs in the pickup order. Checked the table rows and per-section boxes for D1 ([#205](https://github.com/brendanbyrne/beerio-kart/pull/205)) and H2 ([#197](https://github.com/brendanbyrne/beerio-kart/pull/197)) — both merged but their boxes had been missed in earlier reconciliations — and added the still-missing **Merged PR** links for H2, B1 ([#198](https://github.com/brendanbyrne/beerio-kart/pull/198)), B2 ([#199](https://github.com/brendanbyrne/beerio-kart/pull/199)), C1 ([#203](https://github.com/brendanbyrne/beerio-kart/pull/203)), C2 ([#204](https://github.com/brendanbyrne/beerio-kart/pull/204)), and D1 ([#205](https://github.com/brendanbyrne/beerio-kart/pull/205)) so every shipped PR section now points at its merged PR alongside its Issue. Companion to the PR-D2 ([#192](https://github.com/brendanbyrne/beerio-kart/issues/192)) pickup; D2's own sign-off stays open until that PR merges.
- 2026-05-26 — PR-D2 ([#192](https://github.com/brendanbyrne/beerio-kart/issues/192)) merged as [#208](https://github.com/brendanbyrne/beerio-kart/pull/208). Flipped its per-section sign-off `[ ]` → `[x]` and the row-9 table checkbox to match, and added the **Merged PR** link alongside the Issue link. Companion to picking up PR-D3 ([#179](https://github.com/brendanbyrne/beerio-kart/issues/179)) — the next item in the pickup order.
- 2026-05-27 — PR-D3 ([#179](https://github.com/brendanbyrne/beerio-kart/issues/179)) merged as [#210](https://github.com/brendanbyrne/beerio-kart/pull/210). Flipped its per-section sign-off `[ ]` → `[x]` and the row-10 table checkbox to match, and added the **Merged PR** link alongside the Issue link. Companion to picking up PR-E1 ([#182](https://github.com/brendanbyrne/beerio-kart/issues/182)) — the next item in the pickup order.
- 2026-05-27 — PR-E1 scope: clarified that `Onboarding.tsx`'s "two-phase form" doesn't render its own `<form>` — both phases delegate to picker children that dispatch already-typed payloads via callback, so the Zod-at-submit rule applies only to the four real forms (Login, Register, Profile password, DrinkTypeSelector add-form). Surfaced during [#211](https://github.com/brendanbyrne/beerio-kart/pull/211) review when "Zod schema at submit time for each form" prompted "does Onboarding need one?" — answered in the scope text rather than relitigated by future reviewers.
- 2026-05-29 — E-stream sign-off bookkeeping. PR-E1 ([#182](https://github.com/brendanbyrne/beerio-kart/issues/182)) merged as [#211](https://github.com/brendanbyrne/beerio-kart/pull/211) and PR-E2 ([#180](https://github.com/brendanbyrne/beerio-kart/issues/180)) signed off via [#214](https://github.com/brendanbyrne/beerio-kart/pull/214). Flipped both per-section sign-offs `[ ]` → `[x]`, table rows 11 and 12, and added the **Merged PR** links. Note: E2 was signed off at Brendan's request while #214 had review approval but hadn't yet landed on `main` — the box reflects sign-off rather than a confirmed merge.
