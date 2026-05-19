/**
 * API client that handles access token management and automatic refresh.
 *
 * The access token is stored in a module-level variable — NOT localStorage or
 * sessionStorage. This means it's lost on page refresh, which is intentional:
 * the HttpOnly refresh cookie handles re-authentication transparently.
 *
 * On a 401 response, the client automatically attempts a token refresh (the
 * browser sends the HttpOnly cookie automatically). If refresh succeeds, it
 * retries the original request. If refresh also fails, it means the session
 * is truly expired and the user needs to log in again.
 */

import { parseBody } from './result';
import { TokenRefreshSchema } from './types';

let accessToken: string | null = null;

export function setAccessToken(token: string | null) {
  accessToken = token;
}

export function getAccessToken(): string | null {
  return accessToken;
}

/** Callback set by AuthProvider so the API client can trigger a logout
 *  (redirect to login) when a refresh fails. */
let onAuthFailure: (() => void) | null = null;

export function setOnAuthFailure(callback: () => void) {
  onAuthFailure = callback;
}

/**
 * Attempt to get a new access token using the refresh cookie.
 * Returns true if successful, false otherwise.
 */
async function tryRefresh(): Promise<boolean> {
  try {
    const res = await fetch('/api/v1/auth/refresh', {
      method: 'POST',
      // credentials: 'same-origin' is the default for same-origin requests,
      // which means the browser automatically sends the HttpOnly cookie.
    });
    if (!res.ok) return false;
    const data = await parseBody(TokenRefreshSchema, res);
    accessToken = data.access_token;
    return true;
  } catch {
    return false;
  }
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

  // If we get a 401 and we had a token, try refreshing
  if (res.status === 401 && accessToken) {
    const refreshed = await tryRefresh();
    if (refreshed) {
      // Retry with the new token. The shared `headers` object is mutated in
      // place, so `init` already points at the updated Authorization header.
      headers.set('Authorization', `Bearer ${accessToken}`);
      res = await fetch(url, init);
    } else {
      // Refresh failed — session is truly expired
      accessToken = null;
      onAuthFailure?.();
    }
  }

  return res;
}
