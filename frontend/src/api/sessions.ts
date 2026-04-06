import { apiFetch } from './client'
import type { SessionSummary, SessionDetail, SessionRaceInfo, RaceInfo } from './types'

export async function getMySession(): Promise<string | null> {
  const res = await apiFetch('/api/v1/sessions/mine')
  if (!res.ok) return null
  const data = await res.json()
  return data.session_id ?? null
}

export async function createSession(ruleset: string): Promise<SessionDetail> {
  const res = await apiFetch('/api/v1/sessions', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ ruleset }),
  })
  if (!res.ok) {
    const err = await res.json()
    throw new Error(err.error || 'Failed to create session')
  }
  return res.json()
}

export async function listSessions(): Promise<SessionSummary[]> {
  const res = await apiFetch('/api/v1/sessions')
  if (!res.ok) return []
  return res.json()
}

export async function getSession(id: string): Promise<SessionDetail | null> {
  const res = await apiFetch(`/api/v1/sessions/${id}`)
  if (res.status === 404) return null
  if (!res.ok) return null
  return res.json()
}

export async function joinSession(id: string): Promise<void> {
  const res = await apiFetch(`/api/v1/sessions/${id}/join`, { method: 'POST' })
  if (!res.ok) {
    const err = await res.json()
    throw new Error(err.error || 'Failed to join session')
  }
}

export async function leaveSession(id: string): Promise<void> {
  const res = await apiFetch(`/api/v1/sessions/${id}/leave`, { method: 'POST' })
  if (!res.ok) {
    const err = await res.json()
    throw new Error(err.error || 'Failed to leave session')
  }
}

export async function nextTrack(sessionId: string): Promise<SessionRaceInfo> {
  const res = await apiFetch(`/api/v1/sessions/${sessionId}/next-track`, { method: 'POST' })
  if (!res.ok) {
    const err = await res.json()
    throw new Error(err.error || 'Failed to pick track')
  }
  return res.json()
}

export async function skipTurn(sessionId: string): Promise<SessionRaceInfo> {
  const res = await apiFetch(`/api/v1/sessions/${sessionId}/skip-turn`, { method: 'POST' })
  if (!res.ok) {
    const err = await res.json()
    throw new Error(err.error || 'Failed to skip track')
  }
  return res.json()
}

export async function listRaces(sessionId: string): Promise<RaceInfo[]> {
  const res = await apiFetch(`/api/v1/sessions/${sessionId}/races`)
  if (!res.ok) return []
  return res.json()
}
