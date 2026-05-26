import { QueryClientProvider } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom';
import { createQueryClient } from './api/queryClient';
import { AuthProvider, useAuth } from './hooks/useAuth';
import { Login } from './pages/Login';
import { Register } from './pages/Register';
import { Onboarding } from './pages/Onboarding';
import { Home } from './pages/Home';
import { Profile } from './pages/Profile';
import { Session } from './pages/Session';

// One QueryClient for the whole app, created at module scope so it survives
// re-renders (a client recreated in render would drop the cache every time).
// Config + the contract-drift logging live in createQueryClient (api layer).
const queryClient = createQueryClient();

/** Shows a loading spinner while the initial silent refresh is in progress. */
function AuthGate({ children }: { children: React.ReactNode }) {
  const { isLoading } = useAuth();

  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-50">
        <p className="text-gray-400">Loading...</p>
      </div>
    );
  }

  return children;
}

/** Redirects to /login if not authenticated. */
function RequireAuth({ children }: { children: React.ReactNode }) {
  const { isAuthenticated } = useAuth();
  if (!isAuthenticated) return <Navigate to="/login" replace />;
  return children;
}

/** Redirects to / if already authenticated. */
function GuestOnly({ children }: { children: React.ReactNode }) {
  const { isAuthenticated } = useAuth();
  if (isAuthenticated) return <Navigate to="/" replace />;
  return children;
}

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
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
              <Route path="/register" element={<Register />} />
              <Route
                path="/onboarding"
                element={
                  <RequireAuth>
                    <Onboarding />
                  </RequireAuth>
                }
              />
              <Route
                path="/profile"
                element={
                  <RequireAuth>
                    <Profile />
                  </RequireAuth>
                }
              />
              <Route
                path="/session/:id"
                element={
                  <RequireAuth>
                    <Session />
                  </RequireAuth>
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
              {/* Catch-all: unknown paths (e.g. /session with no :id) redirect
                  home rather than rendering an empty <Routes>. PR-F1 will
                  replace this with a real 404/errorElement. */}
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </AuthGate>
        </AuthProvider>
      </BrowserRouter>
      {import.meta.env.DEV && <ReactQueryDevtools initialIsOpen={false} />}
    </QueryClientProvider>
  );
}
