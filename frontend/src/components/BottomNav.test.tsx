import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import type { ReactNode } from 'react';
import { server } from '../mocks/server';
import { BottomNav } from './BottomNav';

// PR-C2 (Issue #186) replaced BottomNav's re-fetch-on-navigation useEffect with
// a useQuery on the shared ['my-session'] key. Per react.md § 13 the test mocks
// the endpoint with MSW and asserts the user-visible behavior: whether the
// Session tab is reachable, and that tapping it navigates to the active
// session.

function renderNav(node: ReactNode, initialPath = '/') {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[initialPath]}>{node}</MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('BottomNav', () => {
  it('enables the Session tab once an active session is found', async () => {
    server.use(
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: 's1' }),
      ),
    );

    renderNav(<BottomNav />);

    const sessionTab = screen.getByRole('button', { name: /session/i });
    // Disabled on first render (query still pending), enabled once it resolves.
    expect(sessionTab).toBeDisabled();
    await waitFor(() => {
      expect(sessionTab).toBeEnabled();
    });
  });

  it('leaves the Session tab disabled when there is no active session', async () => {
    let calls = 0;
    server.use(
      http.get('/api/v1/sessions/mine', () => {
        calls += 1;
        return HttpResponse.json({ session_id: null });
      }),
    );

    renderNav(<BottomNav />);

    await waitFor(() => {
      expect(calls).toBe(1);
    });
    expect(screen.getByRole('button', { name: /session/i })).toBeDisabled();
  });

  it('navigates to the active session when the Session tab is tapped', async () => {
    server.use(
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: 's1' }),
      ),
    );
    const user = userEvent.setup();

    renderNav(
      <Routes>
        <Route path="/" element={<BottomNav />} />
        <Route path="/session/:id" element={<div>Session s1 page</div>} />
      </Routes>,
    );

    const sessionTab = screen.getByRole('button', { name: /session/i });
    await waitFor(() => {
      expect(sessionTab).toBeEnabled();
    });
    await user.click(sessionTab);

    expect(await screen.findByText('Session s1 page')).toBeInTheDocument();
  });
});
