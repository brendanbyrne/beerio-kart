import { useState } from 'react'
import { useDrinkTypes } from '../hooks/useGameData'
import { apiFetch } from '../api/client'
import type { DrinkType } from '../api/types'

interface DrinkTypeSelectorProps {
  selectedId?: string | null
  onSelect: (drinkType: DrinkType) => void
  onSkip?: () => void
}

export default function DrinkTypeSelector({
  selectedId,
  onSelect,
  onSkip,
}: DrinkTypeSelectorProps) {
  const { items, loading, refresh } = useDrinkTypes()
  const [showAddForm, setShowAddForm] = useState(false)
  const [newName, setNewName] = useState('')
  const [newAlcoholic, setNewAlcoholic] = useState(true)
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  if (loading) {
    return <div className="text-center text-gray-400 py-8">Loading drink types...</div>
  }

  async function handleAdd() {
    if (!newName.trim()) return
    setSubmitting(true)
    setError(null)
    try {
      const res = await apiFetch('/api/v1/drink-types', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: newName.trim(), alcoholic: newAlcoholic }),
      })
      if (!res.ok) {
        const data = await res.json()
        setError(data.error || 'Failed to add drink type')
        return
      }
      const created: DrinkType = await res.json()
      refresh()
      onSelect(created)
      setShowAddForm(false)
      setNewName('')
    } catch {
      setError('Network error')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="flex flex-col">
      <h3 className="text-sm font-semibold text-gray-700 mb-2 px-1">What are you drinking?</h3>

      <div className="space-y-1.5 max-h-64 overflow-y-auto mb-3">
        {items.map((dt) => (
          <button
            key={dt.id}
            onClick={() => onSelect(dt)}
            className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl border-2 transition-all text-left ${
              selectedId === dt.id
                ? 'border-blue-500 bg-blue-50'
                : 'border-transparent bg-white hover:border-gray-200'
            }`}
          >
            <span className="text-lg">{dt.alcoholic ? '\uD83C\uDF7A' : '\uD83E\uDDCA'}</span>
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium text-gray-900 truncate">{dt.name}</div>
              <div className="text-xs text-gray-400">
                {dt.alcoholic ? 'Alcoholic' : 'Non-alcoholic'}
              </div>
            </div>
            {selectedId === dt.id && (
              <span className="text-blue-500 text-sm font-bold">{'\u2713'}</span>
            )}
          </button>
        ))}
      </div>

      {!showAddForm ? (
        <button
          onClick={() => setShowAddForm(true)}
          className="text-sm text-blue-500 font-medium py-2 hover:text-blue-600 transition-colors"
        >
          + Not listed? Add new
        </button>
      ) : (
        <div className="bg-gray-50 rounded-xl p-3 space-y-2">
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="Drink name..."
            className="w-full px-3 py-2 bg-white border border-gray-200 rounded-lg text-sm focus:outline-none focus:border-blue-400"
            autoFocus
          />
          <div className="flex items-center gap-3">
            <label className="flex items-center gap-2 text-sm text-gray-600">
              <button
                type="button"
                onClick={() => setNewAlcoholic(!newAlcoholic)}
                className={`w-11 h-6 flex-shrink-0 rounded-full transition-colors relative ${
                  newAlcoholic ? 'bg-blue-500' : 'bg-gray-300'
                }`}
              >
                <span
                  className={`block absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform ${
                    newAlcoholic ? 'translate-x-5' : 'translate-x-0'
                  }`}
                />
              </button>
              {newAlcoholic ? 'Alcoholic' : 'Non-alcoholic'}
            </label>
          </div>
          {error && <p className="text-red-500 text-xs">{error}</p>}
          <div className="flex gap-2">
            <button
              onClick={() => {
                setShowAddForm(false)
                setError(null)
              }}
              className="flex-1 py-2 text-xs font-medium text-gray-500 bg-gray-200 rounded-lg"
            >
              Cancel
            </button>
            <button
              onClick={handleAdd}
              disabled={!newName.trim() || submitting}
              className="flex-1 py-2 text-xs font-semibold text-white bg-blue-500 rounded-lg disabled:bg-gray-300"
            >
              {submitting ? 'Adding...' : 'Add'}
            </button>
          </div>
        </div>
      )}

      {onSkip && (
        <button
          onClick={onSkip}
          className="mt-3 py-2 text-sm text-gray-400 hover:text-gray-500 transition-colors"
        >
          Skip for now
        </button>
      )}
    </div>
  )
}
