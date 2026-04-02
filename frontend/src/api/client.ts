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

let accessToken: string | null = null

export function setAccessToken(token: string | null) {
  accessToken = token
}

export function getAccessToken(): string | null {
  return accessToken
}

/** Callback set by AuthProvider so the API client can trigger a logout
 *  (redirect to login) when a refresh fails. */
let onAuthFailure: (() => void) | null = null

export function setOnAuthFailure(callback: () => void) {
  onAuthFailure = callback
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
    })
    if (!res.ok) return false
    const data = await res.json()
    accessToken = data.access_token
    return true
  } catch {
    return false
  }
}

/**
 * Wrapper around fetch() that adds the access token and handles 401 refresh.
 */
export async function apiFetch(url: string, options: RequestInit = {}): Promise<Response> {
  const headers = new Headers(options.headers)
  if (accessToken) {
    headers.set('Authorization', `Bearer ${accessToken}`)
  }

  let res = await fetch(url, { ...options, headers })

  // If we get a 401 and we had a token, try refreshing
  if (res.status === 401 && accessToken) {
    const refreshed = await tryRefresh()
    if (refreshed) {
      // Retry the original request with the new token
      headers.set('Authorization', `Bearer ${accessToken}`)
      res = await fetch(url, { ...options, headers })
    } else {
      // Refresh failed — session is truly expired
      accessToken = null
      onAuthFailure?.()
    }
  }

  return res
}
