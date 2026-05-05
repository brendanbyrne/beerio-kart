import { useState } from "react";

/*
 * Session Screen Mockup v3 — Pixel 9 Pro reference
 * Physical: 1280 × 2856 @ 495 ppi
 * Logical (CSS): ~427 × 952 @ 3× DPR
 *
 * All four session states rendered simultaneously for comparison.
 *
 * Changes from v2:
 * 1. Lap times: run entry form now requires all 3 lap times + total (4 time inputs)
 * 2. Race history: tappable rows expand inline to show summary + "View Full Race" button
 * 3. Race detail view: replaces the hero track card with full per-race leaderboard
 *    showing all participants' times, lap breakdowns, drinks, DQ status
 * 4. Richer mock data to support lap times and multi-participant results
 * 5. Participant dropdown shows head-to-head record vs each other player
 */

// --- Mock Data ---
const TRACK = { name: "Coconut Mall", cup: "Banana Cup", raceNum: 3 };
const PENDING_TRACK = { name: "Mario Circuit", cup: "Mushroom Cup", raceNum: 2 };

const PARTICIPANTS = [
  { id: 1, name: "Brendan", isHost: true, status: "submitted", time: "2:14.523", dq: false },
  { id: 2, name: "Katie", isHost: false, status: "pending", time: null, dq: false },
  { id: 3, name: "Mike", isHost: false, status: "submitted", time: "2:31.087", dq: false },
  { id: 4, name: "Jess", isHost: false, status: "submitted", time: null, dq: true },
];

// Head-to-head records from the current user's (Brendan's) perspective
// wins-losses format: "your wins - their wins"
const HEAD_TO_HEAD = {
  2: { wins: 3, losses: 5 },  // vs Katie: 3-5
  3: { wins: 6, losses: 2 },  // vs Mike: 6-2
  4: { wins: 4, losses: 4 },  // vs Jess: 4-4
};

const RACE_HISTORY = [
  {
    num: 3, track: "Coconut Mall", cup: "Banana Cup", active: true,
    results: [],
  },
  {
    num: 2, track: "Mario Circuit", cup: "Mushroom Cup", active: false,
    myTime: "2:08.341", winner: "Katie", hasRecord: true,
    results: [
      { name: "Katie", time: "1:58.210", laps: ["0:38.112", "0:39.881", "0:40.217"], lapRecords: [true, false, false], drink: "Sparkling Water", dq: false, winner: true, isRecord: true },
      { name: "Brendan", time: "2:08.341", laps: ["0:41.002", "0:43.118", "0:44.221"], lapRecords: [false, false, false], drink: "Molson Canadian", dq: false, winner: false },
      { name: "Mike", time: "2:15.773", laps: ["0:43.551", "0:45.109", "0:47.113"], lapRecords: [false, false, false], drink: "Guinness", dq: false, winner: false },
      { name: "Jess", time: "2:22.009", laps: ["0:44.801", "0:46.332", "0:50.876"], lapRecords: [false, false, false], drink: "Sparkling Water", dq: true, winner: false },
    ],
  },
  {
    num: 1, track: "Rainbow Road", cup: "Special Cup", active: false,
    myTime: "3:22.109", winner: "Brendan",
    results: [
      { name: "Brendan", time: "3:22.109", laps: ["1:05.441", "1:07.228", "1:09.440"], lapRecords: [false, true, false], drink: "Molson Canadian", dq: false, winner: true },
      { name: "Mike", time: "3:31.887", laps: ["1:08.112", "1:11.442", "1:12.333"], lapRecords: [false, false, false], drink: "Guinness", dq: false, winner: false },
      { name: "Katie", time: "3:45.002", laps: ["1:12.331", "1:14.109", "1:18.562"], lapRecords: [false, false, false], drink: "Sparkling Water", dq: false, winner: false },
      { name: "Jess", time: "3:58.119", laps: ["1:15.221", "1:19.887", "1:23.011"], lapRecords: [false, false, false], drink: "Sparkling Water", dq: false, winner: false },
    ],
  },
];

// --- Shared Components ---

function StatusBadge({ status, dq }) {
  if (dq) return <span className="text-[11px] bg-red-100 text-red-700 px-2 py-0.5 rounded-full font-medium">DQ</span>;
  if (status === "submitted") return <span className="text-[11px] bg-green-100 text-green-700 px-2 py-0.5 rounded-full font-medium">&#10003; Done</span>;
  if (status === "pending") return <span className="text-[11px] bg-amber-100 text-amber-700 px-2 py-0.5 rounded-full font-medium">&#9203; Racing</span>;
  return null;
}

function HeadToHeadBadge({ h2h }) {
  if (!h2h) return null;
  const isWinning = h2h.wins > h2h.losses;
  const isLosing = h2h.wins < h2h.losses;
  const color = isWinning ? "text-green-700 bg-green-50" : isLosing ? "text-red-600 bg-red-50" : "text-gray-500 bg-gray-100";
  return (
    <span className={`text-[10px] font-mono font-semibold px-1.5 py-0.5 rounded ${color}`}>
      {h2h.wins}-{h2h.losses}
    </span>
  );
}

function ParticipantRow({ p, isCurrentUser }) {
  const h2h = !isCurrentUser ? HEAD_TO_HEAD[p.id] : null;
  return (
    <div className="flex items-center min-h-[44px] py-2 px-1 gap-2">
      <div className={`w-8 h-8 rounded-full flex items-center justify-center text-[12px] font-bold ${isCurrentUser ? "bg-blue-100 text-blue-600 ring-2 ring-blue-300" : "bg-gray-200 text-gray-500"}`}>
        {p.name[0]}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <span className="text-[14px] font-semibold text-gray-900">{p.name}</span>
          {isCurrentUser && <span className="text-[10px] text-blue-500 font-medium">(you)</span>}
          {p.isHost && <span className="text-[11px] text-amber-500">&#128081;</span>}
        </div>
        {h2h && (
          <div className="flex items-center gap-1 mt-0.5">
            <span className="text-[10px] text-gray-400">H2H:</span>
            <HeadToHeadBadge h2h={h2h} />
          </div>
        )}
      </div>
    </div>
  );
}

function TrackCard({ track, state, variant }) {
  if (state === "waiting") {
    return (
      <div className="mx-3 rounded-2xl bg-gray-50 border-2 border-dashed border-gray-300 flex flex-col items-center justify-center py-10 px-5">
        <div className="text-3xl mb-2 opacity-50">&#127918;</div>
        <div className="text-gray-400 font-medium text-[13px] text-center">Waiting for host to pick the next track...</div>
      </div>
    );
  }
  if (variant === "compact") {
    return (
      <div className="mx-3 rounded-xl overflow-hidden bg-white border border-gray-200 flex items-center gap-3 px-3 py-2.5 opacity-60">
        <div className="w-10 h-10 rounded-lg bg-gradient-to-br from-sky-400 via-emerald-400 to-amber-300 flex-shrink-0" />
        <div className="flex-1 min-w-0">
          <div className="text-[13px] font-medium text-gray-600 truncate">{track.name}</div>
          <div className="text-[11px] text-gray-400">{track.cup} &middot; Race {track.raceNum} &middot; Current</div>
        </div>
      </div>
    );
  }
  if (variant === "pending") {
    return (
      <div className="mx-3 rounded-2xl overflow-hidden shadow-md bg-white border-2 border-amber-400">
        <div className="h-32 bg-gradient-to-br from-orange-300 via-red-300 to-pink-400 flex items-end">
          <div className="w-full bg-gradient-to-t from-black/60 to-transparent px-4 pb-3 pt-8">
            <div className="text-white font-bold text-lg leading-tight">{track.name}</div>
          </div>
        </div>
        <div className="px-4 py-2 flex items-center justify-between bg-amber-50">
          <span className="text-[11px] text-amber-700 font-medium">{track.cup}</span>
          <span className="text-[11px] text-amber-600 font-semibold">Pending &middot; Race {track.raceNum}</span>
        </div>
      </div>
    );
  }
  return (
    <div className="mx-3 rounded-2xl overflow-hidden shadow-md bg-white border border-gray-200">
      <div className="h-32 bg-gradient-to-br from-sky-400 via-emerald-400 to-amber-300 flex items-end">
        <div className="w-full bg-gradient-to-t from-black/60 to-transparent px-4 pb-3 pt-8">
          <div className="text-white font-bold text-lg leading-tight">{track.name}</div>
        </div>
      </div>
      <div className="px-4 py-2 flex items-center justify-between">
        <span className="text-[11px] text-gray-500 font-medium">{track.cup}</span>
        <span className="text-[11px] text-gray-400">Race {track.raceNum}</span>
      </div>
    </div>
  );
}

// --- Race Detail View (replaces hero track card) ---

function RaceDetailView({ race, onBack }) {
  const isActive = race.active;
  return (
    <div className="mx-3">
      <button onClick={onBack} className="flex items-center gap-1 min-h-[44px] py-2 text-blue-600 text-[13px] font-medium">
        <span>&#8592;</span>
        <span>Back to current</span>
      </button>
      <div className={`rounded-2xl overflow-hidden shadow-md bg-white border mb-3 ${isActive ? "border-blue-200" : "border-gray-200"}`}>
        <div className={`h-24 flex items-end ${isActive ? "bg-gradient-to-br from-blue-400 via-sky-400 to-blue-500" : "bg-gradient-to-br from-sky-400 via-emerald-400 to-amber-300"}`}>
          <div className="w-full bg-gradient-to-t from-black/60 to-transparent px-4 pb-2.5 pt-6">
            <div className="flex items-center gap-2">
              <div className="text-white font-bold text-[16px] leading-tight">{race.track}</div>
              {isActive && <span className="text-[10px] font-medium text-blue-100 bg-white/20 px-2 py-0.5 rounded-full">In Progress</span>}
            </div>
            <div className="text-white/80 text-[11px]">{race.cup} &middot; Race {race.num}</div>
          </div>
        </div>
        <div className="px-3 py-2">
          {/* Active race: show participant submission status */}
          {isActive && PARTICIPANTS.map((p, i) => (
            <div key={p.id} className={`flex items-center justify-between py-2.5 ${i < PARTICIPANTS.length - 1 ? "border-b border-gray-100" : ""}`}>
              <div className="flex items-center gap-2">
                <div className={`w-5 h-5 rounded-full flex items-center justify-center text-[9px] font-bold ${p.id === 1 ? "bg-blue-100 text-blue-600" : "bg-gray-200 text-gray-500"}`}>{p.name[0]}</div>
                <div>
                  <span className={`text-[13px] font-semibold ${p.id === 1 ? "text-blue-700" : "text-gray-900"}`}>{p.name}</span>
                  {p.id === 1 && <span className="text-[10px] text-blue-400 ml-1">(you)</span>}
                  {p.isHost && <span className="text-[11px] text-amber-500 ml-1">&#128081;</span>}
                </div>
              </div>
              <div className="flex items-center gap-2">
                {p.status === "submitted" && !p.dq && p.time && <span className="text-[13px] font-mono text-gray-500">{p.time}</span>}
                <StatusBadge status={p.status} dq={p.dq} />
              </div>
            </div>
          ))}
          {/* Completed race: show full results with laps */}
          {!isActive && race.results.map((r, i) => (
            <div key={i} className={`py-2.5 ${i < race.results.length - 1 ? "border-b border-gray-100" : ""}`}>
              <div className="flex items-center justify-between min-h-[36px]">
                <div className="flex items-center gap-2">
                  <div className="w-5 h-5 rounded-full bg-gray-200 flex items-center justify-center text-[9px] font-bold text-gray-500">
                    {r.winner ? <span className="text-amber-500">&#128081;</span> : (i + 1)}
                  </div>
                  <div>
                    <span className={`text-[13px] font-semibold ${r.dq ? "text-gray-400 line-through" : "text-gray-900"}`}>{r.name}</span>
                    <span className="text-[10px] text-gray-400 ml-1.5">{r.drink}</span>
                  </div>
                </div>
                <div className="flex items-center gap-1.5">
                  <span className={`text-[13px] font-mono ${r.dq ? "text-gray-400 line-through" : r.winner ? "text-green-700 font-bold" : "text-gray-600"}`}>{r.time}</span>
                  {r.isRecord && <span className="text-[10px] text-amber-600 bg-amber-50 border border-amber-200 px-1.5 py-0 rounded-full font-semibold leading-relaxed">&#9733; Record</span>}
                  {r.dq && <span className="text-[10px] bg-red-100 text-red-700 px-1.5 py-0.5 rounded-full font-medium">DQ</span>}
                </div>
              </div>
              <div className="flex gap-3 mt-1 ml-7">
                {r.laps.map((lap, li) => (
                  <div key={li} className="flex items-center gap-0.5 text-[10px] text-gray-400">
                    <span className="text-gray-300">L{li + 1}</span>
                    <span className="font-mono">{lap}</span>
                    {r.lapRecords && r.lapRecords[li] && <span className="text-amber-500">&#9733;</span>}
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// --- Race History (tappable, expandable) ---

function RaceHistoryItem({ race, isLast, onViewDetail }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className={`${!isLast ? "border-b border-gray-100" : ""}`}>
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between min-h-[44px] py-2.5 px-1.5 text-left active:bg-gray-100"
      >
        <div className="flex items-center gap-2.5">
          <div className="w-6 h-6 rounded-full bg-gray-100 flex items-center justify-center text-[11px] font-bold text-gray-400">{race.num}</div>
          <div>
            <div className="flex items-center gap-1.5">
              <span className="text-[13px] font-medium text-gray-800">{race.track}</span>
              {race.hasRecord && <span className="text-[10px] text-amber-600 bg-amber-50 border border-amber-200 px-1.5 py-0 rounded-full font-semibold leading-relaxed">&#9733; Record</span>}
            </div>
            <div className="text-[11px] text-gray-400">{race.cup}</div>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {race.active ? (
            <>
              <span className="text-[11px] font-medium text-blue-600 bg-blue-50 px-2 py-0.5 rounded-full">In Progress</span>
              <span className={`text-gray-400 text-[11px] transition-transform ${expanded ? "rotate-180" : ""}`}>&#9660;</span>
            </>
          ) : (
            <>
              <div className="text-right">
                <div className="text-[13px] font-mono text-gray-600">{race.myTime}</div>
                <div className="text-[11px] text-gray-400">Won by {race.winner}</div>
              </div>
              <span className={`text-gray-400 text-[11px] transition-transform ${expanded ? "rotate-180" : ""}`}>&#9660;</span>
            </>
          )}
        </div>
      </button>
      {/* Active race expansion: participant status */}
      {expanded && race.active && (
        <div className="px-1.5 pb-2.5">
          <div className="bg-blue-50/50 rounded-xl border border-blue-100 px-3 py-2">
            {PARTICIPANTS.map((p, i) => (
              <div key={p.id} className={`flex items-center justify-between py-1.5 ${i < PARTICIPANTS.length - 1 ? "border-b border-blue-100/50" : ""}`}>
                <div className="flex items-center gap-2">
                  <div className={`w-5 h-5 rounded-full flex items-center justify-center text-[9px] font-bold ${p.id === 1 ? "bg-blue-100 text-blue-600" : "bg-gray-200 text-gray-500"}`}>{p.name[0]}</div>
                  <span className={`text-[12px] font-medium ${p.id === 1 ? "text-blue-700" : "text-gray-700"}`}>{p.name}</span>
                  {p.id === 1 && <span className="text-[9px] text-blue-400">(you)</span>}
                </div>
                <div className="flex items-center gap-1.5">
                  {p.status === "submitted" && p.time && <span className="text-[12px] font-mono text-gray-500">{p.time}</span>}
                  <StatusBadge status={p.status} dq={p.dq} />
                </div>
              </div>
            ))}
            <button
              onClick={(e) => { e.stopPropagation(); onViewDetail(race); }}
              className="w-full mt-2 py-2.5 text-[12px] font-semibold text-blue-600 bg-blue-100 rounded-lg active:bg-blue-200 transition-colors min-h-[40px]"
            >
              View Full Race &#8594;
            </button>
          </div>
        </div>
      )}
      {/* Completed race expansion: results preview */}
      {expanded && !race.active && (
        <div className="px-1.5 pb-2.5">
          <div className="bg-white rounded-xl border border-gray-200 px-3 py-2">
            {race.results.slice(0, 3).map((r, i) => (
              <div key={i} className="flex items-center justify-between py-1.5">
                <div className="flex items-center gap-2">
                  <span className="text-[11px] text-gray-400 w-4">{r.winner ? "\u{1F451}" : `${i+1}.`}</span>
                  <span className={`text-[12px] ${r.dq ? "text-gray-400 line-through" : "text-gray-700"} font-medium`}>{r.name}</span>
                </div>
                <div className="flex items-center gap-1.5">
                  <span className={`text-[12px] font-mono ${r.dq ? "text-gray-400" : r.winner ? "text-green-700" : "text-gray-500"}`}>{r.time}</span>
                  {r.isRecord && <span className="text-amber-500 text-[10px]">&#9733;</span>}
                </div>
              </div>
            ))}
            {race.results.length > 3 && (
              <div className="text-[11px] text-gray-400 py-1">+{race.results.length - 3} more</div>
            )}
            <button
              onClick={(e) => { e.stopPropagation(); onViewDetail(race); }}
              className="w-full mt-2 py-2.5 text-[12px] font-semibold text-blue-600 bg-blue-50 rounded-lg active:bg-blue-100 transition-colors min-h-[40px]"
            >
              View Full Race &#8594;
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

// --- Slide-to-Confirm DQ ---

function SlideToConfirm({ confirmed, onConfirm, onReset }) {
  const trackRef = React.useRef(null);
  const [dragging, setDragging] = React.useState(false);
  const [offsetX, setOffsetX] = React.useState(0);
  const thumbW = 44;
  const getTrackW = () => (trackRef.current ? trackRef.current.offsetWidth : 280);

  const handleStart = (clientX) => {
    if (confirmed) return;
    setDragging(true);
  };

  const handleMove = (clientX) => {
    if (!dragging || confirmed) return;
    const rect = trackRef.current.getBoundingClientRect();
    const x = Math.min(Math.max(0, clientX - rect.left - thumbW / 2), getTrackW() - thumbW);
    setOffsetX(x);
  };

  const handleEnd = () => {
    if (!dragging) return;
    setDragging(false);
    const threshold = getTrackW() - thumbW - 4;
    if (offsetX >= threshold) {
      setOffsetX(getTrackW() - thumbW);
      onConfirm();
    } else {
      setOffsetX(0);
    }
  };

  // Reset thumb position when unconfirmed externally
  React.useEffect(() => { if (!confirmed) setOffsetX(0); }, [confirmed]);

  const progress = Math.min(offsetX / (getTrackW() - thumbW), 1);

  if (confirmed) {
    return (
      <button
        onClick={onReset}
        className="w-full flex items-center justify-between px-3.5 min-h-[48px] bg-red-50 border border-red-200 rounded-xl text-left"
      >
        <div>
          <div className="text-[14px] font-semibold text-red-700">Disqualified</div>
          <div className="text-[11px] text-red-400 mt-0.5">Excluded from leaderboards &middot; Tap to undo</div>
        </div>
        <span className="text-red-400 text-[18px]">&#10005;</span>
      </button>
    );
  }

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
      {/* Label that fades as thumb slides */}
      <div
        className="absolute inset-0 flex items-center justify-center pointer-events-none"
        style={{ opacity: 1 - progress * 1.5 }}
      >
        <span className="text-[13px] font-medium text-gray-400 tracking-wide">Slide to DQ &rarr;</span>
      </div>
      {/* Thumb */}
      <div
        className="absolute top-1 h-10 rounded-lg bg-red-500 shadow-md flex items-center justify-center cursor-grab active:cursor-grabbing"
        style={{ width: thumbW, left: offsetX + 4, transition: dragging ? "none" : "left 0.25s ease" }}
        onMouseDown={(e) => { e.preventDefault(); handleStart(e.clientX); }}
        onTouchStart={(e) => handleStart(e.touches[0].clientX)}
      >
        <span className="text-white text-[16px] font-bold">&raquo;</span>
      </div>
    </div>
  );
}

// --- Run Entry Bottom Sheet (v3: with lap times) ---

function RunEntrySheet({ onClose, track }) {
  const [dq, setDq] = useState(false);
  const t = track || TRACK;
  return (
    <div className="absolute inset-0 z-30 flex flex-col justify-end">
      <div className="absolute inset-0 bg-black/40" onClick={onClose} />
      <div className="relative bg-white rounded-t-2xl shadow-2xl flex flex-col" style={{ maxHeight: "92%" }} onClick={(e) => e.stopPropagation()}>
        <div className="flex justify-center pt-2.5 pb-1">
          <div className="w-9 h-1 bg-gray-300 rounded-full" />
        </div>
        <div className="overflow-y-auto px-4 pb-8 flex-1">
          {/* Track header */}
          <div className="flex items-center gap-3 mb-5 mt-1">
            <div className="w-10 h-10 rounded-lg bg-gradient-to-br from-sky-400 to-emerald-400 flex-shrink-0" />
            <div>
              <div className="font-semibold text-gray-900 text-[14px]">{t.name}</div>
              <div className="text-[12px] text-gray-500">{t.cup} &middot; Race {t.raceNum}</div>
            </div>
          </div>

          {/* Total Time */}
          <div className="mb-4">
            <label className="block text-[11px] font-semibold text-gray-500 uppercase tracking-wide mb-2">Total Time</label>
            <div className="flex items-center gap-1.5 justify-center">
              <input className="w-14 h-12 text-center text-xl font-mono bg-gray-100 rounded-xl border border-gray-300 outline-none" value="2" readOnly />
              <span className="text-xl font-mono text-gray-400 font-bold">:</span>
              <input className="w-16 h-12 text-center text-xl font-mono bg-gray-100 rounded-xl border border-gray-300 outline-none" value="14" readOnly />
              <span className="text-xl font-mono text-gray-400 font-bold">.</span>
              <input className="w-[72px] h-12 text-center text-xl font-mono bg-gray-100 rounded-xl border border-gray-300 outline-none" value="523" readOnly />
            </div>
          </div>

          {/* Lap Times — 3 rows, compact but 48px touch targets */}
          <div className="mb-5">
            <label className="block text-[11px] font-semibold text-gray-500 uppercase tracking-wide mb-2">Lap Times</label>
            <div className="space-y-2">
              {[
                { lap: 1, m: "0", s: "41", ms: "002" },
                { lap: 2, m: "0", s: "43", ms: "118" },
                { lap: 3, m: "0", s: "44", ms: "221" },
              ].map((l) => (
                <div key={l.lap} className="flex items-center gap-2">
                  <span className="text-[11px] font-semibold text-gray-400 w-6 text-right">L{l.lap}</span>
                  <div className="flex items-center gap-1 flex-1 justify-center">
                    <input className="w-10 h-10 text-center text-[15px] font-mono bg-gray-50 rounded-lg border border-gray-200 outline-none" value={l.m} readOnly />
                    <span className="text-[15px] font-mono text-gray-300 font-bold">:</span>
                    <input className="w-12 h-10 text-center text-[15px] font-mono bg-gray-50 rounded-lg border border-gray-200 outline-none" value={l.s} readOnly />
                    <span className="text-[15px] font-mono text-gray-300 font-bold">.</span>
                    <input className="w-14 h-10 text-center text-[15px] font-mono bg-gray-50 rounded-lg border border-gray-200 outline-none" value={l.ms} readOnly />
                  </div>
                </div>
              ))}
            </div>
            <div className="text-[10px] text-gray-400 mt-1.5 text-center">Lap times should add up to total time</div>
          </div>

          {/* Drink */}
          <div className="mb-4">
            <label className="block text-[11px] font-semibold text-gray-500 uppercase tracking-wide mb-2">Drink</label>
            <button className="w-full flex items-center justify-between px-3.5 min-h-[48px] bg-gray-50 border border-gray-200 rounded-xl text-left">
              <div className="flex items-center gap-2.5">
                <span className="text-base">&#127866;</span>
                <span className="text-[14px] font-medium text-gray-800">Molson Canadian</span>
              </div>
              <span className="text-gray-400 text-[12px]">Change</span>
            </button>
          </div>

          {/* Race Setup */}
          <div className="mb-4">
            <label className="block text-[11px] font-semibold text-gray-500 uppercase tracking-wide mb-2">Race Setup</label>
            <button className="w-full flex items-center justify-between px-3.5 min-h-[48px] bg-gray-50 border border-gray-200 rounded-xl text-left">
              <div className="flex items-center gap-2.5">
                <div className="flex -space-x-1">
                  <div className="w-7 h-7 rounded-full bg-red-200 border-2 border-white flex items-center justify-center text-[10px]">&#127939;</div>
                  <div className="w-7 h-7 rounded-full bg-blue-200 border-2 border-white flex items-center justify-center text-[10px]">&#128663;</div>
                  <div className="w-7 h-7 rounded-full bg-gray-200 border-2 border-white flex items-center justify-center text-[10px]">&#9898;</div>
                  <div className="w-7 h-7 rounded-full bg-green-200 border-2 border-white flex items-center justify-center text-[10px]">&#129666;</div>
                </div>
                <span className="text-[13px] font-medium text-gray-800">Mario &middot; Standard &middot; Normal &middot; Cloud</span>
              </div>
              <span className="text-gray-400 text-[12px]">Edit</span>
            </button>
          </div>

          {/* DQ — slide to confirm (prevents accidental taps) */}
          <div className="mb-4">
            <label className="block text-[11px] font-semibold text-gray-500 uppercase tracking-wide mb-2">Didn't finish drink?</label>
            <SlideToConfirm
              confirmed={dq}
              onConfirm={() => setDq(true)}
              onReset={() => setDq(false)}
            />
          </div>

          {/* Photo */}
          <div className="mb-6">
            <label className="block text-[11px] font-semibold text-gray-500 uppercase tracking-wide mb-2">Photo (optional)</label>
            <button className="flex items-center gap-2.5 px-3.5 min-h-[48px] bg-gray-50 border border-dashed border-gray-300 rounded-xl text-gray-500 text-[13px]">
              <span className="text-lg">&#128247;</span>
              <span>Add photo</span>
            </button>
          </div>

          {/* Submit */}
          <button className="w-full py-4 bg-blue-600 text-white font-semibold rounded-2xl text-[15px] shadow-sm active:scale-[0.98] transition-transform">
            Submit Run
          </button>
        </div>
      </div>
    </div>
  );
}

// --- Phone Frame Wrapper ---

function PhoneFrame({ label, children }) {
  return (
    <div className="flex flex-col items-center gap-2">
      <div className="text-[11px] font-semibold text-gray-500 uppercase tracking-wider">{label}</div>
      <div
        className="bg-white rounded-[2rem] shadow-xl overflow-hidden relative flex flex-col border border-gray-300"
        style={{ width: 427, height: 952 }}
      >
        {/* Status bar */}
        <div className="h-10 bg-white flex items-end justify-between px-7 pb-0.5 flex-shrink-0">
          <span className="text-[11px] font-semibold text-gray-900">9:41</span>
          <div className="flex gap-1 items-center">
            <svg width="14" height="10" viewBox="0 0 14 10"><rect x="0" y="4" width="2" height="6" rx="0.5" fill="#222"/><rect x="3" y="2.5" width="2" height="7.5" rx="0.5" fill="#222"/><rect x="6" y="1" width="2" height="9" rx="0.5" fill="#222"/><rect x="9" y="0" width="2" height="10" rx="0.5" fill="#222"/></svg>
            <svg width="12" height="10" viewBox="0 0 24 18" fill="#222"><path d="M1.3 7.8a14.6 14.6 0 0 1 21.4 0l-2.2 2.2a11.2 11.2 0 0 0-17 0L1.3 7.8ZM6.4 13a8 8 0 0 1 11.2 0l-2.1 2.2a4.6 4.6 0 0 0-7 0L6.4 13Z"/><circle cx="12" cy="17" r="1.8"/></svg>
            <svg width="22" height="10" viewBox="0 0 28 12" fill="none"><rect x="0.5" y="0.5" width="23" height="11" rx="2" stroke="#222" strokeWidth="1"/><rect x="24" y="3.5" width="2.5" height="5" rx="1" fill="#222"/><rect x="2" y="2" width="16" height="8" rx="1" fill="#34C759"/></svg>
          </div>
        </div>
        {children}
        {/* Bottom tab bar */}
        <div className="bg-white border-t border-gray-200 flex items-center justify-around py-2 pb-7 flex-shrink-0">
          <button className="flex flex-col items-center gap-0.5 px-5 py-1.5 text-gray-400">
            <span className="text-lg">&#127968;</span>
            <span className="text-[10px] font-medium">Home</span>
          </button>
          <button className="flex flex-col items-center gap-0.5 px-5 py-1.5 text-blue-600">
            <span className="text-lg">&#127918;</span>
            <span className="text-[10px] font-medium">Session</span>
          </button>
          <button className="flex flex-col items-center gap-0.5 px-5 py-1.5 text-gray-400">
            <span className="text-lg">&#128100;</span>
            <span className="text-[10px] font-medium">Profile</span>
          </button>
        </div>
      </div>
    </div>
  );
}

// --- Session Header (STICKY, with H2H in participant dropdown) ---

function SessionHeader({ showParticipants, setShowParticipants }) {
  return (
    <div className="bg-white border-b border-gray-100 px-3.5 py-2.5 flex-shrink-0 sticky top-0 z-10">
      <div className="flex items-center justify-between min-h-[40px]">
        <div className="flex items-center gap-2">
          <span className="text-[11px] font-bold text-blue-600 bg-blue-50 px-2 py-1 rounded-md uppercase">Random</span>
          <span className="text-[13px] text-gray-400">&middot;</span>
          <span className="text-[14px] font-medium text-gray-700">Race 3</span>
        </div>
        <button
          onClick={() => setShowParticipants(!showParticipants)}
          className="flex items-center gap-1.5 px-2.5 py-2 rounded-full bg-gray-50 border border-gray-200 min-h-[40px]"
        >
          <div className="flex -space-x-1.5">
            {PARTICIPANTS.slice(0, 3).map((p) => (
              <div key={p.id} className="rounded-full bg-gray-300 border-2 border-white flex items-center justify-center text-[8px] font-bold text-gray-500" style={{ width: 20, height: 20 }}>
                {p.name[0]}
              </div>
            ))}
          </div>
          <span className="text-[12px] text-gray-500 font-medium">{PARTICIPANTS.length}</span>
        </button>
      </div>
      {showParticipants && (
        <div className="mt-2.5 pt-2.5 border-t border-gray-100">
          {PARTICIPANTS.map((p) => (
            <ParticipantRow key={p.id} p={p} isCurrentUser={p.id === 1} />
          ))}
        </div>
      )}
    </div>
  );
}

// --- Individual State Screens ---

function NeedToSubmitScreen() {
  const [showP, setShowP] = useState(false);
  const [showH, setShowH] = useState(false);
  const [showEntry, setShowEntry] = useState(false);
  const [viewingRace, setViewingRace] = useState(null);

  return (
    <PhoneFrame label="Need to Submit">
      <SessionHeader showParticipants={showP} setShowParticipants={setShowP} />
      <div className="flex-1 overflow-y-auto">
        {viewingRace ? (
          <RaceDetailView race={viewingRace} onBack={() => setViewingRace(null)} />
        ) : (
          <>
            <div className="pt-3.5 pb-3.5">
              <TrackCard track={TRACK} state="active" />
            </div>
            <div className="px-3.5 pb-3.5">
              <button
                onClick={() => setShowEntry(true)}
                className="w-full py-4 bg-blue-600 text-white font-semibold rounded-2xl text-[15px] shadow-sm active:scale-[0.98] transition-transform"
              >
                Submit Time
              </button>
            </div>
          </>
        )}
        <div className="px-3.5 pb-20">
          <button onClick={() => setShowH(!showH)} className="w-full flex items-center justify-between min-h-[48px] py-3">
            <span className="font-semibold text-gray-500 uppercase tracking-wide text-[11px]">Race History</span>
            <span className="text-gray-400 text-[11px]">{showH ? "Hide" : "Show"} ({RACE_HISTORY.length})</span>
          </button>
          {showH && (
            <div className="bg-gray-50 rounded-xl px-2.5 py-0.5">
              {RACE_HISTORY.map((r, i) => (
                <RaceHistoryItem
                  key={r.num}
                  race={r}
                  isLast={i === RACE_HISTORY.length - 1}
                  onViewDetail={(race) => { setViewingRace(race); setShowH(false); }}
                />
              ))}
            </div>
          )}
          <div className="mt-5 pt-3.5 border-t border-gray-100">
            <button className="w-full min-h-[48px] py-3 text-[14px] text-red-500 font-medium">Leave Session</button>
          </div>
        </div>
      </div>
      {showEntry && <RunEntrySheet onClose={() => setShowEntry(false)} track={TRACK} />}
    </PhoneFrame>
  );
}

function AlreadySubmittedScreen() {
  const [showP, setShowP] = useState(false);
  const [showH, setShowH] = useState(false);
  const [viewingRace, setViewingRace] = useState(null);

  return (
    <PhoneFrame label="Already Submitted (Host)">
      <SessionHeader showParticipants={showP} setShowParticipants={setShowP} />
      <div className="flex-1 overflow-y-auto">
        {viewingRace ? (
          <RaceDetailView race={viewingRace} onBack={() => setViewingRace(null)} />
        ) : (
          <>
            <div className="pt-3.5 pb-3.5">
              <TrackCard track={TRACK} state="active" />
            </div>
            <div className="px-3.5 pb-3.5 space-y-3.5">
              <div className="bg-green-50 border border-green-200 rounded-2xl p-4 text-center">
                <div className="text-[11px] text-green-700 font-semibold uppercase tracking-wide mb-1">Your Time</div>
                <div className="text-2xl font-mono font-bold text-green-800">2:14.523</div>
              </div>
              <div className="space-y-1">
                <div className="text-[11px] font-semibold text-gray-400 uppercase tracking-wide px-1">Waiting for</div>
                {PARTICIPANTS.filter((p) => p.status === "pending").map((p) => (
                  <div key={p.id} className="flex items-center gap-2.5 min-h-[44px] py-2 px-1">
                    <div className="w-6 h-6 rounded-full bg-gray-200 flex items-center justify-center text-[10px] font-bold text-gray-500">{p.name[0]}</div>
                    <span className="text-[13px] text-gray-600">{p.name}</span>
                    <span className="ml-auto text-[11px] text-amber-500">&#9203;</span>
                  </div>
                ))}
              </div>
              <button className="w-full py-4 bg-white text-blue-600 font-semibold rounded-2xl text-[15px] border-2 border-blue-600 flex items-center justify-center gap-2 active:bg-blue-50 transition-colors">
                <span>Next Track</span>
                <span className="text-lg">&#8594;</span>
              </button>
            </div>
          </>
        )}
        <div className="px-3.5 pb-20">
          <button onClick={() => setShowH(!showH)} className="w-full flex items-center justify-between min-h-[48px] py-3">
            <span className="font-semibold text-gray-500 uppercase tracking-wide text-[11px]">Race History</span>
            <span className="text-gray-400 text-[11px]">{showH ? "Hide" : "Show"} ({RACE_HISTORY.length})</span>
          </button>
          {showH && (
            <div className="bg-gray-50 rounded-xl px-2.5 py-0.5">
              {RACE_HISTORY.map((r, i) => (
                <RaceHistoryItem
                  key={r.num}
                  race={r}
                  isLast={i === RACE_HISTORY.length - 1}
                  onViewDetail={(race) => { setViewingRace(race); setShowH(false); }}
                />
              ))}
            </div>
          )}
          <div className="mt-5 pt-3.5 border-t border-gray-100">
            <button className="w-full min-h-[48px] py-3 text-[14px] text-red-500 font-medium">Leave Session</button>
          </div>
        </div>
      </div>
    </PhoneFrame>
  );
}

function PendingFirstScreen() {
  const [showP, setShowP] = useState(false);
  const [showH, setShowH] = useState(false);
  const [showEntry, setShowEntry] = useState(false);
  const [viewingRace, setViewingRace] = useState(null);

  return (
    <PhoneFrame label="Pending First">
      <SessionHeader showParticipants={showP} setShowParticipants={setShowP} />
      <div className="flex-1 overflow-y-auto">
        {viewingRace ? (
          <RaceDetailView race={viewingRace} onBack={() => setViewingRace(null)} />
        ) : (
          <>
            <div className="pt-3.5 pb-2">
              <TrackCard track={PENDING_TRACK} state="active" variant="pending" />
            </div>
            <div className="px-3.5 pb-2">
              <div className="flex items-center gap-2 px-1 pt-2 pb-1">
                <div className="flex items-center gap-1.5">
                  <div className="w-2 h-2 rounded-full bg-amber-500" />
                  <div className="w-2 h-2 rounded-full bg-amber-200" />
                </div>
                <span className="text-[12px] font-semibold text-amber-700">2 behind</span>
                <span className="text-[11px] text-gray-400 ml-auto">Up next: Rainbow Road</span>
              </div>
              <div className="flex gap-2.5">
                <button
                  onClick={() => setShowEntry(true)}
                  className="flex-1 py-4 text-[15px] font-semibold bg-amber-500 text-white rounded-2xl active:scale-[0.98] transition-transform"
                >
                  Submit Time
                </button>
                <button className="px-6 py-4 text-[14px] font-medium text-amber-700 bg-amber-100 rounded-2xl active:bg-amber-200 transition-colors">
                  Skip
                </button>
              </div>
            </div>
            <div className="pt-2 pb-3.5">
              <TrackCard track={TRACK} state="active" variant="compact" />
            </div>
          </>
        )}
        <div className="px-3.5 pb-20">
          <button onClick={() => setShowH(!showH)} className="w-full flex items-center justify-between min-h-[48px] py-3">
            <span className="font-semibold text-gray-500 uppercase tracking-wide text-[11px]">Race History</span>
            <span className="text-gray-400 text-[11px]">{showH ? "Hide" : "Show"} ({RACE_HISTORY.length})</span>
          </button>
          {showH && (
            <div className="bg-gray-50 rounded-xl px-2.5 py-0.5">
              {RACE_HISTORY.map((r, i) => (
                <RaceHistoryItem
                  key={r.num}
                  race={r}
                  isLast={i === RACE_HISTORY.length - 1}
                  onViewDetail={(race) => { setViewingRace(race); setShowH(false); }}
                />
              ))}
            </div>
          )}
          <div className="mt-5 pt-3.5 border-t border-gray-100">
            <button className="w-full min-h-[48px] py-3 text-[14px] text-red-500 font-medium">Leave Session</button>
          </div>
        </div>
      </div>
      {showEntry && <RunEntrySheet onClose={() => setShowEntry(false)} track={PENDING_TRACK} />}
    </PhoneFrame>
  );
}

function WaitingForTrackScreen() {
  const [showP, setShowP] = useState(false);
  const [showH, setShowH] = useState(true);
  const [viewingRace, setViewingRace] = useState(null);

  return (
    <PhoneFrame label="Waiting for Track (Host)">
      <SessionHeader showParticipants={showP} setShowParticipants={setShowP} />
      <div className="flex-1 overflow-y-auto">
        {viewingRace ? (
          <RaceDetailView race={viewingRace} onBack={() => setViewingRace(null)} />
        ) : (
          <>
            <div className="pt-3.5 pb-3.5">
              <TrackCard track={TRACK} state="waiting" />
            </div>
            <div className="px-3.5 pb-3.5">
              <button className="w-full py-4 bg-white text-blue-600 font-semibold rounded-2xl text-[15px] border-2 border-blue-600 flex items-center justify-center gap-2 active:bg-blue-50 transition-colors">
                <span>Next Track</span>
                <span className="text-lg">&#127922;</span>
              </button>
            </div>
          </>
        )}
        <div className="px-3.5 pb-20">
          <button onClick={() => setShowH(!showH)} className="w-full flex items-center justify-between min-h-[48px] py-3">
            <span className="font-semibold text-gray-500 uppercase tracking-wide text-[11px]">Race History</span>
            <span className="text-gray-400 text-[11px]">{showH ? "Hide" : "Show"} ({RACE_HISTORY.length})</span>
          </button>
          {showH && (
            <div className="bg-gray-50 rounded-xl px-2.5 py-0.5">
              {RACE_HISTORY.map((r, i) => (
                <RaceHistoryItem
                  key={r.num}
                  race={r}
                  isLast={i === RACE_HISTORY.length - 1}
                  onViewDetail={(race) => { setViewingRace(race); setShowH(false); }}
                />
              ))}
            </div>
          )}
          <div className="mt-5 pt-3.5 border-t border-gray-100">
            <button className="w-full min-h-[48px] py-3 text-[14px] text-red-500 font-medium">Leave Session</button>
          </div>
        </div>
      </div>
    </PhoneFrame>
  );
}

// --- Main Export ---

export default function SessionScreenMockup() {
  return (
    <div className="min-h-screen bg-gray-200 py-8 px-4">
      <div className="text-center mb-6">
        <h1 className="text-xl font-bold text-gray-800">Session Screen v3 — All States</h1>
        <p className="text-sm text-gray-500 mt-1">Pixel 9 Pro (427 x 952 CSS px) &middot; Click "Submit Time" for run entry, expand Race History to explore</p>
        <p className="text-xs text-gray-400 mt-1">v3: Lap times in form, tappable race history, full race detail view, H2H records in participant dropdown</p>
        <p className="text-xs text-gray-400 mt-0.5">Note: "Already Submitted" starts with participants expanded to show H2H; "Waiting for Track" starts with Race History expanded</p>
      </div>
      <div className="flex flex-wrap gap-6 justify-center">
        <NeedToSubmitScreen />
        <AlreadySubmittedScreen />
        <PendingFirstScreen />
        <WaitingForTrackScreen />
      </div>
    </div>
  );
}
