import { useEffect, useRef, useState } from 'react'
import { getMySession, listSessions } from '../api/sessions'
import type { SessionSummary } from '../api/types'

const POLL_INTERVAL_MS = 5000

/**
 * Fetches the active session list and the user's current session ID.
 * Polls every 5 seconds. Pauses when the tab is backgrounded.
 */
export function useSessions() {
  const [sessions, setSessions] = useState<SessionSummary[]>([])
  const [mySessionId, setMySessionId] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  useEffect(() => {
    const doFetch = () => {
      Promise.all([listSessions(), getMySession()]).then(([sessionList, activeId]) => {
        setSessions(sessionList)
        setMySessionId(activeId)
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

  return { sessions, mySessionId, loading }
}
