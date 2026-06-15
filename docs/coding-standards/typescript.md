# TypeScript — General Coding Standards

> **Scope.** General TypeScript patterns for the `beerio-kart` frontend. TypeScript 5.9+, ES2023+ target, Vite + esbuild toolchain, ESLint flat config. Language-level rules only — React-specific patterns are in [`react.md`](./react.md), styling rules in [`tailwind.md`](./tailwind.md).
> **Format.** Each rule: *Rule / Why / Example / Source*.
> **Companions.** `react.md`, `tailwind.md`, `../api-contract.md`. Compliance plan (complete, archived): `../designs/archive/2026-05-16-frontend-compliance-plan.md`.

## Index

1. [tsconfig strictness](#1-tsconfig-strictness)
2. [Type system — `type` vs `interface`, `satisfies`, `as`](#2-type-system--type-vs-interface-satisfies-as)
3. [Branded types](#3-branded-types)
4. [Discriminated unions & exhaustiveness](#4-discriminated-unions--exhaustiveness)
5. [Module & export style](#5-module--export-style)
6. [Error handling](#6-error-handling)
7. [Async patterns & cancellation](#7-async-patterns--cancellation)
8. [Runtime validation at the API boundary](#8-runtime-validation-at-the-api-boundary)
9. [Lints, formatter, editor config](#9-lints-formatter-editor-config)
10. [Anti-patterns](#10-anti-patterns)
11. [Backend interop](#11-backend-interop)
12. [Testing](#12-testing)

---

## 1. tsconfig strictness

The configuration in `frontend/tsconfig.app.json` and `frontend/tsconfig.node.json` is the source of truth. Both files share the same strictness floor.

- **Rule:** `"strict": true` plus `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noImplicitOverride`, `noFallthroughCasesInSwitch`, `isolatedModules`, and `verbatimModuleSyntax`. All seven are required.
  - **Why:** `strict` enables the eight original strict flags (`noImplicitAny`, `strictNullChecks`, etc.) but does *not* include the index-access or exact-optional checks, which catch the two highest-frequency runtime bugs in React apps: array/record lookups assumed to be defined, and `{ foo: undefined }` being silently distinct from `{}`. TS 5.9's `tsc --init` template ships `isolatedModules` and `verbatimModuleSyntax` because they're required for per-file transpilers (Vite/esbuild) — match that default.
  - **Example:**
    ```jsonc
    // tsconfig.app.json
    {
      "compilerOptions": {
        "strict": true,
        "noUncheckedIndexedAccess": true,
        "exactOptionalPropertyTypes": true,
        "noImplicitOverride": true,
        "noFallthroughCasesInSwitch": true,
        "isolatedModules": true,
        "verbatimModuleSyntax": true
      }
    }
    ```
  - **Source:** <https://www.typescriptlang.org/tsconfig/> · <https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-9.html> · <https://www.typescriptlang.org/tsconfig/noUncheckedIndexedAccess.html>

- **Rule:** `noUncheckedSideEffectImports: true` and `erasableSyntaxOnly: true` are also on.
  - **Why:** `noUncheckedSideEffectImports` makes `import './foo.css'` fail typecheck if `./foo.css` isn't a declared module — caught at compile time instead of at bundle time. `erasableSyntaxOnly` enforces that no TS syntax survives transpilation as runtime constructs (it bans `enum`, `namespace` with runtime emit, parameter properties), which is the precondition for Node 22+ native TS execution and matches what we lint in § 10.
  - **Source:** <https://www.typescriptlang.org/tsconfig/noUncheckedSideEffectImports.html> · <https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-8.html#--erasablesyntaxonly>

- **Rule:** Do not weaken `strict` per-file via `// @ts-nocheck` or per-block via `// @ts-ignore`. If something genuinely can't typecheck, use `// @ts-expect-error: <reason>`.
  - **Why:** `@ts-expect-error` fails the build *when the underlying error goes away*, which prevents zombie suppressions. `@ts-ignore` is silently still suppressing the day someone else fixes the upstream type. See § 10 for the lint rule.
  - **Source:** <https://typescript-eslint.io/rules/ban-ts-comment/>

## 2. Type system — `type` vs `interface`, `satisfies`, `as`

- **Rule:** Default to `type`. Use `interface` only when you genuinely need declaration merging (rare in app code; common when augmenting third-party types like `Window` or `ImportMetaEnv`) or when extending a long chain of base types where TS's interface-inheritance fast path measurably matters (verified by profiler, not assumed).
  - **Why:** `type` expresses everything (unions, mapped, conditional, tuples, primitives) while `interface` silently merges across declarations — which is a footgun in app code. The handbook's old "prefer interface" guidance is outdated; current community consensus (Matt Pocock, the TS perf wiki) is `type` by default. Use `consistent-type-definitions: ["error", "type"]` to enforce.
  - **Example:**
    ```ts
    type Run = { id: RunId; trackId: TrackId; timeMs: number };

    // interface only when extending into Window / Module / etc.
    // Global augmentation must be inside `declare global` when the file is a module:
    declare global {
      interface Window {
        __BEERIOKART_VERSION__: string;
      }
    }
    ```
  - **Source:** <https://www.totaltypescript.com/type-vs-interface-which-should-you-use> · <https://github.com/microsoft/TypeScript/wiki/Performance#preferring-interfaces-over-intersections>

- **Rule:** Use `satisfies` to validate a value against a type without widening. Use `as` only to tell the compiler something it can't verify (DOM event narrowing, branded-type minting at a trusted boundary). Use a plain type annotation when you want the wider type to win.
  - **Why:** Annotations widen (lose literal types), `as` lies (no checking), `satisfies` checks-but-preserves. This is the right tool for config objects, route maps, `as const` shape enforcement, and tuple-like literals.
  - **Example:**
    ```ts
    const routes = {
      home: '/',
      session: '/sessions/:id',
    } satisfies Record<string, `/${string}`>;
    // routes.home is the literal '/', not string — usable as a typed route key.
    ```
  - **Source:** <https://www.typescriptlang.org/docs/handbook/release-notes/typescript-4-9.html> · <https://www.totaltypescript.com/clarifying-the-satisfies-operator>

- **Rule:** `as` casts and non-null assertions (`!`) require a one-line justifying comment, and the lint rules `no-explicit-any`, `no-non-null-assertion`, and `consistent-type-assertions: { assertionStyle: "as", objectLiteralTypeAssertions: "never" }` are on.
  - **Why:** Most `!` is "I know better than the compiler" and is wrong eventually (see `Session.tsx:13` in the audit, where `useParams<{ id: string }>()` actually returns `Partial<{ id: string }>`). Most `as` casts silently break when the source type changes. Both should be visible and rare.
  - **Source:** <https://typescript-eslint.io/rules/no-non-null-assertion/> · <https://typescript-eslint.io/rules/consistent-type-assertions/>

- **Rule:** Prefer `T | null` over `T | undefined` when modeling "explicitly absent." Reserve `undefined` for "this property is missing entirely" (consistent with `exactOptionalPropertyTypes`).
  - **Why:** `exactOptionalPropertyTypes` makes `{ foo?: string }` strictly distinct from `{ foo: string | undefined }` — the first omits the key, the second sets it to `undefined`. Picking one convention prevents both shapes leaking into the same DTO.

## 3. Branded types

The structural type system has no built-in way to say "`SessionId` and `UserId` are both strings but you cannot pass one where the other is expected." We solve it with phantom-property brands, applied at the API boundary.

- **Rule:** Every domain identifier crossing a module boundary is branded. Centralize the brand helper.
  - **Why:** The Rust backend has `nutype` newtypes for every ID, every username, every race-time. Without branded mirrors on the TS side, the type-safety stops at the wire. Branded types are zero-cost (a phantom symbol property erased at runtime), JSON-round-trippable (the value is still a plain string/number), and produce tolerable error messages.
  - **Example:**
    ```ts
    // src/api/brand.ts
    declare const brand: unique symbol;
    export type Brand<T, B> = T & { readonly [brand]: B };

    // src/api/ids.ts
    export type UserId    = Brand<string, 'UserId'>;
    export type SessionId = Brand<string, 'SessionId'>;
    export type RunId     = Brand<string, 'RunId'>;
    export type RaceId    = Brand<string, 'RaceId'>;

    export type CharacterId = Brand<number, 'CharacterId'>;
    export type TrackId     = Brand<number, 'TrackId'>;
    // ... and so on for each numeric ID

    // Constructor — the only place an unbranded value gains the brand.
    // Used by the runtime parser (§ 8), not scattered through call sites.
    export const UserId = (s: string): UserId => s as UserId;
    ```
  - **Source:** <https://www.typescriptlang.org/play/typescript/language-extensions/nominal-typing.ts.html> · <https://www.totaltypescript.com/workshops/advanced-typescript-patterns/branded-types/using-branded-types-as-entity-id-s/solution>

- **Rule:** Brand at the runtime-parse boundary (see § 8), not at each call site. Inside the typed API layer, IDs are already branded; consumers receive `SessionId`, not `string`.
  - **Why:** "Mint where you validate" is the same principle as Rust's *parse, don't validate*. If every component re-brands via `s as SessionId`, you've reintroduced the unsafety the brand exists to prevent.

- **Rule:** Do not brand bools or `Date`. Reserve brands for primitives whose context is genuinely ambiguous (string-shaped IDs, numeric-shaped IDs, time durations in ms vs s).
  - **Why:** Mirrors `rust.md` § 2's "don't newtype bools" rule. Brand cost is real (every consumer sees the phantom in error messages); apply it where the ambiguity is real.

## 4. Discriminated unions & exhaustiveness

- **Rule:** Model "this thing is one of several shapes" as a discriminated union keyed on a literal `kind` field. Use `kind` as the discriminator name to match the Rust backend's serde tag convention.
  - **Why:** DUs let TS narrow the *whole shape* (not just the tag) and align directly with serde's `#[serde(tag = "kind")]` output — zero translation logic at the boundary. The existing notification payload types around `frontend/src/api/types.ts:170-185` are already structured this way (single-variant today, expandable to a full union) and are the model for everything else.
  - **Example:**
    ```ts
    type SessionStatus =
      | { kind: 'open'; participantCount: number }
      | { kind: 'in_progress'; currentRaceId: RaceId }
      | { kind: 'closed'; endedAt: string };
    ```
  - **Source:** <https://www.typescriptlang.org/docs/handbook/2/narrowing.html#discriminated-unions>

- **Rule:** Exhaustiveness check unions with a `never`-typed default arm.
  - **Why:** Adding a variant later (e.g., a new session status) becomes a compile error at every consumer instead of a silent fallthrough. Same role as `#[non_exhaustive]` + match-arm wildcards on the Rust side.
  - **Example:**
    ```ts
    function label(s: SessionStatus): string {
      switch (s.kind) {
        case 'open':         return `Open · ${s.participantCount}`;
        case 'in_progress':  return 'Racing';
        case 'closed':       return 'Closed';
        default: {
          const _exhaustive: never = s;
          return _exhaustive;
        }
      }
    }
    ```

- **Rule:** Do not use the TS `enum` keyword. Use `as const` objects or string-literal unions instead.
  - **Why:** `enum` emits runtime code; numeric enums are not type-safe (any number assigns); `const enum` breaks under `isolatedModules`/`verbatimModuleSyntax` — exactly the toolchain we use. ESLint's `no-restricted-syntax` covers it; the project is already enum-free (audited 2026-05-16).
  - **Example:**
    ```ts
    // Instead of `enum Cup { Mushroom, Flower }`:
    const Cup = { Mushroom: 'mushroom', Flower: 'flower' } as const;
    type Cup = (typeof Cup)[keyof typeof Cup]; // 'mushroom' | 'flower'
    ```
  - **Source:** <https://www.typescriptlang.org/tsconfig/isolatedModules.html#const-enums>

## 5. Module & export style

- **Rule:** Named exports only. One primary export per file, file name matches the export. No default exports anywhere — pages, components, hooks, utilities, API helpers.
  - **Why:** Default exports break rename-refactors (the importer picks an arbitrary name; grep fails), break tree-shaking heuristics, and let two files declare the same default with no compiler help. Enforced via `import/no-default-export`.
  - **Example:**
    ```ts
    // Good
    export function SessionPanel() { /* ... */ }
    // bad
    export default function SessionPanel() { /* ... */ }
    ```
  - **Source:** <https://github.com/import-js/eslint-plugin-import/blob/main/docs/rules/no-default-export.md>

- **Rule:** Type-only imports use `import type`; type-only exports use `export type`. The `verbatimModuleSyntax` flag (§ 1) enforces this — imports without `type` are emitted verbatim, imports *with* `type` are dropped, no "elide on emit" guesswork.
  - **Why:** Per-file transpilers like esbuild don't have a whole-program view. Without `verbatimModuleSyntax` they sometimes leave dead `import { Foo }` lines that, at runtime, reach for an export that was type-only — a circular-import footgun. The verbatim model is the only one that's correct under per-file transpilation.
  - **Example:**
    ```ts
    import { fetchRuns } from './api';
    import type { Run, RunId } from './api/types';
    export type { Run };
    ```
  - **Source:** <https://www.typescriptlang.org/tsconfig/verbatimModuleSyntax.html>

- **Rule:** Do not include `.js` extensions in source-relative imports. Vite/esbuild resolves `.ts` and `.tsx` for you.
  - **Why:** `.js` extensions are only required when you're targeting raw Node ESM with no bundler. Adding them under Vite creates path-noise and breaks `tsc`-driven IDEs that resolve via the TS file map.
  - **Source:** <https://vite.dev/guide/features.html#typescript>

- **Rule:** Module path aliases (`@/components/...`) go through `vite.config.ts` `resolve.alias` *and* `tsconfig.app.json` `paths` so both the bundler and the type-checker agree.
  - **Why:** If only the bundler knows, `tsc --noEmit` fails. If only `tsc` knows, the dev server breaks. Either set both or neither.

## 6. Error handling

The split mirrors the backend's: *expected* failures are values, *unexpected* failures throw and are caught at the boundary.

- **Rule:** Use a small `Result<T, E>` shape for *expected* domain failures returned from API helpers — validation rejections, 4xx responses, parse errors. Use `throw` for *unexpected* failures (network down, 5xx, programmer error) and catch them at an error boundary (see [`react.md`](./react.md) § Error boundaries).
  - **Why:** Results make handleable failure visible in the signature — you can't forget to handle a domain error. But wrapping every fetch in Result is noisy; for *exceptional* paths, `throw` + boundary is simpler. The error-code envelope from `api-contract.md` § 7 is the natural shape for the `E` side of `Result`.
  - **Example:**
    ```ts
    export type Result<T, E> =
      | { ok: true; value: T }
      | { ok: false; error: E };

    // The backend already emits `{ error, code }` per api-contract.md § 7.
    // ApiError is a discriminated union on the registry's code values:
    export type ApiError =
      | { code: 'invalid_credentials'; message: string }
      | { code: 'username_taken'; message: string }
      | { code: 'not_found';           message: string }
      // ... one variant per registry entry
      | { code: 'unknown'; message: string }; // fallback for unmapped codes

    export async function login(
      username: string, password: string,
    ): Promise<Result<AuthResponse, ApiError>> {
      const res = await fetch('/api/auth/login', { /* ... */ });
      if (res.ok) return { ok: true, value: await parseAuth(res) };
      return { ok: false, error: await parseError(res) };
    }
    ```

- **Rule:** When you re-throw, preserve the chain with `Error.cause`.
  - **Why:** `Error.cause` (ES2022, baseline-supported in every target browser) keeps the original stack reachable through wrapping. Lose-the-cause re-throws are the frontend equivalent of `unwrap()`-and-pray.
  - **Example:**
    ```ts
    try {
      return await loadRun(id);
    } catch (e) {
      throw new Error(`loadRun(${id}) failed`, { cause: e });
    }
    ```
  - **Source:** <https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error/cause>

- **Rule:** Use a discriminated union for `ApiError`, not `Error` subclasses.
  - **Why:** Discriminated unions travel naturally over JSON from the backend (which already serializes its `ErrorCode` enum exactly this way). `instanceof MyError` doesn't survive a `JSON.parse` round-trip; a `kind`/`code` field does.

- **Rule:** Error-message strings constructed in TS follow the same convention as `rust.md` § 1: start with a capital letter, no trailing punctuation. User-facing copy may diverge if the design calls for it (sentence punctuation in long-form error messages), but the internal `Error.message` we synthesize for logs follows the rule.
  - **Why:** Consistency with the backend so `error.message` strings read identically across the wire.

## 7. Async patterns & cancellation

- **Rule:** Every fetch accepts an `AbortSignal` and passes it through to `fetch`. Hooks that initiate fetches create an `AbortController` in their effect and `controller.abort()` in cleanup.
  - **Why:** Without cancellation, navigating away mid-fetch triggers a `setState` on an unmounted component (React 19 dev warns; Strict Mode double-mount surfaces it loudly). With TanStack Query (see `react.md` § Data fetching), cancellation is built in — but our underlying `fetch` wrapper still needs to honor the signal.
  - **Example:**
    ```ts
    export async function fetchSession(
      id: SessionId, signal?: AbortSignal,
    ): Promise<Result<SessionDetail, ApiError>> {
      const res = await apiFetch(`/api/sessions/${id}`, { signal });
      // ...
    }
    ```
  - **Source:** <https://developer.mozilla.org/en-US/docs/Web/API/AbortController> · <https://tanstack.com/query/latest/docs/framework/react/guides/query-cancellation>

- **Rule:** `Promise.all` when failures should short-circuit (loading a page that needs all data). `Promise.allSettled` when each task is independent (parallel widgets, fan-out telemetry).
  - **Why:** `Promise.all` rejects as soon as any input rejects — the others keep running but their results are lost. `allSettled` waits and reports each outcome. Picking the wrong one silently loses errors or silently loses data.
  - **Source:** <https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Promise/allSettled>

- **Rule:** No floating promises. `@typescript-eslint/no-floating-promises` is on. If you genuinely want fire-and-forget, write `void promise()` explicitly.
  - **Why:** A dropped promise rejection becomes an unhandled rejection at the window level — invisible during local dev unless you check the console, fatal in production telemetry. The explicit `void` documents intent.
  - **Source:** <https://typescript-eslint.io/rules/no-floating-promises/>

## 8. Runtime validation at the API boundary

- **Rule:** Every response read from `fetch` is parsed through a runtime schema before being treated as typed data. Use [Zod](https://zod.dev) (`zod` package) for the schemas; the TS types are inferred from the schemas, not declared separately.
  - **Why:** `await res.json()` returns `any`. Assigning it to `SessionDetail` is a lie — the compiler stops checking and the next time the backend renames a field, the bug appears at the first downstream read, far from the cause. Zod parses *and* infers, so the type definition and the runtime check stay in lockstep. This is also where branded IDs (§ 3) get minted: the schema transforms `string` → `UserId` once, at the boundary, and consumers see `UserId` everywhere downstream.
  - **Example:**
    ```ts
    import { z } from 'zod';

    const SessionDetailSchema = z.object({
      id: z.string().transform((s) => s as SessionId),
      status: z.enum(['open', 'in_progress', 'closed']),
      participants: z.array(z.object({
        user_id: z.string().transform((s) => s as UserId),
        username: z.string(),
      })),
    });

    export type SessionDetail = z.infer<typeof SessionDetailSchema>;

    export async function fetchSession(id: SessionId, signal?: AbortSignal) {
      const raw = await apiFetch(`/api/sessions/${id}`, { signal });
      return SessionDetailSchema.parse(raw);
    }
    ```
  - **Source:** <https://zod.dev/?id=basic-usage>

- **Rule:** Validation failures are surfaced as `ApiError { code: 'response_shape_mismatch', ... }` and treated as a programmer error (logged, error boundary, fail-loud) — not silently coerced.
  - **Why:** A schema mismatch means the contract drifted. Silently coercing means the bug ships. Loud failure surfaces drift on the first dev run.

- **Rule:** Do not use Zod for form input validation today. Native HTML validation (`required`, `pattern`, `type="email"`) plus a one-shot schema check at submit is sufficient for our form complexity. Revisit if forms grow live cross-field validation.
  - **Why:** Form-validation libraries (React Hook Form + Zod resolver) are a real adoption with real bundle cost; we're not paying for a problem we don't have. The audit shows our forms are small and submit-time-validated already.

## 9. Lints, formatter, editor config

- **Rule:** ESLint flat config (`eslint.config.js`) extending `typescript-eslint`'s `strictTypeChecked` and `stylisticTypeChecked` presets. Configure `parserOptions.projectService: true` so type-aware rules pick up the project automatically. Required additional plugins: `eslint-plugin-react-hooks` (recommended), `eslint-plugin-jsx-a11y` (recommended; see `react.md` § Accessibility), `eslint-plugin-import` (for `no-default-export` and `consistent-type-specifier-style`), `eslint-config-prettier` (to disable rules Prettier owns).
  - **Why:** `strict-type-checked` is the superset of `recommended` + `strict` + the type-aware rules that catch the high-value bugs (`no-floating-promises`, `no-misused-promises`, `no-unsafe-*`, `await-thenable`). `stylistic-type-checked` adds cheap consistency rules. The type-checked variants are slower (TS has to build the project) but worth it for app code; opt out per-file with overrides if a generated file makes it impractical.
  - **Example:**
    ```js
    // eslint.config.js
    import tseslint from 'typescript-eslint';
    import reactHooks from 'eslint-plugin-react-hooks';
    import jsxA11y from 'eslint-plugin-jsx-a11y';
    import importPlugin from 'eslint-plugin-import';
    import prettier from 'eslint-config-prettier';

    export default tseslint.config(
      ...tseslint.configs.strictTypeChecked,
      ...tseslint.configs.stylisticTypeChecked,
      reactHooks.configs.recommended,
      jsxA11y.flatConfigs.recommended,
      importPlugin.flatConfigs.recommended,
      prettier,
      {
        languageOptions: { parserOptions: { projectService: true } },
        rules: {
          '@typescript-eslint/consistent-type-definitions': ['error', 'type'],
          '@typescript-eslint/consistent-type-imports': 'error',
          'import/no-default-export': 'error',
          'no-restricted-syntax': [
            'error',
            { selector: 'TSEnumDeclaration',
              message: 'Use `as const` objects or string-literal unions instead of enum.' },
          ],
        },
      },
    );
    ```
  - **Source:** <https://typescript-eslint.io/users/configs/> · <https://typescript-eslint.io/getting-started/typed-linting/>

- **Rule:** Key rules to confirm on: `no-floating-promises`, `no-misused-promises`, `await-thenable`, `no-explicit-any`, `no-non-null-assertion`, `consistent-type-imports`, `ban-ts-comment`, `no-unnecessary-condition`, `consistent-type-definitions: ["error", "type"]`, `import/no-default-export`, `react-hooks/rules-of-hooks`, `react-hooks/exhaustive-deps`, `jsx-a11y/recommended`.
  - **Why:** These are the rules that defend the standard. Disabling any of them mid-file requires an inline `// eslint-disable-next-line <rule>: <reason>` comment.

- **Rule:** Prettier owns formatting. Settings in `frontend/.prettierrc` (default config + `singleQuote: true`, `semi: true`, `trailingComma: 'all'`). Both Prettier and ESLint run pre-commit via lefthook.
  - **Why:** No bikeshedding on style; the formatter resolves it. `trailingComma: 'all'` minimizes diff noise on list edits.
  - **Source:** <https://prettier.io/docs/options>

- **Rule:** `frontend/.editorconfig` (or root `.editorconfig`) enforces LF endings, UTF-8, final newline, two-space indent for `*.{ts,tsx,js,jsx,css,json}` and tab indent for `*.go`/`Makefile`-style files (none in this project, but documented).
  - **Why:** Mirrors `rust.md` § 16. LF endings are required by the cross-cutting `CLAUDE.md` convention.

## 10. Anti-patterns

| Anti-pattern | Why bad | Use instead |
|---|---|---|
| `any` | Silently disables type-checking; infects callers via `no-unsafe-*` | `unknown` + narrow, or a Zod schema |
| `x!` non-null assertion | Crashes at runtime if wrong; hides nullability bugs | Narrow with `if (x)`, use `??`, or change the source type |
| `x as T` (type assertion) | Lies to the compiler | `satisfies T`, a Zod parse, or a type guard |
| `// @ts-ignore` | Silently rots — masks unrelated errors that surface later | `// @ts-expect-error: <reason>` |
| `Function` type | Equivalent to `any` for arguments/return | `(...args: Args) => Ret` |
| `Object` / `{}` | Means "anything except null/undefined" | `Record<string, unknown>` or a real shape |
| TS `enum` | Runtime emit, broken under per-file transpile | `as const` object + `keyof typeof` |
| `useEffect` to fetch / derive | See [`react.md`](./react.md) § Effects | TanStack Query / `useMemo` / derived state |
| `await res.json()` typed as a DTO | Untyped, no runtime check | Zod schema + `parse` |
| String-concatenated class names | Hard to read, drifts | `clsx` (see `tailwind.md`) |

Each row in this table maps to an ESLint rule listed in § 9.

## 11. Backend interop

- **Rule:** The wire format defined in `../api-contract.md` is the source of truth. TS types in `frontend/src/api/types.ts` must match it field-for-field, including snake_case field names (we do *not* rename to camelCase at the boundary; the wire shape is established by the endpoint examples throughout `api-contract.md` and the existing `frontend/src/api/types.ts`).
  - **Why:** Renaming at the boundary adds translation code on every request and creates a third name for every concept (Rust field, wire field, TS field). Match the wire and the cost is zero.

- **Rule:** Discriminator field is `kind` to match the backend's `#[serde(tag = "kind")]` convention.
  - **Why:** Zero translation logic at the parse step. Already in use for `NotificationPayload`; extend to every other tagged enum.

- **Rule:** Error envelope is `{ error: string, code: ErrorCode }` per `api-contract.md` § 7. The TS `ApiError` union derives its `code` values directly from the registry; treat additions to the registry as a frontend breaking change that requires updating the union.
  - **Why:** The `code` is the machine-readable contract; relying on `error` text for branching is the anti-pattern the registry exists to eliminate.

- **Rule:** Type-sync between Rust DTOs and TS DTOs is currently hand-maintained, with the Zod schemas (per § 8) as the source of truth and TS types derived via `z.infer<typeof Schema>`. Drift is guarded by a cheap CI check (next rule), not by codegen. The full decision — current state, options considered, adoption trigger — lives in [ADR 0039](../decisions/0039-api-client-generation.md); the tool evaluation it draws from is [`../research/rust-to-ts-codegen.md`](../research/rust-to-ts-codegen.md) (typeshare, ts-rs, specta, schemars, utoipa). The at-threshold path is `schemars` + [`json-schema-to-zod`](https://www.npmjs.com/package/json-schema-to-zod) + a thin brand-mint overlay (generates the runtime Zod schemas, with [`frontend/src/api/brand.ts`](../../frontend/src/api/brand.ts) swapping in the branded `*IdSchema` for ID fields). The trigger is "Zod-maintenance friction," with DTO count (~30) as a proxy.
  - **Why:** Every tool that parses Rust source directly (typeshare, ts-rs, specta) fails on `nutype`-generated structs because `nutype` rewrites the struct during macro expansion. `schemars` works because `nutype` has a first-class `schemars08` feature flag. Targeting Zod schemas (rather than static TS types) means the generated artifact replaces hand-written work instead of sitting alongside it. Hand-rolling for now preserves optionality and keeps the contract legible to both Cowork and Claude Code.

- **Rule:** A CI check ([`.github/workflows/dto-drift.yml`](../../.github/workflows/dto-drift.yml), backed by [`.github/scripts/dto-drift-check.sh`](../../.github/scripts/dto-drift-check.sh)) fails any PR that changes a backend wire-contract file without also touching [`frontend/src/api/types.ts`](../../frontend/src/api/types.ts). The backend has no single `dtos/` directory, so "wire-contract files" is an explicit watch list: the serialized request/response structs in `backend/src/routes/`, the serialized enums in `backend/src/domain/enums.rs`, and the `backend/src/services/` modules that derive `Serialize` on a struct mirrored in `types.ts` (`auth.rs`, `sessions/types.rs`, `sessions/detail.rs`, `sessions/lifecycle.rs`, `runs/read.rs`, `users.rs`, `notifications.rs`). The non-obvious entries are the tell: `SessionSummary` (the `GET /sessions` response) lives in `sessions/lifecycle.rs`, not its route, and `AccessClaims` (the JWT payload `AccessTokenPayloadSchema` decodes) lives in `auth.rs` — both are mirrored on the wire but defined away from where they're returned, so "watch the routes" alone would miss them. That list is duplicated in the workflow's `paths:` filter and the script's `DTO_PATHS_REGEX` — **extend both** when a new `Serialize` DTO module lands. (PR-H1 / Issue [#185](https://github.com/brendanbyrne/beerio-kart/issues/185).)
  - **Why:** A path-diff heuristic is the cheap drift net ADR 0039 calls for while the mirror stays hand-maintained — no Rust parsing, no codegen. False positives (a backend edit that doesn't move the wire shape) are acceptable by design: a one-line touch to `types.ts`, even a comment, clears the check. The asymmetry justifies the noise — a false positive costs one line; silent drift costs a runtime Zod parse failure surfacing far from the field that moved. **`domain/enums.rs` is watched but the `domain/strings.rs` / `numeric.rs` newtypes are not**, on purpose: a serialized enum's variant rename (`active` → `open`) changes the wire string without touching any DTO struct's field list, so nothing else would flag it; a newtype (`Username`, `ImagePath`) serializes transparently as its inner primitive and only reaches the wire through a DTO field in one of the watched structs, which is where a shape change shows up.

## 12. Testing

Tests are a deliverable, not optional. The frontend follows the same principle as the backend ([`../../backend/CLAUDE.md`](../../backend/CLAUDE.md) § Testing): **every requirement placed on the code should be unit- or integration-testable, within reason.** This section covers the language-level patterns (Vitest, pure code, schemas); React-specific patterns (RTL, MSW, hooks) live in [`react.md`](./react.md) § 13.

- **Rule:** Tests trace to requirements, not to lines. Before writing a test, name the behavior or invariant it verifies. High coverage is a *byproduct* of thoroughly verifying requirements, not the goal.
  - **Why:** A test that traverses a code path without asserting anything meaningful is worse than no test — it gives false confidence. Tests written for coverage rot fastest.
  - **Source:** Mirrors `rust.md` § 7.

- **Rule:** Use [Vitest](https://vitest.dev) for unit and component tests. A test that targets a specific source file lives in `*.test.ts` (or `*.test.tsx` for components) next to that file — not in a sibling `__tests__/` directory — and its name mirrors that source: `Foo.test.tsx` for `Foo.tsx`, with an optional concern infix (`App.test.tsx`, `App.routing.test.tsx`) when one source warrants several focused test files. Co-location holds even when the test renders a broad tree to exercise one file's behavior. The sole exception is end-to-end flow tests that span multiple screens and don't map to a single source file: those live in `src/__tests__/` (see [`react.md`](./react.md) § 13).
  - **Why:** Vitest is the Vite-native runner — same ESM resolution, same TS handling, same path aliases. No second build pipeline. Co-located tests match the project's existing pattern (Rust uses `#[cfg(test)] mod tests` in the same file) and keep tests where the code is. "Targets a specific source file" — not "is a narrow unit test" — is the test for co-location: a test that imports and verifies one module belongs beside it however much it mounts; only a cross-screen journey with no single home goes in `__tests__/`.
  - **Source:** <https://vitest.dev/guide/>

- **Rule:** Every branded-type constructor, Zod schema, parser, formatter, and pure utility has unit tests covering: the happy path, at least one rejection case for validators, and one round-trip case for anything that serializes.
  - **Why:** These are the components where bugs are silent. A schema that accepts `null` where it shouldn't typechecks fine and breaks downstream. A branded-type constructor that mints the wrong brand passes all consumer typechecks until something blows up at the wire boundary.
  - **Example:**
    ```ts
    describe('UsernameSchema', () => {
      it('accepts a normal username', () => {
        expect(UsernameSchema.parse('alice')).toBe('alice');
      });
      it('rejects an empty string', () => {
        expect(() => UsernameSchema.parse('')).toThrow();
      });
      it('round-trips through JSON', () => {
        const parsed = UsernameSchema.parse('bob');
        expect(JSON.parse(JSON.stringify(parsed))).toBe('bob');
      });
    });
    ```

- **Rule:** Test names are sentences. `it('rejects an empty username', ...)`, not `it('test1', ...)`. The test list IS the behavioral spec — read the test list and the requirement coverage should be visible.
  - **Source:** Mirrors `rust.md` § 7 and the project's existing convention.

- **Rule:** Use Vitest's `describe.each` / `test.each` for table-driven cases when the same logic runs over half-a-dozen inputs. A plain `for` loop is fine for two or three.
  - **Source:** <https://vitest.dev/api/#test-each>

- **Rule:** "Within reason" — these do **not** earn tests: pure presentational components with no interactive behavior, thin layout/composition wrappers, generated types, app shell code that just renders children, one-time bootstrap code. **If the PR description can't name a user-visible behavior that would silently break, the test is theater.**
  - **Why:** The point is requirement coverage, not coverage-percentage theater. Insisting on tests for trivial components dilutes the rule and makes contributors cynical about the practice.

- **Rule:** CI runs `bun test` on every PR. Coverage via `vitest run --coverage` uploads to Codecov, mirroring the backend setup in `../design.md` § Coverage & CI. Patch coverage threshold: 80% on new/changed code; total coverage must not regress.
  - **Source:** Mirrors `docs/design.md` § Coverage & CI.

- **Rule:** `vi.fn()` mocks and `unwrap()`-equivalent shortcuts (`!`, `as`) are tolerated in test code where they aren't in production code — don't bend test code into the production shape if the verbosity hides the assertion.
  - **Why:** The audience for test code is the test reader. Production-shape patterns sometimes obscure the behavior being verified.

## Document history

- 2026-05-16 — Initial creation. Sourced from research conducted 2026-05-16 (TypeScript 5.9, ESLint 9, React 19.2 baselines). Companion files: `react.md`, `tailwind.md`, both created same day. Compliance plan: `../designs/2026-05-16-frontend-compliance-plan.md`. Type-sync research: `../research/rust-to-ts-codegen.md`.
- 2026-05-18 — Added § 12 Testing. Mirrors the policy in `backend/CLAUDE.md` § Testing and the patterns in `rust.md` § 7. Surfaced during the compliance-Issue filing — the audit had flagged "no tests" as a notable gap outside standards scope; promoting it into the standards closes that gap. Companion update: `react.md` § 13 (React-specific testing); `frontend/CLAUDE.md` § Testing (policy block); compliance plan re-sequences PR-H2 (Vitest scaffolding) from optional to required and bumps it ahead of the runtime-behavior PRs.
- 2026-05-21 — Updated the `api-contract.md` cross-references in § 6 (§ 8 → § 7, error-code registry) after `api-contract.md` § 2 (API client generation) was deleted and §§ 3–11 renumbered to §§ 2–10. Rewrote the § 11 type-sync rule to point at the new [ADR 0039](../decisions/0039-api-client-generation.md) as the authoritative decision (current hand-rolled-with-Zod state, options, adoption trigger), with `../research/rust-to-ts-codegen.md` as the tool evaluation it draws from; named `schemars` + `json-schema-to-zod` + a brand-mint overlay as the at-threshold path. Companion to ADR 0039 and the `api-contract.md` renumber.
- 2026-05-31 — Clarified the § 12 co-location rule so it no longer reads as an absolute ban on `__tests__/` (which contradicted `react.md` § 13 and `frontend/CLAUDE.md` § Testing, both of which reserve `src/__tests__/` for end-to-end flow tests), and added the filename convention: a test's name mirrors the source it targets, with an optional concern infix when one source warrants several focused test files. Companion to Issue [#206](https://github.com/brendanbyrne/beerio-kart/issues/206), which moved the two App-module tests out of `src/__tests__/` (both target the `App` module, not a user flow) and — on PR review — renamed `routing.test.tsx` → `App.routing.test.tsx` to make the `App.tsx` pairing explicit. `frontend/CLAUDE.md` § Testing updated to match.
- 2026-06-15 — § 11: added the DTO drift-check rule and retired the obsolete "CI does not yet enforce drift detection" clause (PR-H1 / Issue [#185](https://github.com/brendanbyrne/beerio-kart/issues/185)). A CI path-diff check fails any PR that edits a backend wire-contract file (`routes/`, `domain/enums.rs`, the serialized `services/` DTO modules) without also touching `frontend/src/api/types.ts`. Implementation lives in `.github/workflows/dto-drift.yml` + `.github/scripts/dto-drift-check.sh`; companion `research/rust-to-ts-codegen.md` § Cheap drift detection is now marked implemented.
