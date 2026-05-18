import { afterAll, afterEach, beforeAll } from 'vitest';
import { cleanup } from '@testing-library/react';
import { server } from './mocks/server';

// Extends Vitest's `expect` with the jest-dom matchers (toBeInTheDocument,
// toHaveTextContent, ...). The `/vitest` entry also augments Vitest's
// assertion types, so the matchers are typed in test files.
import '@testing-library/jest-dom/vitest';

// Start the MSW mock server once, before any test runs. `onUnhandledRequest:
// 'error'` makes a request to an un-mocked endpoint fail loudly instead of
// silently hitting the network.
beforeAll(() => {
  server.listen({ onUnhandledRequest: 'error' });
});

// Between tests: drop per-test handler overrides and unmount any rendered
// React trees so state never leaks across tests. (React Testing Library only
// auto-cleans when Vitest globals are enabled; this project uses explicit
// imports, so cleanup is wired here.)
afterEach(() => {
  server.resetHandlers();
  cleanup();
});

afterAll(() => {
  server.close();
});
