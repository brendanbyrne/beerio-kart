import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react'
import { getAccessToken, setAccessToken, setOnAuthFailure } from '../api/client'

interface User {
  id: string
  username: string
}

interface AuthContextValue {
  user: User | null
  isAuthenticated: boolean
  isLoading: boolean
  login: (username: string, password: string) => Promise<string | null>
  register: (username: string, password: string) => Promise<string | null>
  logout: () => Promise<void>
}

const AuthContext = createContext<AuthContextValue | null>(null)

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  const clearAuth = useCallback(() => {
    setAccessToken(null)
    setUser(null)
  }, [])

  // On mount: attempt silent refresh to restore session from the HttpOnly cookie.
  useEffect(() => {
    setOnAuthFailure(clearAuth)

    async function silentRefresh() {
      try {
        const res = await fetch('/api/v1/auth/refresh', { method: 'POST' })
        if (res.ok) {
          const data = await res.json()
          setAccessToken(data.access_token)
          // Decode the access token to get user info (the payload is the
          // middle segment, base64url-encoded). This avoids an extra API call.
          const payload = JSON.parse(atob(data.access_token.split('.')[1]))
          setUser({ id: payload.sub, username: payload.username })
        }
      } catch {
        // No valid refresh cookie — user needs to log in
      } finally {
        setIsLoading(false)
      }
    }

    silentRefresh()
  }, [clearAuth])

  /** Returns an error message on failure, or null on success. */
  const login = useCallback(async (username: string, password: string) => {
    const res = await fetch('/api/v1/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    })

    const data = await res.json()
    if (!res.ok) return data.error || 'Login failed'

    setAccessToken(data.access_token)
    setUser(data.user)
    return null
  }, [])

  /** Returns an error message on failure, or null on success. */
  const register = useCallback(async (username: string, password: string) => {
    const res = await fetch('/api/v1/auth/register', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    })

    const data = await res.json()
    if (!res.ok) return data.error || 'Registration failed'

    setAccessToken(data.access_token)
    setUser(data.user)
    return null
  }, [])

  const logout = useCallback(async () => {
    try {
      // Send the access token so the server can bump refresh_token_version
      const token = getAccessToken()
      await fetch('/api/v1/auth/logout', {
        method: 'POST',
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      })
    } catch {
      // Even if the server call fails, clear local state
    }
    clearAuth()
  }, [clearAuth])

  return (
    <AuthContext.Provider
      value={{
        user,
        isAuthenticated: user !== null,
        isLoading,
        login,
        register,
        logout,
      }}
    >
      {children}
    </AuthContext.Provider>
  )
}

// eslint-disable-next-line react-refresh/only-export-components -- co-located with AuthProvider by convention
export function useAuth() {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used within an AuthProvider')
  return ctx
}
