import { useEffect, useRef, useState } from 'react'
import { listSessions } from '../api/sessions'
import type { SessionSummary } from '../api/types'

const POLL_INTERVAL_MS = 5000

/**
 * Fetches the active session list and polls every 5 seconds.
 * Pauses polling when the tab is backgrounded (Page Visibility API).
 */
export function useSessions() {
  const [sessions, setSessions] = useState<SessionSummary[]>([])
  const [loading, setLoading] = useState(true)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  useEffect(() => {
    const doFetch = () => {
      listSessions().then((data) => {
        setSessions(data)
        setLoading(false)
      })
    }

    const startPolling = () => {
      if (intervalRef.current) return
      intervalRef.current = setInterval(doFetch, POLL_INTERVAL_MS)
    }

    const stopPolling = () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current)
        intervalRef.current = null
      }
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
      stopPolling()
      document.removeEventListener('visibilitychange', handleVisibility)
    }
  }, [])

  return { sessions, loading }
}
