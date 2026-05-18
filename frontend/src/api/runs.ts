import { apiFetch } from './client';
import type { RaceId, RunId, TrackId, UserId } from './brand';
import type { CreateRunRequest, RunDetail, RunDefaults } from './types';

// Brand mint point (PR-B1).
//
// The run DTOs returned here carry branded IDs; each helper mints them with
// an `as` cast on the parsed body. PR-B2 (Issue #191) replaces these casts
// with Zod-parsed boundaries. `createRun`'s request body stays raw — see the
// `CreateRunRequest` doc comment in types.ts.

export async function createRun(body: CreateRunRequest): Promise<RunDetail> {
  const res = await apiFetch('/api/v1/runs', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Failed to submit run');
  }
  return res.json() as Promise<RunDetail>;
}

export async function listRuns(params?: {
  session_race_id?: RaceId;
  user_id?: UserId;
  track_id?: TrackId;
}): Promise<RunDetail[]> {
  const query = new URLSearchParams();
  if (params?.session_race_id)
    query.set('session_race_id', params.session_race_id);
  if (params?.user_id) query.set('user_id', params.user_id);
  if (params?.track_id) query.set('track_id', String(params.track_id));
  const qs = query.toString();
  const res = await apiFetch(`/api/v1/runs${qs ? `?${qs}` : ''}`);
  if (!res.ok) return [];
  return res.json() as Promise<RunDetail[]>;
}

export async function getRun(id: RunId): Promise<RunDetail> {
  const res = await apiFetch(`/api/v1/runs/${id}`);
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Run not found');
  }
  return res.json() as Promise<RunDetail>;
}

export async function deleteRun(id: RunId): Promise<void> {
  const res = await apiFetch(`/api/v1/runs/${id}`, { method: 'DELETE' });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Failed to delete run');
  }
}

export async function getRunDefaults(): Promise<RunDefaults> {
  const res = await apiFetch('/api/v1/runs/defaults');
  if (!res.ok) {
    return {
      drink_type_id: null,
      character_id: null,
      body_id: null,
      wheel_id: null,
      glider_id: null,
      source: 'none',
    };
  }
  return res.json() as Promise<RunDefaults>;
}
