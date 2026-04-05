import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { apiFetch } from '../api/client'
import { useAuth } from '../hooks/useAuth'
import { useUserProfile } from '../hooks/useUserProfile'
import { useCharacters, useBodies, useWheels, useGliders } from '../hooks/useGameData'
import RaceSetupPicker from '../components/RaceSetupPicker'
import DrinkTypeSelector from '../components/DrinkTypeSelector'
import BottomNav from '../components/BottomNav'
import type { DrinkType } from '../api/types'

type EditMode = null | 'race-setup' | 'drink-type' | 'password'

export default function Profile() {
  const navigate = useNavigate()
  const { user, logout } = useAuth()
  const { profile, refresh } = useUserProfile(user?.id)
  const { items: characters } = useCharacters()
  const { items: bodies } = useBodies()
  const { items: wheels } = useWheels()
  const { items: gliders } = useGliders()

  const [editMode, setEditMode] = useState<EditMode>(null)
  const [saving, setSaving] = useState(false)

  // Password change state
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [passwordError, setPasswordError] = useState<string | null>(null)
  const [passwordSuccess, setPasswordSuccess] = useState(false)

  const charName = characters.find((c) => c.id === profile?.preferred_character_id)?.name
  const charImage = characters.find((c) => c.id === profile?.preferred_character_id)?.image_path
  const bodyName = bodies.find((b) => b.id === profile?.preferred_body_id)?.name
  const wheelName = wheels.find((w) => w.id === profile?.preferred_wheel_id)?.name
  const gliderName = gliders.find((g) => g.id === profile?.preferred_glider_id)?.name

  async function handleSaveRaceSetup(setup: {
    characterId: number
    bodyId: number
    wheelId: number
    gliderId: number
  }) {
    if (!user) return
    setSaving(true)
    try {
      await apiFetch(`/api/v1/users/${user.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          preferred_character_id: setup.characterId,
          preferred_body_id: setup.bodyId,
          preferred_wheel_id: setup.wheelId,
          preferred_glider_id: setup.gliderId,
        }),
      })
      refresh()
      setEditMode(null)
    } catch {
      // silent
    } finally {
      setSaving(false)
    }
  }

  async function handleSaveDrinkType(dt: DrinkType) {
    if (!user) return
    setSaving(true)
    try {
      await apiFetch(`/api/v1/users/${user.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ preferred_drink_type_id: dt.id }),
      })
      refresh()
      setEditMode(null)
    } catch {
      // silent
    } finally {
      setSaving(false)
    }
  }

  async function handleChangePassword() {
    setPasswordError(null)
    setPasswordSuccess(false)
    if (newPassword.length < 8) {
      setPasswordError('New password must be at least 8 characters')
      return
    }
    setSaving(true)
    try {
      const res = await apiFetch('/api/v1/auth/password', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          current_password: currentPassword,
          new_password: newPassword,
        }),
      })
      if (!res.ok) {
        const data = await res.json()
        setPasswordError(data.error || 'Failed to change password')
        return
      }
      setPasswordSuccess(true)
      setCurrentPassword('')
      setNewPassword('')
      setTimeout(() => {
        setEditMode(null)
        setPasswordSuccess(false)
      }, 1500)
    } catch {
      setPasswordError('Network error')
    } finally {
      setSaving(false)
    }
  }

  async function handleLogout() {
    await logout()
    navigate('/login')
  }

  if (!profile) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <p className="text-gray-400">Loading profile...</p>
      </div>
    )
  }

  // Full-screen edit modes
  if (editMode === 'race-setup') {
    return (
      <div className="min-h-screen bg-gray-50 flex flex-col">
        <div className="px-4 pt-4 pb-2 flex items-center">
          <button onClick={() => setEditMode(null)} className="text-blue-500 text-sm font-medium">
            &larr; Back
          </button>
          <h2 className="flex-1 text-center text-base font-semibold text-gray-900">Race Setup</h2>
          <div className="w-12" />
        </div>
        <div className="flex-1 flex flex-col min-h-0">
          {saving ? (
            <div className="text-center text-gray-400 py-8">Saving...</div>
          ) : (
            <RaceSetupPicker
              initialCharacterId={profile.preferred_character_id}
              initialBodyId={profile.preferred_body_id}
              initialWheelId={profile.preferred_wheel_id}
              initialGliderId={profile.preferred_glider_id}
              onComplete={handleSaveRaceSetup}
              submitLabel="Save Setup"
            />
          )}
        </div>
      </div>
    )
  }

  if (editMode === 'drink-type') {
    return (
      <div className="min-h-screen bg-gray-50 flex flex-col">
        <div className="px-4 pt-4 pb-2 flex items-center">
          <button onClick={() => setEditMode(null)} className="text-blue-500 text-sm font-medium">
            &larr; Back
          </button>
          <h2 className="flex-1 text-center text-base font-semibold text-gray-900">
            Preferred Drink
          </h2>
          <div className="w-12" />
        </div>
        <div className="flex-1 px-4 pt-2">
          {saving ? (
            <div className="text-center text-gray-400 py-8">Saving...</div>
          ) : (
            <DrinkTypeSelector
              selectedId={profile.preferred_drink_type?.id}
              onSelect={handleSaveDrinkType}
            />
          )}
        </div>
      </div>
    )
  }

  // Default: profile view
  return (
    <div className="min-h-screen bg-gray-50 pb-20">
      {/* Header */}
      <div className="bg-white px-5 pt-6 pb-5 border-b border-gray-100">
        <h1 className="text-xl font-bold text-gray-900">{profile.username}</h1>
        <p className="text-xs text-gray-400 mt-0.5">
          Joined {new Date(profile.created_at).toLocaleDateString()}
        </p>
      </div>

      <div className="px-4 pt-4 space-y-3">
        {/* Race Setup Card */}
        <button
          onClick={() => setEditMode('race-setup')}
          className="w-full bg-white rounded-xl border border-gray-200 p-4 text-left"
        >
          <div className="flex items-center justify-between mb-2">
            <h3 className="text-sm font-semibold text-gray-700">Race Setup</h3>
            <span className="text-xs text-blue-500 font-medium">Edit</span>
          </div>
          {charName ? (
            <div className="flex items-center gap-3">
              {charImage && (
                <img src={`/${charImage}`} alt={charName} className="w-12 h-12 object-contain" />
              )}
              <div className="text-xs text-gray-500 space-y-0.5">
                <div>
                  <span className="font-medium text-gray-700">{charName}</span>
                </div>
                <div>
                  {bodyName} / {wheelName} / {gliderName}
                </div>
              </div>
            </div>
          ) : (
            <p className="text-sm text-gray-400">Not set yet</p>
          )}
        </button>

        {/* Drink Preference Card */}
        <button
          onClick={() => setEditMode('drink-type')}
          className="w-full bg-white rounded-xl border border-gray-200 p-4 text-left"
        >
          <div className="flex items-center justify-between mb-1">
            <h3 className="text-sm font-semibold text-gray-700">Preferred Drink</h3>
            <span className="text-xs text-blue-500 font-medium">Edit</span>
          </div>
          {profile.preferred_drink_type ? (
            <div className="flex items-center gap-2">
              <span>
                {profile.preferred_drink_type.alcoholic ? '\uD83C\uDF7A' : '\uD83E\uDDCA'}
              </span>
              <span className="text-sm text-gray-700">{profile.preferred_drink_type.name}</span>
              <span className="text-xs text-gray-400">
                ({profile.preferred_drink_type.alcoholic ? 'Alcoholic' : 'Non-alcoholic'})
              </span>
            </div>
          ) : (
            <p className="text-sm text-gray-400">Not set yet</p>
          )}
        </button>

        {/* Password Change Card */}
        <div className="bg-white rounded-xl border border-gray-200 p-4">
          {editMode === 'password' ? (
            <div className="space-y-3">
              <h3 className="text-sm font-semibold text-gray-700">Change Password</h3>
              <input
                type="password"
                value={currentPassword}
                onChange={(e) => setCurrentPassword(e.target.value)}
                placeholder="Current password"
                className="w-full px-3 py-2 bg-gray-50 border border-gray-200 rounded-lg text-sm focus:outline-none focus:border-blue-400"
              />
              <input
                type="password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                placeholder="New password (min 8 characters)"
                className="w-full px-3 py-2 bg-gray-50 border border-gray-200 rounded-lg text-sm focus:outline-none focus:border-blue-400"
              />
              {passwordError && <p className="text-red-500 text-xs">{passwordError}</p>}
              {passwordSuccess && <p className="text-green-600 text-xs">Password changed!</p>}
              <div className="flex gap-2">
                <button
                  onClick={() => {
                    setEditMode(null)
                    setPasswordError(null)
                    setCurrentPassword('')
                    setNewPassword('')
                  }}
                  className="flex-1 py-2 text-xs font-medium text-gray-500 bg-gray-100 rounded-lg"
                >
                  Cancel
                </button>
                <button
                  onClick={handleChangePassword}
                  disabled={saving || !currentPassword || !newPassword}
                  className="flex-1 py-2 text-xs font-semibold text-white bg-blue-500 rounded-lg disabled:bg-gray-300"
                >
                  {saving ? 'Saving...' : 'Change Password'}
                </button>
              </div>
            </div>
          ) : (
            <button
              onClick={() => setEditMode('password')}
              className="w-full flex items-center justify-between"
            >
              <h3 className="text-sm font-semibold text-gray-700">Password</h3>
              <span className="text-xs text-blue-500 font-medium">Change</span>
            </button>
          )}
        </div>

        {/* Logout */}
        <button
          onClick={handleLogout}
          className="w-full py-3 text-sm font-semibold text-red-500 bg-white rounded-xl border border-gray-200 hover:bg-red-50 transition-colors"
        >
          Log Out
        </button>
      </div>

      <BottomNav />
    </div>
  )
}
