import { useEffect, useRef, useState } from 'react'
import { getSession } from '../api/sessions'
import type { SessionDetail } from '../api/types'

const POLL_INTERVAL_MS = 2500

/**
 * Polls GET /sessions/:id every 2.5 seconds.
 * Pauses polling when the tab is backgrounded (Page Visibility API).
 * Stops polling once the session ends (closed or not found).
 */
export function useSession(id: string) {
  const [session, setSession] = useState<SessionDetail | null>(null)
  const [loading, setLoading] = useState(true)
  const [ended, setEnded] = useState(false)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const endedRef = useRef(false)

  useEffect(() => {
    let cancelled = false
    endedRef.current = false

    const stopPolling = () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current)
        intervalRef.current = null
      }
    }

    const doFetch = async () => {
      if (endedRef.current) return
      const data = await getSession(id)
      if (cancelled) return
      if (data === null || data.status === 'closed') {
        endedRef.current = true
        setEnded(true)
        stopPolling()
      }
      setSession(data)
      setLoading(false)
    }

    const startPolling = () => {
      if (intervalRef.current || endedRef.current) return
      intervalRef.current = setInterval(doFetch, POLL_INTERVAL_MS)
    }

    const handleVisibility = () => {
      if (document.visibilityState === 'visible') {
        doFetch()
        startPolling()
      } else {
        stopPolling()
      }
    }

    doFetch()
    startPolling()
    document.addEventListener('visibilitychange', handleVisibility)

    return () => {
      cancelled = true
      stopPolling()
      document.removeEventListener('visibilitychange', handleVisibility)
    }
  }, [id])

  return { session, loading, ended }
}
