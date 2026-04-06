import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuth } from '../hooks/useAuth'
import { useUserProfile } from '../hooks/useUserProfile'
import { useCharacters } from '../hooks/useGameData'
import { useSessions } from '../hooks/useSessions'
import { createSession } from '../api/sessions'
import BottomNav from '../components/BottomNav'

export default function Home() {
  const { user } = useAuth()
  const { profile } = useUserProfile(user?.id)
  const { items: characters } = useCharacters()
  const { sessions, loading: sessionsLoading } = useSessions()
  const navigate = useNavigate()

  const [showCreate, setShowCreate] = useState(false)
  const [creating, setCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const preferredChar = characters.find((c) => c.id === profile?.preferred_character_id)

  const handleCreate = async () => {
    setCreating(true)
    setError(null)
    try {
      const session = await createSession('random')
      navigate(`/session/${session.id}`)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to create session')
      setCreating(false)
    }
  }

  return (
    <div className="min-h-screen bg-gray-50 pb-20">
      {/* Header */}
      <div className="bg-white px-5 pt-6 pb-5 border-b border-gray-100">
        <div className="flex items-center gap-3">
          {preferredChar ? (
            <img
              src={`/${preferredChar.image_path}`}
              alt={preferredChar.name}
              className="w-12 h-12 object-contain"
            />
          ) : (
            <div className="w-12 h-12 bg-gray-200 rounded-full flex items-center justify-center text-lg">
              {'\uD83C\uDFAE'}
            </div>
          )}
          <div>
            <h1 className="text-lg font-bold text-gray-900">
              Hey, {profile?.username ?? user?.username}!
            </h1>
            {preferredChar && <p className="text-xs text-gray-400">{preferredChar.name} main</p>}
          </div>
        </div>
      </div>

      {/* Content */}
      <div className="px-4 pt-4 space-y-3">
        {/* Start Session button */}
        <button
          onClick={() => setShowCreate(true)}
          className="w-full py-4 bg-blue-500 text-white rounded-xl text-sm font-semibold active:bg-blue-600 transition-colors"
        >
          Start a Session
        </button>

        {/* Create session modal */}
        {showCreate && (
          <div
            className="fixed inset-0 bg-black/40 z-50 flex items-end justify-center"
            onClick={() => {
              setShowCreate(false)
              setError(null)
            }}
          >
            <div
              className="bg-white w-full max-w-lg rounded-t-2xl p-5 space-y-4"
              onClick={(e) => e.stopPropagation()}
            >
              <h2 className="text-base font-bold text-gray-900">Pick a Ruleset</h2>
              <button
                onClick={handleCreate}
                disabled={creating}
                className="w-full py-3 bg-blue-500 text-white rounded-xl text-sm font-semibold disabled:opacity-50 active:bg-blue-600 transition-colors"
              >
                {creating ? 'Creating...' : 'Random'}
              </button>
              <p className="text-xs text-gray-400 text-center">
                Tracks are chosen randomly each round
              </p>
              {error && <p className="text-xs text-red-500 text-center">{error}</p>}
              <button
                onClick={() => {
                  setShowCreate(false)
                  setError(null)
                }}
                className="w-full py-2 text-sm text-gray-500"
              >
                Cancel
              </button>
            </div>
          </div>
        )}

        {/* Active sessions list */}
        {sessionsLoading ? (
          <div className="bg-white rounded-xl border border-gray-200 p-6 text-center">
            <p className="text-sm text-gray-400">Loading sessions...</p>
          </div>
        ) : sessions.length > 0 ? (
          <div className="space-y-2">
            <h2 className="text-xs font-semibold text-gray-400 uppercase tracking-wider px-1">
              Active Sessions
            </h2>
            {sessions.map((s) => (
              <button
                key={s.id}
                onClick={() => navigate(`/session/${s.id}`)}
                className="w-full bg-white rounded-xl border border-gray-200 p-4 text-left active:bg-gray-50 transition-colors"
              >
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-sm font-semibold text-gray-900">
                      {s.host_username}&apos;s session
                    </p>
                    <div className="flex items-center gap-2 mt-1">
                      <span className="text-xs text-gray-400">
                        {s.participant_count} player{s.participant_count !== 1 ? 's' : ''}
                      </span>
                      <span className="text-xs text-gray-400">
                        {'\u00B7'} Race {s.race_number}
                      </span>
                    </div>
                  </div>
                  <span className="text-[10px] font-medium text-blue-500 bg-blue-50 px-2 py-0.5 rounded-full">
                    {s.ruleset}
                  </span>
                </div>
              </button>
            ))}
          </div>
        ) : (
          <div className="bg-white rounded-xl border border-gray-200 p-6 text-center">
            <p className="text-3xl mb-2">{'\uD83C\uDFCE\uFE0F'}</p>
            <p className="text-sm text-gray-500">No active sessions yet</p>
            <p className="text-xs text-gray-400 mt-1">Start one and invite your friends to join</p>
          </div>
        )}

        {error && !showCreate && <p className="text-xs text-red-500 text-center">{error}</p>}
      </div>

      <BottomNav />
    </div>
  )
}
