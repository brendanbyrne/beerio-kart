# Beerio Kart roadmap

Where Beerio Kart is going at the cup-by-cup level. Status of individual work items lives on the [project board](https://github.com/users/brendanbyrne/projects/3); this file is the narrative.

## How this works

Two milestone types: **product cups** for user-facing feature work-chunks, **workstreams** for cross-cutting infrastructure that runs concurrent with product cups.

- **Product cups** are GitHub milestones named after Mario Kart 8 Deluxe cups (Mushroom, Flower, Star, Special, Shell, Banana, Leaf, Lightning, then Crossing/Bell/Egg/Triforce, then the eight Booster Course Pass cups). Cups are claimed in chronological start order — the Nth product work-chunk gets the Nth cup. No semantic mapping; cup names are arbitrary chronological labels. Title format: `<CupName>: <Description>`.
- **Workstream milestones** are GitHub milestones with topical names instead of cup names — `Docs:`, `Hardening:`, etc. Use these when the work is a cross-cutting concern (code hygiene, doc restructure, accessibility audit) running alongside product cups rather than a discrete user-visible release. Title format: `<Topic>: <Description>`.
- **Issues** under a milestone are the working checklist. One Issue per discrete unit of work; the project board shows their status (Backlog → Ready → In Progress → Done).
- **This file** is the narrative — each milestone gets a section describing the goal, scope, deferred work, and success criteria. When a milestone closes, its section keeps a brief retrospective and the cup/topic keeps its name forever.
- **Future-cup Scope sections double as the future-work record.** Cups not yet active list their work as bullets here, not as GitHub Issues. When a cup becomes the next active work-chunk, its bullets transcribe to Issues at that time and the Scope list in this file gets a "see Milestone X for current status" pointer. This keeps the GitHub Backlog scoped to "things we're committed to right now," not "everything we'd ever want to do."

For the underlying conventions, see [`project-workflow.md`](./project-workflow.md) § Milestone lifecycle and [`designs/archive/2026-05-04-design-doc-restructure.md`](./designs/archive/2026-05-04-design-doc-restructure.md) §12 (archived).

## Cup mapping (product)

| Cup | Work chunk | Status | Closed |
|-----|------------|--------|--------|
| Mushroom | Foundation (was Phase 1) | Closed | 2026-03-31 |
| Flower | Deployment (was Phase 2) | Closed | 2026-04-02 |
| Star | Sessions & Run Recording (was Phase 3) | Open, in progress | — |
| Special | *(freed 2026-05-11 — was Documentation overhaul; renamed to `Docs:` workstream)* | Available | — |
| Shell | Session Rulesets (was Phase 4) | Future | — |
| Banana | Stats & Leaderboards (was Phase 5) | Future | — |
| Leaf | Social & Head-to-Head (was Phase 6) | Future | — |
| Lightning | (next thing — TBD) | Reserved | — |

OCR work (was Phase 7) is **not** yet milestoned — too speculative to commit to a cup. It gets a cup when it's next-up.

## Workstream mapping (cross-cutting)

| Topic | Work chunk | Status | Closed |
|-------|------------|--------|--------|
| Docs | Documentation overhaul (renamed from `Special:` for convention consistency, 2026-05-11) | Closed | 2026-05-11 |
| Hardening | Backend compliance plan — code hygiene, standards conformance, type-driven design, infrastructure (per `designs/archive/compliance-plan.md`, archived) | Closed | 2026-05-15 |

---

## Mushroom — Foundation

**Status:** Closed 2026-03-31.

Initial scaffolding — Rust + Axum backend, React + Vite + Tailwind frontend with Bun, SeaORM with SQLite and migrations for all base tables, MK8 Deluxe seed data (tracks, cups, characters, bodies, wheels, gliders), basic auth (argon2 + JWT), Dockerfile + `compose.yaml`. The cup that proved the stack works end-to-end. Everything since builds on this foundation.

[Milestone Mushroom](https://github.com/brendanbyrne/beerio-kart/milestone/1)

---

## Flower — Deployment

**Status:** Closed 2026-04-02.

Single-container Docker on Unraid, Cloudflare Tunnel + Full-strict TLS (per ADR 0033), refresh-token auth (short access token + HttpOnly refresh cookie per ADR 0031), prod/dev config split, password change endpoint. Brought the app online in production. After Flower the app could be reached from any phone over HTTPS with secure auth.

[Milestone Flower](https://github.com/brendanbyrne/beerio-kart/milestone/2)

---

## Star — Sessions & Run Recording

**Status:** Open, in progress.

**Goal.** The core gameplay loop: friends gather, start a session, race tracks together, submit times, see results. Sessions hold a sequence of races; runs belong to session races; pending races get tracked when someone disconnects mid-session.

**Scope.**

- Session lifecycle: create, join, leave, auto-close via race-derived liveness (per [ADR-0035](./decisions/0035-race-anchored-session-lifetime.md)), host transfer on leave.
- Session races: track choice (Random ruleset for MVP), `skip turn` to pass a stalled chooser.
- Run entry within session context: time, drink type, race setup, DQ flag, optional photo.
- Pending race tracking: 3-deep UI cap, submit-in-order, skip option, per-race 1-hour expiry (per [ADR-0035](./decisions/0035-race-anchored-session-lifetime.md) — replaces the 5-minute disconnect grace originally scoped here).
- Photo upload (separate endpoint) and auto-flagging for record-breaking runs without photos (per ADR 0025).
- Supporting APIs: users, drink types, runs, sessions, pre-seeded data reads.
- Background task: stale session cleanup (Tokio) — DB hygiene only per [ADR-0035](./decisions/0035-race-anchored-session-lifetime.md); user-facing liveness is derived at read time.
- Home screen: active sessions list + "Start a Session" primary action + recent runs.
- Build chore: `justfile` recipes for `dev`, `test`, `entities`, `build`.

**Deferred to later cups.**

- All non-Random rulesets (Default, Least Played, Round-robin) → Shell.
- Stats / leaderboards / personal history → Banana.
- Rivals lists, H2H comparisons, admin page, flagging UI → Leaf.

**Success criteria.** A friend group can start a session, race, submit times, and see the session's race history. Random is the only available ruleset. Photos verify record-breaking runs. The H2H derivation (per ADR 0010) functions correctly when reads land in Banana.

[Milestone Star](https://github.com/brendanbyrne/beerio-kart/milestone/3)

---

## Docs — Documentation overhaul

**Status:** Closed 2026-05-11.

**Type:** Workstream (renamed from cup `Special:` on 2026-05-11 — non-product work shouldn't consume cup names per the convention update).

Restructured `docs/` from a few sprawling files (a monolithic `DESIGN.md`, ad-hoc review notes, no clear narrative-vs-decision separation) into a coherent multi-doc structure: 34 ADRs in `decisions/` distilled from prior `DESIGN.md` bullets, design records in `designs/` for point-in-time sign-off narratives, the cup-by-cup story in `roadmap.md`, a slimmed `design.md` (~250 lines) for architecture, the operational `project-workflow.md`, an `api-contract.md` for wire-format conventions, a `user-workflows.md` for end-user flows and screens, a `data-model.md` for the schema, and CLAUDE.md files at appropriate scopes (repo root, `backend/`, `frontend/`, `docs/`). All six PRs of [`designs/archive/2026-05-04-design-doc-restructure.md`](./designs/archive/2026-05-04-design-doc-restructure.md) landed (archived 2026-05-15). The workstream that turned scattered project documentation into something a new contributor can navigate in under five minutes via `docs/README.md`.

[Milestone Docs: Documentation overhaul](https://github.com/brendanbyrne/beerio-kart/milestone/4)

---

## Hardening — Backend compliance plan

**Status:** Closed 2026-05-15.

**Type:** Workstream. Ran concurrent with product cups; spanned Star.

**Goal.** Brought the backend into conformance with the standards in `coding-standards/` and executed the sequenced PR list in [`designs/archive/compliance-plan.md`](./designs/archive/compliance-plan.md). Code hygiene, type-driven design, infrastructure (graceful shutdown, Tower middleware, tracing instrumentation), and documentation polish — all the work that supported product cups but didn't itself ship user-facing functionality.

**Scope.** The full sequenced list lives in `designs/archive/compliance-plan.md` (archived 2026-05-15). All numbered streams (A–J) and the lazy lint-cleanup stream H1+ are signed off.

**Deferred.** Work whose scope was naturally part of a product cup stayed in that cup (e.g., `users.email` pre-check, which lands with whatever profile-update endpoint introduces a real email value).

**Success criteria.** `compliance-plan.md` reached all-signed-off; the standards docs in `coding-standards/` describe the code as it actually is; the backend's structural hygiene supports the next product cup without surprise. All met.

[Milestone Hardening: Backend compliance plan](https://github.com/brendanbyrne/beerio-kart/milestone/8)

---

## Shell — Session Rulesets

**Status:** Future.

**Goal.** Three additional rulesets beyond Star's Random, plus the ruleset selection UI at session creation.

**Scope.**

- Default ruleset — lowest-leaderboard-points player chooses, with recusal; falls back to random when everyone recuses. Tiebreaker: oldest account creation time.
- Least Played ruleset — track with fewest runs in the chosen drink category. Drink-category config picked at session creation; defaults to the host's preferred drink category.
- Round-robin ruleset — "Can Choose" / "Can't Choose" groups; recusal moves you to "Can't Choose"; resets when "Can Choose" is empty.
- Ruleset selection UI at session creation, with brief inline explanations of each.

Each ruleset is its own Rust trait impl per ADR 0022. **Six test scenarios per ruleset** (normal flow, recusal by one, recusal by all, mid-session join, mid-session leave, host leave) — 24 effective items behind the four deliverables.

**Deferred.** Mid-session ruleset changes (deferred per ADR 0017).

**Success criteria.** All four rulesets pass the six-scenario test matrix. Ruleset selection is a one-tap choice at session creation with brief inline explanations.

[Milestone Shell](https://github.com/brendanbyrne/beerio-kart/milestone/5)

---

## Banana — Stats & Leaderboards

**Status:** Future.

**Goal.** Read-only views over accumulated run data: personal stats, per-track time-series with charts, session history in profile, track / cup / global leaderboards with the drink-category toggle (alcoholic / non-alcoholic / combined), user-rank pinned at the bottom.

**Scope.**

- Personal stats page — PBs, averages, run count, most-played track, best track.
- Session history in profile — date, participants, race count, personal W-L per session. Tap into a session for race-by-race breakdown.
- Full run history with detail view, paginated per ADR 0032.
- Per-track time-series chart of all runs.
- Track leaderboard — alcoholic / non-alcoholic / combined toggle (per ADR 0006); DQ'd runs excluded (per ADR 0012); user's preferred drink category sets the default toggle position.
- Cup-level leaderboard — same toggle.
- Global leaderboard — most track records held (per ADR 0003).
- User rank pinned at the bottom of leaderboards when not in the top N.
- Notification variants for leaderboard-relevant events (per [ADR-0038](./decisions/0038-notifications-system.md)): `TrackRecordLost` (notify the previous holder when their track or lap record is beaten) and `LeaderboardRankChanged` (notify users whose top-N position shifts). Trigger sites in `services::runs::create_run` and `delete_run`.

**Success criteria.** A user can find their PB on any track in two taps. Leaderboards refresh as runs come in. The drink-category toggle feels natural, not buried.

[Milestone Banana](https://github.com/brendanbyrne/beerio-kart/milestone/6)

---

## Leaf — Social & Head-to-Head

**Status:** Future.

**Goal.** The social layer: rivals lists, H2H comparison views, profile pages with improvement trends, run flagging (user-initiated), and the admin page (env-variable-gated) for resolving flags.

**Scope.**

- "Players you've competed with" — derived from shared session races (no separate table per ADR 0010).
- Head-to-head comparison view — wins / losses / ties derived per ADR 0010. DQ'd runs excluded; ties count as 0-0 draws.
- Win/loss records — H2H does not distinguish alcoholic vs non-alcoholic (drink category matters for leaderboards only).
- Profile page with improvement trends.
- Flagging a run — user-initiated, preset reasons + freeform note + visibility choice (`hide_while_pending` per ADR 0029). User can only flag their own runs, and only if a photo is attached.
- Admin page — lightweight, env-variable-gated (per ADR 0008 / 0019).
- Admin actions on flagged runs — resolve, edit-and-resolve (admin-only exception to immutability per ADR 0009), or delete.
- `H2hLeadChanged` notification variant (per [ADR-0038](./decisions/0038-notifications-system.md)) — notify users when a head-to-head record flips winner against an opponent they share session races with. Trigger sites in `services::runs::create_run` and `delete_run`; H2H derivation already lives in this cup.

**Success criteria.** From any player's profile, see your H2H record against them in one tap. Flagged runs appear in admin's queue with all the context needed to resolve.

[Milestone Leaf](https://github.com/brendanbyrne/beerio-kart/milestone/7)

---

## Lightning — Reserved

The next major work-chunk after Leaf claims this cup. No specific assignment yet. OCR (see below) is the most likely candidate.

---

## OCR — no cup assigned yet

The eventual goal: photograph the in-game results screen → automatic time extraction. Reduces manual-entry friction, increases verification quality (closes the unrelated-photo loophole flagged in ADR 0025), and retires the preferred-race-setup field on profiles.

**Scope (when this becomes a cup).**

- Photo capture with each run for verification + OCR training data (some of this lands incidentally in Star's auto-flag mechanism).
- Use phone camera to photograph the TV screen showing race time.
- Extract time via OCR — `docs/research/ocr-strategy.md` covers the candidate-tools survey. Likely browser-side (Tesseract.js or similar).
- Auto-populate the time field from the photo.
- Extract race setup from the end-of-race screen.
- Retire the preferred-race-setup fields on user profiles once OCR is reliable as the default path.

When OCR becomes the next active work-chunk, it claims a cup name (likely Lightning per chronological order) and the bullets above transcribe to Issues at that time. Until then, this section is the canonical record.

---

## Future cups not yet allocated

The remaining pool: Crossing, Bell, Egg, Triforce (MK8 Deluxe additions); Golden Dash, Lucky Cat, Turnip, Propeller, Rock, Moon, Fruit, Boomerang (Booster Course Pass). 20 cups in total — enough for any plausible Beerio Kart lifetime. Cups get claimed when their work-chunk starts.

---

## Random ideas

Small ideas captured here without committing to a cup or filing as Issues. Things in this list are sub-cup-sized — too small to deserve their own milestone, too unstructured to file as Issues right now (per the "recorded but not visible in GitHub backlog" rule). When one ages into something concrete enough to act on, it graduates to an Issue.

- Turn list of previous players into invite emails to join a session.
- Ability for a user to change their username.
- Ability to send emails (account recovery is the first concrete use case).

Sharp-edged items get filed as Issues directly — they don't live here. The first such graduation was [#75 — handle concurrent `next_track` calls gracefully](https://github.com/brendanbyrne/beerio-kart/issues/75), which started life in this section before getting its own Issue.
