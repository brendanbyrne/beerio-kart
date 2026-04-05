import { useCallback, useEffect, useState } from 'react'
import { apiFetch } from '../api/client'
import type { UserDetailProfile } from '../api/types'

export function useUserProfile(userId: string | undefined) {
  const [profile, setProfile] = useState<UserDetailProfile | null>(null)
  const [loading, setLoading] = useState(true)
  const [version, setVersion] = useState(0)

  const refresh = useCallback(() => {
    setVersion((v) => v + 1)
  }, [])

  useEffect(() => {
    if (!userId) return

    async function load() {
      try {
        const res = await apiFetch(`/api/v1/users/${userId}`)
        const data = await res.json()
        setProfile(data)
      } catch {
        // Silently fail
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [userId, version])

  return { profile, loading, refresh }
}
