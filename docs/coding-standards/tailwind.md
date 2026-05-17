# Tailwind — Coding Standards

> **Scope.** Tailwind v4.x styling rules for the `beerio-kart` frontend. Mobile-first, utility-first, Firefox-compatible. Component-level patterns are in [`react.md`](./react.md); language rules in [`typescript.md`](./typescript.md).
> **Format.** Each rule: *Rule / Why / Example / Source*.
> **Companions.** `typescript.md`, `react.md`, `../user-workflows.md` (UI screen specs). Compliance plan: `../designs/2026-05-16-frontend-compliance-plan.md`.

## Index

1. [Utility-first ethos](#1-utility-first-ethos)
2. [Mobile-first breakpoints](#2-mobile-first-breakpoints)
3. [v4 CSS-first config (`@theme`)](#3-v4-css-first-config-theme)
4. [Conditional classes — `clsx`, not string concat](#4-conditional-classes--clsx-not-string-concat)
5. [Touch targets and reachability](#5-touch-targets-and-reachability)
6. [Dark mode](#6-dark-mode)
7. [Firefox & Safari compatibility](#7-firefox--safari-compatibility)
8. [When to escape Tailwind](#8-when-to-escape-tailwind)
9. [Arbitrary values](#9-arbitrary-values)
10. [Anti-patterns](#10-anti-patterns)

---

## 1. Utility-first ethos

- **Rule:** Compose styles from utility classes in JSX. Don't extract presentational components, mixins, or `@apply` rules until the pattern has appeared in at least three places.
  - **Why:** Tailwind's pitch is that the styling lives where the markup lives — one place to read, one place to change, zero hunting. Pulling rules into a stylesheet recreates the indirection Tailwind is designed to eliminate. The audit shows our usage is already utility-first; this rule codifies it.
  - **Source:** <https://tailwindcss.com/docs/utility-first> · <https://tailwindcss.com/docs/styling-with-utility-classes>

- **Rule:** When a class string repeats in three or more components, extract a React component (not an `@apply` rule). Pass props for the variants that differ.
  - **Why:** Components compose, can take props, and live in the same module system as the rest of the code. `@apply` rules live in CSS and lose the per-call-site flexibility Tailwind exists to provide.

## 2. Mobile-first breakpoints

- **Rule:** Default styles target the smallest screen (no prefix). Use `sm:`/`md:`/`lg:` to widen up — *never* `max-sm:`/`max-md:` to narrow down.
  - **Why:** The reference device is the Pixel 9 Pro (~427 × 952 CSS px, see [`frontend/CLAUDE.md`](../../frontend/CLAUDE.md) § UI reference device). The unprefixed layout has to look right on it; everything else is progressive enhancement. Mixing min-width and max-width breakpoints is a documented Tailwind footgun — the cascade gets non-obvious fast.
  - **Example:**
    ```tsx
    // good — mobile default, widens on tablet
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 md:grid-cols-3" />

    // bad — backwards reading: "three columns until it shrinks"
    <div className="grid grid-cols-3 gap-4 max-md:grid-cols-2 max-sm:grid-cols-1" />
    ```
  - **Source:** <https://tailwindcss.com/docs/responsive-design>

- **Rule:** Hover-dependent styling (`hover:`) is paired with `focus-visible:` and (where the interaction matters on touch) with `active:` or `aria-pressed:`. Don't ship UI whose interactivity is visible *only* on hover.
  - **Why:** Mobile-first means there is no hover. A button whose "current" state shows only as a hover ring is invisible on every phone the app actually runs on.

## 3. v4 CSS-first config (`@theme`)

Tailwind v4 (4.x) moved configuration from `tailwind.config.js` into CSS via `@theme` blocks. The v3 pattern — `theme.extend` in a JS file, `@tailwind base/components/utilities` directives — does not apply.

- **Rule:** Design tokens live in `frontend/src/index.css` inside an `@theme` block. The single import at the top of the file is `@import "tailwindcss";` (already correct; see the file).
  - **Why:** v4's CSS-first config is the supported path; it eliminates the JS↔CSS context switch and the JIT-vs-CSS-vars discrepancy that v3 had. The tokens become real CSS variables (`--color-brand-primary`) that arbitrary-value escapes can reach.
  - **Example:**
    ```css
    /* frontend/src/index.css */
    @import 'tailwindcss';

    @theme {
      --color-brand-primary: #2563eb;       /* blue-600, current primary */
      --color-brand-primary-hover: #1d4ed8; /* blue-700 */
      --color-success: #16a34a;             /* green-600 */
      --color-danger: #dc2626;              /* red-600 */

      --font-display: 'Inter', ui-sans-serif, system-ui;

      --spacing-touch: 2.75rem; /* 44px — minimum touch target */
    }

    /* Non-utility custom rules stay in the same file */
    .safe-area-pb {
      padding-bottom: env(safe-area-inset-bottom, 0px);
    }
    ```
    Tokens become utilities automatically: `bg-brand-primary`, `text-success`, `min-h-touch`.
  - **Source:** <https://tailwindcss.com/docs/upgrade-guide#changes-from-v3> · <https://tailwindcss.com/docs/theme>

- **Rule:** Add a token to `@theme` rather than scattering literal hex/rgb values when (a) the color/spacing/font has a semantic name (brand, success, danger), or (b) the value appears in more than two places. Continue to use stock Tailwind utilities (`bg-gray-50`, `text-gray-900`) for incidental UI.
  - **Why:** Semantic tokens mean a brand-color change is one CSS edit. Promoting *every* color to a token is overhead; the audit shows incidental grays are fine as stock palette values.

## 4. Conditional classes — `clsx`, not string concat

- **Rule:** Use [`clsx`](https://github.com/lukeed/clsx) (or its sibling `classnames`) for conditional class composition. Do not string-concatenate or use nested template literals.
  - **Why:** Template-literal class concat with ternaries (the current pattern in `BottomNav.tsx`, `DrinkTypeSelector.tsx`, `Profile.tsx`, etc.) gets unreadable fast and is bug-prone — a missing space joins two classes silently. `clsx` is 200 bytes, handles falsy values cleanly, and produces a clear visual hierarchy.
  - **Example:**
    ```tsx
    import clsx from 'clsx';

    // good
    <button
      className={clsx(
        'min-h-touch rounded-lg px-4 font-medium',
        isActive    && 'bg-brand-primary text-white',
        !isActive   && 'bg-gray-100 text-gray-700',
        disabled    && 'opacity-50 pointer-events-none',
      )}
    />

    // bad — readability and spacing-bug risk
    <button
      className={`min-h-touch rounded-lg px-4 font-medium ${
        isActive ? 'bg-brand-primary text-white' : 'bg-gray-100 text-gray-700'
      } ${disabled ? 'opacity-50 pointer-events-none' : ''}`}
    />
    ```
  - **Source:** <https://github.com/lukeed/clsx>

- **Rule:** When a component has more than ~3 variant axes (e.g., `variant × size × tone`), use [`class-variance-authority`](https://cva.style) (`cva`) for variant management. Below that threshold, `clsx` is enough.
  - **Why:** `cva` is the right tool for a `Button` component with `variant: 'primary' | 'ghost'`, `size: 'sm' | 'md' | 'lg'`, `tone: 'neutral' | 'danger'` — that's 12 combinations that `clsx` ternaries handle poorly. For one-off conditional styling on a page, `cva` is overkill.
  - **Source:** <https://cva.style/docs>

- **Rule:** Never construct class names from data at runtime (`bg-${color}-500`). Use a lookup map of full class strings instead.
  - **Why:** Tailwind v4's compiler is still scan-based — it only emits classes that appear as literal substrings in source. Dynamic concatenation produces classes the compiler never sees, which means they don't end up in the CSS bundle. The lookup-map pattern keeps every class as a literal.
  - **Example:**
    ```tsx
    // bad — bg-blue-500 never emitted if no other file uses it as a literal
    <div className={`bg-${color}-500`} />

    // good
    const tone = {
      success: 'bg-green-500',
      warning: 'bg-yellow-500',
      danger:  'bg-red-500',
    } as const;
    <div className={tone[status]} />
    ```
  - **Source:** <https://tailwindcss.com/docs/detecting-classes-in-source-files>

## 5. Touch targets and reachability

- **Rule:** Every interactive element (button, link, toggle, slider thumb, sheet handle) has a hit area of at least 44 × 44 CSS pixels. The standard utility for this is `min-h-touch` (44 px via the `--spacing-touch` token in § 3) plus enough horizontal padding to clear 44 px width.
  - **Why:** WCAG 2.1 AA Success Criterion 2.5.5 (Target Size — Enhanced) is 44 × 44 px; Apple HIG and Material both echo it. The audit found several violations: `py-2 text-xs` cancel/confirm buttons (`DrinkTypeSelector.tsx`, `Profile.tsx`, `Home.tsx`), step-pill buttons in `RaceSetupPicker.tsx`. `py-2.5` produces ~40 px which is borderline. `py-3` (or `min-h-touch`) is the safe floor.
  - **Source:** <https://www.w3.org/WAI/WCAG21/Understanding/target-size.html>

- **Rule:** Bottom-nav and bottom-sheet UI uses `safe-area-pb` (see `frontend/src/index.css`) to clear the iOS home indicator and the Pixel gesture bar.
  - **Why:** Without it, the bottom row of interactive elements overlaps the system gesture area on notched phones and becomes either unreachable or accidentally activated.

- **Rule:** Single-handed use is a design principle (`docs/design.md`). Place primary actions in the lower two-thirds of the screen, within thumb reach. Bottom nav lives at the bottom for this reason.
  - **Why:** The Pixel 9 Pro's 952 px logical height puts the top third out of thumb range for one-handed grip. Top-anchored actions are a usability regression on the reference device.

## 6. Dark mode

- **Rule:** Dark mode is out of scope for the current milestone. When it's added, use the v4 `@variant` model with class-based switching (`<html class="dark">`) and a `prefers-color-scheme` fallback.
  - **Why:** The user-controlled toggle pattern is what most apps want (matches OS default *and* lets users override). Keep this rule on file so future dark-mode work doesn't reach for the v3 `darkMode: 'class'` config-file approach, which no longer applies.
  - **Source:** <https://tailwindcss.com/docs/dark-mode>

## 7. Firefox & Safari compatibility

- **Rule:** Firefox is a required target ([`docs/design.md`](../design.md) § Technical Constraints). Don't use Chrome-only CSS or `-webkit-` prefixes without Firefox equivalents. Test on Firefox before shipping any UI change.
  - **Why:** This is a project-wide constraint, not a Tailwind-specific one — but Tailwind utilities sometimes alias to vendor-prefixed properties (`overscroll-contain`, `backdrop-blur`, scroll-behavior utilities) whose Firefox support has historically lagged Chrome. Check [caniuse](https://caniuse.com) before adopting any utility whose underlying property name you don't recognize.

- **Rule:** iOS Safari has the largest mobile-browser quirks surface (viewport units, `100vh` issues, `position: fixed` + virtual keyboard, scroll bounce). When designing fixed/sticky UI, account for `dvh`/`svh` over `vh`, and test against iOS Safari before merging.
  - **Why:** `vh` on iOS measures the largest possible viewport (with URL bar collapsed); `dvh` ("dynamic viewport height") tracks the current viewport. Bottom-sheet UIs that use `h-[92vh]` or similar will draw off-screen with the URL bar visible.
  - **Source:** <https://developer.mozilla.org/en-US/docs/Web/CSS/length#viewport-percentage_lengths>

## 8. When to escape Tailwind

- **Rule:** Reach for plain CSS (in `frontend/src/index.css` or a sibling `.module.css`) only when (a) the utility doesn't exist and an arbitrary value (§ 9) would be ugly, (b) you need a `@keyframes` animation, (c) you need a media query Tailwind doesn't expose. The existing `.safe-area-pb` is the model — small, named, lives in `index.css`.
  - **Why:** Tailwind covers 95% of needs; the remaining 5% is real and trying to force it into utilities produces unreadable arbitrary-value chains. Plain CSS in `index.css` with a clear class name is the cleaner answer.

- **Rule:** Inline `style={{ ... }}` is allowed only for *runtime-computed* values — DOM-measured positions, animation progress, dynamic dimensions. Static styling goes through utilities.
  - **Why:** The slide-to-confirm in `RunEntrySheet.tsx` legitimately needs `style={{ width: thumbW, left: offsetX + 4 }}` because both depend on a measured track width. Static `style={{ maxHeight: '92%' }}` (also in `RunEntrySheet.tsx`) should be `max-h-[92%]` instead.

## 9. Arbitrary values

- **Rule:** Use arbitrary-value syntax (`min-h-[44px]`, `mt-[3px]`, `bg-[#1d4ed8]`) for one-off measurements that don't fit the scale. Promote to a `@theme` token if the value appears in more than two places.
  - **Why:** Arbitrary values are the v3-and-up escape hatch and are correctly used in the existing code (`min-h-[52px]` in `BottomNav.tsx`, `max-h-[80vh]` in modal). The "promote on third use" rule keeps `@theme` from accumulating one-off tokens.
  - **Source:** <https://tailwindcss.com/docs/adding-custom-styles#using-arbitrary-values>

## 10. Anti-patterns

| Anti-pattern | Why bad | Use instead |
|---|---|---|
| `@apply` to extract presentational components | Recreates CSS-class indirection Tailwind exists to eliminate | Extract a React component |
| `bg-${color}-500` runtime concat | Compiler doesn't see the literal; class missing from bundle | Lookup map of full class strings |
| Template-literal class concat with ternaries | Bug-prone (missing space), hard to read | `clsx` |
| `max-sm:` / `max-md:` to narrow down | Mixes min/max breakpoints; cascade gets confusing | Default to smallest, widen with `sm:`/`md:` |
| Hover-only interactive feedback | Invisible on touch — and we're mobile-first | Add `focus-visible:` and `active:` siblings |
| `style={{ marginTop: '4px' }}` for static values | Bypasses the design-token system | `mt-1` (or arbitrary `mt-[4px]`) |
| `tailwind.config.js` | v3 config, not used by v4 | `@theme` block in `index.css` |
| `vh` for full-height mobile layout | Wrong on iOS Safari | `dvh` (dynamic), `svh` (small), or measured height |
| Buttons under 44 × 44 px | Fails WCAG 2.1 AA target-size | `min-h-touch` (or `min-h-[44px]`) + adequate padding |

## Document history

- 2026-05-16 — Initial creation. Sourced from Tailwind v4 documentation, project audit conducted 2026-05-16, and design principles in `docs/design.md`. Companion files: `typescript.md`, `react.md`, both created same day. Reference device (Pixel 9 Pro) and Firefox constraint sourced from `frontend/CLAUDE.md` and `docs/design.md` § Technical Constraints respectively.
