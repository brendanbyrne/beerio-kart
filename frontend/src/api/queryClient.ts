import { QueryCache, QueryClient } from '@tanstack/react-query';
import { logIfResponseShapeMismatch } from './result';

/**
 * Builds the app's single QueryClient.
 *
 * Defaults follow the compliance plan (react.md § 4): revalidate on tab focus,
 * retry a failed fetch once, and treat data as fresh for 30s. Per-query
 * overrides (e.g. the long staleTime on static reference data) live in the
 * hooks themselves.
 *
 * The `QueryCache.onError` keeps contract drift visible (typescript.md § 8).
 * The legacy useEffect hooks called `logIfResponseShapeMismatch` directly; the
 * migrated read hooks instead degrade to empty/null and never expose `error`,
 * so without this a backend response-shape change would fail *silently* —
 * TanStack Query v5 ships no default error logger. Routing every query error
 * through the same helper restores B2's loud-on-drift behavior centrally.
 * `logIfResponseShapeMismatch` only logs `response_shape_mismatch`; network
 * errors stay quiet, matching the hooks' degrade-to-empty contract.
 *
 * (Error boundaries arrive in PR-F1 (#190), but they catch *render* errors,
 * not query errors — react.md § 9 — so the cache `onError` is the right home
 * regardless.)
 *
 * Exposed as a factory so tests can construct an isolated client and assert
 * the drift-logging behavior.
 */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    queryCache: new QueryCache({
      onError: (error, query) => {
        logIfResponseShapeMismatch(error, query.queryHash);
      },
    }),
    defaultOptions: {
      queries: {
        refetchOnWindowFocus: true,
        retry: 1,
        staleTime: 30_000,
      },
    },
  });
}
