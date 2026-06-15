import { useState, useEffect } from 'react';
import { clsx } from 'clsx';
import { Navigate, useNavigate, useParams } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import { useAuth } from '../hooks/useAuth';
import { useSession } from '../hooks/useSession';
import {
  joinSession,
  leaveSession,
  nextTrack,
  skipTurn,
} from '../api/sessions';
import { formatTime } from '../utils/time';
import type { SessionId } from '../api/brand';
import { RunEntrySheet } from '../components/RunEntrySheet';
import { BottomNav } from '../components/BottomNav';

export function Session() {
  // The route param is the external boundary where a raw URL string becomes
  // a SessionId — typing useParams with the branded type is the mint.
  // useParams returns `Partial<{ id: SessionId }>` regardless of the route's
  // declared path, so an early redirect is the type-safe way to narrow id
  // from `SessionId | undefined` to `SessionId` for everything below. In
  // production this branch is preempted by App.tsx's `path="*"` catch-all
  // (real `/session` traffic redirects from there before Session ever
  // mounts); the guard remains the type-narrowing path for tests and a
  // defense if a future route mounts Session without a `:id` segment. If
  // you remove one, look at the other.
  const { id } = useParams<{ id: SessionId }>();
  if (!id) return <Navigate to="/" replace />;
  return <SessionView id={id} />;
}

function SessionView({ id }: { id: SessionId }) {
  const { user } = useAuth();
  const { session, loading, ended } = useSession(id);
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  // Invalidate the queries a membership/state change makes stale. Membership
  // changes (join/leave) ripple into the bottom-nav and Home list as well as
  // this session's detail; track changes only touch this session's detail.
  const invalidateMembership = () => {
    void queryClient.invalidateQueries({ queryKey: ['my-session'] });
    void queryClient.invalidateQueries({ queryKey: ['sessions'] });
    void queryClient.invalidateQueries({ queryKey: ['session', id] });
  };
  const invalidateSession = () => {
    void queryClient.invalidateQueries({ queryKey: ['session', id] });
  };
  const [leaving, setLeaving] = useState(false);
  const [joiningSession, setJoiningSession] = useState(false);
  const [joinError, setJoinError] = useState<string | null>(null);
  const [headerExpanded, setHeaderExpanded] = useState(false);
  const [pickingTrack, setPickingTrack] = useState(false);
  const [skippingTrack, setSkippingTrack] = useState(false);
  const [trackError, setTrackError] = useState<string | null>(null);
  const [historyExpanded, setHistoryExpanded] = useState(true);
  const [trackImageError, setTrackImageError] = useState(false);
  const [showRunEntry, setShowRunEntry] = useState(false);

  const handleLeave = async () => {
    setLeaving(true);
    try {
      await leaveSession(id);
      invalidateMembership();
      void navigate('/');
    } catch {
      setLeaving(false);
    }
  };

  const handleJoin = async () => {
    setJoiningSession(true);
    setJoinError(null);
    try {
      await joinSession(id);
      invalidateMembership();
    } catch (e) {
      setJoinError(e instanceof Error ? e.message : 'Failed to join session');
      setJoiningSession(false);
    }
  };

  const handleNextTrack = async () => {
    setPickingTrack(true);
    setTrackError(null);
    try {
      await nextTrack(id);
      invalidateSession();
    } catch (e) {
      setTrackError(e instanceof Error ? e.message : 'Failed to pick track');
    } finally {
      setPickingTrack(false);
    }
  };

  const handleSkipTrack = async () => {
    setSkippingTrack(true);
    setTrackError(null);
    try {
      await skipTurn(id);
      invalidateSession();
    } catch (e) {
      setTrackError(e instanceof Error ? e.message : 'Failed to skip track');
    } finally {
      setSkippingTrack(false);
    }
  };

  // Reset image error state when the track changes
  useEffect(() => {
    setTrackImageError(false);
  }, [session?.current_race?.id]);

  // Clear track error when a successful poll shows the state changed
  useEffect(() => {
    if (trackError) setTrackError(null);
  }, [session?.current_race?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  // Auto-collapse history when > 3 races
  const hasMany = (session?.races.length ?? 0) > 3;
  useEffect(() => {
    if (hasMany) setHistoryExpanded(false);
  }, [hasMany]);

  // Rendered in every branch (loading / ended / default) so the tab title
  // tracks the route from mount, not only once session data resolves.
  const pageTitle = <title>Session · Beerio Kart</title>;

  if (loading) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        {pageTitle}
        <p className="text-gray-400">Loading session...</p>
      </div>
    );
  }

  if (ended || !session) {
    return (
      <div className="min-h-screen bg-gray-50 flex flex-col items-center justify-center gap-4 px-4">
        {pageTitle}
        <p className="text-3xl">{'\uD83C\uDFC1'}</p>
        <p className="text-base font-semibold text-gray-900">Session ended</p>
        <button
          onClick={() => {
            void navigate('/');
          }}
          className="px-6 py-2 bg-brand-primary text-white rounded-xl text-sm font-semibold"
        >
          Back to Home
        </button>
      </div>
    );
  }

  const activeParticipants = session.participants.filter((p) => !p.left_at);
  const isParticipant = activeParticipants.some((p) => p.user_id === user?.id);
  const isHost = user?.id === session.host_id;
  const currentRace = session.current_race;
  const pastRaces = [...session.races]
    .reverse()
    .filter((r) => r.id !== currentRace?.id);

  // Submission status
  const submissions = currentRace?.submissions ?? [];
  const mySubmission = submissions.find((s) => s.user_id === user?.id);
  const hasSubmitted = !!mySubmission;

  return (
    <div className="min-h-screen bg-gray-50 pb-20">
      {pageTitle}
      {/* Zone 1 — Session Header (sticky top) */}
      <div className="sticky top-0 z-10 bg-white border-b border-gray-200">
        <button
          onClick={() => {
            setHeaderExpanded(!headerExpanded);
          }}
          className="w-full px-4 py-3 flex items-center justify-between"
        >
          <div className="flex items-center gap-2">
            <span className="text-[10px] font-medium text-brand-primary bg-brand-tint px-2 py-0.5 rounded-full">
              {session.ruleset}
            </span>
            <span className="text-sm font-semibold text-gray-900">
              Race {session.race_number}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-gray-400">
              {activeParticipants.length} player
              {activeParticipants.length !== 1 ? 's' : ''}
            </span>
            <span className="text-gray-400 text-xs">
              {headerExpanded ? '\u25B2' : '\u25BC'}
            </span>
          </div>
        </button>

        {headerExpanded && (
          <div className="px-4 pb-3 border-t border-gray-100 pt-2 space-y-1.5">
            {activeParticipants.map((p) => (
              <div
                key={p.user_id}
                className="flex items-center justify-between py-1"
              >
                <div className="flex items-center gap-2">
                  {p.user_id === session.host_id && (
                    <span className="text-xs">{'\uD83C\uDFE0'}</span>
                  )}
                  <span className="text-sm text-gray-900">{p.username}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Zone 2 — Track Card */}
      <div className="px-4 pt-4">
        {currentRace ? (
          <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
            {trackImageError ? (
              <div className="h-40 bg-gray-100 flex items-center justify-center">
                <span className="text-4xl">{'\uD83C\uDFCE\uFE0F'}</span>
              </div>
            ) : (
              <img
                src={`/${currentRace.image_path}`}
                alt={currentRace.track_name}
                className="w-full h-40 object-contain bg-gray-100"
                onError={() => {
                  setTrackImageError(true);
                }}
              />
            )}
            <div className="px-4 py-3">
              <div className="flex items-center justify-between">
                <h2 className="text-lg font-bold text-gray-900">
                  {currentRace.track_name}
                </h2>
                <span className="text-[10px] font-semibold text-brand-primary bg-brand-tint px-2 py-0.5 rounded-full">
                  Race {currentRace.race_number}
                </span>
              </div>
              <p className="text-sm text-gray-500 mt-0.5">
                {currentRace.cup_name}
              </p>
            </div>
          </div>
        ) : (
          <div className="bg-white rounded-xl border border-gray-200 p-6 text-center">
            <div className="w-12 h-12 mx-auto mb-3 bg-gray-100 rounded-lg flex items-center justify-center">
              <span className="text-xl text-gray-300">
                {'\uD83C\uDFCE\uFE0F'}
              </span>
            </div>
            <p className="text-sm text-gray-500">
              {isHost
                ? 'Tap Next Track to get started!'
                : `Waiting for ${session.host_username} to pick a track...`}
            </p>
          </div>
        )}
      </div>

      {/* Zone 3 — Action Area */}
      <div className="px-4 pt-4 space-y-3">
        {/* Track controls */}
        {isParticipant && (
          <div className="space-y-2">
            {isHost && (
              <button
                onClick={() => {
                  void handleNextTrack();
                }}
                disabled={pickingTrack}
                className="w-full py-3 text-sm font-semibold text-white bg-brand-primary rounded-xl disabled:opacity-50 active:bg-brand-primary-strong transition-colors"
              >
                {pickingTrack ? 'Picking track...' : 'Next Track'}
              </button>
            )}
            {currentRace && (
              <button
                onClick={() => {
                  void handleSkipTrack();
                }}
                disabled={skippingTrack}
                className="w-full py-2.5 text-sm font-medium text-gray-500 bg-white border border-gray-200 rounded-xl disabled:opacity-50 active:bg-gray-50 transition-colors"
              >
                {skippingTrack ? 'Re-rolling...' : 'Skip Track'}
              </button>
            )}
            {trackError && (
              <p className="text-xs text-danger text-center">{trackError}</p>
            )}
          </div>
        )}

        {/* Submit Time / Your Time */}
        {isParticipant && currentRace && (
          <>
            {hasSubmitted ? (
              <div
                className={clsx(
                  'rounded-xl border p-4 text-center',
                  mySubmission.disqualified
                    ? 'bg-red-50 border-red-200'
                    : 'bg-green-50 border-green-200',
                )}
              >
                <p
                  className={clsx(
                    'text-xs font-semibold uppercase tracking-wider mb-1',
                    mySubmission.disqualified ? 'text-danger' : 'text-success',
                  )}
                >
                  {mySubmission.disqualified ? 'Your Time (DQ)' : 'Your Time'}
                </p>
                <p
                  className={clsx(
                    'text-2xl font-mono font-bold',
                    mySubmission.disqualified
                      ? 'text-red-600 line-through'
                      : 'text-green-700',
                  )}
                >
                  {formatTime(mySubmission.track_time)}
                </p>
              </div>
            ) : (
              <button
                onClick={() => {
                  setShowRunEntry(true);
                }}
                className="w-full py-3 text-sm font-semibold text-white bg-brand-primary rounded-xl active:bg-brand-primary-strong transition-colors"
              >
                Submit Time
              </button>
            )}
          </>
        )}

        {/* Participant cards — submission-aware */}
        <div>
          <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wider px-1 mb-2">
            Players
          </h3>
          <div className="bg-white rounded-xl border border-gray-200 divide-y divide-gray-100">
            {activeParticipants.map((p) => {
              const sub = submissions.find((s) => s.user_id === p.user_id);
              return (
                <div
                  key={p.user_id}
                  className="px-4 py-3 flex items-center justify-between"
                >
                  <div className="flex items-center gap-2">
                    {p.user_id === session.host_id && (
                      <span className="text-xs">{'\uD83C\uDFE0'}</span>
                    )}
                    <span className="text-sm font-medium text-gray-900">
                      {p.username}
                    </span>
                  </div>
                  {currentRace ? (
                    sub ? (
                      sub.disqualified ? (
                        <span className="text-xs font-medium text-danger">
                          DQ {formatTime(sub.track_time)}
                        </span>
                      ) : (
                        <span className="text-xs font-medium text-success">
                          {'\u2705'} {formatTime(sub.track_time)}
                        </span>
                      )
                    ) : (
                      <span className="text-xs text-gray-400">
                        {'\u23F3'} Racing...
                      </span>
                    )
                  ) : (
                    <span className="text-xs text-gray-400">
                      {'\u23F3'} waiting
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        </div>

        {/* Race History */}
        {pastRaces.length > 0 && (
          <div>
            <button
              onClick={() => {
                setHistoryExpanded(!historyExpanded);
              }}
              className="w-full flex items-center justify-between px-1 mb-2"
            >
              <div className="flex items-center gap-2">
                <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wider">
                  Race History
                </h3>
                <span className="text-[10px] font-medium text-gray-400 bg-gray-100 px-1.5 py-0.5 rounded-full">
                  {pastRaces.length}
                </span>
              </div>
              <span className="text-gray-400 text-xs">
                {historyExpanded ? '\u25B2' : '\u25BC'}
              </span>
            </button>
            {historyExpanded && (
              <div className="bg-white rounded-xl border border-gray-200 divide-y divide-gray-100">
                {pastRaces.map((race) => (
                  <div
                    key={race.id}
                    className="px-4 py-3 flex items-center justify-between"
                  >
                    <div className="flex items-center gap-3">
                      <span className="text-xs font-semibold text-gray-400 w-5 text-center">
                        {race.race_number}
                      </span>
                      <div>
                        <p className="text-sm font-medium text-gray-900">
                          {race.track_name}
                        </p>
                        <p className="text-xs text-gray-400">{race.cup_name}</p>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Session info */}
        <div className="bg-white rounded-xl border border-gray-200 p-4">
          <div className="flex justify-between text-xs text-gray-400">
            <span>Host: {session.host_username}</span>
            <span>Ruleset: {session.ruleset}</span>
          </div>
        </div>

        {isParticipant ? (
          <button
            onClick={() => {
              void handleLeave();
            }}
            disabled={leaving}
            className="w-full py-3 text-sm font-medium text-danger bg-white border border-red-200 rounded-xl disabled:opacity-50 active:bg-red-50 transition-colors"
          >
            {leaving ? 'Leaving...' : 'Leave Session'}
          </button>
        ) : (
          <div className="space-y-2">
            <button
              onClick={() => {
                void handleJoin();
              }}
              disabled={joiningSession}
              className="w-full py-3 text-sm font-semibold text-white bg-brand-primary rounded-xl disabled:opacity-50 active:bg-brand-primary-strong transition-colors"
            >
              {joiningSession ? 'Joining...' : 'Join Session'}
            </button>
            {joinError && (
              <p className="text-xs text-danger text-center">{joinError}</p>
            )}
          </div>
        )}
      </div>

      <BottomNav />

      {/* Run entry bottom sheet */}
      {showRunEntry && currentRace && (
        <RunEntrySheet
          race={currentRace}
          onClose={() => {
            setShowRunEntry(false);
          }}
          onSubmitted={() => {
            // The submitted run isn't in the cached session detail yet;
            // invalidate so it shows up without waiting for the next poll.
            invalidateSession();
            setShowRunEntry(false);
          }}
        />
      )}
    </div>
  );
}
