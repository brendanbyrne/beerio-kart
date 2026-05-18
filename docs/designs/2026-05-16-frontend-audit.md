# Frontend Code Audit

> **Purpose.** Per-file compliance baseline for `frontend/src/` against the coding standards introduced 2026-05-16 in [`../coding-standards/typescript.md`](../coding-standards/typescript.md), [`react.md`](../coding-standards/react.md), and [`tailwind.md`](../coding-standards/tailwind.md). Drives the sequenced PRs in the [compliance plan](./2026-05-16-frontend-compliance-plan.md).
> **Status.** Initial. Findings recorded; no PR work has shipped against them yet.
> **Scope.** Every `.ts` and `.tsx` file under `frontend/src/`, plus `frontend/tsconfig*.json`, `frontend/vite.config.ts`, `frontend/eslint.config.js`, `frontend/package.json`. Excludes generated files (`node_modules/`, `dist/`).
> **Auditor.** Cowork (Sonnet subagent), 2026-05-16.

## Executive Summary

1. **Default exports are universal.** Every page, component, and the App itself uses `export default`. Migrating to named-only exports is the single largest mechanical refactor, touching 11 export sites and every consumer's imports.
2. **`tsconfig.app.json` is partway to the standard.** `strict`, `verbatimModuleSyntax`, and `noFallthroughCasesInSwitch` are already on. Missing: `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, explicit `isolatedModules`. Enabling these will surface latent bugs (especially around array-index access in `RaceSetupPicker` and optional-prop spreads) — expect a multi-day cleanup, not a flag flip.
3. **The hand-rolled data layer is the largest standards gap by line count.** Six `useEffect`-driven fetch hooks (`useAuth`, `useGameData`, `useUserProfile`, `useSession`, `useSessions`, plus inline `useEffect` in `RunEntrySheet`) all do data-derivation/external-sync inside `useEffect`, mostly without `AbortController`. None have request cancellation; `useSessions` and `useGameData` race on rapid mount/unmount. TanStack Query migration will delete a lot of code.
4. **Type system is structurally lax.** All API DTOs are `interface` (not `type`); IDs cross every boundary as raw `string`/`number` (no branded `UserId`/`SessionId`/`RunId`); status fields like `SessionDetail.status` and `SessionSummary.ruleset` are `string` instead of discriminated unions; non-null assertions (`!`) appear in `main.tsx`, `Session.tsx`, and `RaceSetupPicker.tsx`; type-only re-exports are clean (`import type` is used consistently in `api/` and `hooks/`).
5. **Tailwind usage is mobile-first and utility-first** (compliant by default), and the project is already on Tailwind v4 with the Vite plugin. But there's no `@theme` block yet, and most components string-concatenate classes via template literals with conditionals. No `clsx`/`cva` dependency. Touch targets are mostly ≥44 px but several (`py-2 text-xs` cancel/confirm buttons, step pills) are well under.

---

## Configuration files

### `frontend/tsconfig.json`

Solution-style root. No compiler options of its own. **No changes needed.**

### `frontend/tsconfig.app.json`

**Current:** `strict: true`, `verbatimModuleSyntax: true`, `noUnusedLocals/Parameters`, `noFallthroughCasesInSwitch`, `erasableSyntaxOnly`, `noUncheckedSideEffectImports`, `jsx: react-jsx`, `moduleDetection: force`.
**Missing vs. standard:** `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `isolatedModules` (note: `verbatimModuleSyntax` is a superset of `isolatedModules`, so this is arguably already satisfied — but explicit `isolatedModules: true` is still recommended for clarity and so the rule survives any future `verbatimModuleSyntax` removal).

### `frontend/tsconfig.node.json`

Same gaps as `tsconfig.app.json` (missing `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`). Only covers `vite.config.ts`, so the blast radius is tiny.

### `frontend/vite.config.ts`

Compliant — named imports, `defineConfig` named call, no defaults. Tailwind v4 plugin wired. No proxy issues.

### `frontend/eslint.config.js`

Has: `@eslint/js` recommended, `typescript-eslint` recommended, `react-hooks` flat recommended, `react-refresh/vite`.
**Missing vs. standard:** `eslint-plugin-jsx-a11y`, `eslint-plugin-import` (for `no-default-export`), `eslint-config-prettier`, the `tseslint.configs.strictTypeChecked` / `stylisticTypeChecked` upgrade, `@typescript-eslint/consistent-type-definitions: ["error", "type"]`, `@typescript-eslint/no-explicit-any` enforcement, `@typescript-eslint/no-non-null-assertion`, `@typescript-eslint/ban-ts-comment` (for `@ts-expect-error`), `no-restricted-syntax` ban on `TSEnumDeclaration`. `ecmaVersion: 2020` is stale (tsconfig targets ES2023+).

### `frontend/package.json`

React 19.2, react-router-dom 7.13, Tailwind 4.2, TypeScript 5.9, ESLint 9.39, Vite 8 — all compliant versions.
**Missing:** `@tanstack/react-query`, `clsx` (or `cva`), `eslint-plugin-jsx-a11y`, `eslint-plugin-import`, `react-error-boundary`, `zod`. `prettier` is present; no `eslint-config-prettier` to disable conflicting rules.

---

## Source files

### `src/main.tsx`

Purpose: React 19 root mount.
**Compliant:** StrictMode wrapper; uses `createRoot`; named import of `App` (consumer side is fine).
**Violations:**
- TS: `document.getElementById('root')!` non-null assertion (line 6).
- React: `import App from './App'` — consumes a default export (line 4).

**Migration cost:** Small — replace `!` with a runtime check, switch to `import { App }`.

### `src/App.tsx`

Purpose: Router shell with three guard wrappers (AuthGate / RequireAuth / GuestOnly).
**Compliant:** Function components only; co-located prop type via inline `{ children: React.ReactNode }`.
**Violations:**
- TS: `export default function App` (lines 39, 93).
- React: All three guard components defined in this file have inline prop annotations instead of a co-located `type Props = …` (lines 11, 26, 33). No error boundary anywhere — app or route. Routes are not `lazy()`-loaded (lines 5–8 are eager imports). All page imports are default (lines 3–8). Uses `react-router-dom` in legacy `BrowserRouter` mode instead of `react-router 7`'s `createBrowserRouter` + `RouterProvider`.
- Tailwind: No violations on the loading screen.

**Migration cost:** Medium — `createBrowserRouter` migration, route-level error boundaries, and `lazy()` is one focused PR.

### `src/api/client.ts`

Purpose: Module-singleton access-token holder + `fetch` wrapper with 401-refresh-and-retry.
**Compliant:** Named exports throughout; clean async; descriptive JSDoc.
**Violations:**
- TS: `let accessToken` and `let onAuthFailure` are module-level mutable singletons (lines 14, 26) — not a stated standard violation but untestable. `await res.json()` returns `any`, so `data.access_token` (line 45) is implicitly `any`. No runtime validation; the function swallows network errors and returns `false`.
- Tailwind: n/a.

**Migration cost:** Medium — replacing the singleton with a context-provided client + adding Zod parsing on refresh response.

### `src/api/types.ts`

Purpose: Hand-written TS mirror of the Rust API DTOs.
**Compliant:** `NotificationPayload` (around lines 170–185) is already structured as a discriminated union with `kind` tag — exactly what the standard wants, even though it's currently a single-variant union.
**Violations:**
- TS: Every DTO is `interface` not `type` (lines 1, 7, 15, 22, 30, 36, 47, 58, 65, 73, 80, 87, 98, 112, 132, 141, 151, 171, 182, 189). No branded types — `id: string` and `user_id: string` (lines 36, 47, 65, 73, 80, 87, 98, 112, 151) are raw strings; same with numeric IDs (`character_id: number`, etc.). `RunDefaults.source` (line 138) uses an inline union literal (compliant). `SessionDetail.status: string` (line 156) and `SessionSummary.ruleset: string` (line 70) should be discriminated string-literal unions.

**Migration cost:** Large — branded IDs ripple into every API function and component prop; ~80+ call sites.

### `src/api/sessions.ts`

Purpose: Session REST helpers.
**Compliant:** Named exports; uses `import type`; consistent error pattern.
**Violations:**
- TS: `await res.json()` is `any` — `data.session_id` (line 8), `err.error` (lines 18, 40, 49, 56, 65) all silently typed as `any`. No `AbortController` support. Errors thrown as `new Error(...)` without a typed error shape — the Result-or-throw split is not in place.

**Migration cost:** Medium — adding a generic `apiCall<T>(schema, …)` helper and threading `signal` parameters.

### `src/api/runs.ts`

Purpose: Run REST helpers.
**Compliant:** Named exports; `import type`; same shape as `sessions.ts`.
**Violations:**
- TS: Same `any`-typed `res.json()` and `err.error` pattern. `getRunDefaults` returns a hard-coded fallback on error (lines 51–60) instead of throwing or returning a `Result`. No `AbortSignal` parameter.

**Migration cost:** Medium.

### `src/utils/time.ts`

Purpose: Format ms as `M:SS.mmm`; parse three string fields to ms.
**Compliant:** Named exports; pure functions; small surface; correct types.
**Violations:** None.

**Migration cost:** None.

### `src/hooks/useAuth.tsx`

Purpose: `AuthProvider` + `useAuth` context for the access token / user.
**Compliant:** Named exports; `import type { ReactNode }`; `useCallback` for stable refs; ESLint-disable comment is correctly used and explained (line 119).
**Violations:**
- TS: `User` and `AuthContextValue` are `interface` (lines 4, 9). `payload.sub`, `payload.username` (line 45) are `any`-typed JWT parse. `data.access_token` and `data.user` (lines 38, 65, 81) are `any`. JWT decode (lines 43–44) has no validation — a malformed payload throws and is swallowed.
- React: The mount `useEffect` (lines 30–55) does an external-sync task (network fetch) — *this* is a legitimate `useEffect` use (token refresh from cookie on mount). But the `login`/`register`/`logout` callbacks ignore the API client entirely and call `fetch` directly (lines 59, 75, 93), bypassing the refresh-on-401 logic. No `AbortController` for the silent refresh — unmounting during refresh leaves a `setState` on unmounted component.

**Migration cost:** Medium — splits cleanly into "swap to TanStack Query mutation" + "add Zod-validated JWT payload type."

### `src/hooks/useGameData.ts`

Purpose: Five fetch hooks for static game data (characters/bodies/wheels/gliders/drinkTypes) plus a `refresh()` for drink types.
**Compliant:** Named exports; `import type`; DRY `useSimpleList` factory.
**Violations:**
- TS: `await res.json()` returns `any` and is assigned to `SimpleItem[]`/`DrinkType[]` with no validation (lines 14, 57).
- React: `useEffect` for data-fetching (lines 10, 53) — the anti-pattern the standard bans for non-external-sync. `version`-as-refresh-key (lines 47, 49) is a known TanStack-Query-replaces-this pattern. No `AbortController` — rapid remount leaks fetches and sets state on unmounted components.

**Migration cost:** Medium — replace whole file with `useQuery({ queryKey: [endpoint], queryFn: … })` wrappers; ~30 lines become ~15.

### `src/hooks/useUserProfile.ts`

Purpose: Fetch one user's detail profile by ID, with `refresh()`.
**Compliant:** Named export; `import type`.
**Violations:**
- TS: `res.json()` is `any`, assigned to `UserDetailProfile`.
- React: `useEffect` for fetching (line 14). `version` counter as refresh trigger (lines 8, 11). No `AbortController`. No error state — failures silently leave `profile` null; loading does clear via `finally` (line 21) but errors are swallowed.

**Migration cost:** Small — direct TanStack Query swap.

### `src/hooks/useSession.ts`

Purpose: Poll `GET /sessions/:id` every 2.5 s, pause on tab hidden, stop on session ended.
**Compliant:** Named export; `import type`; visibility-API pause is thoughtful; `cancelled` flag in cleanup is the correct manual cancellation pattern given no `AbortController`.
**Violations:**
- TS: `useRef<ReturnType<typeof setInterval> | null>` (line 16) is fine but verbose; `id: string` could be `SessionId` (branded).
- React: `useEffect` for external sync (polling) — legitimate. But the entire effect's complexity (lines 19–66) is exactly what TanStack Query's `refetchInterval` + `refetchIntervalInBackground: false` does in two lines. `endedRef` (line 17) is a mutable ref to mirror state — symptom of needing query-cancellation, not a true escape hatch.

**Migration cost:** Medium — replace with `useQuery({ refetchInterval: 2500, enabled: !ended })`.

### `src/hooks/useSessions.ts`

Purpose: Poll session list + "my session" every 5 s, pause on tab hidden.
**Compliant:** Same shape as `useSession.ts`.
**Violations:**
- TS: `Promise.all(...).then(...)` (line 19) without `.catch()` — rejection becomes unhandled.
- React: `useEffect` for polling — legitimate, but again duplicates TanStack Query. No `AbortController`. No request deduplication if two `useSessions()` mount concurrently.

**Migration cost:** Small — direct query swap.

### `src/components/BottomNav.tsx`

Purpose: Bottom tab bar (Home / Session / Profile) with disabled state.
**Compliant:** Function component; mobile-first Tailwind; `min-h-[52px]` meets touch-target rule (line 44).
**Violations:**
- TS: `export default function BottomNav` (line 5).
- React: `useEffect` fires `getMySession()` on every `location.pathname` change (lines 11–13) — data-derivation triggered by route change, the exact anti-pattern. Could be a TanStack Query with `queryKey: ['my-session']` invalidated by the auth/session mutations. `<button>` with click navigation (line 40) instead of `<Link>` — breaks middle-click/cmd-click and keyboard accessibility. No `aria-current="page"` for active tab.
- Tailwind: Template-literal class concatenation with nested ternary (lines 44–50) — not banned per se, but no `clsx`/`cva` makes it noisy. `safe-area-pb` is the custom class defined in `index.css` (line 4) — correct usage.

**Migration cost:** Small — swap `<button onClick={navigate}>` for `<NavLink>`, replace the useEffect with a query.

### `src/components/DrinkTypeSelector.tsx`

Purpose: List of drink types with inline "add new" form.
**Compliant:** Function component; uses the `useDrinkTypes` hook; min `h-6` toggle and `min-h-[44px]`-equivalent items.
**Violations:**
- TS: Default export (line 12). `DrinkTypeSelectorProps` is `interface` (line 6). `data.error` (line 40) is `any`. The DrinkType POST response is typed via annotation `const created: DrinkType = await res.json()` (line 43) — same anti-pattern as `as` casts (the annotation lies; no runtime check).
- React: Local `useState` for form fields (lines 19–22) — should be uncontrolled form with `useActionState` per the standard. `autoFocus` (line 99 — the closing `/>` is line 100) is an a11y problem and unlikely to survive React 19 strict-mode double-invoke cleanly.
- Tailwind: Template-literal concat with ternary (lines 64–68, 106–108, 111–113) — eligible for `clsx`/`cva`. "Skip for now" button is `py-2 text-sm` (line 144) — ~32 px tall, fails 44 px touch-target rule. "Cancel"/"Add" buttons are `py-2 text-xs` (lines 126, 133) — also under 44 px.

**Migration cost:** Medium — form migration + clsx refactor + add Zod schemas.

### `src/components/RaceSetupPicker.tsx`

Purpose: Four-step picker (character → body → wheel → glider) with auto-advance.
**Compliant:** Function component; mobile-first; `text-center` semantics; `loading="lazy"` on images.
**Violations:**
- TS: Default export (line 29). `RaceSetupPickerProps` is `interface` (line 5). `STEPS[currentStepIndex + 1]` and `STEPS[currentStepIndex - 1]` (lines 85, 91) — under `noUncheckedIndexedAccess` these become `Step | undefined` and break the `setStep` call. Non-null assertions on `characterId!`, `bodyId!`, `wheelId!`, `gliderId!` (lines 173–176) — banned `!`, even though `allSelected` proves they're not null.
- React: `setTimeout(() => setStep(...), 150)` (line 85) inside an event handler without cleanup — fine in this case but a smell. Auto-advancing via `setTimeout` is a UX choice that should be explicit.
- Tailwind: Class concat with ternary (lines 106–112, 139–143). Step indicator buttons are `py-1.5 text-xs` (line 106) — well under 44 px touch target. Grid item buttons are `p-1.5` (line 139) — content is a 56×56 image so total ~70 px, OK.

**Migration cost:** Medium — switch to `satisfies`/refactor to use discriminated state per step, replace `!` with narrowing.

### `src/components/RunEntrySheet.tsx`

Purpose: Bottom-sheet form to submit a run (times + drink + setup + DQ) with auto-advancing time inputs and a slide-to-confirm DQ control.
**Compliant:** Composed of three smaller components (`TimeInputGroup`, `SlideToConfirm`); `React.RefObject` typed correctly; `useMemo` and `useCallback` used appropriately for handler stability; sum-mismatch warning is good UX.
**Violations:**
- TS: Default export (line 24). `TimeFields`, `RunEntrySheetProps`, `TimeInputGroupProps`, `SlideToConfirmProps` all `interface` (lines 10, 18, 389, 497). `(e.target as HTMLImageElement)` cast (line 176) — banned `as` cast (should be `e.currentTarget`). `e.touches[0].clientX` (line 568) — under `noUncheckedIndexedAccess`, `e.touches[0]` is `Touch | undefined`. The early-return guard at lines 117–129 is the *only* way the type system can infer non-null on `parsedTotal`/`parsedLap1`/.../`drinkTypeId` — fine, but illustrates that branded `Time` types would let the parser produce a value-or-error union.
- React: `useEffect` for loading defaults (lines 60–75) — data-fetching anti-pattern. `useEffect` to measure track width (lines 511–518) — legitimate external sync (DOM measurement + resize listener), passes. No `AbortController` on the `getRunDefaults` fetch. The handlers `setError(e instanceof Error ? e.message : ...)` (line 150) are good error-extraction but the `setError` followed by `setSubmitting(false)` is duplicated across the codebase — extract to a util.
- Tailwind: Heavy template-literal class concat in `inputClass`/`sepClass` (lines 414–419), button classes (lines 231–248, 325–331). Inline styles (`style={{ maxHeight: '92%' }}` line 161, `style={{ width: thumbW, left: offsetX + 4, transition: ... }}` line 581, `style={{ opacity: 1 - progress * 1.5 }}` line 573) — the slide-to-confirm needs dynamic positioning so inline styles are unavoidable there; the static `maxHeight` should move to a Tailwind arbitrary value `max-h-[92%]`. Touch-target wrappers `min-h-[48px]` on drink-row "Change" buttons (lines 264, 294) — OK.

**Migration cost:** Large — three sub-components, form-state model needs `useActionState`, the time-input auto-advance refs interact with all of them, and the slide-to-confirm has DOM measurement. Probably splits into 2–3 PRs.

### `src/pages/Login.tsx`

Purpose: Login form.
**Compliant:** Function component; `<form onSubmit>`; proper `<label htmlFor>` pairs; `autoComplete` attrs; `required` validation.
**Violations:**
- TS: Default export (line 5).
- React: Controlled inputs (lines 45–50, 60–66) — standard prefers uncontrolled unless live validation needed. Should use `useActionState` + `useFormStatus` for submit state instead of local `submitting` flag (line 11).
- Tailwind: No violations. `py-2.5` button is ~40 px tall — borderline on 44 px rule (line 71).

**Migration cost:** Small — `useActionState` migration is mechanical for this size of form.

### `src/pages/Register.tsx`

Purpose: Register form. Structural twin of `Login.tsx`.
**Compliant:** Same as Login.
**Violations:** Same as Login — default export, controlled inputs, no `useActionState`.

**Migration cost:** Small.

### `src/pages/Onboarding.tsx`

Purpose: Two-phase onboarding (race setup → drink type) post-registration.
**Compliant:** Function component; co-located `Phase` discriminated literal type (line 9).
**Violations:**
- TS: Default export (line 11). `data.error` is `any` (lines 39, 62).
- React: Local `useState<Phase>` (line 14) — phase machine is fine. Calls `apiFetch` directly inside the page rather than via a typed `updateUserProfile` API helper — that helper doesn't exist in `api/`. Same try/catch/setError/setSaving boilerplate as everywhere else.
- Tailwind: No violations.

**Migration cost:** Small — extract API helper, swap to `useMutation`.

### `src/pages/Profile.tsx`

Purpose: Profile view with three editable sections (race setup, drink type, password) + logout.
**Compliant:** Function component; mode-driven sub-views; semantic structure.
**Violations:**
- TS: Default export (line 14). `EditMode = null | 'race-setup' | 'drink-type' | 'password'` (line 12) — fine as a union but conventionally written `'race-setup' | 'drink-type' | 'password' | null`. `data.error` is `any` throughout.
- React: Same pattern as Onboarding — direct `apiFetch` in component, manual try/catch/setError. `setTimeout(() => { setEditMode(null); setPasswordSuccess(false) }, 1500)` (line 121) — no cleanup; if user navigates away within 1.5 s, you set state on unmounted component. Password "Change Password" button (line 308) is `py-2 text-xs` — well under 44 px touch target. Cancel button same (line 303). All edit cards are `<button>` with full-card click (lines 215, 251, 317) — fine, but if the inner block has interactive elements you get nested-interactive warnings.
- Tailwind: No `clsx` — uses template-literal concat.

**Migration cost:** Medium — same migrations as Onboarding plus password change extraction.

### `src/pages/Home.tsx`

Purpose: Landing page with start-session CTA, create-session modal, and active-sessions list.
**Compliant:** Function component; uses `useSessions` and `useUserProfile` hooks; good empty state.
**Violations:**
- TS: Default export (line 10). `data.error` is `any`.
- React: Modal is conditionally rendered (`showCreate && …`) without focus trap, no `Escape` key handler, no `role="dialog"` / `aria-modal` — a11y violation. Modal click-out handler (line 81) doubles as backdrop dismiss — fine but should also handle `Escape`.
- Tailwind: Active-session buttons (line 131) — full-card click, ≥44 px due to `p-4`. Class concat with ternary (lines 131–133). "Cancel" in modal `py-2 text-sm` (line 107) — borderline ~32 px.

**Migration cost:** Medium — add modal a11y (native `<dialog>` or radix/`@reach/dialog`).

### `src/pages/Session.tsx`

Purpose: Live session view with sticky header, current-race card, action bar, participants, race history, and submit-time entry point.
**Compliant:** Function component; uses `useSession` polling hook; mobile-first; thoughtful collapse behavior.
**Violations:**
- TS: Default export (line 10). Non-null assertions `id!` (lines 13, 29, 40, 51, 63) — `useParams<{ id: string }>()` returns `Partial<{ id: string }>` so `id` is `string | undefined`; the `!` is banned. Once the user reaches `Session.tsx` without `id`, the polling hook is given `undefined!`.
- React: Three `useEffect`s lines 72, 77, 83 — all three are data-derivation, exactly the anti-pattern. The first two could be `useMemo`/derived state (track ID changed → reset image error). The "auto-collapse history when > 3 races" `useEffect` (line 83) sets state derived purely from `hasMany` — should just be `const historyExpanded = hasManyManualOverride ?? !hasMany`. The `eslint-disable react-hooks/exhaustive-deps` (line 79) is a code smell hiding a missing dep. `pastRaces = [...session.races].reverse().filter(...)` (line 114) is computed on every render — fine for small N but should be `useMemo`.
- Tailwind: Class concat with ternary throughout (lines 131–133, 231–247, etc.). "Submit Time" button is `py-3` — ~44 px, OK. "Skip Track" `py-2.5` (line 217) — ~40 px, borderline. "Leave Session" same.

**Migration cost:** Large — `id!` removal needs route-param validation pattern, three `useEffect`s need refactor to derived state, polling migrates to TanStack Query.

---

## Cross-cutting violations

These appear in many files and should be tackled as horizontal sweeps, not per-file:

1. **Default exports** — every page (`Login`, `Register`, `Onboarding`, `Profile`, `Home`, `Session`), every component (`App`, `BottomNav`, `DrinkTypeSelector`, `RaceSetupPicker`, `RunEntrySheet`), and `main.tsx`'s import of `App` use defaults. 11 export sites + ~25 import sites.
2. **`interface` over `type`** — every props type and every DTO. ~25 declarations.
3. **No branded ID types** — `string` IDs (`UserId`, `SessionId`, `RunId`, `RaceId`, `DrinkTypeId`) and `number` IDs (`CharacterId`, `BodyId`, `WheelId`, `GliderId`, `TrackId`, `CupId`) cross every API boundary as raw primitives.
4. **`useEffect` for data-fetching/derivation** — `useGameData` (×2), `useUserProfile`, `useSession`, `useSessions`, `RunEntrySheet` (defaults load), `BottomNav` (session check), `Session.tsx` (3 derivation effects).
5. **`any`-typed `await res.json()`** — every API helper in `api/sessions.ts`, `api/runs.ts`, `api/client.ts`, `useAuth.tsx`, `useGameData.ts`, `useUserProfile.ts`, and every page's direct `apiFetch` call (`Profile`, `Onboarding`, `DrinkTypeSelector`).
6. **Non-null assertions `!`** — `main.tsx:6`, `Session.tsx:13/29/40/51/63`, `RaceSetupPicker.tsx:173-176`.
7. **`as Foo` casts and type-annotation-as-validation without runtime checks** — `RunEntrySheet.tsx:176` (`as HTMLImageElement`), `DrinkTypeSelector.tsx:43` (response-type annotation).
8. **Controlled form inputs without `useActionState`** — `Login`, `Register`, `Profile` (password), `DrinkTypeSelector` (new drink form), `Onboarding`.
9. **Template-literal class concat without `clsx`/`cva`** — every component with conditional styling. ~10 files.
10. **No `@theme` / design tokens** — Tailwind v4 is installed but no CSS-first config visible in `index.css`; stock palette only.
11. **`react-router-dom` 7 used in legacy `BrowserRouter` mode** — should be `createBrowserRouter` + `RouterProvider`.
12. **No error boundaries** — neither app-level nor route-level.
13. **No `AbortController` plumbing** — zero fetches accept a signal.
14. **No request cancellation / no TanStack Query** — every hook re-implements the same poll/refresh/loading/error machinery.

---

## Categorized fix list

| Category | File count | Files |
|---|---|---|
| Default exports | 12 sites (11 declarations + 1 import) | `main.tsx`, `App.tsx`, `BottomNav.tsx`, `DrinkTypeSelector.tsx`, `RaceSetupPicker.tsx`, `RunEntrySheet.tsx`, `Login.tsx`, `Register.tsx`, `Onboarding.tsx`, `Profile.tsx`, `Home.tsx`, `Session.tsx` |
| `interface` → `type` | 6 source files (~25 declarations) | `api/types.ts`, `useAuth.tsx`, `DrinkTypeSelector.tsx`, `RaceSetupPicker.tsx`, `RunEntrySheet.tsx`, `App.tsx` (inline) |
| Missing branded IDs | All API DTOs + all components consuming them | `api/types.ts` (declarations) plus every page/hook |
| `useEffect` for data-fetching | 7 hooks/components | `useGameData.ts`, `useUserProfile.ts`, `useSession.ts`, `useSessions.ts`, `useAuth.tsx` (silent refresh — legitimate), `RunEntrySheet.tsx` (defaults load), `BottomNav.tsx` |
| `useEffect` for state derivation | 1 page (3 effects) | `Session.tsx` |
| `any`-typed `res.json()` | 9 files | `api/client.ts`, `api/sessions.ts`, `api/runs.ts`, `useAuth.tsx`, `useGameData.ts`, `useUserProfile.ts`, `DrinkTypeSelector.tsx`, `Onboarding.tsx`, `Profile.tsx` |
| Non-null `!` | 3 files | `main.tsx`, `Session.tsx`, `RaceSetupPicker.tsx` |
| `as Foo` cast / annotation-only validation | 2 files | `RunEntrySheet.tsx`, `DrinkTypeSelector.tsx` |
| Controlled inputs (no `useActionState`) | 5 files | `Login.tsx`, `Register.tsx`, `Profile.tsx`, `DrinkTypeSelector.tsx`, `Onboarding.tsx` |
| Class concat without `clsx`/`cva` | 8 files | `BottomNav.tsx`, `DrinkTypeSelector.tsx`, `RaceSetupPicker.tsx`, `RunEntrySheet.tsx`, `Profile.tsx`, `Home.tsx`, `Session.tsx`, `App.tsx` (light) |
| Touch targets <44 px | 4 files | `DrinkTypeSelector.tsx` (skip/cancel/add), `RaceSetupPicker.tsx` (step pills), `Profile.tsx` (password cancel/save), `Home.tsx` (modal cancel) |
| No `@theme` tokens | repo-wide | `index.css` is `@import 'tailwindcss';` plus one custom rule (`safe-area-pb`) |
| No error boundary | 1 file | `App.tsx` |
| No `AbortController` | All 6 hooks + all API helpers | repo-wide |
| Legacy `BrowserRouter` | 1 file | `App.tsx` |
| tsconfig flags missing | 2 files | `tsconfig.app.json`, `tsconfig.node.json` |
| ESLint plugins/rules missing | 1 file | `eslint.config.js` |

---

## Things not found (already compliant)

- **No `enum` keyword usage** in any file. The `no-restricted-syntax` rule keeps it that way after PR-A1.
- **No `// @ts-ignore`** in any file. One ESLint-disable in `useAuth.tsx:119` is justified and commented.
- **No `localStorage`/`sessionStorage`** usage in the audited files. JWT refresh goes through an HTTP-only cookie + module-scope memory (not great for testability, but not a standards violation).
- **Tailwind is mobile-first by default** — no `max-*:` breakpoint inversions found.
- **`import type` / `export type`** is used consistently in `api/` and `hooks/`.

## Things not found (notable gaps outside standards scope)

- **No tests.** No `*.test.ts(x)`, no `vitest.config.ts`, no `__tests__/`. The standards don't mandate testing today; flagged here because it's a real gap. Tracked as optional PR-H2 in the compliance plan.
- **No bundle analysis.** No `vite-bundle-visualizer` or equivalent. Worth adding when route splitting lands in PR-F1, to verify the lazy chunks materialize.
- **No `index.html` audit performed.** Out of scope for source-file audit but worth a one-pass review for meta tags, viewport settings, favicon, and any inline `<script>`s.

---

## Document history

- 2026-05-16 — Initial creation. Sourced from the Sonnet subagent that audited every file in `frontend/src/` plus the four config files on 2026-05-16. Drives the sequenced PRs in [`./2026-05-16-frontend-compliance-plan.md`](./2026-05-16-frontend-compliance-plan.md). Created as a sibling to the compliance plan (rather than absorbed into it) to match the backend pattern from `archive/2026-04-15-rust-audit.md` + `archive/compliance-plan.md`.
