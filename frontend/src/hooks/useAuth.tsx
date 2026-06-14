import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from 'react';
import {
  apiFetch,
  getAccessToken,
  setAccessToken,
  setOnAuthFailure,
  type AuthFailureReason,
} from '../api/client';
import { parseApiError, parseBody } from '../api/result';
import {
  AccessTokenPayloadSchema,
  AuthSessionSchema,
  TokenRefreshSchema,
} from '../api/types';

interface User {
  id: string;
  username: string;
}

interface AuthContextValue {
  user: User | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  /** A user-facing notice for the login screen after an involuntary sign-out
   *  (e.g. "signed out for security" on reuse detection). `null` when there's
   *  nothing to show. */
  authNotice: string | null;
  login: (username: string, password: string) => Promise<string | null>;
  register: (username: string, password: string) => Promise<string | null>;
  changePassword: (
    currentPassword: string,
    newPassword: string,
  ) => Promise<string | null>;
  logout: () => Promise<void>;
}

/** Notice shown on the login screen after reuse detection forces a sign-out. */
const SECURITY_LOGOUT_NOTICE =
  'You were signed out for your security. Please log in again.';

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [authNotice, setAuthNotice] = useState<string | null>(null);

  // Kept as useCallback (a react.md § 7 carve-out): clearAuth is a transitive
  // dependency of the mount effect below and is registered into module scope
  // via setOnAuthFailure, so it must be referentially stable. exhaustive-deps
  // (a static rule that can't see the Compiler's memoization) enforces this,
  // and react.md § 6 forbids silencing it — so the explicit hook stays.
  // Clears any stale notice so an ordinary logout doesn't carry a security
  // message; an involuntary sign-out sets one *after* this via handleAuthFailure.
  const clearAuth = useCallback(() => {
    setAccessToken(null);
    setUser(null);
    setAuthNotice(null);
  }, []);

  // Invoked by the API client when a refresh terminally fails. `reuse` (theft,
  // ADR-0040) carries a security notice; an ordinary expiry does not.
  const handleAuthFailure = useCallback(
    (reason: AuthFailureReason) => {
      clearAuth();
      if (reason === 'reuse') setAuthNotice(SECURITY_LOGOUT_NOTICE);
    },
    [clearAuth],
  );

  // On mount: attempt silent refresh to restore session from the HttpOnly cookie.
  useEffect(() => {
    setOnAuthFailure(handleAuthFailure);

    // This mount refresh deliberately uses a raw fetch, NOT the single-flight
    // `tryRefresh` in client.ts: at mount `accessToken` is null so `apiFetch`
    // never fires a competing refresh, and only this path decodes the JWT to
    // restore the user. Under React StrictMode the effect double-invokes,
    // firing two parallel mount refreshes — outside the single-flight guard —
    // but the backend's ~10s grace window (ADR-0040) returns the existing
    // successor instead of false-positive-revoking, so it's safe. Reconsider if
    // a token can ever be set before mount.
    async function silentRefresh() {
      try {
        const res = await fetch('/api/v1/auth/refresh', { method: 'POST' });
        if (res.ok) {
          const data = await parseBody(TokenRefreshSchema, res);
          setAccessToken(data.access_token);
          // Decode the access token to get user info (the payload is the
          // middle segment, base64url-encoded). This avoids an extra API call.
          // JWT payloads use base64url encoding (- and _ instead of + and /),
          // but atob() only handles standard base64 — convert before decoding.
          const payloadSegment = data.access_token.split('.')[1];
          if (payloadSegment) {
            const base64 = payloadSegment.replace(/-/g, '+').replace(/_/g, '/');
            const raw: unknown = JSON.parse(atob(base64));
            const payload = AccessTokenPayloadSchema.parse(raw);
            setUser({ id: payload.sub, username: payload.username });
          }
        }
      } catch {
        // No valid refresh cookie — user needs to log in
      } finally {
        setIsLoading(false);
      }
    }

    silentRefresh();
  }, [handleAuthFailure]);

  /** Returns an error message on failure, or null on success. */
  const login = async (username: string, password: string) => {
    const res = await fetch('/api/v1/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    });

    if (!res.ok) return (await parseApiError(res)).message;

    const data = await parseBody(AuthSessionSchema, res);
    setAccessToken(data.access_token);
    setUser(data.user);
    setAuthNotice(null);
    return null;
  };

  /** Returns an error message on failure, or null on success. */
  const register = async (username: string, password: string) => {
    const res = await fetch('/api/v1/auth/register', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    });

    if (!res.ok) return (await parseApiError(res)).message;

    const data = await parseBody(AuthSessionSchema, res);
    setAccessToken(data.access_token);
    setUser(data.user);
    setAuthNotice(null);
    return null;
  };

  /** Returns an error message on failure, or null on success. */
  const changePassword = async (
    currentPassword: string,
    newPassword: string,
  ) => {
    const res = await apiFetch('/api/v1/auth/password', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        current_password: currentPassword,
        new_password: newPassword,
      }),
    });
    if (!res.ok) return (await parseApiError(res)).message;
    return null;
  };

  const logout = async () => {
    try {
      // Send the access token so the server can bump refresh_token_version
      const token = getAccessToken();
      await fetch('/api/v1/auth/logout', {
        method: 'POST',
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      });
    } catch {
      // Even if the server call fails, clear local state
    }
    clearAuth();
  };

  return (
    <AuthContext.Provider
      value={{
        user,
        isAuthenticated: user !== null,
        isLoading,
        authNotice,
        login,
        register,
        changePassword,
        logout,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

// eslint-disable-next-line react-refresh/only-export-components -- co-located with AuthProvider by convention
export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within an AuthProvider');
  return ctx;
}
