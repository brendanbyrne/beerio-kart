import { useActionState, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { apiFetch } from '../api/client';
import { parseApiError } from '../api/result';
import { useAuth } from '../hooks/useAuth';
import { RaceSetupPicker } from '../components/RaceSetupPicker';
import { DrinkTypeSelector } from '../components/DrinkTypeSelector';
import type { DrinkType } from '../api/types';

type Phase = 'race-setup' | 'drink-type';

type Setup = {
  characterId: number;
  bodyId: number;
  wheelId: number;
  gliderId: number;
};

type SaveState = { error: string | null };

const INITIAL: SaveState = { error: null };

export function Onboarding() {
  const navigate = useNavigate();
  const { user } = useAuth();
  const [phase, setPhase] = useState<Phase>('race-setup');

  const [raceState, saveRaceSetup, savingRaceSetup] = useActionState<
    SaveState,
    Setup
  >(async (_prev, setup) => {
    if (!user) return { error: 'Not signed in' };
    try {
      const res = await apiFetch(`/api/v1/users/${user.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          preferred_character_id: setup.characterId,
          preferred_body_id: setup.bodyId,
          preferred_wheel_id: setup.wheelId,
          preferred_glider_id: setup.gliderId,
        }),
      });
      if (!res.ok) return { error: (await parseApiError(res)).message };
      setPhase('drink-type');
      return { error: null };
    } catch {
      return { error: 'Network error — please try again' };
    }
  }, INITIAL);

  const [drinkState, saveDrinkType, savingDrinkType] = useActionState<
    SaveState,
    DrinkType
  >(async (_prev, dt) => {
    if (!user) return { error: 'Not signed in' };
    try {
      const res = await apiFetch(`/api/v1/users/${user.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ preferred_drink_type_id: dt.id }),
      });
      if (!res.ok) return { error: (await parseApiError(res)).message };
      void navigate('/');
      return { error: null };
    } catch {
      return { error: 'Network error — please try again' };
    }
  }, INITIAL);

  const saving = savingRaceSetup || savingDrinkType;
  const error = phase === 'race-setup' ? raceState.error : drinkState.error;

  return (
    <div className="min-h-screen bg-gray-50 flex flex-col">
      <title>
        {`${phase === 'race-setup' ? 'Pick Your Setup' : 'Pick Your Drink'} · Beerio Kart`}
      </title>
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
        {error && <p className="text-danger text-sm mt-2">{error}</p>}
      </div>

      {/* Content */}
      <div className="flex-1 flex flex-col min-h-0">
        {saving && (
          <div className="text-center text-gray-400 py-8">Saving...</div>
        )}

        {!saving && phase === 'race-setup' && (
          <RaceSetupPicker
            onComplete={saveRaceSetup}
            onSkip={() => {
              setPhase('drink-type');
            }}
            submitLabel="Next: Pick Drink"
          />
        )}

        {!saving && phase === 'drink-type' && (
          <div className="px-4 flex-1">
            <DrinkTypeSelector
              onSelect={saveDrinkType}
              onSkip={() => {
                void navigate('/');
              }}
            />
          </div>
        )}
      </div>
    </div>
  );
}
