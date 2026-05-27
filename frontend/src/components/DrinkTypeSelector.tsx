import { useActionState, useState } from 'react';
import { clsx } from 'clsx';
import * as z from 'zod';
import { useDrinkTypes } from '../hooks/useGameData';
import { apiFetch } from '../api/client';
import { parseApiError, parseBody } from '../api/result';
import { DrinkTypeSchema } from '../api/types';
import type { DrinkType } from '../api/types';
import { SubmitButton } from './SubmitButton';

interface DrinkTypeSelectorProps {
  selectedId?: string | null;
  onSelect: (drinkType: DrinkType) => void;
  onSkip?: () => void;
}

export function DrinkTypeSelector({
  selectedId,
  onSelect,
  onSkip,
}: DrinkTypeSelectorProps) {
  const { items, loading, refresh } = useDrinkTypes();
  const [showAddForm, setShowAddForm] = useState(false);

  if (loading) {
    return (
      <div className="text-center text-gray-400 py-8">
        Loading drink types...
      </div>
    );
  }

  return (
    <div className="flex flex-col">
      <h3 className="text-sm font-semibold text-gray-700 mb-2 px-1">
        What are you drinking?
      </h3>

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
            <span className="text-lg">
              {dt.alcoholic ? '\uD83C\uDF7A' : '\uD83E\uDDCA'}
            </span>
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium text-gray-900 truncate">
                {dt.name}
              </div>
              <div className="text-xs text-gray-400">
                {dt.alcoholic ? 'Alcoholic' : 'Non-alcoholic'}
              </div>
            </div>
            {selectedId === dt.id && (
              <span className="text-blue-500 text-sm font-bold">
                {'\u2713'}
              </span>
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
        <AddDrinkTypeForm
          onAdded={(created) => {
            refresh();
            onSelect(created);
            setShowAddForm(false);
          }}
          onCancel={() => setShowAddForm(false)}
        />
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
  );
}

type AddState = { error: string | null };

const ADD_INITIAL: AddState = { error: null };

// `alcoholic` rides as a hidden input ('true' | 'false'); the schema's
// transform turns it into a boolean before the POST body is built.
const AddDrinkTypeFormSchema = z.object({
  name: z.string().trim().min(1).max(60),
  alcoholic: z.enum(['true', 'false']).transform((v) => v === 'true'),
});

// Extracted so its useActionState starts fresh each time the add card is
// opened — the parent mounts/unmounts it via the showAddForm toggle.
function AddDrinkTypeForm({
  onAdded,
  onCancel,
}: {
  onAdded: (created: DrinkType) => void;
  onCancel: () => void;
}) {
  const [alcoholic, setAlcoholic] = useState(true);

  const [state, submit] = useActionState<AddState, FormData>(
    async (_prev, formData) => {
      const parsed = AddDrinkTypeFormSchema.safeParse(
        Object.fromEntries(formData),
      );
      if (!parsed.success) {
        return { error: 'Name is required' };
      }

      try {
        const res = await apiFetch('/api/v1/drink-types', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(parsed.data),
        });
        if (!res.ok) {
          const err = await parseApiError(res);
          return { error: err.message };
        }
        const created = await parseBody(DrinkTypeSchema, res);
        onAdded(created);
        return { error: null };
      } catch {
        return { error: 'Network error' };
      }
    },
    ADD_INITIAL,
  );

  return (
    <form action={submit} className="bg-gray-50 rounded-xl p-3 space-y-2">
      <input
        type="text"
        name="name"
        placeholder="Drink name..."
        className="w-full px-3 py-2 bg-white border border-gray-200 rounded-lg text-sm focus:outline-none focus:border-blue-400"
        autoFocus
        required
        maxLength={60}
      />
      <input
        type="hidden"
        name="alcoholic"
        value={alcoholic ? 'true' : 'false'}
      />
      <div className="flex items-center gap-3">
        <label className="flex items-center gap-2 text-sm text-gray-600">
          <button
            type="button"
            onClick={() => setAlcoholic(!alcoholic)}
            className={clsx(
              'w-11 h-6 flex-shrink-0 rounded-full transition-colors relative',
              alcoholic ? 'bg-blue-500' : 'bg-gray-300',
            )}
            aria-label="Toggle alcoholic"
            aria-pressed={alcoholic}
          >
            <span
              className={clsx(
                'block absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform',
                alcoholic ? 'translate-x-5' : 'translate-x-0',
              )}
            />
          </button>
          {alcoholic ? 'Alcoholic' : 'Non-alcoholic'}
        </label>
      </div>
      {state.error && <p className="text-red-500 text-xs">{state.error}</p>}
      <div className="flex gap-2">
        <button
          type="button"
          onClick={onCancel}
          className="flex-1 py-2 text-xs font-medium text-gray-500 bg-gray-200 rounded-lg"
        >
          Cancel
        </button>
        <SubmitButton
          className="flex-1 py-2 text-xs font-semibold text-white bg-blue-500 rounded-lg"
          pendingLabel="Adding..."
        >
          Add
        </SubmitButton>
      </div>
    </form>
  );
}
