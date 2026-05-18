import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Vitest configuration — see docs/coding-standards/typescript.md § 12 and
// docs/coding-standards/react.md § 13. Set up by PR-H2 (Issue #193).
//
// A standalone vitest.config.ts (rather than a `test` block merged into
// vite.config.ts) takes precedence when both files exist, so it carries its
// own `plugins` list — only `react()` is needed for tests; the Tailwind
// plugin is build-only.
export default defineConfig({
  plugins: [react()],
  test: {
    // jsdom gives component tests a DOM without a real browser.
    environment: 'jsdom',
    // setupTests.ts starts/stops MSW and extends `expect` with jest-dom.
    setupFiles: ['./src/setupTests.ts'],
    // Co-located unit/component tests and the integration tests react.md
    // § 13 reserves for src/__tests__/ — a single `*.test.` glob covers both
    // (integration tests use the `*.test.tsx` suffix too), so src/__tests__/
    // can also hold un-run helpers/fixtures without Vitest trying to run them
    // as suites.
    include: ['src/**/*.test.{ts,tsx}'],
    coverage: {
      // istanbul, not v8: the v8 provider relies on Node's V8 coverage hooks,
      // which Bun's test workers don't feed (it reports a flat 0%). istanbul
      // instruments the source instead, so it is runtime-agnostic and works
      // under the project's Bun toolchain. See PR-H2 (Issue #193).
      provider: 'istanbul',
      // text → CI log summary; html → local report; lcov → Codecov upload.
      reporter: ['text', 'html', 'lcov'],
      reportsDirectory: './coverage',
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        // Test code and test infrastructure are not production surface.
        'src/**/*.test.{ts,tsx}',
        'src/__tests__/**',
        'src/mocks/**',
        'src/setupTests.ts',
        // Bootstrap / wiring — carved out by typescript.md § 12.
        'src/main.tsx',
        'src/vite-env.d.ts',
      ],
    },
  },
});
