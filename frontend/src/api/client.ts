/**
 * API client that handles access token management and automatic refresh.
 *
 * The access token is stored in a module-level variable — NOT localStorage or
 * sessionStorage. This means it's lost on page refresh, which is intentional:
 * the HttpOnly refresh cookie handles re-authentication transparently.
 *
 * On a 401 response, the client automatically attempts a token refresh (the
 * browser sends the HttpOnly cookie automatically). The outcome is classified
 * (see `RefreshOutcome`) so the client reacts to the actual signal rather than
 * treating every non-2xx the same:
 *  - a fresh token → retry the original request;
 *  - the refresh cookie is genuinely gone (401) → end the session;
 *  - reuse detected (401 `token_reuse_detected`, ADR-0040) → end the session
 *    with a "signed out for security" notice;
 *  - the *server* failed to answer (5xx / network) → keep the session; the
 *    error is transient and a later refresh may succeed. Logging a user out on
 *    a server hiccup is an availability harm with no security benefit (the
 *    client logout revokes nothing server-side), so 5xx must NOT evict.
 *
 * Refresh is **single-flight**: concurrent 401s share one in-flight refresh, so
 * the app never fires parallel refreshes that would race the backend's token
 * rotation (the client half of the ADR-0040 grace-window mitigation).
 */

import { logIfResponseShapeMismatch, parseApiError, parseBody } from './result';
import { TokenRefreshSchema } from './types';

let accessToken: string | null = null;

export function setAccessToken(token: string | null) {
  accessToken = token;
}

export function getAccessToken(): string | null {
  return accessToken;
}

/** Why the session ended, passed to `onAuthFailure` so the UI can distinguish
 *  an ordinary re-login from a security sign-out. */
export type AuthFailureReason = 'expired' | 'reuse';

/** Callback set by AuthProvider so the API client can trigger a logout
 *  (redirect to login) when a refresh terminally fails. */
let onAuthFailure: ((reason: AuthFailureReason) => void) | null = null;

export function setOnAuthFailure(
  callback: (reason: AuthFailureReason) => void,
) {
  onAuthFailure = callback;
}

/**
 * Classified result of a refresh attempt.
 * - `refreshed` — got a new access token; retry the original request.
 * - `expired` — the refresh cookie is gone/invalid, or the 2xx body was
 *   malformed (a backend bug re-login resolves): end the session normally.
 * - `reuse` — reuse detected (401 `token_reuse_detected`): end the session and
 *   show a security notice.
 * - `transient` — 5xx or a network error: NOT an auth signal. Keep the session.
 */
type RefreshOutcome = 'refreshed' | 'expired' | 'reuse' | 'transient';

/** The in-flight refresh, shared by concurrent callers (single-flight). */
let inFlightRefresh: Promise<RefreshOutcome> | null = null;

/**
 * Attempt to get a new access token using the refresh cookie. Single-flight:
 * concurrent callers await the same promise; a fresh one starts only once the
 * previous settles.
 */
function tryRefresh(): Promise<RefreshOutcome> {
  inFlightRefresh ??= refreshOnce().finally(() => {
    inFlightRefresh = null;
  });
  return inFlightRefresh;
}

async function refreshOnce(): Promise<RefreshOutcome> {
  let res: Response;
  try {
    res = await fetch('/api/v1/auth/refresh', {
      method: 'POST',
      // credentials: 'same-origin' is the default for same-origin requests,
      // which means the browser automatically sends the HttpOnly cookie.
    });
  } catch {
    // Network error — the request never got an HTTP answer. Transient.
    return 'transient';
  }

  if (res.ok) {
    try {
      const data = await parseBody(TokenRefreshSchema, res);
      accessToken = data.access_token;
      return 'refreshed';
    } catch (e) {
      // 2xx but malformed (missing access_token) — a backend/contract bug, not
      // an expired session. Surface it (§ 8); end the session so re-login (a
      // different endpoint) can recover rather than loop on the broken body.
      logIfResponseShapeMismatch(e, '/api/v1/auth/refresh');
      return 'expired';
    }
  }

  // A non-401 failure (5xx, proxy error) is a server problem, not a verdict on
  // the cookie — keep the session and let a later refresh retry.
  if (res.status !== 401) return 'transient';

  // 401: the refresh cookie itself was rejected. Reuse detection (theft) gets a
  // security notice; everything else is an ordinary expiry/invalidation.
  const { code } = await parseApiError(res);
  return code === 'token_reuse_detected' ? 'reuse' : 'expired';
}

/**
 * Options accepted by `apiFetch`. Identical to `RequestInit` except `signal`
 * is widened to explicitly include `undefined`: `exactOptionalPropertyTypes`
 * otherwise rejects a caller that builds `{ signal }` from an optional
 * `AbortSignal` parameter — the API-helper pattern in sessions.ts / runs.ts.
 */
export type ApiFetchOptions = Omit<RequestInit, 'signal'> & {
  signal?: AbortSignal | undefined;
};

/**
 * Wrapper around fetch() that adds the access token and handles 401 refresh.
 */
export async function apiFetch(
  url: string,
  options: ApiFetchOptions = {},
): Promise<Response> {
  const { signal, ...rest } = options;
  const headers = new Headers(rest.headers);
  if (accessToken) {
    headers.set('Authorization', `Bearer ${accessToken}`);
  }

  // Spread `signal` in only when present — `exactOptionalPropertyTypes`
  // forbids handing `fetch` an explicit `signal: undefined`.
  const init: RequestInit = {
    ...rest,
    headers,
    ...(signal ? { signal } : {}),
  };

  let res = await fetch(url, init);

  // If we get a 401 and we had a token, try refreshing.
  if (res.status === 401 && accessToken) {
    const outcome = await tryRefresh();
    switch (outcome) {
      case 'refreshed':
        // Retry with the new token. The shared `headers` object is mutated in
        // place, so `init` already points at the updated Authorization header.
        headers.set('Authorization', `Bearer ${accessToken}`);
        res = await fetch(url, init);
        break;
      case 'transient':
        // Server hiccup during refresh — do NOT log the user out. Return the
        // original 401 so the caller surfaces an error; the session is kept and
        // the next refresh may succeed.
        break;
      case 'expired':
      case 'reuse':
        // The refresh cookie is genuinely no longer usable. End the session.
        accessToken = null;
        onAuthFailure?.(outcome);
        break;
    }
  }

  return res;
}
