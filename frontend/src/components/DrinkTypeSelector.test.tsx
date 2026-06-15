import { http, HttpResponse } from 'msw';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactElement } from 'react';
import { server } from '../mocks/server';
import { DrinkTypeSelector } from './DrinkTypeSelector';

// Covers the "add a custom drink type" flow: the list loads from the API,
// a submitted name either surfaces the backend's error message (failure) or
// selects the freshly created type (success). MSW mocks the network at the
// fetch boundary (react.md § 13).

// useDrinkTypes is a TanStack Query hook (PR-C1), so the component needs a
// QueryClientProvider. A fresh client per render isolates the cache; retry is
// off so a failed POST settles immediately.
function renderWithClient(ui: ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  );
}

const water = {
  id: 'd1',
  name: 'Water',
  alcoholic: false,
  created_by: null,
  created_at: '2026-05-18T00:00:00.000Z',
};

// The selector calls useDrinkTypes(), which GETs the list on mount. Default
// it to a single existing type; individual tests override the POST.
function mockList() {
  server.use(http.get('/api/v1/drink-types', () => HttpResponse.json([water])));
}

async function openAddForm(user: ReturnType<typeof userEvent.setup>) {
  // Wait out the hook's loading state, then reveal the add form.
  await user.click(await screen.findByRole('button', { name: /add new/i }));
  await user.type(screen.getByPlaceholderText(/drink name/i), 'Cider');
}

describe('DrinkTypeSelector add-drink flow', () => {
  // Generic backend-failure path. Note this is NOT the duplicate-name case:
  // POST /drink-types dedups by name and returns the existing row with 200
  // (api-contract.md § 1.4), so a duplicate never errors. This exercises the
  // form's surfacing of an actual server error (e.g. a 500).
  it('surfaces a backend error when the create request fails', async () => {
    mockList();
    server.use(
      http.post('/api/v1/drink-types', () =>
        HttpResponse.json(
          { error: 'Something went wrong', code: 'internal' },
          { status: 500 },
        ),
      ),
    );
    const onSelect = vi.fn();
    const user = userEvent.setup();
    renderWithClient(<DrinkTypeSelector onSelect={onSelect} />);

    await openAddForm(user);
    await user.click(screen.getByRole('button', { name: /^add$/i }));

    expect(await screen.findByText('Something went wrong')).toBeInTheDocument();
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('selects the created drink type on success', async () => {
    mockList();
    const created = { ...water, id: 'd2', name: 'Cider', alcoholic: true };
    server.use(
      http.post('/api/v1/drink-types', () => HttpResponse.json(created)),
    );
    const onSelect = vi.fn();
    const user = userEvent.setup();
    renderWithClient(<DrinkTypeSelector onSelect={onSelect} />);

    await openAddForm(user);
    await user.click(screen.getByRole('button', { name: /^add$/i }));

    await vi.waitFor(() => {
      expect(onSelect).toHaveBeenCalledWith(
        expect.objectContaining({ id: 'd2', name: 'Cider' }),
      );
    });
  });

  it('catches an empty name with the Zod backstop if native validation is bypassed', async () => {
    mockList();
    const onSelect = vi.fn();
    const user = userEvent.setup();
    renderWithClient(<DrinkTypeSelector onSelect={onSelect} />);

    await user.click(await screen.findByRole('button', { name: /add new/i }));
    // Strip `required` so the submit reaches the action and exercises the
    // Zod safeguard — the second line of defense per react.md § 8.
    screen.getByPlaceholderText(/drink name/i).removeAttribute('required');
    await user.click(screen.getByRole('button', { name: /^add$/i }));

    expect(await screen.findByText('Name is required')).toBeInTheDocument();
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('marks the matching item as selected when a selectedId is supplied', async () => {
    // The check mark renders only on the item whose id equals selectedId — the
    // selected branch of the item styling. The other tests render with no
    // selectedId (every item unselected), so this exercises the other side.
    mockList();
    renderWithClient(<DrinkTypeSelector selectedId="d1" onSelect={vi.fn()} />);

    // '✓' is the check shown next to the selected drink (Water = d1).
    expect(await screen.findByText('✓')).toBeInTheDocument();
  });
});
