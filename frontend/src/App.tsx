import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import { AuthProvider, useAuth } from './hooks/useAuth'
import Login from './pages/Login'
import Register from './pages/Register'
import { useEffect, useState } from 'react'
import { apiFetch } from './api/client'

/** Shows a loading spinner while the initial silent refresh is in progress. */
function AuthGate({ children }: { children: React.ReactNode }) {
  const { isLoading } = useAuth()

  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-900">
        <p className="text-gray-400">Loading...</p>
      </div>
    )
  }

  return children
}

/** Redirects to /login if not authenticated. */
function RequireAuth({ children }: { children: React.ReactNode }) {
  const { isAuthenticated } = useAuth()
  if (!isAuthenticated) return <Navigate to="/login" replace />
  return children
}

/** Redirects to / if already authenticated. */
function GuestOnly({ children }: { children: React.ReactNode }) {
  const { isAuthenticated } = useAuth()
  if (isAuthenticated) return <Navigate to="/" replace />
  return children
}

/** Placeholder home page — shows the hello endpoint response + logout button. */
function Home() {
  const { user, logout } = useAuth()
  const [message, setMessage] = useState('Loading...')

  useEffect(() => {
    apiFetch('/api/v1/hello')
      .then((res) => res.json())
      .then((data) => setMessage(data.message))
      .catch(() => setMessage('Failed to connect to backend'))
  }, [])

  return (
    <div className="min-h-screen flex flex-col items-center justify-center bg-gray-900 gap-4">
      <h1 className="text-4xl font-bold text-white">{message}</h1>
      <p className="text-gray-400">
        Logged in as <span className="text-white font-semibold">{user?.username}</span>
      </p>
      <button onClick={logout} className="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700">
        Log Out
      </button>
    </div>
  )
}

function App() {
  return (
    <BrowserRouter>
      <AuthProvider>
        <AuthGate>
          <Routes>
            <Route
              path="/login"
              element={
                <GuestOnly>
                  <Login />
                </GuestOnly>
              }
            />
            <Route
              path="/register"
              element={
                <GuestOnly>
                  <Register />
                </GuestOnly>
              }
            />
            <Route
              path="/"
              element={
                <RequireAuth>
                  <Home />
                </RequireAuth>
              }
            />
          </Routes>
        </AuthGate>
      </AuthProvider>
    </BrowserRouter>
  )
}

export default App
