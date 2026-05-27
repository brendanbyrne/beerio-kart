import { useActionState, useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { apiFetch } from '../api/client';
import { parseApiError } from '../api/result';
import { useAuth } from '../hooks/useAuth';
import { useUserProfile } from '../hooks/useUserProfile';
import {
  useCharacters,
  useBodies,
  useWheels,
  useGliders,
} from '../hooks/useGameData';
import { RaceSetupPicker } from '../components/RaceSetupPicker';
import { DrinkTypeSelector } from '../components/DrinkTypeSelector';
import { SubmitButton } from '../components/SubmitButton';
import { BottomNav } from '../components/BottomNav';
import { readString } from '../utils/forms';
import type { DrinkType } from '../api/types';

type EditMode = null | 'race-setup' | 'drink-type' | 'password';

export function Profile() {
  const navigate = useNavigate();
  const { user, logout } = useAuth();
  const { profile, refresh } = useUserProfile(user?.id);
  const { items: characters } = useCharacters();
  const { items: bodies } = useBodies();
  const { items: wheels } = useWheels();
  const { items: gliders } = useGliders();

  const [editMode, setEditMode] = useState<EditMode>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const char = characters.find((c) => c.id === profile?.preferred_character_id);
  const body = bodies.find((b) => b.id === profile?.preferred_body_id);
  const wheel = wheels.find((w) => w.id === profile?.preferred_wheel_id);
  const glider = gliders.find((g) => g.id === profile?.preferred_glider_id);

  async function handleSaveRaceSetup(setup: {
    characterId: number;
    bodyId: number;
    wheelId: number;
    gliderId: number;
  }) {
    if (!user) return;
    setSaving(true);
    setSaveError(null);
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
      if (!res.ok) {
        setSaveError((await parseApiError(res)).message);
        return;
      }
      refresh();
      setEditMode(null);
    } catch {
      setSaveError('Network error — please try again');
    } finally {
      setSaving(false);
    }
  }

  async function handleSaveDrinkType(dt: DrinkType) {
    if (!user) return;
    setSaving(true);
    setSaveError(null);
    try {
      const res = await apiFetch(`/api/v1/users/${user.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ preferred_drink_type_id: dt.id }),
      });
      if (!res.ok) {
        setSaveError((await parseApiError(res)).message);
        return;
      }
      refresh();
      setEditMode(null);
    } catch {
      setSaveError('Network error — please try again');
    } finally {
      setSaving(false);
    }
  }

  async function handleLogout() {
    await logout();
    navigate('/login');
  }

  if (!profile) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <p className="text-gray-400">Loading profile...</p>
      </div>
    );
  }

  // Full-screen edit modes
  if (editMode === 'race-setup') {
    return (
      <div className="min-h-screen bg-gray-50 flex flex-col">
        <div className="px-4 pt-4 pb-2 flex items-center">
          <button
            onClick={() => setEditMode(null)}
            className="text-blue-500 text-sm font-medium"
          >
            &larr; Back
          </button>
          <h2 className="flex-1 text-center text-base font-semibold text-gray-900">
            Race Setup
          </h2>
          <div className="w-12" />
        </div>
        {saveError && (
          <p className="text-red-500 text-sm text-center px-4">{saveError}</p>
        )}
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
    );
  }

  if (editMode === 'drink-type') {
    return (
      <div className="min-h-screen bg-gray-50 flex flex-col">
        <div className="px-4 pt-4 pb-2 flex items-center">
          <button
            onClick={() => setEditMode(null)}
            className="text-blue-500 text-sm font-medium"
          >
            &larr; Back
          </button>
          <h2 className="flex-1 text-center text-base font-semibold text-gray-900">
            Preferred Drink
          </h2>
          <div className="w-12" />
        </div>
        {saveError && (
          <p className="text-red-500 text-sm text-center px-4">{saveError}</p>
        )}
        <div className="flex-1 px-4 pt-2">
          {saving ? (
            <div className="text-center text-gray-400 py-8">Saving...</div>
          ) : (
            <DrinkTypeSelector
              selectedId={profile.preferred_drink_type?.id ?? null}
              onSelect={handleSaveDrinkType}
            />
          )}
        </div>
      </div>
    );
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
          {char ? (
            <div className="grid grid-cols-4 gap-2">
              {[
                { label: 'Character', item: char },
                { label: 'Body', item: body },
                { label: 'Wheels', item: wheel },
                { label: 'Glider', item: glider },
              ].map(({ label, item }) => (
                <div key={label} className="flex flex-col items-center">
                  {item && (
                    <img
                      src={`/${item.image_path}`}
                      alt={item.name}
                      className="w-12 h-12 object-contain"
                    />
                  )}
                  <span className="text-[10px] text-gray-500 text-center leading-tight mt-0.5">
                    {item?.name}
                  </span>
                </div>
              ))}
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
            <h3 className="text-sm font-semibold text-gray-700">
              Preferred Drink
            </h3>
            <span className="text-xs text-blue-500 font-medium">Edit</span>
          </div>
          {profile.preferred_drink_type ? (
            <div className="flex items-center gap-2">
              <span>
                {profile.preferred_drink_type.alcoholic
                  ? '\uD83C\uDF7A'
                  : '\uD83E\uDDCA'}
              </span>
              <span className="text-sm text-gray-700">
                {profile.preferred_drink_type.name}
              </span>
              <span className="text-xs text-gray-400">
                (
                {profile.preferred_drink_type.alcoholic
                  ? 'Alcoholic'
                  : 'Non-alcoholic'}
                )
              </span>
            </div>
          ) : (
            <p className="text-sm text-gray-400">Not set yet</p>
          )}
        </button>

        {/* Password Change Card */}
        <div className="bg-white rounded-xl border border-gray-200 p-4">
          {editMode === 'password' ? (
            <PasswordChangeForm onDone={() => setEditMode(null)} />
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
  );
}

type PasswordState =
  | { status: 'idle' }
  | { status: 'error'; error: string }
  | { status: 'success' };

const PASSWORD_INITIAL: PasswordState = { status: 'idle' };

// Extracted so its useActionState starts fresh each time the password card
// is opened — the parent mounts/unmounts it via the editMode toggle, which
// means a previous "success" status can't bleed into the next open.
function PasswordChangeForm({ onDone }: { onDone: () => void }) {
  const { changePassword } = useAuth();

  const [state, submit] = useActionState<PasswordState, FormData>(
    async (_prev, formData) => {
      const currentPassword = readString(formData, 'current_password');
      const newPassword = readString(formData, 'new_password');
      const err = await changePassword(currentPassword, newPassword);
      if (err) return { status: 'error', error: err };
      return { status: 'success' };
    },
    PASSWORD_INITIAL,
  );

  // On success, close the form after a short pause so the message is
  // visible. The setState (parent's setEditMode via onDone) lives inside
  // the timer callback, not synchronously in the effect body, so the
  // "setState in effect" lint rule (project_setstate_in_effect_is_error)
  // does not apply.
  useEffect(() => {
    if (state.status !== 'success') return;
    const timer = setTimeout(onDone, 1500);
    return () => {
      clearTimeout(timer);
    };
  }, [state.status, onDone]);

  return (
    <form action={submit} className="space-y-3">
      <h3 className="text-sm font-semibold text-gray-700">Change Password</h3>
      <input
        type="password"
        name="current_password"
        autoComplete="current-password"
        placeholder="Current password"
        className="w-full px-3 py-2 bg-gray-50 border border-gray-200 rounded-lg text-sm focus:outline-none focus:border-blue-400"
        required
      />
      <input
        type="password"
        name="new_password"
        autoComplete="new-password"
        placeholder="New password (min 8 characters)"
        className="w-full px-3 py-2 bg-gray-50 border border-gray-200 rounded-lg text-sm focus:outline-none focus:border-blue-400"
        minLength={8}
        required
      />
      {state.status === 'error' && (
        <p className="text-red-500 text-xs">{state.error}</p>
      )}
      {state.status === 'success' && (
        <p className="text-green-600 text-xs">Password changed!</p>
      )}
      <div className="flex gap-2">
        <button
          type="button"
          onClick={onDone}
          className="flex-1 py-2 text-xs font-medium text-gray-500 bg-gray-100 rounded-lg"
        >
          Cancel
        </button>
        <SubmitButton
          className="flex-1 py-2 text-xs font-semibold text-white bg-blue-500 rounded-lg"
          pendingLabel="Saving..."
        >
          Change Password
        </SubmitButton>
      </div>
    </form>
  );
}
