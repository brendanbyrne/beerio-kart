import { useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { useAuth } from '../hooks/useAuth'
import { useSession } from '../hooks/useSession'
import { joinSession, leaveSession } from '../api/sessions'
import BottomNav from '../components/BottomNav'

export default function Session() {
  const { id } = useParams<{ id: string }>()
  const { user } = useAuth()
  const { session, loading, ended } = useSession(id!)
  const navigate = useNavigate()
  const [leaving, setLeaving] = useState(false)
  const [joiningSession, setJoiningSession] = useState(false)
  const [joinError, setJoinError] = useState<string | null>(null)
  const [headerExpanded, setHeaderExpanded] = useState(false)

  const handleLeave = async () => {
    setLeaving(true)
    try {
      await leaveSession(id!)
      navigate('/')
    } catch {
      setLeaving(false)
    }
  }

  const handleJoin = async () => {
    setJoiningSession(true)
    setJoinError(null)
    try {
      await joinSession(id!)
    } catch (e) {
      setJoinError(e instanceof Error ? e.message : 'Failed to join session')
      setJoiningSession(false)
    }
  }

  if (loading) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <p className="text-gray-400">Loading session...</p>
      </div>
    )
  }

  if (ended || !session) {
    return (
      <div className="min-h-screen bg-gray-50 flex flex-col items-center justify-center gap-4 px-4">
        <p className="text-3xl">{'\uD83C\uDFC1'}</p>
        <p className="text-base font-semibold text-gray-900">Session ended</p>
        <button
          onClick={() => navigate('/')}
          className="px-6 py-2 bg-blue-500 text-white rounded-xl text-sm font-semibold"
        >
          Back to Home
        </button>
      </div>
    )
  }

  const activeParticipants = session.participants.filter((p) => !p.left_at)
  const isParticipant = activeParticipants.some((p) => p.user_id === user?.id)
  const isHost = user?.id === session.host_id

  return (
    <div className="min-h-screen bg-gray-50 pb-20">
      {/* Zone 1 — Session Header (sticky top) */}
      <div className="sticky top-0 z-10 bg-white border-b border-gray-200">
        <button
          onClick={() => setHeaderExpanded(!headerExpanded)}
          className="w-full px-4 py-3 flex items-center justify-between"
        >
          <div className="flex items-center gap-2">
            <span className="text-[10px] font-medium text-blue-500 bg-blue-50 px-2 py-0.5 rounded-full">
              {session.ruleset}
            </span>
            <span className="text-sm font-semibold text-gray-900">Race {session.race_number}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-gray-400">
              {activeParticipants.length} player{activeParticipants.length !== 1 ? 's' : ''}
            </span>
            <span className="text-gray-400 text-xs">{headerExpanded ? '\u25B2' : '\u25BC'}</span>
          </div>
        </button>

        {/* Expanded participant list */}
        {headerExpanded && (
          <div className="px-4 pb-3 border-t border-gray-100 pt-2 space-y-1.5">
            {activeParticipants.map((p) => (
              <div key={p.user_id} className="flex items-center justify-between py-1">
                <div className="flex items-center gap-2">
                  {p.user_id === session.host_id && (
                    <span className="text-xs">{'\uD83D\uDC51'}</span>
                  )}
                  <span className="text-sm text-gray-900">{p.username}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Zone 2 — Track Card */}
      <div className="px-4 pt-4">
        <div className="bg-white rounded-xl border border-gray-200 p-6 text-center">
          <div className="w-12 h-12 mx-auto mb-3 bg-gray-100 rounded-lg flex items-center justify-center">
            <span className="text-xl text-gray-300">{'\uD83C\uDFCE\uFE0F'}</span>
          </div>
          <p className="text-sm text-gray-500">Waiting for host to pick...</p>
          {isHost && session.race_number === 0 && (
            <p className="text-xs text-gray-400 mt-1">Track selection coming in a future update</p>
          )}
        </div>
      </div>

      {/* Zone 3 — Action Area */}
      <div className="px-4 pt-4 space-y-3">
        {/* Participant cards */}
        <div>
          <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wider px-1 mb-2">
            Players
          </h3>
          <div className="bg-white rounded-xl border border-gray-200 divide-y divide-gray-100">
            {activeParticipants.map((p) => (
              <div key={p.user_id} className="px-4 py-3 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  {p.user_id === session.host_id && (
                    <span className="text-xs">{'\uD83D\uDC51'}</span>
                  )}
                  <span className="text-sm font-medium text-gray-900">{p.username}</span>
                </div>
                <span className="text-xs text-gray-400">{'\u23F3'} waiting</span>
              </div>
            ))}
          </div>
        </div>

        {/* Session info */}
        <div className="bg-white rounded-xl border border-gray-200 p-4">
          <div className="flex justify-between text-xs text-gray-400">
            <span>Host: {session.host_username}</span>
            <span>Ruleset: {session.ruleset}</span>
          </div>
        </div>

        {isParticipant ? (
          /* Leave button — only shown to participants */
          <button
            onClick={handleLeave}
            disabled={leaving}
            className="w-full py-3 text-sm font-medium text-red-500 bg-white border border-red-200 rounded-xl disabled:opacity-50 active:bg-red-50 transition-colors"
          >
            {leaving ? 'Leaving...' : 'Leave Session'}
          </button>
        ) : (
          /* Join button — shown to non-participants */
          <div className="space-y-2">
            <button
              onClick={handleJoin}
              disabled={joiningSession}
              className="w-full py-3 text-sm font-semibold text-white bg-blue-500 rounded-xl disabled:opacity-50 active:bg-blue-600 transition-colors"
            >
              {joiningSession ? 'Joining...' : 'Join Session'}
            </button>
            {joinError && <p className="text-xs text-red-500 text-center">{joinError}</p>}
          </div>
        )}
      </div>

      <BottomNav />
    </div>
  )
}
