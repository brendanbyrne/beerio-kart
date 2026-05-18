import { setupServer } from 'msw/node';
import { handlers } from './handlers';

/**
 * MSW server for Node-based (Vitest) tests. Started, reset, and stopped in
 * src/setupTests.ts. Individual tests add per-request overrides with
 * `server.use(...)`.
 */
export const server = setupServer(...handlers);
