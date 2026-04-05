import { useCallback, useEffect, useState } from 'react'
import { listSessions } from '../api/sessions'
import type { SessionSummary } from '../api/types'

/** Fetches the active session list and polls every 5 seconds. */
export function useSessions() {
  const [sessions, setSessions] = useState<SessionSummary[]>([])
  const [loading, setLoading] = useState(true)

  const refresh = useCallback(() => {
    listSessions().then((data) => {
      setSessions(data)
      setLoading(false)
    })
  }, [])

  useEffect(() => {
    refresh()
    const interval = setInterval(refresh, 5000)
    return () => clearInterval(interval)
  }, [refresh])

  return { sessions, loading, refresh }
}
