# Beerio Kart — User Workflows & UI Screens

> **Scope.** End-user-facing flows: how a player navigates the app from registration through racing, viewing stats, and admin review. Plus the screen-by-screen breakdown of what each surface contains.
>
> **Not in this file.** Game rules, design principles, tech stack, and architecture live in [`design.md`](./design.md). Endpoint catalog and wire-format conventions live in [`api-contract.md`](./api-contract.md). Database schema lives in [`data-model.md`](./data-model.md). For project-operational workflow (Issue lifecycle, milestones, PRs, triage), see [`workflow.md`](./workflow.md) — note the singular "workflow," distinct from this file.

The workflows below reference UI Screens by number (e.g., "lands on the session screen — see § 2.3"). Screens are listed in § 2.

---

## 1. User workflows

### 1.1 New user joins

1. Gets URL from a friend, opens on phone.
2. Registers (username + password), auto-logged-in.
3. Lands on home/dashboard — empty state.
4. Prompted to set up preferred race setup (character, body, wheels, glider) and preferred drink type. Drink type selector includes "not listed? add new" option.
5. Home screen shows active sessions. If friends are already playing, the most natural next step is "tap to join." If nobody's playing, "Start a Session" is the primary action.

### 1.2 Starting a session

1. Taps "Start a Session" on home screen.
2. Selects a ruleset: Random (default for MVP), Default, Least Played, or Round-robin. Brief explanation of each shown inline.
3. Session is created. User is the host and first participant.
4. Lands on the session screen — waiting for others to join, or can proceed solo.
5. For Random and Least Played rulesets: host taps to trigger the first track selection. For Default and Round-robin: the chooser is determined by the ruleset and prompted to pick.

### 1.3 Joining a session

1. Home screen shows list of active sessions, sorted by most recent activity. Each shows: host name, participant count, current race number, ruleset.
2. Taps a session to join.
3. Lands on the session screen, sees current state: what track is being raced, who's in, who has pending races.
4. Can immediately submit a time for the current race.

Future enhancement: prioritize sessions containing players you've competed with before (sort by known rivals). Future consideration: session passwords via the `POST /sessions/:id/join` endpoint.

### 1.4 The session loop (core play loop)

1. A track is selected (by the chooser or automatically, depending on ruleset).
2. Everyone in the session sees the track. Each person races on their TV in Time Trial mode.
3. After racing, each person submits their time:
   - Track is already known (from the session race) — no track selection needed.
   - Enter time (M:SS.mmm — single digit minutes, no leading zero, manual entry for v1, camera/OCR later). Auto-advance moves focus forward through all 12 fields (total → L1 → L2 → L3); backspace on an empty field moves backward. Lap times must sum exactly to total time.
   - Drink defaults to previous, fallback to preferred. Can change or add new inline.
   - Race setup defaults to previous, fallback to preferred. Can change.
   - Option to mark the run as DQ'd (didn't finish drink before finishing race).
   - Optional photo upload.
   - If time is a track record and no photo: prompt, then auto-flag if skipped.
4. Session screen shows who has submitted, who's still pending.
5. Next track selection happens when the chooser/host triggers it (depending on ruleset). The chooser can pick while others still have pending races — this doesn't block.
6. If someone has pending races from earlier, they see those when they go to submit. Pending races shown in order, submit or skip each. Max 3 pending in UI (oldest expire first). Schema supports unlimited for future flexibility.
7. Choosing and submitting are independent actions — the chooser can pick the next track even if they haven't submitted their own time yet.
8. Repeat until the group decides to stop.

Earmarked: the track selection sub-workflow (how the chooser browses/searches for a track within a session) will be specified as part of Milestone Star detailed design. Starting point: browse by cup or search by name, consistent with the existing track browser concept.

### 1.5 Leaving a session / session end

1. Player taps "Leave Session."
2. If they have pending races: warning that pending times will be forfeited after a 5-minute grace period. If they rejoin within the grace period, pending races are preserved.
3. If the leaving player is the host: host role transfers to the earliest-joined remaining participant.
4. Session ends when all participants have left.
5. If no activity for 1 hour, session auto-closes and no further run submissions are accepted.

### 1.6 Checking personal stats

1. Opens profile.
2. Sees overall stats: total runs, most-played track, best track (highest leaderboard position), overall rank.
3. Sees session history: list of sessions participated in (date, participants, race count, personal W-L for that session). Tap into a session for race-by-race breakdown.
4. Sees full run history (all runs, newest first) — tappable to view details, flag, or delete.
5. Can drill into a specific track — time chart over time, PB, average.
6. Sees "players you've competed with" list (derived from shared session races) — tap one to see H2H record.

### 1.7 Tracks & leaderboards

1. Opens "Tracks & Leaderboards."
2. Sees global leaderboard — most track records held per player, your rank pinned at bottom if not in top N.
3. Alcoholic/non-alcoholic/combined toggle (defaults to match user's preferred drink category).
4. Below or alongside: cups listed in game order (by ID).
5. Taps a cup — cup-level leaderboard + its 4 tracks in position order.
6. Taps a track — your PB, time history chart, run history on this track, track leaderboard.
7. Taps a player on any leaderboard — their stats at that level (track/cup/global).
8. Taps that player again — full profile.

Note: earmarked for later discussion — potential shared leaderboard component across global/cup/track levels with consistent visual style but different data.

### 1.8 Flagging a run

1. User views one of their own runs (from run history in profile).
2. Run has a photo attached.
3. Taps "Flag for Review."
4. Selects a reason from preset list: "Time is incorrect", "Wrong track", "Wrong race setup", "Wrong drink type", "Other."
5. Optionally adds a short note for context.
6. Chooses visibility: keep visible (default) or hide until reviewed.
7. Run marked as flagged, appears in admin queue.

### 1.9 Admin reviews flagged runs

1. Brendan opens admin page (accessible only if user ID matches env variable).
2. Sees list of unresolved flags: player name, track, entered time, flag reason, note, visibility status, whether auto-generated.
3. Taps a flag — run details alongside photo.
4. Actions:
   - **Resolve:** Run is correct as-is. Sets `resolved_at`.
   - **Edit and resolve:** Correct the time/track/setup/etc, then resolve. (Admin-only exception to immutability.)
   - **Delete:** Data is unsalvageable. Run removed, user can re-enter.

---

## 2. UI screens (mobile-first)

The reference device is the **Pixel 9 Pro** (1280 × 2856 physical, ~427 × 952 CSS px at 3× DPR). All layout and component sizing decisions assume that target. Firefox is a required browser alongside Chrome/Safari mobile.

### 2.1 Login / register

Simple form. Username + password. No email required for v1.

### 2.2 Home / dashboard

- Active sessions list (sorted by most recent activity; each shows host, participants, race number, ruleset).
- "Start a Session" button (primary action).
- Recent runs (your last 5).
- Your overall rank (most track records held).
- Preferred Race Setup (character + kart displayed).

### 2.3 Session screen

The main play screen. Shows:
- Current track being raced.
- Participant list with submission status (submitted / pending / DQ'd).
- Pending race indicator (who has unsubmitted races from earlier).
- "Submit Time" action — opens the run entry form for the current (or pending) race.
- Next track controls (host/chooser triggers, depending on ruleset).
- "Skip Turn" option (any participant can pass the chooser's turn).
- "Leave Session" button.
- Session history (tracks raced so far, results).

### 2.4 Run entry (within session)

Streamlined compared to standalone entry — the track is already known from the session.
1. Enter time (M:SS.mmm — single digit minutes, no leading zero, manual entry for v1, camera/OCR later). Auto-advance moves focus forward through all 12 fields (total → L1 → L2 → L3); backspace on an empty field moves backward. Lap times must sum exactly to total time.
2. Drink defaults to previous, fallback to preferred. Can change or add new inline.
3. Race setup defaults to previous, fallback to preferred. Can change.
4. Option to mark run as DQ'd (didn't finish drink before finishing race).
5. Optional photo upload.
6. If record-breaking without photo: prompt, then auto-flag if skipped.
7. If pending races exist: shown in order, submit or skip each before current race.

### 2.5 Tracks & leaderboards

- Global leaderboard: most track records held, your rank pinned at bottom.
- Alcoholic/non-alcoholic/combined toggle (defaults to user's preferred drink category).
- Cups listed in game order, each showing its 4 tracks.
- Drill into cup: cup-level leaderboard + tracks.
- Drill into track: your PB, time chart, run history on this track, track leaderboard.
- Tap a player: their stats at that level. Tap again: full profile.

### 2.6 Profile / personal stats

- Overall stats: total runs, most-played track, best track, overall rank.
- Session history: list of sessions (date, participants, race count, W-L). Tap for race-by-race breakdown.
- Full run history (newest first) — tappable for details, flag, or delete.
- Drill into a track for personal breakdown.
- "Players you've competed with" (derived from shared session races) — tap for H2H.

### 2.7 Admin (Brendan only)

- List of unresolved flags with run details, photos, reasons, notes.
- Actions: resolve, edit and resolve, or delete run.

### 2.8 Shared UI components (earmarked for discussion)

- **Drink type selector**: reusable wherever a drink is chosen (run entry, onboarding, profile). Includes "not listed? add new" inline form.
- **Leaderboard component**: potential shared component for global/cup/track levels with consistent visual style, different data.

---

## 3. Document history

- 2026-05-06 — Initial creation. Sourced from `design.md` § "User Workflows" (workflows 1-9) and § "UI Screens" (screens 1-7 + shared components). Content copied with two minor editorial changes: the Workflow 1.4 "Phase 3 detailed design" reference updated to "Milestone Star detailed design" (cup-name convention is now canonical), and the screens preamble adds the Pixel 9 Pro reference-device sentence (factored out of `.claude/CLAUDE.md` § UI Reference Device for proximity to the screens themselves). Filename `user-workflows.md` diverges from the design record's proposed `workflows.md` due to grep collision with the operational [`workflow.md`](./workflow.md) — see PR 4 discussion. PR 4 of the docs restructure.
