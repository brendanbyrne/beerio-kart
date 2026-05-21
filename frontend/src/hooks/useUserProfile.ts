import { useCallback, useEffect, useState } from 'react';
import { apiFetch } from '../api/client';
import { logIfResponseShapeMismatch, parseBody } from '../api/result';
import { UserDetailProfileSchema } from '../api/types';
import type { UserDetailProfile } from '../api/types';

export function useUserProfile(userId: string | undefined) {
  const [profile, setProfile] = useState<UserDetailProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [version, setVersion] = useState(0);

  const refresh = useCallback(() => {
    setVersion((v) => v + 1);
  }, []);

  useEffect(() => {
    if (!userId) return;

    async function load() {
      try {
        const res = await apiFetch(`/api/v1/users/${userId}`);
        const data = await parseBody(UserDetailProfileSchema, res);
        setProfile(data);
      } catch (e) {
        logIfResponseShapeMismatch(e, 'GET /api/v1/users/:id');
      } finally {
        setLoading(false);
      }
    }
    load();
  }, [userId, version]);

  return { profile, loading, refresh };
}
