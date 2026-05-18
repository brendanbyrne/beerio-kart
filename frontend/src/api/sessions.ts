import { apiFetch } from './client';
import { SessionId } from './brand';
import type {
  RaceInfo,
  SessionDetail,
  SessionRaceInfo,
  SessionRuleset,
  SessionSummary,
} from './types';

// Brand mint point (PR-B1).
//
// The wire delivers raw JSON; the session DTOs carry branded IDs. Until
// PR-B2 (Issue #191) adds Zod-parsed boundaries, each helper mints the
// brands with a single `as` cast on the parsed body — the explicit,
// centralized mint site. `getMySession` pulls one bare id out of its
// payload, so it uses the `SessionId` constructor directly.

export async function getMySession(): Promise<SessionId | null> {
  const res = await apiFetch('/api/v1/sessions/mine');
  if (!res.ok) return null;
  const data = (await res.json()) as { session_id?: string | null };
  return data.session_id ? SessionId(data.session_id) : null;
}

export async function createSession(
  ruleset: SessionRuleset,
): Promise<SessionDetail> {
  const res = await apiFetch('/api/v1/sessions', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ ruleset }),
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Failed to create session');
  }
  return res.json() as Promise<SessionDetail>;
}

export async function listSessions(): Promise<SessionSummary[]> {
  const res = await apiFetch('/api/v1/sessions');
  if (!res.ok) return [];
  return res.json() as Promise<SessionSummary[]>;
}

export async function getSession(id: SessionId): Promise<SessionDetail | null> {
  const res = await apiFetch(`/api/v1/sessions/${id}`);
  if (res.status === 404) return null;
  if (!res.ok) return null;
  return res.json() as Promise<SessionDetail>;
}

export async function joinSession(id: SessionId): Promise<void> {
  const res = await apiFetch(`/api/v1/sessions/${id}/join`, { method: 'POST' });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Failed to join session');
  }
}

export async function leaveSession(id: SessionId): Promise<void> {
  const res = await apiFetch(`/api/v1/sessions/${id}/leave`, {
    method: 'POST',
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Failed to leave session');
  }
}

export async function nextTrack(
  sessionId: SessionId,
): Promise<SessionRaceInfo> {
  const res = await apiFetch(`/api/v1/sessions/${sessionId}/next-track`, {
    method: 'POST',
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Failed to pick track');
  }
  return res.json() as Promise<SessionRaceInfo>;
}

export async function skipTurn(sessionId: SessionId): Promise<SessionRaceInfo> {
  const res = await apiFetch(`/api/v1/sessions/${sessionId}/skip-turn`, {
    method: 'POST',
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Failed to skip track');
  }
  return res.json() as Promise<SessionRaceInfo>;
}

export async function listRaces(sessionId: SessionId): Promise<RaceInfo[]> {
  const res = await apiFetch(`/api/v1/sessions/${sessionId}/races`);
  if (!res.ok) return [];
  return res.json() as Promise<RaceInfo[]>;
}
