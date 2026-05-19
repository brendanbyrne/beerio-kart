import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { server } from '../mocks/server';
import { SessionId } from './brand';
import { ApiErrorException } from './result';
import {
  createSession,
  getMySession,
  getSession,
  joinSession,
  leaveSession,
  listRaces,
  listSessions,
  nextTrack,
  skipTurn,
} from './sessions';

// Verifies the session API helpers: each returns the parsed body on a 2xx
// and its documented fallback (null / [] / a thrown ApiErrorException) on a
// failure. The response bodies are minimal valid wire payloads; each helper
// parses them through its Zod schema (PR-B2), which also mints the branded
// IDs. MSW intercepts at the fetch boundary — no stubbing of `fetch` or the
// helpers themselves.

const sessionDetail = {
  id: 's1',
  host_id: 'u1',
  host_username: 'alice',
  ruleset: 'random',
  status: 'active',
  created_at: '2026-05-18T00:00:00.000Z',
  participants: [],
  race_number: 0,
  current_race: null,
  races: [],
};

const sessionSummary = {
  id: 's1',
  host_username: 'alice',
  participant_count: 2,
  race_number: 1,
  ruleset: 'random',
};

const raceInfo = {
  id: 'r1',
  race_number: 1,
  track_id: 1,
  track_name: 'Mario Circuit',
  cup_name: 'Mushroom',
  run_count: 0,
  created_at: '2026-05-18T00:00:00.000Z',
};

const sessionRace = {
  id: 'r1',
  race_number: 1,
  track_id: 1,
  track_name: 'Mario Circuit',
  cup_name: 'Mushroom',
  image_path: 'tracks/mario-circuit.png',
  created_at: '2026-05-18T00:00:00.000Z',
  submissions: [],
};

describe('getMySession', () => {
  it('returns the session id when the user is in a session', async () => {
    server.use(
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: 's1' }),
      ),
    );
    expect(await getMySession()).toBe('s1');
  });

  it('returns null when the payload carries no session id', async () => {
    server.use(
      http.get('/api/v1/sessions/mine', () =>
        HttpResponse.json({ session_id: null }),
      ),
    );
    expect(await getMySession()).toBeNull();
  });

  it('returns null when the request fails', async () => {
    server.use(
      http.get(
        '/api/v1/sessions/mine',
        () => new HttpResponse(null, { status: 500 }),
      ),
    );
    expect(await getMySession()).toBeNull();
  });
});

describe('createSession', () => {
  it('returns the created session on success', async () => {
    server.use(
      http.post('/api/v1/sessions', () => HttpResponse.json(sessionDetail)),
    );
    const session = await createSession('random');
    expect(session.id).toBe('s1');
    expect(session.status).toBe('active');
  });

  it('throws the backend error message on failure', async () => {
    server.use(
      http.post('/api/v1/sessions', () =>
        HttpResponse.json({ error: 'Bad ruleset' }, { status: 400 }),
      ),
    );
    await expect(createSession('random')).rejects.toThrow('Bad ruleset');
  });

  it('throws response_shape_mismatch when a 2xx body fails its schema', async () => {
    server.use(
      // A 200 whose body is missing every SessionDetail field but `id` —
      // contract drift the Zod parse must catch loudly (typescript.md § 8).
      http.post('/api/v1/sessions', () => HttpResponse.json({ id: 's1' })),
    );
    await expect(createSession('random')).rejects.toBeInstanceOf(
      ApiErrorException,
    );
  });
});

describe('listSessions', () => {
  it('returns the parsed session list on success', async () => {
    server.use(
      http.get('/api/v1/sessions', () => HttpResponse.json([sessionSummary])),
    );
    const sessions = await listSessions();
    expect(sessions).toHaveLength(1);
    expect(sessions[0]?.ruleset).toBe('random');
  });

  it('returns an empty list when the request fails', async () => {
    server.use(
      http.get(
        '/api/v1/sessions',
        () => new HttpResponse(null, { status: 500 }),
      ),
    );
    expect(await listSessions()).toEqual([]);
  });
});

describe('getSession', () => {
  it('returns the session detail on success', async () => {
    server.use(
      http.get('/api/v1/sessions/s1', () => HttpResponse.json(sessionDetail)),
    );
    const session = await getSession(SessionId('s1'));
    expect(session?.id).toBe('s1');
  });

  it('returns null on a 404', async () => {
    server.use(
      http.get(
        '/api/v1/sessions/s1',
        () => new HttpResponse(null, { status: 404 }),
      ),
    );
    expect(await getSession(SessionId('s1'))).toBeNull();
  });

  it('returns null on a server error', async () => {
    server.use(
      http.get(
        '/api/v1/sessions/s1',
        () => new HttpResponse(null, { status: 500 }),
      ),
    );
    expect(await getSession(SessionId('s1'))).toBeNull();
  });
});

describe('joinSession', () => {
  it('resolves when the join succeeds', async () => {
    server.use(
      http.post(
        '/api/v1/sessions/s1/join',
        () => new HttpResponse(null, { status: 200 }),
      ),
    );
    await expect(joinSession(SessionId('s1'))).resolves.toBeUndefined();
  });

  it('throws the backend error message on failure', async () => {
    server.use(
      http.post('/api/v1/sessions/s1/join', () =>
        HttpResponse.json({ error: 'Session is closed' }, { status: 409 }),
      ),
    );
    await expect(joinSession(SessionId('s1'))).rejects.toThrow(
      'Session is closed',
    );
  });
});

describe('leaveSession', () => {
  it('resolves when the leave succeeds', async () => {
    server.use(
      http.post(
        '/api/v1/sessions/s1/leave',
        () => new HttpResponse(null, { status: 200 }),
      ),
    );
    await expect(leaveSession(SessionId('s1'))).resolves.toBeUndefined();
  });

  it('throws the backend error message on failure', async () => {
    server.use(
      http.post('/api/v1/sessions/s1/leave', () =>
        HttpResponse.json({ error: 'Not a participant' }, { status: 403 }),
      ),
    );
    await expect(leaveSession(SessionId('s1'))).rejects.toThrow(
      'Not a participant',
    );
  });
});

describe('nextTrack', () => {
  it('returns the new race on success', async () => {
    server.use(
      http.post('/api/v1/sessions/s1/next-track', () =>
        HttpResponse.json(sessionRace),
      ),
    );
    const race = await nextTrack(SessionId('s1'));
    expect(race.id).toBe('r1');
  });

  it('throws the backend error message on failure', async () => {
    server.use(
      http.post('/api/v1/sessions/s1/next-track', () =>
        HttpResponse.json({ error: 'Pending races first' }, { status: 409 }),
      ),
    );
    await expect(nextTrack(SessionId('s1'))).rejects.toThrow(
      'Pending races first',
    );
  });
});

describe('skipTurn', () => {
  it('returns the new race on success', async () => {
    server.use(
      http.post('/api/v1/sessions/s1/skip-turn', () =>
        HttpResponse.json(sessionRace),
      ),
    );
    const race = await skipTurn(SessionId('s1'));
    expect(race.id).toBe('r1');
  });

  it('throws the backend error message on failure', async () => {
    server.use(
      http.post('/api/v1/sessions/s1/skip-turn', () =>
        HttpResponse.json({ error: 'Not your turn' }, { status: 409 }),
      ),
    );
    await expect(skipTurn(SessionId('s1'))).rejects.toThrow('Not your turn');
  });
});

describe('listRaces', () => {
  it('returns the parsed race list on success', async () => {
    server.use(
      http.get('/api/v1/sessions/s1/races', () =>
        HttpResponse.json([raceInfo]),
      ),
    );
    const races = await listRaces(SessionId('s1'));
    expect(races).toHaveLength(1);
    expect(races[0]?.track_name).toBe('Mario Circuit');
  });

  it('returns an empty list when the request fails', async () => {
    server.use(
      http.get(
        '/api/v1/sessions/s1/races',
        () => new HttpResponse(null, { status: 500 }),
      ),
    );
    expect(await listRaces(SessionId('s1'))).toEqual([]);
  });
});
