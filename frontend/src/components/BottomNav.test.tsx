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
// a useQuery on the shared ['my-session'] key. PR-F1 (Issue #190) then swapped
// the <button onClick={navigate}> tabs for <NavLink> (react.md § 11): an
// enabled tab is now a real link (role "link"), while the Session tab stays a
// disabled <button> when there's no session to reach. Per react.md § 13 the
// test mocks the endpoint with MSW and asserts the user-visible behavior.

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

    // While the query is pending there's no session to reach, so the tab is a
    // disabled <button>; once it resolves the tab becomes a real <NavLink>.
    expect(screen.getByRole('button', { name: /session/i })).toBeDisabled();
    expect(
      await screen.findByRole('link', { name: /session/i }),
    ).toBeInTheDocument();
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
    // No session ever arrives, so the tab stays a disabled button (never a
    // link the user could follow).
    expect(screen.getByRole('button', { name: /session/i })).toBeDisabled();
    expect(
      screen.queryByRole('link', { name: /session/i }),
    ).not.toBeInTheDocument();
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

    const sessionTab = await screen.findByRole('link', { name: /session/i });
    await user.click(sessionTab);

    expect(await screen.findByText('Session s1 page')).toBeInTheDocument();
  });

  it('marks the current route tab with aria-current="page"', async () => {
    let calls = 0;
    server.use(
      http.get('/api/v1/sessions/mine', () => {
        calls += 1;
        return HttpResponse.json({ session_id: null });
      }),
    );

    renderNav(<BottomNav />, '/profile');

    // Let the my-session query settle so no state update lands after the test.
    await waitFor(() => {
      expect(calls).toBe(1);
    });
    expect(screen.getByRole('link', { name: /profile/i })).toHaveAttribute(
      'aria-current',
      'page',
    );
    // The Home tab uses `end`, so "/" must NOT match the "/profile" prefix.
    expect(screen.getByRole('link', { name: /home/i })).not.toHaveAttribute(
      'aria-current',
    );
  });
});
