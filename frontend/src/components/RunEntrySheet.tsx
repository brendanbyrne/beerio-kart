import { useState, useRef, useEffect, useCallback, useMemo } from 'react'
import type { SessionRaceInfo, CreateRunRequest, RunDefaults, DrinkType } from '../api/types'
import { createRun, getRunDefaults } from '../api/runs'
import { parseTimeFields } from '../utils/time'
import { useDrinkTypes } from '../hooks/useGameData'
import { useCharacters, useBodies, useWheels, useGliders } from '../hooks/useGameData'
import DrinkTypeSelector from './DrinkTypeSelector'
import RaceSetupPicker from './RaceSetupPicker'

interface TimeFields {
  m: string
  ss: string
  mmm: string
}

const emptyTime: TimeFields = { m: '', ss: '', mmm: '' }

interface RunEntrySheetProps {
  race: SessionRaceInfo
  onClose: () => void
  onSubmitted: () => void
}

export default function RunEntrySheet({ race, onClose, onSubmitted }: RunEntrySheetProps) {
  const [totalTime, setTotalTime] = useState<TimeFields>(emptyTime)
  const [lap1, setLap1] = useState<TimeFields>(emptyTime)
  const [lap2, setLap2] = useState<TimeFields>(emptyTime)
  const [lap3, setLap3] = useState<TimeFields>(emptyTime)

  const [drinkTypeId, setDrinkTypeId] = useState<string | null>(null)
  const [drinkType, setDrinkType] = useState<DrinkType | null>(null)
  const [characterId, setCharacterId] = useState<number | null>(null)
  const [bodyId, setBodyId] = useState<number | null>(null)
  const [wheelId, setWheelId] = useState<number | null>(null)
  const [gliderId, setGliderId] = useState<number | null>(null)
  const [defaultsSource, setDefaultsSource] = useState<string>('')

  const [dq, setDq] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const totalMmmRef = useRef<HTMLInputElement>(null)
  const lap1MRef = useRef<HTMLInputElement>(null)
  const lap1MmmRef = useRef<HTMLInputElement>(null)
  const lap2MRef = useRef<HTMLInputElement>(null)
  const lap2MmmRef = useRef<HTMLInputElement>(null)
  const lap3MRef = useRef<HTMLInputElement>(null)

  const [showDrinkPicker, setShowDrinkPicker] = useState(false)
  const [showSetupPicker, setShowSetupPicker] = useState(false)

  // Load game data for resolving default names
  const { items: drinkTypes } = useDrinkTypes()
  const { items: characters } = useCharacters()
  const { items: bodies } = useBodies()
  const { items: wheels } = useWheels()
  const { items: gliders } = useGliders()

  // Load defaults on mount
  useEffect(() => {
    getRunDefaults().then((d: RunDefaults) => {
      setDrinkTypeId(d.drink_type_id)
      setCharacterId(d.character_id)
      setBodyId(d.body_id)
      setWheelId(d.wheel_id)
      setGliderId(d.glider_id)
      setDefaultsSource(
        d.source === 'previous_run'
          ? 'From your last run'
          : d.source === 'preferences'
            ? 'From your preferences'
            : '',
      )
    })
  }, [])

  // Derive drink type object from ID + loaded data (or explicit user pick)
  const resolvedDrinkType = useMemo(() => {
    if (drinkType) return drinkType
    if (drinkTypeId && drinkTypes.length > 0) {
      return drinkTypes.find((dt) => dt.id === drinkTypeId) ?? null
    }
    return null
  }, [drinkType, drinkTypeId, drinkTypes])

  // Derive setup summary from IDs + loaded game data
  const setupSummary = useMemo(() => {
    if (!characterId || !characters.length || !bodies.length || !wheels.length || !gliders.length)
      return ''
    const parts = [
      characters.find((c) => c.id === characterId)?.name,
      bodies.find((b) => b.id === bodyId)?.name,
      wheels.find((w) => w.id === wheelId)?.name,
      gliders.find((g) => g.id === gliderId)?.name,
    ].filter(Boolean)
    return parts.length === 4 ? parts.join(' \u00B7 ') : ''
  }, [characterId, bodyId, wheelId, gliderId, characters, bodies, wheels, gliders])

  const parsedTotal = parseTimeFields(totalTime.m, totalTime.ss, totalTime.mmm)
  const parsedLap1 = parseTimeFields(lap1.m, lap1.ss, lap1.mmm)
  const parsedLap2 = parseTimeFields(lap2.m, lap2.ss, lap2.mmm)
  const parsedLap3 = parseTimeFields(lap3.m, lap3.ss, lap3.mmm)

  const allTimesFilled =
    parsedTotal !== null && parsedLap1 !== null && parsedLap2 !== null && parsedLap3 !== null
  const hasSetup = characterId !== null && bodyId !== null && wheelId !== null && gliderId !== null
  const canSubmit = allTimesFilled && drinkTypeId !== null && hasSetup && !submitting

  // Lap sum warning
  const lapSum =
    parsedLap1 !== null && parsedLap2 !== null && parsedLap3 !== null
      ? parsedLap1 + parsedLap2 + parsedLap3
      : null
  const sumDiff = lapSum !== null && parsedTotal !== null ? Math.abs(lapSum - parsedTotal) : null
  const showSumWarning = sumDiff !== null && sumDiff > 0

  const handleSubmit = async () => {
    if (
      !canSubmit ||
      parsedTotal === null ||
      parsedLap1 === null ||
      parsedLap2 === null ||
      parsedLap3 === null ||
      drinkTypeId === null ||
      characterId === null ||
      bodyId === null ||
      wheelId === null ||
      gliderId === null
    )
      return
    setSubmitting(true)
    setError(null)
    try {
      const body: CreateRunRequest = {
        session_race_id: race.id,
        track_time: parsedTotal,
        lap1_time: parsedLap1,
        lap2_time: parsedLap2,
        lap3_time: parsedLap3,
        character_id: characterId,
        body_id: bodyId,
        wheel_id: wheelId,
        glider_id: gliderId,
        drink_type_id: drinkTypeId,
        disqualified: dq,
      }
      await createRun(body)
      onSubmitted()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to submit run')
      setSubmitting(false)
    }
  }

  return (
    <div className="fixed inset-0 z-30 flex flex-col justify-end">
      <div className="absolute inset-0 bg-black/40" onClick={onClose} />

      <div
        className="relative bg-white rounded-t-2xl shadow-2xl flex flex-col"
        style={{ maxHeight: '92%' }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex justify-center pt-2.5 pb-1">
          <div className="w-9 h-1 bg-gray-300 rounded-full" />
        </div>

        <div className="overflow-y-auto px-4 pb-8 flex-1">
          {/* Track header */}
          <div className="flex items-center gap-3 mb-5 mt-1">
            <img
              src={`/${race.image_path}`}
              alt={race.track_name}
              className="w-10 h-10 rounded-lg object-contain bg-gray-100 flex-shrink-0"
              onError={(e) => {
                ;(e.target as HTMLImageElement).style.display = 'none'
              }}
            />
            <div>
              <div className="font-semibold text-gray-900 text-[14px]">{race.track_name}</div>
              <div className="text-[12px] text-gray-500">
                {race.cup_name} &middot; Race {race.race_number}
              </div>
            </div>
          </div>

          {/* Total Time */}
          <div className="mb-4">
            <label className="block text-[11px] font-semibold text-gray-500 uppercase tracking-wide mb-2">
              Total Time
            </label>
            <TimeInputGroup
              fields={totalTime}
              setFields={setTotalTime}
              large
              mmmRef={totalMmmRef}
              onComplete={() => lap1MRef.current?.focus()}
            />
          </div>

          {/* Lap Times */}
          <div className="mb-5">
            <label className="block text-[11px] font-semibold text-gray-500 uppercase tracking-wide mb-2">
              Lap Times
            </label>
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <span className="text-[11px] font-semibold text-gray-400 w-6 text-right">L1</span>
                <div className="flex-1">
                  <TimeInputGroup
                    fields={lap1}
                    setFields={setLap1}
                    mRef={lap1MRef}
                    mmmRef={lap1MmmRef}
                    onComplete={() => lap2MRef.current?.focus()}
                    onBackspace={() => totalMmmRef.current?.focus()}
                  />
                </div>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-[11px] font-semibold text-gray-400 w-6 text-right">L2</span>
                <div className="flex-1">
                  <TimeInputGroup
                    fields={lap2}
                    setFields={setLap2}
                    mRef={lap2MRef}
                    mmmRef={lap2MmmRef}
                    onComplete={() => lap3MRef.current?.focus()}
                    onBackspace={() => lap1MmmRef.current?.focus()}
                  />
                </div>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-[11px] font-semibold text-gray-400 w-6 text-right">L3</span>
                <div className="flex-1">
                  <TimeInputGroup
                    fields={lap3}
                    setFields={setLap3}
                    mRef={lap3MRef}
                    onBackspace={() => lap2MmmRef.current?.focus()}
                  />
                </div>
              </div>
            </div>
            {showSumWarning && sumDiff !== null && (
              <p className="text-[10px] text-amber-600 mt-1.5 text-center">
                Lap times don&apos;t add up to total (off by {(sumDiff / 1000).toFixed(3)}s)
              </p>
            )}
            {!showSumWarning && allTimesFilled && (
              <p className="text-[10px] text-gray-400 mt-1.5 text-center">
                Lap times should add up to total time
              </p>
            )}
          </div>

          {/* Drink */}
          <div className="mb-4">
            <label className="block text-[11px] font-semibold text-gray-500 uppercase tracking-wide mb-2">
              Drink
            </label>
            <button
              onClick={() => setShowDrinkPicker(true)}
              className="w-full flex items-center justify-between px-3.5 min-h-[48px] bg-gray-50 border border-gray-200 rounded-xl text-left"
            >
              {resolvedDrinkType ? (
                <div className="flex items-center gap-2.5">
                  <span className="text-base">
                    {resolvedDrinkType.alcoholic ? '\uD83C\uDF7A' : '\uD83E\uDDCA'}
                  </span>
                  <span className="text-[14px] font-medium text-gray-800">
                    {resolvedDrinkType.name}
                  </span>
                </div>
              ) : drinkTypeId ? (
                <span className="text-[14px] text-gray-500">Loading...</span>
              ) : (
                <span className="text-[14px] text-gray-400">Select drink</span>
              )}
              <span className="text-gray-400 text-[12px]">Change</span>
            </button>
            {defaultsSource && (
              <p className="text-[10px] text-gray-400 mt-1 px-1">{defaultsSource}</p>
            )}
          </div>

          {/* Race Setup */}
          <div className="mb-4">
            <label className="block text-[11px] font-semibold text-gray-500 uppercase tracking-wide mb-2">
              Race Setup
            </label>
            <button
              onClick={() => setShowSetupPicker(true)}
              className="w-full flex items-center justify-between px-3.5 min-h-[48px] bg-gray-50 border border-gray-200 rounded-xl text-left"
            >
              {hasSetup ? (
                <span className="text-[13px] font-medium text-gray-800">
                  {setupSummary || 'Setup selected'}
                </span>
              ) : (
                <span className="text-[14px] text-gray-400">Select race setup</span>
              )}
              <span className="text-gray-400 text-[12px]">Edit</span>
            </button>
            {defaultsSource && (
              <p className="text-[10px] text-gray-400 mt-1 px-1">{defaultsSource}</p>
            )}
          </div>

          {/* DQ */}
          <div className="mb-6">
            <label className="block text-[11px] font-semibold text-gray-500 uppercase tracking-wide mb-2">
              Didn&apos;t finish drink?
            </label>
            <SlideToConfirm
              key={dq ? 'dq' : 'no-dq'}
              confirmed={dq}
              onConfirm={() => setDq(true)}
              onReset={() => setDq(false)}
            />
          </div>

          {error && <p className="text-xs text-red-500 text-center mb-3">{error}</p>}

          <button
            onClick={handleSubmit}
            disabled={!canSubmit}
            className="w-full py-4 bg-blue-600 text-white font-semibold rounded-2xl text-[15px] shadow-sm active:scale-[0.98] transition-transform disabled:opacity-50 disabled:active:scale-100"
          >
            {submitting ? 'Submitting...' : 'Submit Run'}
          </button>
        </div>
      </div>

      {showDrinkPicker && (
        <div className="fixed inset-0 z-50 bg-white flex flex-col">
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200">
            <button onClick={() => setShowDrinkPicker(false)} className="text-sm text-blue-500">
              Back
            </button>
            <h2 className="text-sm font-semibold text-gray-900">Select Drink</h2>
            <div className="w-10" />
          </div>
          <div className="flex-1 overflow-y-auto p-4">
            <DrinkTypeSelector
              selectedId={drinkTypeId}
              onSelect={(dt: DrinkType) => {
                setDrinkTypeId(dt.id)
                setDrinkType(dt)
                setShowDrinkPicker(false)
              }}
            />
          </div>
        </div>
      )}

      {showSetupPicker && (
        <div className="fixed inset-0 z-50 bg-white flex flex-col">
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200">
            <button onClick={() => setShowSetupPicker(false)} className="text-sm text-blue-500">
              Back
            </button>
            <h2 className="text-sm font-semibold text-gray-900">Race Setup</h2>
            <div className="w-10" />
          </div>
          <div className="flex-1 overflow-y-auto p-4">
            <RaceSetupPicker
              initialCharacterId={characterId}
              initialBodyId={bodyId}
              initialWheelId={wheelId}
              initialGliderId={gliderId}
              onComplete={(setup) => {
                setCharacterId(setup.characterId)
                setBodyId(setup.bodyId)
                setWheelId(setup.wheelId)
                setGliderId(setup.gliderId)
                setShowSetupPicker(false)
              }}
            />
          </div>
        </div>
      )}
    </div>
  )
}

// ── Time Input Group (self-contained with auto-advance) ─────────────

interface TimeInputGroupProps {
  fields: TimeFields
  setFields: React.Dispatch<React.SetStateAction<TimeFields>>
  large?: boolean
  mRef?: React.RefObject<HTMLInputElement | null>
  mmmRef?: React.RefObject<HTMLInputElement | null>
  onComplete?: () => void
  onBackspace?: () => void
}

function TimeInputGroup({
  fields,
  setFields,
  large,
  mRef,
  mmmRef: extMmmRef,
  onComplete,
  onBackspace,
}: TimeInputGroupProps) {
  const fallbackMRef = useRef<HTMLInputElement>(null)
  const ssRef = useRef<HTMLInputElement>(null)
  const fallbackMmmRef = useRef<HTMLInputElement>(null)
  const resolvedMRef = mRef ?? fallbackMRef
  const resolvedMmmRef = extMmmRef ?? fallbackMmmRef

  const inputClass = large
    ? 'h-12 text-center text-xl font-mono bg-gray-100 rounded-xl border border-gray-300 outline-none focus:border-blue-400 focus:ring-1 focus:ring-blue-200'
    : 'h-10 text-center text-[15px] font-mono bg-gray-50 rounded-lg border border-gray-200 outline-none focus:border-blue-400 focus:ring-1 focus:ring-blue-200'
  const sepClass = large
    ? 'text-xl font-mono text-gray-400 font-bold'
    : 'text-[15px] font-mono text-gray-300 font-bold'

  const handleChange = (field: 'm' | 'ss' | 'mmm', maxLen: number) => {
    return (e: React.ChangeEvent<HTMLInputElement>) => {
      const val = e.target.value.replace(/\D/g, '')
      if (val.length > maxLen) return
      setFields((prev) => ({ ...prev, [field]: val }))
      if (val.length === maxLen) {
        if (field === 'm') ssRef.current?.focus()
        else if (field === 'ss') resolvedMmmRef.current?.focus()
        else if (field === 'mmm') onComplete?.()
      }
    }
  }

  const handleKeyDown = (field: 'm' | 'ss' | 'mmm') => {
    return (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Backspace' && e.currentTarget.value === '') {
        e.preventDefault()
        if (field === 'mmm') ssRef.current?.focus()
        else if (field === 'ss') resolvedMRef.current?.focus()
        else if (field === 'm') onBackspace?.()
      }
    }
  }

  return (
    <div className="flex items-center gap-1.5 justify-center">
      <input
        ref={resolvedMRef}
        className={`${inputClass} ${large ? 'w-14' : 'w-10'}`}
        value={fields.m}
        onChange={handleChange('m', 1)}
        onKeyDown={handleKeyDown('m')}
        inputMode="numeric"
        pattern="[0-9]*"
        placeholder="M"
        maxLength={1}
      />
      <span className={sepClass}>:</span>
      <input
        ref={ssRef}
        className={`${inputClass} ${large ? 'w-16' : 'w-12'}`}
        value={fields.ss}
        onChange={handleChange('ss', 2)}
        onKeyDown={handleKeyDown('ss')}
        inputMode="numeric"
        pattern="[0-9]*"
        placeholder="SS"
        maxLength={2}
      />
      <span className={sepClass}>.</span>
      <input
        ref={resolvedMmmRef}
        className={`${inputClass} ${large ? 'w-[72px]' : 'w-14'}`}
        value={fields.mmm}
        onChange={handleChange('mmm', 3)}
        onKeyDown={handleKeyDown('mmm')}
        inputMode="numeric"
        pattern="[0-9]*"
        placeholder="mmm"
        maxLength={3}
      />
    </div>
  )
}

// ── Slide to Confirm DQ ─────────────────────────────────────────────

interface SlideToConfirmProps {
  confirmed: boolean
  onConfirm: () => void
  onReset: () => void
}

function SlideToConfirm({ confirmed, onConfirm, onReset }: SlideToConfirmProps) {
  const trackRef = useRef<HTMLDivElement>(null)
  const [dragging, setDragging] = useState(false)
  const [offsetX, setOffsetX] = useState(0)
  const [trackWidth, setTrackWidth] = useState(280)
  const thumbW = 44

  // Measure track width after mount and on resize
  useEffect(() => {
    const measure = () => {
      if (trackRef.current) setTrackWidth(trackRef.current.offsetWidth)
    }
    measure()
    window.addEventListener('resize', measure)
    return () => window.removeEventListener('resize', measure)
  }, [])

  const handleMove = useCallback(
    (clientX: number) => {
      if (!dragging || confirmed || !trackRef.current) return
      const rect = trackRef.current.getBoundingClientRect()
      const x = Math.min(Math.max(0, clientX - rect.left - thumbW / 2), trackWidth - thumbW)
      setOffsetX(x)
    },
    [dragging, confirmed, trackWidth],
  )

  const handleEnd = useCallback(() => {
    if (!dragging) return
    setDragging(false)
    const threshold = trackWidth - thumbW - 4
    if (offsetX >= threshold) {
      setOffsetX(trackWidth - thumbW)
      onConfirm()
    } else {
      setOffsetX(0)
    }
  }, [dragging, offsetX, onConfirm, trackWidth])

  if (confirmed) {
    return (
      <button
        onClick={onReset}
        className="w-full flex items-center justify-between px-3.5 min-h-[48px] bg-red-50 border border-red-200 rounded-xl text-left"
      >
        <div>
          <div className="text-[14px] font-semibold text-red-700">Disqualified</div>
          <div className="text-[11px] text-red-400 mt-0.5">
            Excluded from leaderboards &middot; Tap to undo
          </div>
        </div>
        <span className="text-red-400 text-[18px]">{'\u2715'}</span>
      </button>
    )
  }

  const progress = Math.min(offsetX / (trackWidth - thumbW || 1), 1)

  return (
    <div
      ref={trackRef}
      className="relative w-full h-12 rounded-xl bg-gray-100 border border-gray-200 overflow-hidden select-none touch-none"
      onMouseMove={(e) => handleMove(e.clientX)}
      onMouseUp={handleEnd}
      onMouseLeave={handleEnd}
      onTouchMove={(e) => handleMove(e.touches[0].clientX)}
      onTouchEnd={handleEnd}
    >
      <div
        className="absolute inset-0 flex items-center justify-center pointer-events-none"
        style={{ opacity: 1 - progress * 1.5 }}
      >
        <span className="text-[13px] font-medium text-gray-400 tracking-wide">
          Slide to DQ &rarr;
        </span>
      </div>
      <div
        className="absolute top-1 h-10 rounded-lg bg-red-500 shadow-md flex items-center justify-center cursor-grab active:cursor-grabbing"
        style={{
          width: thumbW,
          left: offsetX + 4,
          transition: dragging ? 'none' : 'left 0.25s ease',
        }}
        onMouseDown={(e) => {
          e.preventDefault()
          if (!confirmed) setDragging(true)
        }}
        onTouchStart={() => {
          if (!confirmed) setDragging(true)
        }}
      >
        <span className="text-white text-[16px] font-bold">&raquo;</span>
      </div>
    </div>
  )
}
