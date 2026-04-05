import { useState } from 'react'
import { useCharacters, useBodies, useWheels, useGliders } from '../hooks/useGameData'
import type { SimpleItem } from '../api/types'

interface RaceSetupPickerProps {
  initialCharacterId?: number | null
  initialBodyId?: number | null
  initialWheelId?: number | null
  initialGliderId?: number | null
  onComplete: (setup: {
    characterId: number
    bodyId: number
    wheelId: number
    gliderId: number
  }) => void
  onSkip?: () => void
  submitLabel?: string
}

type Step = 'character' | 'body' | 'wheel' | 'glider'
const STEPS: Step[] = ['character', 'body', 'wheel', 'glider']
const STEP_LABELS: Record<Step, string> = {
  character: 'Character',
  body: 'Body',
  wheel: 'Wheels',
  glider: 'Glider',
}

export default function RaceSetupPicker({
  initialCharacterId,
  initialBodyId,
  initialWheelId,
  initialGliderId,
  onComplete,
  onSkip,
  submitLabel = 'Confirm Setup',
}: RaceSetupPickerProps) {
  const { items: characters, loading: loadingChars } = useCharacters()
  const { items: bodies, loading: loadingBodies } = useBodies()
  const { items: wheels, loading: loadingWheels } = useWheels()
  const { items: gliders, loading: loadingGliders } = useGliders()

  const [step, setStep] = useState<Step>('character')
  const [characterId, setCharacterId] = useState<number | null>(initialCharacterId ?? null)
  const [bodyId, setBodyId] = useState<number | null>(initialBodyId ?? null)
  const [wheelId, setWheelId] = useState<number | null>(initialWheelId ?? null)
  const [gliderId, setGliderId] = useState<number | null>(initialGliderId ?? null)

  const loading = loadingChars || loadingBodies || loadingWheels || loadingGliders

  if (loading) {
    return <div className="text-center text-gray-400 py-8">Loading game data...</div>
  }

  const currentStepIndex = STEPS.indexOf(step)

  const itemsForStep: Record<Step, SimpleItem[]> = {
    character: characters,
    body: bodies,
    wheel: wheels,
    glider: gliders,
  }

  const selectedForStep: Record<Step, number | null> = {
    character: characterId,
    body: bodyId,
    wheel: wheelId,
    glider: gliderId,
  }

  const setterForStep: Record<Step, (id: number) => void> = {
    character: setCharacterId,
    body: setBodyId,
    wheel: setWheelId,
    glider: setGliderId,
  }

  const items = itemsForStep[step]
  const selected = selectedForStep[step]

  function handleSelect(id: number) {
    setterForStep[step](id)
    // Auto-advance to next step after selection
    if (currentStepIndex < STEPS.length - 1) {
      setTimeout(() => setStep(STEPS[currentStepIndex + 1]), 150)
    }
  }

  function handleBack() {
    if (currentStepIndex > 0) {
      setStep(STEPS[currentStepIndex - 1])
    }
  }

  const allSelected =
    characterId !== null && bodyId !== null && wheelId !== null && gliderId !== null

  return (
    <div className="flex flex-col h-full">
      {/* Step indicators */}
      <div className="flex gap-1 px-4 mb-3">
        {STEPS.map((s) => (
          <button
            key={s}
            onClick={() => setStep(s)}
            className={`flex-1 text-center py-1.5 text-xs font-medium rounded-lg transition-colors ${
              s === step
                ? 'bg-blue-500 text-white'
                : selectedForStep[s] !== null
                  ? 'bg-blue-100 text-blue-600'
                  : 'bg-gray-100 text-gray-400'
            }`}
          >
            {STEP_LABELS[s]}
            {selectedForStep[s] !== null && s !== step && ' \u2713'}
          </button>
        ))}
      </div>

      {/* Current step label */}
      <div className="px-4 mb-2 flex items-center justify-between">
        <h3 className="text-sm font-semibold text-gray-700">
          Pick your {STEP_LABELS[step].toLowerCase()}
        </h3>
        {currentStepIndex > 0 && (
          <button onClick={handleBack} className="text-xs text-blue-500 font-medium">
            &larr; Back
          </button>
        )}
      </div>

      {/* Item grid */}
      <div className="flex-1 overflow-y-auto px-4 pb-2">
        <div className="grid grid-cols-4 gap-2">
          {items.map((item) => (
            <button
              key={item.id}
              onClick={() => handleSelect(item.id)}
              className={`flex flex-col items-center p-1.5 rounded-xl border-2 transition-all ${
                selected === item.id
                  ? 'border-blue-500 bg-blue-50 shadow-sm'
                  : 'border-transparent bg-white hover:border-gray-200'
              }`}
            >
              <img
                src={`/${item.image_path}`}
                alt={item.name}
                className="w-14 h-14 object-contain"
                loading="lazy"
              />
              <span className="text-[10px] text-gray-600 mt-0.5 text-center leading-tight line-clamp-2">
                {item.name}
              </span>
            </button>
          ))}
        </div>
      </div>

      {/* Action buttons */}
      <div className="px-4 py-3 border-t border-gray-100 flex gap-2">
        {onSkip && (
          <button
            onClick={onSkip}
            className="flex-1 py-2.5 text-sm font-medium text-gray-500 bg-gray-100 rounded-xl hover:bg-gray-200 transition-colors"
          >
            Skip for now
          </button>
        )}
        <button
          onClick={() => {
            if (allSelected) {
              onComplete({
                characterId: characterId!,
                bodyId: bodyId!,
                wheelId: wheelId!,
                gliderId: gliderId!,
              })
            }
          }}
          disabled={!allSelected}
          className={`flex-1 py-2.5 text-sm font-semibold rounded-xl transition-colors ${
            allSelected
              ? 'bg-blue-500 text-white hover:bg-blue-600'
              : 'bg-gray-200 text-gray-400 cursor-not-allowed'
          }`}
        >
          {submitLabel}
        </button>
      </div>
    </div>
  )
}
