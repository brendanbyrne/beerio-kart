import js from '@eslint/js';
import globals from 'globals';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import importPlugin from 'eslint-plugin-import';
import prettier from 'eslint-config-prettier';
import tseslint from 'typescript-eslint';
import vitest from '@vitest/eslint-plugin';
import testingLibrary from 'eslint-plugin-testing-library';
import { defineConfig, globalIgnores } from 'eslint/config';

// ESLint flat config — see docs/coding-standards/typescript.md § 9.
//
// PR-A1 (Issue #187) is purely additive infrastructure: it installs the
// standards' lint presets but must produce ZERO new errors in CI or the
// lefthook pre-commit hook. Any rule that fires on the existing (pre-
// compliance) code is therefore started at `warn`, not `error`. Later PRs
// in the frontend compliance plan fix the underlying code and flip each
// rule back to `error`. The `warn`-level blocks below cite the PR that
// owns the fix. See docs/designs/2026-05-16-frontend-compliance-plan.md.

export default defineConfig([
  // `dist` is the build output; `coverage` is the generated Vitest report (both
  // git-ignored) — linting either just produces noise on generated files.
  globalIgnores(['dist', 'coverage']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      // strictTypeChecked + stylisticTypeChecked replace the bare
      // `recommended` preset (typescript.md § 9).
      tseslint.configs.strictTypeChecked,
      tseslint.configs.stylisticTypeChecked,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
      jsxA11y.flatConfigs.recommended,
      importPlugin.flatConfigs.recommended,
      // eslint-config-prettier comes last so it can switch off the
      // formatting rules the other presets enable — Prettier owns format.
      prettier,
    ],
    languageOptions: {
      // Matches the ES2023 `target` in tsconfig.app.json.
      ecmaVersion: 2023,
      globals: globals.browser,
      parserOptions: {
        // Picks up tsconfig.app.json / tsconfig.node.json automatically so
        // the type-aware rules in strictTypeChecked have type information.
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      // --- Standards rules, enforced now (no existing violations) ---
      '@typescript-eslint/consistent-type-imports': 'error',
      '@typescript-eslint/ban-ts-comment': [
        'error',
        { 'ts-expect-error': 'allow-with-description' },
      ],
      'no-restricted-syntax': [
        'error',
        {
          selector: 'TSEnumDeclaration',
          message:
            'Use `as const` objects or string-literal unions instead of enum.',
        },
      ],

      // --- Standards rules, started at `warn` (AC-named) ---
      // consistent-type-definitions: ~25 `interface` declarations remain;
      //   api/types.ts is converted in PR-B1, prop types in later PRs.
      '@typescript-eslint/consistent-type-definitions': ['warn', 'type'],
      // import/no-default-export: all default exports converted in PR-D1.
      'import/no-default-export': 'error',
      // no-explicit-any / no-non-null-assertion: flipped to error in PR-F1
      // (#190). Offenders were removed earlier — the `any`-typed
      // `await res.json()` calls by PR-B2's Zod parses, the `!` assertions by
      // PR-D2 — so the flip lands with zero violations in production code.
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-non-null-assertion': 'error',

      // consistent-type-specifier-style — named in typescript.md § 9 but
      // omitted from the PR-A1 AC; added per PR review. `prefer-top-level`
      // matches § 5's `import type { … }` example and consistent-type-
      // imports' `separate-type-imports` fix style. Fires on 3 inline-type
      // imports (useAuth.tsx, Login.tsx, Register.tsx); warn until later
      // PRs touching those files convert them, then flips to error.
      'import/consistent-type-specifier-style': ['warn', 'prefer-top-level'],

      // --- strictTypeChecked / stylisticTypeChecked rules that once fired on
      // pre-compliance code. Each flips back to `error` the moment its offender
      // count reaches zero (the ratchet only ever tightens). PR-B2 landed the
      // Zod parses + AbortSignal plumbing that cleared the no-unsafe-* family;
      // the families below are now verified clean, so they are enforced. ---
      '@typescript-eslint/no-unsafe-assignment': 'error',
      '@typescript-eslint/no-unsafe-member-access': 'error',
      '@typescript-eslint/no-unsafe-argument': 'error',
      '@typescript-eslint/no-unsafe-call': 'error',
      '@typescript-eslint/no-unsafe-return': 'error',
      '@typescript-eslint/no-unnecessary-type-assertion': 'error',
      '@typescript-eslint/no-deprecated': 'error',
      '@typescript-eslint/prefer-regexp-exec': 'error',

      // Still at `warn`: offenders remain, owned by the frontend compliance-plan
      // PR that touches each file. Flip to `error` as each reaches zero.
      '@typescript-eslint/no-floating-promises': 'warn',
      '@typescript-eslint/no-misused-promises': 'warn',
      '@typescript-eslint/no-unnecessary-condition': 'warn',
      '@typescript-eslint/no-confusing-void-expression': 'warn',
      '@typescript-eslint/prefer-nullish-coalescing': 'warn',
      '@typescript-eslint/no-unnecessary-type-arguments': 'warn',
      '@typescript-eslint/restrict-template-expressions': 'warn',

      // --- jsx-a11y rules restored to error by PR-G2 (the accessibility
      // sweep). All offenders are fixed: sub-44px touch targets bumped to
      // min-h-touch, the add-drink autoFocus removed, the Home/RunEntrySheet
      // modals given focus traps + keyboard-accessible backdrop buttons, the
      // DQ slider made keyboard-operable, and the section <label>s that wrapped
      // no control converted to <span>. ---
      'jsx-a11y/no-static-element-interactions': 'error',
      'jsx-a11y/click-events-have-key-events': 'error',
      'jsx-a11y/label-has-associated-control': 'error',
      'jsx-a11y/no-autofocus': 'error',

      // --- Off: eslint-plugin-import's module-resolution rules. These
      // duplicate what `tsc --noEmit` already verifies, and without a
      // TypeScript resolver they false-fire on every `.ts`/`.tsx` import.
      // The standard scopes eslint-plugin-import to its syntactic rules
      // (import/no-default-export); resolution stays TypeScript's job. ---
      'import/no-unresolved': 'off',
      'import/named': 'off',
      'import/namespace': 'off',
      'import/default': 'off',
      'import/no-named-as-default-member': 'off',
    },
  },
  {
    // --- Test files and test infrastructure (added in PR-H2, Issue #193) ---
    // typescript.md § 12 explicitly permits `vi.fn()` mocks and the `!` / `as`
    // shortcuts in test code where production code can't use them: the
    // audience for a test is its reader, and bending test code into the
    // production-safety shape can bury the assertion being verified. MSW
    // handler factories and mock return values are also inherently loosely
    // typed. This block turns those rules off for tests; everything else
    // (consistent-type-imports, ban-ts-comment, the enum ban, react-hooks,
    // jsx-a11y) still applies. The un-typed-value rules below are now `error`
    // in production code (flipped once their offender count hit zero) — this
    // override keeps the example-test patterns valid past that flip.
    files: [
      '**/*.test.{ts,tsx}',
      'src/__tests__/**/*.{ts,tsx}',
      'src/setupTests.ts',
      'src/mocks/**/*.ts',
    ],
    rules: {
      '@typescript-eslint/no-explicit-any': 'off',
      '@typescript-eslint/no-non-null-assertion': 'off',
      '@typescript-eslint/no-unsafe-assignment': 'off',
      '@typescript-eslint/no-unsafe-member-access': 'off',
      '@typescript-eslint/no-unsafe-argument': 'off',
      '@typescript-eslint/no-unsafe-call': 'off',
      '@typescript-eslint/no-unsafe-return': 'off',
    },
  },
  {
    // --- Vitest lint preset (#217) ---
    // Locks in the test-quality state the suite already satisfies: vitest's
    // `recommended` turns on valid-expect, valid-expect-in-promise,
    // no-standalone-expect, expect-expect, no-focused-tests, and
    // no-disabled-tests in one move. Scoped to test files. See
    // docs/coding-standards/testing.md.
    ...vitest.configs.recommended,
    files: ['**/*.test.{ts,tsx}', 'src/__tests__/**/*.{ts,tsx}'],
    rules: {
      ...vitest.configs.recommended.rules,
      // Started at `warn`, not `error`, per this file's preamble: it fires on
      // the legitimate `try { …; expect.unreachable() } catch { expect(err) }`
      // shape used to assert on a thrown error's nested fields (result.test.ts,
      // runs.test.ts). Flip to `error` if those convert to `.rejects`.
      'vitest/no-conditional-expect': 'warn',
    },
  },
  {
    // --- Testing Library (React) lint preset (#217) ---
    // Adds await-async-queries / await-async-utils and the query-hygiene rules
    // (prefer-screen-queries, …). Scoped to test files.
    ...testingLibrary.configs['flat/react'],
    files: ['**/*.test.{ts,tsx}', 'src/__tests__/**/*.{ts,tsx}'],
    rules: {
      ...testingLibrary.configs['flat/react'].rules,
      // Started at `warn`, not `error`, per this file's preamble: it fires on
      // the unavoidable raw-node drag simulation for the custom div-based DQ
      // slider (RunEntrySheet.test.tsx), which has no Testing Library query.
      // Flip to `error` if that drag is reworked or the rule disabled inline.
      'testing-library/no-node-access': 'warn',
    },
  },
  {
    // --- Config files (added in PR-H2, Issue #193) ---
    // vite.config.ts / vitest.config.ts must default-export their config
    // object — it's the Vite/Vitest contract, so they can never satisfy
    // `import/no-default-export` (typescript.md § 5). Disable it for config
    // files only, so the warn list reflects real violations. Without this,
    // PR-D1 flipping that rule to error would fail the build on config files.
    files: ['**/*.config.{ts,js}'],
    rules: {
      'import/no-default-export': 'off',
    },
  },
]);
