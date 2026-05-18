import js from '@eslint/js';
import globals from 'globals';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import importPlugin from 'eslint-plugin-import';
import prettier from 'eslint-config-prettier';
import tseslint from 'typescript-eslint';
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
  globalIgnores(['dist']),
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
      // import/no-default-export: 11 default-export sites; flipped in PR-D1.
      'import/no-default-export': 'warn',
      // no-explicit-any / no-non-null-assertion: flipped to error in PR-F1.
      '@typescript-eslint/no-explicit-any': 'warn',
      '@typescript-eslint/no-non-null-assertion': 'warn',

      // consistent-type-specifier-style — named in typescript.md § 9 but
      // omitted from the PR-A1 AC; added per PR review. `prefer-top-level`
      // matches § 5's `import type { … }` example and consistent-type-
      // imports' `separate-type-imports` fix style. Fires on 3 inline-type
      // imports (useAuth.tsx, Login.tsx, Register.tsx); warn until later
      // PRs touching those files convert them, then flips to error.
      'import/consistent-type-specifier-style': ['warn', 'prefer-top-level'],

      // --- Warn-down: strictTypeChecked / stylisticTypeChecked rules that
      // fire on existing code. PR-B2 lands the Zod parses and AbortSignal
      // plumbing that clear the no-unsafe-*/floating-promise families; the
      // remaining stylistic rules are cleared by the PR that touches each
      // file. All flip back to `error` as their offenders are removed. ---
      '@typescript-eslint/no-unsafe-assignment': 'warn',
      '@typescript-eslint/no-unsafe-member-access': 'warn',
      '@typescript-eslint/no-unsafe-argument': 'warn',
      '@typescript-eslint/no-unsafe-call': 'warn',
      '@typescript-eslint/no-unsafe-return': 'warn',
      '@typescript-eslint/no-floating-promises': 'warn',
      '@typescript-eslint/no-misused-promises': 'warn',
      '@typescript-eslint/no-unnecessary-condition': 'warn',
      '@typescript-eslint/no-confusing-void-expression': 'warn',
      '@typescript-eslint/prefer-nullish-coalescing': 'warn',
      '@typescript-eslint/no-unnecessary-type-arguments': 'warn',
      '@typescript-eslint/no-unnecessary-type-assertion': 'warn',
      '@typescript-eslint/restrict-template-expressions': 'warn',
      '@typescript-eslint/no-deprecated': 'warn',
      '@typescript-eslint/prefer-regexp-exec': 'warn',

      // --- Warn-down: jsx-a11y rules that fire on existing code.
      // PR-G2 (the accessibility sweep) fixes these and restores error. ---
      'jsx-a11y/no-static-element-interactions': 'warn',
      'jsx-a11y/click-events-have-key-events': 'warn',
      'jsx-a11y/label-has-associated-control': 'warn',
      'jsx-a11y/no-autofocus': 'warn',

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
    // jsx-a11y) still applies. The four un-typed-value rules below are at
    // `warn` today but flip to `error` in PR-F1 / PR-H1 — the override keeps
    // these example-test patterns valid past those flips.
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
