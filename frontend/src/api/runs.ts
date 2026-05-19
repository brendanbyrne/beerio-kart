import * as z from 'zod';
import { apiFetch } from './client';
import { ApiErrorException, parseApiError, parseBody } from './result';
import { RunDefaultsSchema, RunDetailSchema } from './types';
import type { RaceId, RunId, TrackId, UserId } from './brand';
import type { ApiError, Result } from './result';
import type { CreateRunRequest, RunDefaults, RunDetail } from './types';

// Runtime-validated run API (PR-B2).
//
// Responses are parsed through their Zod schemas (`parseBody`), which mints
// the branded IDs. `createRun`'s request body stays raw — see the
// `CreateRunRequest` doc comment in types.ts. `getRunDefaults` returns a
// `Result`: a missing defaults endpoint is an expected outcome the caller
// branches on, not an exception (typescript.md § 6). Every helper threads an
// optional `AbortSignal` into `fetch` (§ 7).

export async function createRun(
  body: CreateRunRequest,
  signal?: AbortSignal,
): Promise<RunDetail> {
  const res = await apiFetch('/api/v1/runs', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
    signal,
  });
  if (!res.ok) throw new ApiErrorException(await parseApiError(res));
  return parseBody(RunDetailSchema, res);
}

export async function listRuns(
  params?: {
    session_race_id?: RaceId;
    user_id?: UserId;
    track_id?: TrackId;
  },
  signal?: AbortSignal,
): Promise<RunDetail[]> {
  const query = new URLSearchParams();
  if (params?.session_race_id)
    query.set('session_race_id', params.session_race_id);
  if (params?.user_id) query.set('user_id', params.user_id);
  if (params?.track_id) query.set('track_id', String(params.track_id));
  const qs = query.toString();
  const res = await apiFetch(`/api/v1/runs${qs ? `?${qs}` : ''}`, { signal });
  if (!res.ok) return [];
  return parseBody(z.array(RunDetailSchema), res);
}

export async function getRun(
  id: RunId,
  signal?: AbortSignal,
): Promise<RunDetail> {
  const res = await apiFetch(`/api/v1/runs/${id}`, { signal });
  if (!res.ok) throw new ApiErrorException(await parseApiError(res));
  return parseBody(RunDetailSchema, res);
}

export async function deleteRun(
  id: RunId,
  signal?: AbortSignal,
): Promise<void> {
  const res = await apiFetch(`/api/v1/runs/${id}`, {
    method: 'DELETE',
    signal,
  });
  if (!res.ok) throw new ApiErrorException(await parseApiError(res));
}

/**
 * `GET /runs/defaults` — the pre-filled run-entry values. Returns a `Result`
 * rather than a silent hardcoded fallback: the caller decides what an absent
 * defaults response means (PR-B2 replaced the old `source: 'none'` stub).
 */
export async function getRunDefaults(
  signal?: AbortSignal,
): Promise<Result<RunDefaults, ApiError>> {
  const res = await apiFetch('/api/v1/runs/defaults', { signal });
  if (!res.ok) return { ok: false, error: await parseApiError(res) };
  return { ok: true, value: await parseBody(RunDefaultsSchema, res) };
}
