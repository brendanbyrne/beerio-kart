import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { apiFetch } from '../api/client'
import { useAuth } from '../hooks/useAuth'
import RaceSetupPicker from '../components/RaceSetupPicker'
import DrinkTypeSelector from '../components/DrinkTypeSelector'
import type { DrinkType } from '../api/types'

type Phase = 'race-setup' | 'drink-type'

export default function Onboarding() {
  const navigate = useNavigate()
  const { user } = useAuth()
  const [phase, setPhase] = useState<Phase>('race-setup')
  const [saving, setSaving] = useState(false)

  async function saveRaceSetup(setup: {
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
      setPhase('drink-type')
    } catch {
      // Silently continue
    } finally {
      setSaving(false)
    }
  }

  async function saveDrinkType(dt: DrinkType) {
    if (!user) return
    setSaving(true)
    try {
      await apiFetch(`/api/v1/users/${user.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ preferred_drink_type_id: dt.id }),
      })
      navigate('/')
    } catch {
      navigate('/')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="min-h-screen bg-gray-50 flex flex-col">
      {/* Header */}
      <div className="px-5 pt-6 pb-4">
        <h1 className="text-xl font-bold text-gray-900">
          {phase === 'race-setup' ? 'Pick Your Setup' : 'Pick Your Drink'}
        </h1>
        <p className="text-sm text-gray-500 mt-1">
          {phase === 'race-setup'
            ? 'Choose your go-to character, kart body, wheels, and glider. You can change these later.'
            : "What's your drink of choice? You can always change this later too."}
        </p>
      </div>

      {/* Content */}
      <div className="flex-1 flex flex-col min-h-0">
        {saving && <div className="text-center text-gray-400 py-8">Saving...</div>}

        {!saving && phase === 'race-setup' && (
          <RaceSetupPicker
            onComplete={saveRaceSetup}
            onSkip={() => setPhase('drink-type')}
            submitLabel="Next: Pick Drink"
          />
        )}

        {!saving && phase === 'drink-type' && (
          <div className="px-4 flex-1">
            <DrinkTypeSelector onSelect={saveDrinkType} onSkip={() => navigate('/')} />
          </div>
        )}
      </div>
    </div>
  )
}
