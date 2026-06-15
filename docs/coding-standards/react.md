# React — Coding Standards

> **Scope.** React 19.2, react-router-dom 7, mobile-first SPA, no SSR. Component shape, hooks, data fetching, forms, errors, accessibility, routing, and file layout. TS rules live in [`typescript.md`](./typescript.md); styling in [`tailwind.md`](./tailwind.md).
> **Format.** Each rule: *Rule / Why / Example / Source*.
> **Companions.** `typescript.md`, `tailwind.md`, `../api-contract.md`, `../user-workflows.md` (UI screen specs). Compliance plan: `../designs/2026-05-16-frontend-compliance-plan.md`.

## Index

1. [Component shape](#1-component-shape)
2. [Rules of Hooks & the React Compiler](#2-rules-of-hooks--the-react-compiler)
3. [State management](#3-state-management)
4. [Data fetching](#4-data-fetching)
5. [React 19 primitives — what to adopt](#5-react-19-primitives--what-to-adopt)
6. [Effects](#6-effects)
7. [Memoization](#7-memoization)
8. [Forms](#8-forms)
9. [Error boundaries](#9-error-boundaries)
10. [Accessibility floor](#10-accessibility-floor)
11. [Routing (react-router 7)](#11-routing-react-router-7)
12. [File organization](#12-file-organization)
13. [Testing](#13-testing)

---

## 1. Component shape

- **Rule:** Function components only. No class components. Named exports only (see [`typescript.md`](./typescript.md) § 5). One primary component per file, file name matches the component (`SessionPanel.tsx` exports `SessionPanel`).
  - **Why:** Function components have been the React-team-recommended shape since hooks shipped; class components don't support hooks, the React Compiler, or several React 19 primitives. Named exports give consistent symbol names at every import site — `grep` works, refactors work, no "default-named-something-else" confusion.
  - **Source:** <https://react.dev/learn/your-first-component>

- **Rule:** Co-locate `type Props = { ... }` immediately above the component. Do not inline the props type into the function signature unless the component takes only `children`.
  - **Why:** The props type is the component's contract. Putting it one scroll above the implementation matches the format consumers will read, and makes the contract greppable as `type SessionPanelProps`.
  - **Example:**
    ```tsx
    type Props = { sessionId: SessionId; onClose: () => void };

    export function SessionPanel({ sessionId, onClose }: Props) {
      // ...
    }
    ```

- **Rule:** Extract a sub-component when (a) it has its own state or effects, (b) it's reused, or (c) the JSX exceeds roughly one screen of code *and* the chunk is conceptually independent. Otherwise keep the larger component intact.
  - **Why:** Premature extraction fragments related logic and forces prop-drilling for state that wanted to stay local. The audit shows `RunEntrySheet.tsx` correctly extracted `TimeInputGroup` and `SlideToConfirm` because they're conceptually distinct *and* manage their own DOM concerns; `Login.tsx` and `Register.tsx` correctly stayed as one piece each.
  - **Source:** <https://kentcdodds.com/blog/colocation>

## 2. Rules of Hooks & the React Compiler

- **Rule:** Hooks at the top level only. No hooks in loops, conditions, or nested functions. Hooks called only from function components or other custom hooks. Enforced by `eslint-plugin-react-hooks` (`rules-of-hooks`, `exhaustive-deps`) on its `recommended` preset.
  - **Why:** React identifies each hook by call order. Conditional or nested calls desynchronize state across renders and corrupt the component. The one exception is React 19's `use()`, which can be called conditionally — it's the only hook the rules carve out, and it does so by being a *language* primitive, not a state hook.
  - **Source:** <https://react.dev/reference/rules/rules-of-hooks>

- **Rule:** The React Compiler is on. Install `babel-plugin-react-compiler` (v1.x stable since October 2025) in `vite.config.ts` and the matching `eslint-plugin-react-hooks` rules. Compiler-managed memoization replaces hand-written `useMemo`/`useCallback`/`React.memo` for the default case (see § 7).
  - **Why:** v1.0 is production-tested at Meta, has first-party Vite templates, and removes a class of perf footguns. The compiler still requires the Rules of Hooks to hold — it doesn't relax them, it depends on them.
  - **Source:** <https://react.dev/blog/2025/10/07/react-compiler-1> · <https://react.dev/learn/react-compiler/introduction>

- **Rule:** A custom hook starts with `use`, returns a tuple or an object with named fields (never a single bare value if there's more than one return), and accepts dependencies as arguments rather than reading from module scope.
  - **Why:** The naming triggers the lint rules; the explicit-dependency rule makes the hook testable and composable. Module-scope reads inside a custom hook are an unannotated dependency the compiler can't track.

## 3. State management

- **Rule:** Default to `useState` in the component that owns the state. Lift only when two siblings need to read or mutate the same value. Use Context **only** for low-frequency, app-wide values (current user, theme, locale).
  - **Why:** Every consumer of a Context re-renders when the Context value changes. That's correct for config-shaped data and pathological for high-frequency or server-derived data. Putting `currentSession` in Context, for example, would re-render the entire tree on every polling tick.
  - **Source:** <https://react.dev/learn/sharing-state-between-components>

- **Rule:** No external state library (Zustand, Jotai, Redux Toolkit) until measured pain. The default stack is `useState` + custom hooks + TanStack Query (§ 4) for server data + a single `AuthContext` for the signed-in user.
  - **Why:** External stores solve coordination problems we don't have yet. Adopting one before the problem appears adds bundle weight, an extra mental model, and yet another place state can live. TanStack Query already covers "shared server cache," which is the most common reason teams reach for Zustand.

## 4. Data fetching

- **Rule:** TanStack Query v5 owns all server data. Every endpoint is wrapped in a `useQuery` (read) or `useMutation` (write). The raw `fetch` wrapper (`apiFetch` in `frontend/src/api/client.ts`) remains the `queryFn`; TanStack Query handles cache, deduplication, refetch, retry, and tab-focus revalidation.
  - **Why:** Five things we already re-implement in custom hooks (deduplication, cache, background refetch, retry-with-backoff, focus revalidation) come for free with TanStack Query. Our polling model ([ADR 0018](../decisions/0018-realtime-via-polling-not-websockets.md)) is its wheelhouse — `refetchInterval` plus `refetchIntervalInBackground: false` reproduces the visibility-API pause we hand-roll today in `useSession.ts` and `useSessions.ts`. The migration deletes more code than it adds.
  - **Example:**
    ```tsx
    export function useSession(id: SessionId) {
      return useQuery({
        queryKey: ['session', id],
        queryFn: ({ signal }) => fetchSession(id, signal),
        refetchInterval: (q) => (q.state.data?.ended_at ? false : 2500),
      });
    }
    ```
  - **Source:** <https://tanstack.com/query/latest/docs/framework/react/comparison> · <https://tanstack.com/query/latest/docs/framework/react/guides/window-focus-refetching>

- **Rule:** Don't reach for React 19's `use()` to consume promises directly. Let TanStack Query own the Suspense integration via `useSuspenseQuery` if Suspense-style loading UX is wanted.
  - **Why:** `use()` is a great primitive but provides no cache, no polling, no retry. Calling it on raw fetches reinvents the library we already picked.

- **Rule:** Query keys are arrays starting with the resource name, then identifiers, then filters. Keep them stable — same key across components that need the same data.
  - **Why:** Stable keys are what TanStack Query uses for dedup and cache invalidation. Drift in key construction means two components that ask for "the same session" each pay for their own fetch.
  - **Example:**
    ```ts
    ['session', sessionId]
    ['sessions', { status: 'open' }]
    ['user-profile', userId]
    ```

## 5. React 19 primitives — what to adopt

For a no-SSR SPA on React 19.2:

| Primitive | Adopt | Notes |
|---|---|---|
| Ref-as-prop (no `forwardRef`) | Yes | `forwardRef` is deprecated. Pass `ref` as a normal prop. |
| `use()` for context | Yes | Replaces `useContext`. Cleaner ergonomics, can be called conditionally. |
| `use()` for promises | No (direct) | Use `useSuspenseQuery` via TanStack Query if Suspense UX is wanted. |
| `useActionState` | Yes | Pair with `<form action={...}>`. See § 8. |
| `useFormStatus` | Yes | Lets reusable submit buttons read parent-form pending state. |
| `useOptimistic` | Yes, for the right UX | Lap submission and "drink type added" are the obvious candidates. |
| Document Metadata (`<title>`, `<meta>` in JSX) | Yes | Replaces ad-hoc `useEffect(() => { document.title = ... })`. |
| Owner Stacks | Dev only | Already on in React 19.2 dev builds. No code change required. |

Sources: [React 19 release notes](https://react.dev/blog/2024/12/05/react-19) · [React 19.2 release notes](https://react.dev/blog/2025/10/01/react-19-2).

## 6. Effects

- **Rule:** Use `useEffect` only to synchronize React with something *outside* React: setting up a `setInterval`, attaching a `resize`/`visibilitychange`/`keydown` listener, measuring the DOM, integrating a third-party widget, or reading from a browser storage API. Do *not* use it to derive state, transform props, reset state on prop change, or fetch data.
  - **Why:** The "You Might Not Need an Effect" guide is the most-cited React pillar of the last two years. Effects used for derivation cause cascading renders, race conditions, and the bugs Strict Mode exists to surface. For data fetching specifically, TanStack Query (§ 4) replaces effect-based fetching entirely; for derived values, use a `const` computed in render (or `useMemo` if expensive).
  - **Source:** <https://react.dev/learn/you-might-not-need-an-effect> · <https://react.dev/learn/synchronizing-with-effects>

- **Rule:** Every effect that starts something cancellable returns a cleanup function. For fetches: pass `AbortController.signal` and `controller.abort()` in cleanup. For intervals: `clearInterval`. For event listeners: `removeEventListener` with the same reference.
  - **Why:** Strict Mode double-invokes every effect in dev specifically to surface missing cleanup. The double-mount must be indistinguishable from a single mount — if your cleanup is right, it is.

- **Rule:** Do not silence `react-hooks/exhaustive-deps`. If the dependency array fights you, the effect is doing the wrong thing — refactor it.
  - **Why:** Stale-closure bugs in effects are some of the most expensive to debug. The lint rule exists because the closure semantics aren't obvious. The audit found one suppression in `Session.tsx:79` masking a missing dep; that suppression goes away in the compliance plan.
  - **Source:** <https://react.dev/reference/eslint-plugin-react-hooks/lints/exhaustive-deps>

## 7. Memoization

- **Rule:** With the React Compiler on (§ 2), do not write `useMemo`, `useCallback`, or `React.memo` by default. Use them only when (a) the profiler shows a real bottleneck, (b) a value is passed to a third-party library that requires referential stability (e.g., a chart's `data` prop), (c) a computation is genuinely expensive (>1 ms) and the Compiler missed it, or (d) a reference's stability is load-bearing for *correctness*, not just render cost — e.g. a function used as a `useEffect` dependency or registered into module/external scope, where a changing identity would re-fire the effect or swap out the registered callback.
  - **Why:** The Compiler memoizes intermediate values and component outputs more aggressively than hand-written memoization, and it can memoize conditionally — something the hooks can't. Hand-written memo without measurement is noise that adds dependency-array bugs and obscures the actual hot paths.
  - **Why (d) is different:** Carve-outs (a)–(c) are performance judgments; (d) is a correctness one. The Compiler's memoization is an optimization, not a contract — it may legitimately choose not to memoize, so never rely on it to keep an effect from re-firing. `exhaustive-deps` surfaces these cases (it warns that a non-memoized function "makes the dependencies of useEffect change on every render"); that warning is the signal to reach for an explicit `useCallback`, not to silence the rule (§ 6). Live example: `useAuth`'s `clearAuth` is both a mount-effect dependency and registered via `setOnAuthFailure`, so it stays an explicit `useCallback` while the provider's other callbacks drop theirs.
  - **Source:** <https://react.dev/learn/react-compiler/introduction> · <https://react.dev/reference/react/useMemo>

- **Rule:** Where memoization stays (per the carve-outs above), the dep array is non-negotiable: every value read inside the memoized function is in the deps. `exhaustive-deps` enforces this.

## 8. Forms

- **Rule:** For new forms, use React 19 form actions with `useActionState`. Read input values from `FormData` in the action; keep inputs uncontrolled unless live cross-field validation or computed display is required.
  - **Why:** `useActionState` collapses `isSubmitting`/`error`/`data` into one return value and integrates with `<form action={...}>` natively. Uncontrolled inputs are faster and simpler for the common case (login, register, "create a session"). Controlled inputs cost a re-render per keystroke and force you to mirror state for no benefit.
  - **Example:**
    ```tsx
    const [state, submit, pending] = useActionState(
      async (_prev, formData: FormData) => {
        const parsed = SessionFormSchema.safeParse(
          Object.fromEntries(formData),
        );
        if (!parsed.success) {
          return { error: parsed.error.flatten() };
        }
        const result = await createSession(parsed.data);
        return result.ok ? { ok: true } : { error: result.error };
      },
      { ok: false },
    );

    return (
      <form action={submit}>
        <input name="name" required minLength={1} maxLength={60} />
        <SubmitButton />
      </form>
    );

    function SubmitButton() {
      const { pending } = useFormStatus();
      return (
        <button type="submit" disabled={pending}>
          {pending ? 'Creating…' : 'Create'}
        </button>
      );
    }
    ```
  - **Source:** <https://react.dev/reference/react/useActionState> · <https://react.dev/reference/react-dom/hooks/useFormStatus>

- **Rule:** Native HTML validation (`required`, `minLength`, `maxLength`, `pattern`, `type="email"`) is the first line. Schema validation at submit (Zod, per [`typescript.md`](./typescript.md) § 8) is the second. No form library (React Hook Form, Formik) for now — revisit if a single form grows past ~10 fields or needs live cross-field validation.
  - **Why:** Native validation gets accessibility for free (screen readers announce it, focus jumps to the first invalid field, mobile keyboards adapt to `inputmode`). Form libraries are a real adoption cost we don't owe; our forms are mostly 2–6 fields.
  - **Source:** <https://developer.mozilla.org/en-US/docs/Learn_web_development/Extensions/Forms/Form_validation>

- **Rule:** Submit state goes through `useFormStatus`, not a separate `useState<boolean>`. Reusable submit buttons read `useFormStatus().pending` so the parent doesn't need to thread `disabled` props.
  - **Why:** One less prop, one less place for state to drift.

## 9. Error boundaries

- **Rule:** Wrap (1) the whole app with a generic "something broke" fallback, and (2) each top-level route with a route-scoped fallback. Use `react-error-boundary` for the function-component API and `useErrorBoundary()` reset hook.
  - **Why:** Error boundaries are the only way to recover from a render crash without unmounting the entire tree. Multiple targeted boundaries isolate failures — a busted leaderboard widget shouldn't blank the race screen.
  - **Source:** <https://react.dev/reference/eslint-plugin-react-hooks/lints/error-boundaries> · <https://www.npmjs.com/package/react-error-boundary>

- **Rule:** Error boundaries catch render-time errors only — *not* event handlers, async code, or fetch errors. Async/event-handler failures go through TanStack Query's `error` state (for query failures) and a global `window.onunhandledrejection` reporter (for unhandled async).
  - **Why:** This is a documented React limitation; conflating the two leads to bug reports like "the error boundary didn't catch my fetch failure." TanStack Query's per-query `error` is the right place for "your data didn't load" UI.

## 10. Accessibility floor

- **Rule:** Ship nothing without: semantic HTML (`<button>` not `<div onClick>`, `<a>`/`<Link>` not `<button onClick={navigate}>`), visible focus styles (do not strip Tailwind's `focus-visible:ring`), keyboard reachability for every interactive element, labels on every form control, alt text on every meaningful image (`alt=""` on decorative ones), color contrast ≥ 4.5:1 for body text and ≥ 3:1 for large text/UI components. Manage focus on route change and on modal open/close. Touch targets ≥ 44×44 px (CSS pixels).
  - **Why:** WCAG 2.1 AA is the floor and is now a legal requirement across the EU as of June 28, 2025 (European Accessibility Act). Mobile-first compounds this — touch targets must be reachable by an adult thumb, and screen readers on mobile are heavily used. The audit found several buttons under 44 px (`py-2 text-xs` cancel/confirm) that violate this rule.
  - **Source:** <https://www.w3.org/WAI/WCAG21/quickref/> · <https://webaim.org/articles/contrast/>

- **Rule:** `eslint-plugin-jsx-a11y` is on with the `recommended` preset. Pair it with `@axe-core/react` in dev for runtime checks the linter can't see (focus traps, live regions, color contrast).
  - **Why:** The linter catches the cheap, statically-detectable issues (missing alt, invalid ARIA, non-interactive elements with click handlers). Axe catches what runs only at render time. Together they cover most of the common-failure spectrum.
  - **Source:** <https://github.com/jsx-eslint/eslint-plugin-jsx-a11y> · <https://github.com/dequelabs/axe-core-npm/tree/develop/packages/react>

- **Rule:** Modals (`<dialog>` or a custom overlay) trap focus, close on `Escape`, restore focus to the trigger on close, and use `role="dialog"` + `aria-modal="true"`. The `RunEntrySheet` bottom-sheet and the `Home` create-session modal both fall under this rule.
  - **Why:** A modal that doesn't trap focus lets the user tab to elements behind it, which (a) is bad keyboard UX and (b) breaks the "only one thing on screen" contract the modal implies. Native `<dialog>` does most of this for free; if we use it, much of this rule is satisfied by the platform.
  - **How:** Use the [`useModalA11y`](../../frontend/src/hooks/useModalA11y.ts) hook for new overlays rather than re-rolling a trap — it provides the focus trap, Escape, and focus-restore-to-trigger (and suspends via an `active` flag when a full-screen sub-view owns focus). It's a hand-rolled hook rather than native `<dialog>` because jsdom doesn't implement `showModal()`'s trap / Escape / focus-restore, which would leave those behaviors untestable (§ 13; `typescript.md` § 12). The `Home` create-session modal and the `RunEntrySheet` bottom sheet consume it. For an overlay layered on top of another overlay (e.g. `RunEntrySheet`'s drink/setup pickers over the sheet), mark the parent `inert` and pass `active={false}` to suspend its trap while the child owns focus. Because making the parent `inert` synchronously blurs the focused trigger before the child mounts (and Safari never focuses a button on click), capture the trigger at click time (`e.currentTarget`) and hand it to the child's `useModalA11y` as `restoreFocusRef`, so focus restores to the real opener rather than `<body>`.
  - **Source:** <https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/> · <https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/dialog>

## 11. Routing (react-router 7)

- **Rule:** Use react-router 7 in **declarative mode** with `createBrowserRouter` + `RouterProvider`. Do *not* adopt framework mode (file-based routing, SSR config) — we're a static-served SPA.
  - **Why:** Framework mode buys SSR and file-based routes; we have neither. Declarative mode keeps the routing tree explicit and greppable.
  - **Source:** <https://reactrouter.com/start/modes> · <https://reactrouter.com/how-to/spa>

- **Rule:** Do *not* adopt route loaders/actions as the primary data layer. TanStack Query (§ 4) owns data fetching.
  - **Why:** Loaders shine when the router can prefetch on hover and parallelize requests across nested routes, paired with SSR for instant first paint. With a static-served SPA and polling, neither benefit lands — and putting fetching in two places (loaders *and* TanStack Query) creates a cache-coherence problem.

- **Rule:** Nest routes for shared layouts via `<Outlet />`. Top-level routes are code-split with `React.lazy` + `<Suspense>`.
  - **Why:** Route-based code splitting is the single highest-impact bundle optimization for an SPA (commonly 40–70% smaller initial bundle). On mobile networks the difference is felt on first paint.
  - **Example:**
    ```tsx
    const SessionPage = lazy(() => import('./pages/SessionPage'));

    const router = createBrowserRouter([
      {
        element: <AppShell />,
        errorElement: <RouteError />,
        children: [
          { path: '/', element: <Home /> },
          {
            path: '/sessions/:id',
            element: (
              <Suspense fallback={<PageSkeleton />}>
                <SessionPage />
              </Suspense>
            ),
          },
        ],
      },
    ]);
    ```
  - **Source:** <https://reactrouter.com/explanation/code-splitting>

- **Rule:** Use `<NavLink>` for navigation buttons (not `<button onClick={navigate}>`). Set `aria-current="page"` on the active link — `<NavLink>` does this for you when you pass a className function.
  - **Why:** `<NavLink>` is a real anchor — middle-click and cmd-click open in new tabs (browser-default behavior users expect), keyboard `Enter` activates, screen readers announce it as a link. `<button onClick={navigate}>` breaks all three.

- **Rule:** `useParams<{ id: string }>()` actually returns `Partial<{ id: string }>` under strict TS. Don't `params.id!`. Validate the route param and either narrow or redirect.
  - **Why:** This is the single most common source of `!` in the audit (five sites in `Session.tsx` alone). The fix is a one-liner: `if (!id) return <Navigate to="/" replace />;`.

## 12. File organization

- **Rule:** Current layout (`src/components/`, `src/pages/`, `src/hooks/`, `src/api/`, `src/utils/`) is fine for the project's current size. Do *not* migrate to feature folders until the app has ~30+ components and the flat layout starts hurting.
  - **Why:** Feature folders win when the codebase has clear feature boundaries that change together. Until then, the by-type layout is easier to navigate because every consumer knows where to look. Premature reorganization is churn for its own sake.

- **Rule:** Per-component types and helpers live in the component file (or a sibling `<Component>.types.ts` if the types are large and reused). Cross-component domain types live in `src/api/types.ts` (the wire-format mirror) or a new `src/domain/` directory for app-side types that don't cross the wire.
  - **Why:** Co-location: types that change together should live together. The current pattern of inline `interface Props` is fine; promote to a sibling file only when the types are reused by another component.
  - **Source:** <https://kentcdodds.com/blog/colocation>

- **Rule:** Promote-on-second-use for `src/components/` (truly generic UI primitives like `Button`, `Modal`) and `src/hooks/` (cross-feature hooks like `useDebounce`, `useMediaQuery`). Don't pre-create empty primitives folders.
  - **Why:** "We might need this someday" is the most common path to a `src/components/` directory full of one-off wrappers nobody reuses. Add primitives only when the second use case shows up.

- **Rule:** `src/pages/` holds route entry points. Pages are thin — they compose hooks and components rather than implementing logic directly. If a page accumulates substantial logic, that's a sign sub-components or hooks want extraction.

## 13. Testing

This section covers React-specific testing. The umbrella policy ("tests are a deliverable, not optional; every requirement should be unit- or integration-testable, within reason") lives in [`typescript.md`](./typescript.md) § 12, which also covers Vitest setup, schema/parser tests, and the "within reason" carve-outs. Read that first.

- **Rule:** Use [React Testing Library](https://testing-library.com/docs/react-testing-library/intro/) for component tests. Render with `render()`, query by semantic role/label/text (in priority order), interact with [`@testing-library/user-event`](https://testing-library.com/docs/user-event/intro) v14+.
  - **Why:** RTL forces tests written from the user's perspective — what's on screen, what they click, what they read. Tests that introspect state or props break on refactor without catching real bugs.
  - **Example:**
    ```tsx
    test('submitting the login form calls login with the entered credentials', async () => {
      const user = userEvent.setup();
      const onSubmit = vi.fn();
      render(<LoginForm onSubmit={onSubmit} />);

      await user.type(screen.getByLabelText('Username'), 'alice');
      await user.type(screen.getByLabelText('Password'), 'secret');
      await user.click(screen.getByRole('button', { name: /sign in/i }));

      expect(onSubmit).toHaveBeenCalledWith({ username: 'alice', password: 'secret' });
    });
    ```

- **Rule:** Query priority is `getByRole` first, then `getByLabelText` (for form inputs), then `getByText`, then `getByDisplayValue`. Reserve `getByTestId` for the rare case where no semantic option works.
  - **Why:** The query order mirrors how a screen reader navigates. Tests that pass for screen-reader users also satisfy the a11y rules in § 10. `getByTestId` selectors couple tests to implementation details and rot fastest.
  - **Source:** <https://testing-library.com/docs/queries/about#priority>

- **Rule:** Mock the network at the fetch boundary using [MSW (Mock Service Worker)](https://mswjs.io/), not by stubbing `fetch` or individual API helpers.
  - **Why:** MSW intercepts at the network layer, so the API client + Zod parsing + hooks all exercise real logic. Stubbing `fetch` forces each test to re-implement your wire format; stubbing API helpers bypasses the parsers that PR-B2 made loud-on-failure.
  - **Source:** <https://mswjs.io/docs/>

- **Rule:** Hook tests use `renderHook` from React Testing Library. Wrap with `QueryClientProvider` (for TanStack Query hooks), `AuthContext.Provider` (for auth-dependent hooks), and any other Provider the hook depends on.
  - **Example:**
    ```tsx
    function wrapper({ children }: { children: React.ReactNode }) {
      const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false } },
      });
      return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
    }

    test('useSession returns data once the request resolves', async () => {
      const { result } = renderHook(() => useSession(sessionId), { wrapper });
      await waitFor(() => expect(result.current.data).toBeDefined());
    });
    ```
  - **Source:** <https://testing-library.com/docs/react-testing-library/api/#renderhook>

- **Rule:** Don't test internal state, refs, or hook return-value shape directly — test the rendered output or the side effects (function calls, navigation, network requests). If you find yourself reaching for `act` to poke at internals, the test is fragile to refactor.
  - **Why:** RTL's whole pitch is "the more your tests resemble the way your software is used, the more confidence they can give you" (Kent C. Dodds). Internal-state tests fail the resemble-real-use test.

- **Rule:** Integration tests for major user flows live in `frontend/src/__tests__/` and exercise multi-screen sequences (login → create session → submit run) end-to-end against MSW. Cover the happy path plus the most common failure (auth expired, network down, validation rejection).
  - **Why:** Unit tests verify pieces in isolation; integration tests verify the pieces are wired together. The cost-benefit on integration tests is highest for the user flows that actually break the app when they fail.

- **Rule:** Tests for accessibility behavior (focus management, keyboard navigation, ARIA wiring) live next to the component they cover. Use `userEvent.tab()` to drive keyboard navigation; use `@axe-core/react` in a separate axe-only test pass rather than in every component test.
  - **Why:** Per-component axe runs slow test suites down. A dedicated axe pass on the rendered app catches regressions cheaply.

## Document history

- 2026-05-16 — Initial creation. Sourced from research conducted 2026-05-16 (React 19.2, React Compiler v1.0, TanStack Query v5, react-router 7 baselines). Recommendations on TanStack Query and React Compiler adoption are forward-looking — the project does not yet use either; the compliance plan (`../designs/2026-05-16-frontend-compliance-plan.md`) sequences the migration. Companion files: `typescript.md`, `tailwind.md`, both created same day.
- 2026-05-18 — Added § 13 Testing covering React Testing Library, MSW, hook tests with provider wrappers, query priority, and the rule against testing internals. Umbrella policy (mandate + carve-outs) lives in `typescript.md` § 12; this section is the React-specific patterns. Companion: compliance plan re-sequences PR-H2 (Vitest scaffolding) ahead of the runtime-behavior PRs so each subsequent PR can include tests.
- 2026-05-28 — § 7 Memoization: added carve-out (d) for references whose stability is load-bearing for *correctness* (a `useEffect` dependency or an external-/module-scope registration), distinct from the three performance carve-outs. The Compiler's memoization is an optimization, not a contract, so correctness must not depend on it — documents the reasoning behind keeping `useAuth`'s `clearAuth` an explicit `useCallback`. Surfaced by PR [#214](https://github.com/brendanbyrne/beerio-kart/pull/214) (PR-E2, Issue [#180](https://github.com/brendanbyrne/beerio-kart/issues/180)) review.
- 2026-05-31 — § 10 Accessibility floor: the Modals rule now points at the `useModalA11y` hook as the canonical overlay primitive (focus trap + Escape + focus-restore, suspendable for full-screen sub-views) rather than re-rolling a trap per modal, and records why it's hand-rolled instead of native `<dialog>` (jsdom can't exercise `showModal()`'s a11y behaviors, so native would force untestable assertions). Added with the hook in PR [#221](https://github.com/brendanbyrne/beerio-kart/pull/221) (PR-G2, Issue [#184](https://github.com/brendanbyrne/beerio-kart/issues/184)) on reviewer suggestion.
- 2026-06-14 — § 10 Accessibility floor: extended the Modals "How:" with the overlay-over-overlay pattern — mark the parent `inert` + `active={false}`, and capture the trigger at click time (`e.currentTarget`) to pass as the child's `restoreFocusRef`, because the inert flip blurs the focused trigger before the child mounts (so `document.activeElement` would restore to `<body>`). Companion to PR [#234](https://github.com/brendanbyrne/beerio-kart/pull/234) (Issue [#222](https://github.com/brendanbyrne/beerio-kart/issues/222)), which added the `restoreFocusRef` param.
