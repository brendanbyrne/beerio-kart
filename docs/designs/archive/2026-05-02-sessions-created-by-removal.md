# Sessions: drop `created_by` — design record (2026-05-02)

> **Status: complete.** Implemented via PR-E3 (see [`compliance-plan.md`](./compliance-plan.md) Stream E). ADR captured at [`../../decisions/0001-sessions-no-created-by-column.md`](../../decisions/0001-sessions-no-created-by-column.md). Retained for historical reference.

## 1. Context

The `sessions` table currently has both:

- `created_by: UUID (foreign key -> users, not null)` — the user who originally created the session.
- `host_id: UUID (foreign key -> users, not null)` — the user currently hosting; transfers on leave.

Per `docs/design.md`: "host_id starts as created_by. If the host leaves, host role transfers to the earliest-joined remaining participant."

While drafting the SeaORM coding standard's multi-FK relations rule (`seaorm.md` § 11), we noticed `sessions` has two FKs to `users` — exactly the case that rule warns about. That prompted the question: do we actually need `created_by`, or does `host_id` carry all the information we use?

After discussion: **`created_by` is not used by any current product feature.** The UI surfaces only the current host (🏠 icon, per the "Session UI icons" note). The host-on-leave transfer logic only reads `host_id` and the participant list; it doesn't consult `created_by`. The "who originally started this session" fact is invisible to the user and doesn't appear in any leaderboard, stat, or session detail screen.

If a future feature ever needs original-creator info, re-adding the column is one append-only migration. Cost of keeping it now: schema clutter, an unused FK, and a live instance of the multi-FK ambiguity case that codegen has historically gotten wrong (sea-orm #405).

- [x] Approved — context accurate

## 2. Decision

**Drop the `created_by` column from the `sessions` table.** `host_id` remains and continues to behave as documented (starts as the user who creates the session; transfers to earliest-joined remaining participant on host leave).

- [x] Approved — decision

## 3. Schema impact

The consolidated migration file (`backend/migration/src/m20260101_000001_initial_schema.rs`) is edited in place per the prelaunch policy (`seaorm.md` § 5):

- Remove the `Sessions::CreatedBy` column definition from the `Table::create()` builder.
- Remove the corresponding `ForeignKey::create()` from `Sessions::CreatedBy` → `Users::Id`.
- Verify no index references `created_by` (none expected based on the current schema, but confirm during implementation).
- Reset dev DB after the migration edit, per prelaunch policy.

Entity regeneration via `just entities` picks up the column removal automatically. The generated `entities/sessions.rs` will no longer have a `created_by` field on `Model` / `ActiveModel`.

- [x] Approved — schema impact

## 4. Code impact

Files expected to need changes (verify exact set during implementation):

- **`backend/src/services/sessions.rs`** — session creation must stop populating `created_by`. The current `create_session` helper sets both `created_by` and `host_id` to the creator; only `host_id` remains.
- **`backend/src/routes/sessions.rs`** — if any response DTO exposes `created_by`, remove it. Most likely none do, but verify.
- **`backend/src/entities/sessions.rs`** — regenerated via `just entities`; no hand edits.
- **Tests** — any test that constructs a `sessions::ActiveModel` with `created_by: Set(...)` or asserts on `model.created_by` needs updating. Targets: `services/sessions.rs::tests`, integration tests under `backend/tests/`.

The host-on-leave transfer logic stays as-is. Session creation continues to set `host_id` to the creator's user ID — that part of the behavior is unchanged.

- [x] Approved — code impact (estimate)

## 5. `docs/design.md` text updates

### 5.1 Sessions table schema

In the `sessions` table definition (around lines 256–263 of `docs/design.md`):

**Before:**
```
sessions
├── id: UUID (primary key)
├── created_by: UUID (foreign key -> users, not null)
├── host_id: UUID (foreign key -> users, not null — current host, transfers on leave)
├── ruleset: TEXT (not null — "random", "default", "least_played", "round_robin")
├── least_played_drink_category: TEXT (nullable — "alcoholic" or "non_alcoholic"; only used when ruleset is "least_played")
├── status: TEXT (not null — "active", "closed")
├── created_at: TIMESTAMP (not null)
└── last_activity_at: TIMESTAMP (not null)
```

**After:**
```
sessions
├── id: UUID (primary key)
├── host_id: UUID (foreign key -> users, not null — starts as the user who created the session; transfers on leave)
├── ruleset: TEXT (not null — "random", "default", "least_played", "round_robin")
├── least_played_drink_category: TEXT (nullable — "alcoholic" or "non_alcoholic"; only used when ruleset is "least_played")
├── status: TEXT (not null — "active", "closed")
├── created_at: TIMESTAMP (not null)
└── last_activity_at: TIMESTAMP (not null)
```

### 5.2 Sessions table notes

In the notes block following the schema (around lines 265–272), replace:

> `host_id` starts as `created_by`. If the host leaves, host role transfers to the earliest-joined remaining participant.

with:

> `host_id` is set to the creating user when the session is created. If the host leaves, host role transfers to the earliest-joined remaining participant.

### 5.3 Resolved Decisions

Add a new bullet to the "Resolved Decisions" section:

> - **No separate `created_by` column on sessions.** `host_id` carries the original creator until host transfers on leave. No current product feature uses the original-creator information. If needed later, re-adding the column is one append-only migration.

### 5.4 No other changes

The "Session UI icons" note (host = 🏠) is unaffected. Workflows 2 (Starting a Session) and 5 (Leaving a Session) reference host transfer in narrative form and don't mention `created_by` directly — no changes needed. The crown 👑 ("most fastest track times") is unrelated.

- [x] Approved — `docs/design.md` updates

## 6. Other documents

No changes expected to:

- `.claude/CLAUDE.md`
- `docs/api-contract.md` (the API doesn't expose `created_by`)
- `docs/coding-standards/rust.md`, `tokio.md`
- `docs/compliance-plan.md` (PR-E3 already lists this exact change)

One change in `docs/coding-standards/seaorm.md` § 11: the rule warns about multi-FK relations and currently includes a parenthetical pointing at this very case ("a live instance of this case (`sessions.created_by` and `sessions.host_id` both → `users.id`) is being eliminated by dropping `created_by`. After that change lands, the multi-FK case is hypothetical for current schema but the rule still applies for any future table that reuses a target."). Once PR-E3 lands, that parenthetical can be simplified to "no current schema instances; the rule remains in force for any future table."

- [x] Approved — no other doc changes; `seaorm.md` § 11 footnote tightens after PR-E3

## 7. Risk

Low.

- **No data preservation needed.** Prelaunch.
- **No external API impact.** `created_by` is an internal schema detail; no endpoint exposes it.
- **No behavioral change.** Host-on-leave transfer logic is unchanged. Session creation still sets `host_id` to the creator.
- **Reversible.** If a future feature needs original-creator info, re-adding the column is one append-only migration plus a backfill (or "starts being populated for new sessions; old sessions have it null" — depending on the use case).

The only non-trivial verification: confirm that no service code or test outside `services/sessions.rs` reads `created_by`. A quick `grep -rn "created_by" backend/` during implementation catches this.

- [x] Approved — risk acceptable

## 8. Implementation sequencing

- This design record is the sign-off gate. After all checkboxes are checked, implementation lands as **PR-E3** in `docs/compliance-plan.md`.
- PR-E3 dependencies: none. Can land in parallel with other Phase A/B PRs.
- After the implementation PR merges, the `seaorm.md` § 11 parenthetical noted in § 6 above gets a one-line update.
- After the design.md updates are made (§ 5 above) and the seaorm.md § 11 footnote is tightened, this design record is archived (kept in `docs/designs/archive/` as historical record per project convention).

- [x] Approved — sequencing

---

## Sign-off summary

When all eight checkboxes above are checked, the design is approved and implementation can proceed via PR-E3.

- [x] **All sections approved — clear to implement**
