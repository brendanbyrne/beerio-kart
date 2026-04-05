import { useAuth } from '../hooks/useAuth'
import { useUserProfile } from '../hooks/useUserProfile'
import { useCharacters } from '../hooks/useGameData'
import BottomNav from '../components/BottomNav'

export default function Home() {
  const { user } = useAuth()
  const { profile } = useUserProfile(user?.id)
  const { items: characters } = useCharacters()

  const preferredChar = characters.find((c) => c.id === profile?.preferred_character_id)

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
        {/* Start Session button (placeholder) */}
        <button
          disabled
          className="w-full py-4 bg-gray-200 text-gray-400 rounded-xl text-sm font-semibold cursor-not-allowed"
        >
          Start a Session
        </button>

        {/* Empty state */}
        <div className="bg-white rounded-xl border border-gray-200 p-6 text-center">
          <p className="text-3xl mb-2">{'\uD83C\uDFCE\uFE0F'}</p>
          <p className="text-sm text-gray-500">No active sessions yet</p>
          <p className="text-xs text-gray-400 mt-1">Sessions will appear here once you start one</p>
        </div>
      </div>

      <BottomNav />
    </div>
  )
}
