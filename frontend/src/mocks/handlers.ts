import type { RequestHandler } from 'msw';

/**
 * MSW request handlers.
 *
 * Empty for now. Later compliance PRs add a handler per API endpoint as they
 * introduce the typed client and Zod parsing (PR-B2 onward) — tests mock the
 * network at the fetch boundary, not by stubbing `fetch` or API helpers.
 * See docs/coding-standards/react.md § 13.
 */
export const handlers: RequestHandler[] = [];
