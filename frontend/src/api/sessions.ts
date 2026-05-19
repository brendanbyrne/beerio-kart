import * as z from 'zod';
import { apiFetch } from './client';
import { SessionIdSchema } from './brand';
import { ApiErrorException, parseApiError, parseBody } from './result';
import {
  RaceInfoSchema,
  SessionDetailSchema,
  SessionRaceInfoSchema,
  SessionSummarySchema,
} from './types';
import type { SessionId } from './brand';
import type {
  RaceInfo,
  SessionDetail,
  SessionRaceInfo,
  SessionRuleset,
  SessionSummary,
} from './types';

// Runtime-validated session API (PR-B2).
//
// Every response is parsed through its Zod schema (`parseBody`), which is also
// where the branded IDs are minted — the PR-B1 `as` casts are gone. A non-2xx
// from a helper that has nothing to fall back on throws `ApiErrorException`
// carrying the typed error envelope; helpers with a documented empty result
// (`null` / `[]`) keep returning it. Every helper accepts an optional
// `AbortSignal` and threads it into `fetch` (typescript.md § 7).

/** `GET /sessions/mine` returns just `{ session_id }` — a tiny local shape. */
const mySessionSchema = z.object({
  session_id: SessionIdSchema.nullable().optional(),
});

export async function getMySession(
  signal?: AbortSignal,
): Promise<SessionId | null> {
  const res = await apiFetch('/api/v1/sessions/mine', { signal });
  if (!res.ok) return null;
  const data = await parseBody(mySessionSchema, res);
  return data.session_id ?? null;
}

export async function createSession(
  ruleset: SessionRuleset,
  signal?: AbortSignal,
): Promise<SessionDetail> {
  const res = await apiFetch('/api/v1/sessions', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ ruleset }),
    signal,
  });
  if (!res.ok) throw new ApiErrorException(await parseApiError(res));
  return parseBody(SessionDetailSchema, res);
}

export async function listSessions(
  signal?: AbortSignal,
): Promise<SessionSummary[]> {
  const res = await apiFetch('/api/v1/sessions', { signal });
  if (!res.ok) return [];
  return parseBody(z.array(SessionSummarySchema), res);
}

export async function getSession(
  id: SessionId,
  signal?: AbortSignal,
): Promise<SessionDetail | null> {
  const res = await apiFetch(`/api/v1/sessions/${id}`, { signal });
  if (res.status === 404) return null;
  if (!res.ok) return null;
  return parseBody(SessionDetailSchema, res);
}

export async function joinSession(
  id: SessionId,
  signal?: AbortSignal,
): Promise<void> {
  const res = await apiFetch(`/api/v1/sessions/${id}/join`, {
    method: 'POST',
    signal,
  });
  if (!res.ok) throw new ApiErrorException(await parseApiError(res));
}

export async function leaveSession(
  id: SessionId,
  signal?: AbortSignal,
): Promise<void> {
  const res = await apiFetch(`/api/v1/sessions/${id}/leave`, {
    method: 'POST',
    signal,
  });
  if (!res.ok) throw new ApiErrorException(await parseApiError(res));
}

export async function nextTrack(
  sessionId: SessionId,
  signal?: AbortSignal,
): Promise<SessionRaceInfo> {
  const res = await apiFetch(`/api/v1/sessions/${sessionId}/next-track`, {
    method: 'POST',
    signal,
  });
  if (!res.ok) throw new ApiErrorException(await parseApiError(res));
  return parseBody(SessionRaceInfoSchema, res);
}

export async function skipTurn(
  sessionId: SessionId,
  signal?: AbortSignal,
): Promise<SessionRaceInfo> {
  const res = await apiFetch(`/api/v1/sessions/${sessionId}/skip-turn`, {
    method: 'POST',
    signal,
  });
  if (!res.ok) throw new ApiErrorException(await parseApiError(res));
  return parseBody(SessionRaceInfoSchema, res);
}

export async function listRaces(
  sessionId: SessionId,
  signal?: AbortSignal,
): Promise<RaceInfo[]> {
  const res = await apiFetch(`/api/v1/sessions/${sessionId}/races`, { signal });
  if (!res.ok) return [];
  return parseBody(z.array(RaceInfoSchema), res);
}
