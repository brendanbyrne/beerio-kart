import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { server } from '../mocks/server';
import { RaceId, RunId } from './brand';
import { createRun, deleteRun, getRun, getRunDefaults, listRuns } from './runs';
import type { CreateRunRequest } from './types';

// Verifies the run API helpers: each returns the parsed body on a 2xx and
// its documented fallback (a thrown ApiErrorException, [], or — for
// getRunDefaults — an error Result) on a failure. Each helper parses the 2xx
// body through its Zod schema (PR-B2). MSW intercepts at the fetch boundary.

const runDetail = {
  id: 'run1',
  user_id: 'u1',
  session_race_id: 'r1',
  track_id: 1,
  track_time: 90_000,
  lap1_time: 30_000,
  lap2_time: 30_000,
  lap3_time: 30_000,
  character_id: 1,
  body_id: 2,
  wheel_id: 3,
  glider_id: 4,
  drink_type_id: 'd1',
  drink_type_name: 'Water',
  disqualified: false,
  created_at: '2026-05-18T00:00:00.000Z',
};

const runRequest: CreateRunRequest = {
  session_race_id: 'r1',
  track_time: 90_000,
  lap1_time: 30_000,
  lap2_time: 30_000,
  lap3_time: 30_000,
  character_id: 1,
  body_id: 2,
  wheel_id: 3,
  glider_id: 4,
  drink_type_id: 'd1',
  disqualified: false,
};

describe('createRun', () => {
  it('returns the created run on success', async () => {
    server.use(http.post('/api/v1/runs', () => HttpResponse.json(runDetail)));
    const run = await createRun(runRequest);
    expect(run.id).toBe('run1');
  });

  it('throws the backend error message on failure', async () => {
    server.use(
      http.post('/api/v1/runs', () =>
        HttpResponse.json({ error: 'Lap times mismatch' }, { status: 400 }),
      ),
    );
    await expect(createRun(runRequest)).rejects.toThrow('Lap times mismatch');
  });
});

describe('listRuns', () => {
  it('returns the parsed run list on success', async () => {
    server.use(http.get('/api/v1/runs', () => HttpResponse.json([runDetail])));
    const runs = await listRuns();
    expect(runs).toHaveLength(1);
    expect(runs[0]?.id).toBe('run1');
  });

  it('passes filter params through as a query string', async () => {
    server.use(
      http.get('/api/v1/runs', ({ request }) => {
        const url = new URL(request.url);
        expect(url.searchParams.get('session_race_id')).toBe('r1');
        return HttpResponse.json([runDetail]);
      }),
    );
    expect(await listRuns({ session_race_id: RaceId('r1') })).toHaveLength(1);
  });

  it('returns an empty list when the request fails', async () => {
    server.use(
      http.get('/api/v1/runs', () => new HttpResponse(null, { status: 500 })),
    );
    expect(await listRuns()).toEqual([]);
  });
});

describe('getRun', () => {
  it('returns the run on success', async () => {
    server.use(
      http.get('/api/v1/runs/run1', () => HttpResponse.json(runDetail)),
    );
    const run = await getRun(RunId('run1'));
    expect(run.id).toBe('run1');
  });

  it('throws the backend error message on failure', async () => {
    server.use(
      http.get('/api/v1/runs/run1', () =>
        HttpResponse.json({ error: 'Run not found' }, { status: 404 }),
      ),
    );
    await expect(getRun(RunId('run1'))).rejects.toThrow('Run not found');
  });
});

describe('deleteRun', () => {
  it('resolves when the delete succeeds', async () => {
    server.use(
      http.delete(
        '/api/v1/runs/run1',
        () => new HttpResponse(null, { status: 204 }),
      ),
    );
    await expect(deleteRun(RunId('run1'))).resolves.toBeUndefined();
  });

  it('throws the backend error message on failure', async () => {
    server.use(
      http.delete('/api/v1/runs/run1', () =>
        HttpResponse.json({ error: 'Forbidden' }, { status: 403 }),
      ),
    );
    await expect(deleteRun(RunId('run1'))).rejects.toThrow('Forbidden');
  });
});

describe('getRunDefaults', () => {
  it('returns an ok Result wrapping the parsed defaults on success', async () => {
    server.use(
      http.get('/api/v1/runs/defaults', () =>
        HttpResponse.json({
          drink_type_id: 'd1',
          character_id: 1,
          body_id: 2,
          wheel_id: 3,
          glider_id: 4,
          source: 'previous_run',
        }),
      ),
    );
    const result = await getRunDefaults();
    expect(result.ok).toBe(true);
    // Narrow the discriminated Result with a throwing guard so the assertions
    // sit at the top level, not inside an `if` (vitest/no-conditional-expect).
    if (!result.ok) throw new Error('expected an ok Result');
    expect(result.value.source).toBe('previous_run');
    expect(result.value.character_id).toBe(1);
  });

  it('returns an error Result carrying the typed code when the request fails', async () => {
    server.use(
      http.get('/api/v1/runs/defaults', () =>
        HttpResponse.json(
          { error: 'Defaults unavailable', code: 'internal' },
          { status: 500 },
        ),
      ),
    );
    const result = await getRunDefaults();
    expect(result.ok).toBe(false);
    if (result.ok) throw new Error('expected an error Result');
    expect(result.error.code).toBe('internal');
  });
});
