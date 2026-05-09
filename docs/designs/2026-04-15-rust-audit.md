# Rust Backend Code Audit

Date: 2026-04-15
Auditor: Cowork
Scope: `backend/src/` excluding generated `entities/` and migrations

## Executive Summary

The backend is in good shape overall — auth, middleware, error handling, config, and the small utility modules are clean and well-tested. Two files carry most of the complexity debt: `services/sessions.rs` (1465 lines) and `services/runs.rs` (972 lines). Patterns are mostly consistent but a handful of duplications and magic strings have accumulated.

**Top recommendations by impact:**

1. **Extract "load active session" and "require participant" helpers** — eliminates ~40 lines of repeated code across 6 functions.
2. **Introduce status/ruleset enums** — replaces magic strings scattered across the codebase.
3. **Decompose `get_session_detail` and `create_run`** — both are 150+ line functions doing 4-5 distinct things.
4. **Collapse the 5 FK validation blocks** in `create_run` and `update_user` into a generic helper.
5. **Add `services/users.rs`** — the only route module with nontrivial business logic in the route layer.

Each section below has a checkbox — sign off by marking Approved / Needs discussion / Skip.

---

## 1. Anti-Patterns

### 1.1 ~~TOCTOU race condition in `join_session`~~ — Already Fixed

**Correction:** The partial UNIQUE index on `session_participants` `(user_id) WHERE left_at IS NULL` already exists (migration `m20260330_000009_create_session_participants.rs`, lines 33-39). The TOCTOU race I claimed doesn't actually manifest — the DB rejects the second concurrent INSERT. The app-level `check_not_in_any_session` is just nicer error messaging.

Similarly, `runs` has its UNIQUE `(session_race_id, user_id)` constraint from migration 12 (the 3C-2 fix).

**No action needed.**

- [x] Already done — no PR work
- [ ] Needs discussion
- [ ] Skip

---

### 1.2 Magic strings for session status and ruleset

**Files:** `services/sessions.rs`, `services/runs.rs`

Literal `"active"`, `"closed"`, and `"random"` are used throughout. Examples:
- `if session.status != "active"` appears ~6 times
- `active.status = Set("closed".to_string())` appears ~3 times
- `VALID_RULESETS: &[&str] = &["random"]`

**Recommended fix:** Define enums with `Display` / `FromStr` derives for DB round-tripping:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus { Active, Closed }

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self { Self::Active => "active", Self::Closed => "closed" }
    }
}
// ... plus FromStr for reading from DB
```

This prevents typos ("actve"), makes refactoring safe (rename via compiler), and sets up for Phase 4's additional rulesets. Same treatment for `Ruleset { Random, Default, RoundRobin }` even though only Random exists today.

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

### 1.3 Silent `"Unknown"` fallback for missing FK references

**File:** `services/sessions.rs::get_session_detail` (line ~310)

```rust
let host_username = users::Entity::find_by_id(&session.host_id)
    .one(db)
    .await?
    .map(|u| u.username)
    .unwrap_or_else(|| "Unknown".to_string());
```

Similarly in `next_track` and `skip_turn`:

```rust
.unwrap_or_else(|| "Unknown Cup".to_string())
```

If `session.host_id` doesn't resolve to a user, or `chosen.cup_id` doesn't resolve to a cup, something is seriously broken (DB corruption or a bug) — but the response will show "Unknown" and nothing alerts us. This hides real failures.

**Recommended fix:** Return `AppError::Internal(...)` instead. FK integrity means these should never fail; if they do, that's a 500 we should know about.

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

### 1.4 Stringly-typed timestamps throughout

**Files:** All services and entities.

Every service function does `Utc::now().to_rfc3339()` and stores the result as `String`. SQLite doesn't have a native timestamp type, so string storage is necessary — but the Rust-side representation should be `DateTime<Utc>` with serialization at the boundary.

Current consequences:
- No comparison or arithmetic without re-parsing.
- `close_stale_sessions` does `(Utc::now() - Duration::hours(1)).to_rfc3339()` and then uses string `.lt()` — works because RFC3339 is lexicographically ordered, but fragile.

**Recommended fix:** This is a larger refactor because SeaORM entities are generated. Options:
1. Live with it, document the "string RFC3339 everywhere" convention.
2. Add helper functions `now_rfc3339()` and `parse_rfc3339(&str) -> DateTime<Utc>` for consistency.
3. Full migration: change entities to use `DateTime<Utc>` via custom SeaORM column type.

I'd lean option 2 — minimal-effort improvement without disturbing the entity generation story.

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

Let's do option 3.  And remember, you can delete and rebuild the database.

---

### 1.5 Inconsistent error variant choices

**Files:** `services/sessions.rs`, `services/runs.rs`

Examples where the HTTP status feels off:
- `leave_session`: "Not currently in this session" → `BadRequest` (400). Arguably should be 404 or 409.
- `join_session`: "Cannot join a closed session" → `BadRequest` (400). Arguably 409 (state conflict).
- `next_track`: "Session is not active" → `BadRequest` (400). Same — could be 409.
- `create_run`: "Must be an active participant" → `Forbidden` (403). That's correct.
- `delete_run`: "Cannot delete run from a closed session" → `BadRequest` (400). Could be 409.

**Recommended fix:** Adopt a simple convention:
- `BadRequest`: malformed input or invalid value from client.
- `Conflict`: valid input but clashes with server state.
- `NotFound`: resource genuinely doesn't exist.
- `Forbidden`: exists but not allowed for this user.

Re-classify the above under this rule. "Session closed" errors become 409.

**Divergence noted (2026-05-09):** `leave_session` kept `BadRequest` for "Not currently in this session" rather than re-classifying to `Conflict` or `NotFound`. Reasoning is captured inline at [`backend/src/services/sessions.rs:859-863`](../../backend/src/services/sessions.rs#L859-L863): `require_active_participant` returns `Forbidden` (an authorization guard), but trying to leave a session you aren't in is bad input from the client (wrong session named) rather than a state clash on the server. `Conflict` would imply "valid input but clashes with server state," which doesn't fit. `NotFound` would also be ambiguous — the session resource exists; only the participation doesn't. The other call sites in the list above were re-classified per the rule.

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

### 1.6 `.expect("validated above")` in `routes/users.rs`

**File:** `routes/users.rs::update_user` (lines ~170-173)

```rust
let char_id = req.preferred_character_id.expect("validated above");
let body_id = req.preferred_body_id.expect("validated above");
// ... etc
```

This works — the count check ensures all four are Some — but the compiler can't verify that invariant. If someone reorders code, the expect panics.

**Recommended fix:** Introduce an intermediate type that makes the invariant type-level:

```rust
struct RaceSetupUpdate { char_id: i32, body_id: i32, wheel_id: i32, glider_id: i32 }

impl RaceSetupUpdate {
    fn try_from_request(req: &UpdateProfileRequest) -> Result<Option<Self>, AppError> {
        match (req.preferred_character_id, req.preferred_body_id, req.preferred_wheel_id, req.preferred_glider_id) {
            (None, None, None, None) => Ok(None),
            (Some(c), Some(b), Some(w), Some(g)) => Ok(Some(Self { char_id: c, ... })),
            _ => Err(AppError::BadRequest("Race setup all-or-nothing".into())),
        }
    }
}
```

Compiler-enforced. No `.expect`.

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

## 2. Code Reuse Opportunities

### 2.1 Extract "load active session" helper

**Files:** `services/sessions.rs`, `services/runs.rs`

The pattern "find session by ID, error if not found, error if not active" appears 6+ times (`join_session`, `leave_session`, `next_track`, `skip_turn`, `create_run`, `delete_run`).

**Recommended fix:** Add to `services/sessions.rs`:

```rust
pub async fn load_active_session(
    db: &impl ConnectionTrait,
    session_id: &str,
) -> Result<sessions::Model, AppError> {
    let session = sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".into()))?;
    if session.status != SessionStatus::Active.as_str() {
        return Err(AppError::Conflict("Session is not active".into()));
    }
    Ok(session)
}
```

Saves ~40 lines. Pairs naturally with the status enum from 1.2.

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

### 2.2 Extract "require participant" helper

**Files:** `services/sessions.rs`, `services/runs.rs`

The pattern "find session_participants row for (session_id, user_id) where left_at IS NULL" appears in `create_run`, `leave_session`, and implicitly via `check_not_in_any_session`. Each has slightly different error messaging.

**Recommended fix:**

```rust
pub async fn require_active_participant(
    db: &impl ConnectionTrait,
    session_id: &str,
    user_id: &str,
) -> Result<session_participants::Model, AppError> {
    session_participants::Entity::find()
        .filter(
            Condition::all()
                .add(session_participants::Column::SessionId.eq(session_id))
                .add(session_participants::Column::UserId.eq(user_id))
                .add(session_participants::Column::LeftAt.is_null()),
        )
        .one(db)
        .await?
        .ok_or_else(|| AppError::Forbidden("Not a participant in this session".into()))
}
```

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

### 2.3 Extract "touch session" helper

**Files:** `services/sessions.rs`, `services/runs.rs`

Every mutation ends with the same 3-line pattern:

```rust
let mut active_session: sessions::ActiveModel = session.into();
active_session.last_activity_at = Set(now);
active_session.update(&txn).await?;
```

Appears in: `join_session`, `leave_session`, `next_track`, `skip_turn`, `create_run`, `delete_run`. That's ~18 lines of repetition.

**Recommended fix:**

```rust
pub async fn touch_session(
    txn: &impl ConnectionTrait,
    session_id: &str,
) -> Result<(), AppError> {
    sessions::Entity::update_many()
        .col_expr(sessions::Column::LastActivityAt, Expr::value(now_rfc3339()))
        .filter(sessions::Column::Id.eq(session_id))
        .exec(txn)
        .await?;
    Ok(())
}
```

Single-query update, no read required.

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

### 2.4 Collapse FK validation blocks

**Files:** `services/runs.rs::create_run`, `routes/users.rs::update_user`

Five identical 5-line blocks in each file validate character / body / wheel / glider / drink_type IDs exist:

```rust
if characters::Entity::find_by_id(id).one(db).await?.is_none() {
    return Err(AppError::BadRequest("Invalid character_id".into()));
}
// × 5
```

Total: ~50 lines of near-identical code across two files.

**Recommended fix:** Generic helper using SeaORM's `EntityTrait`:

```rust
pub async fn require_exists<E: EntityTrait>(
    db: &impl ConnectionTrait,
    id: <E::PrimaryKey as sea_orm::PrimaryKeyTrait>::ValueType,
    entity_name: &str,
) -> Result<(), AppError>
where E::PrimaryKey: sea_orm::PrimaryKeyToColumn,
{
    if E::find_by_id(id).one(db).await?.is_none() {
        return Err(AppError::BadRequest(format!("Invalid {entity_name}_id")));
    }
    Ok(())
}
```

Called as `require_exists::<characters::Entity>(db, body.character_id, "character").await?`. The type-parameter dance might end up messier than expected depending on SeaORM generics — an alternative is a declarative macro.

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

### 2.5 Extract track selection logic shared between `next_track` and `skip_turn`

**File:** `services/sessions.rs`

`next_track` (lines ~576-678) and `skip_turn` (lines ~685-807) share the bulk of their logic: load used tracks, load all tracks, filter, reset pool if empty, pick random, insert session_race. The only real differences are:
- `skip_turn` also excludes the skipped track from reset
- `skip_turn` keeps the same `race_number` and deletes the old race first

**Recommended fix:** Extract a helper that takes an exclusion list and returns the chosen track Model:

```rust
async fn pick_random_track(
    db: &impl ConnectionTrait,
    exclude_ids: &[i32],
) -> Result<tracks::Model, AppError> {
    let all_tracks = tracks::Entity::find().all(db).await?;
    let available: Vec<&tracks::Model> = all_tracks.iter()
        .filter(|t| !exclude_ids.contains(&t.id))
        .collect();
    let pool = if available.is_empty() {
        tracing::info!("Track pool exhausted — resetting");
        all_tracks.iter().collect()
    } else {
        available
    };
    let mut rng = rand::thread_rng();
    pool.choose(&mut rng)
        .map(|t| (*t).clone())
        .ok_or_else(|| AppError::Internal("No tracks available".into()))
}
```

Then `next_track` and `skip_turn` each become ~30 lines instead of ~100.

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

### 2.6 Consolidate shared test helpers

**Files:** `services/sessions.rs` tests, `services/runs.rs` tests

`setup_db` and `create_user` are duplicated verbatim in each test module. The runs tests additionally duplicate `seed_game_data`. That's ~80 lines of duplicated setup code.

**Recommended fix:** Create `backend/src/test_helpers.rs` (gated with `#[cfg(test)]`):

```rust
#[cfg(test)]
pub mod test_helpers {
    // setup_db, create_user, seed_game_data, seed_tracks_for_test
}
```

Or alternatively use a `tests/common/mod.rs` if moving to integration tests.

- [x] Approved — go with option 1 (`src/test_helpers.rs`). Keep a note for future: if we later restructure to integration tests (`tests/` directory), revisit in favor of `tests/common/mod.rs`.
- [ ] Needs discussion
- [ ] Skip

---

### 2.7 Create a `services/users.rs` module

**File:** `routes/users.rs::update_user`

The update handler is 127 lines with significant business logic: race-setup all-or-nothing validation, FK checks, conditional field updates, response building. This is the only route file with this level of logic — elsewhere (sessions, runs, auth, drink_types) routes are thin pass-throughs to services.

**Recommended fix:** Create `services/users.rs` and move the logic there. Routes handler shrinks to ~10 lines like the other modules.

Fits naturally with #2.4 (FK validation helper) and #1.6 (RaceSetupUpdate type).

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

## 3. Single Function Bloat

### 3.1 `get_session_detail` (~150 lines, 5 distinct queries)

**File:** `services/sessions.rs::get_session_detail` (lines ~297-449)

Does: load session, load host username, load participants (JOIN), count races, load current race (JOIN), load submissions for current race (JOIN), load race history (JOIN). All inline in one function.

**Recommended fix:** Decompose:

```rust
pub async fn get_session_detail(db, session_id) -> Result<SessionDetail> {
    let session = sessions::Entity::find_by_id(session_id).one(db).await?
        .ok_or_else(|| AppError::NotFound(...))?;
    let host_username = load_username(db, &session.host_id).await?;
    let participants = load_participants(db, session_id).await?;
    let current_race = load_current_race(db, session_id).await?;  // incl. submissions
    let races = load_race_history(db, session_id).await?;
    let race_number = races.last().map(|r| r.race_number).unwrap_or(1) as usize;
    Ok(SessionDetail { session, host_username, participants, current_race, races, race_number, ... })
}
```

Bonus: `race_number` can be derived from `races.last()` instead of a separate COUNT query, saving a round trip on every poll. Important since this is the hottest endpoint.

- [x] Approved — decompose and derive `race_number` from `races.last()`.
- [ ] Needs discussion
- [ ] Skip

---

### 3.2 `create_run` (~150 lines, 7 validation blocks)

**File:** `services/runs.rs::create_run` (lines ~117-273)

Time validation (4 inline blocks), session lookup, participant check, duplicate check, 5 FK validations, then insert. All inline.

**Recommended fix:** Extract a validation pipeline:

```rust
async fn validate_run_request(
    db: &impl ConnectionTrait,
    user_id: &str,
    body: &CreateRunRequest,
) -> Result<session_races::Model, AppError> {
    validate_time_fields(body)?;
    let session_race = load_session_race_for_active_session(db, &body.session_race_id).await?;
    require_active_participant(db, &session_race.session_id, user_id).await?;
    ensure_no_duplicate_submission(db, &body.session_race_id, user_id).await?;
    validate_game_data_fks(db, body).await?;
    Ok(session_race)
}
```

Then `create_run` becomes ~30 lines: validate → insert → touch_session → return.

**Adoption status (2026-05-09):** Extracted `validate_run_request` and `insert_run` from `create_run`. The orchestrator is now 4 lines (validate → insert → fetch). The validation interior was kept cohesive (one helper, not seven) — fragmenting each gate into its own function would hide the validation surface behind names and add parameter-threading noise without buying reuse, since none of the gates have a second caller. Note that the audit's predicted ~30 lines doesn't match reality because two pending-races validation gates were added post-audit (skip mutual-exclusion and ordered-submit guard); `validate_run_request` covers all gates, including the post-audit ones. Per Issue [#83](https://github.com/brendanbyrne/beerio-kart/issues/83).

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

### 3.3 `leave_session` host-transfer logic

**File:** `services/sessions.rs::leave_session` (lines ~495-571)

~80 lines with branchy host-transfer logic inline.

**Recommended fix:** Extract:

```rust
async fn transfer_host_or_close(
    txn: &impl ConnectionTrait,
    session: &sessions::Model,
    leaving_user_id: &str,
) -> Result<Option<String>, AppError> {
    // Returns the new host_id if transferred, None if session was closed
}
```

Then `leave_session` reads top-to-bottom as "find participant, mark left, transfer-or-close, touch, commit."

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

## 4. Better Abstraction Opportunities

### 4.1 Introduce a `SessionContext` value object

**Files:** `services/sessions.rs`, `services/runs.rs`

Many functions need the combination of (session, current race, participant check). A small context type could bundle the common loads:

```rust
pub struct SessionContext {
    pub session: sessions::Model,
}

impl SessionContext {
    pub async fn load_active(db, session_id) -> Result<Self> { ... }
    pub async fn require_host(&self, user_id) -> Result<()> { ... }
    pub async fn require_participant(&self, db, user_id) -> Result<session_participants::Model> { ... }
    pub async fn touch(&self, txn) -> Result<()> { ... }
}
```

Usage:
```rust
let ctx = SessionContext::load_active(db, session_id).await?;
ctx.require_host(user_id)?;
// ... business logic ...
ctx.touch(&txn).await?;
```

Aggregates helpers from #2.1–2.3. Optional; depends on taste. Could also be free functions if the struct feels heavy.

- [x] Approved - I like the struct
- [ ] Needs discussion
- [ ] Skip

---

### 4.2 Newtypes for IDs

**Files:** All services.

Every ID is `String` (or `i32`). Nothing stops you passing `user_id` where `session_id` is expected. Rust idiom for this is newtypes:

```rust
pub struct UserId(String);
pub struct SessionId(String);
pub struct RunId(String);
```

With `Deref<Target=str>` or `AsRef<str>` for ergonomics.

**Trade-off:** Touches every service function signature. Big diff, modest practical benefit for a small codebase. Worth it if you're worried about ID confusion; skip if you're not.

**Adoption status (2026-05-09):** Adopted project-wide for the four ID kinds — `UserId`, `SessionId`, `RunId`, `SessionRaceId`. Newtypes live in [`backend/src/domain/ids.rs`](../../backend/src/domain/ids.rs), re-exported from `crate::domain`. Service and middleware function signatures take and return the newtypes; route handlers wrap `Path<String>` extractors at the boundary. Request body types kept as `String` (untrusted client input) — services wrap on entry. Response DTOs use the newtypes directly; `serde(transparent)` keeps wire format identical. SeaORM call sites work without `.as_str()` because `domain/ids.rs` provides `From<&Self> for sea_orm::Value` and `From<Self> for String` per newtype. Per Issue [#82](https://github.com/brendanbyrne/beerio-kart/issues/82).

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

### 4.3 Move dynamic SQL filter building in `list_runs` to SeaORM query builder

**File:** `services/runs.rs::list_runs`

Currently hand-builds SQL with a `params` vec and parameter index tracking. Recent 3C-2 fix addressed the fragility, but it's still more prone to injection bugs than the query builder. SeaORM supports dynamic conditions:

```rust
let mut query = runs::Entity::find()
    .find_also_related(users::Entity)
    .find_also_related(drink_types::Entity);

if let Some(sr) = filters.session_race_id {
    query = query.filter(runs::Column::SessionRaceId.eq(sr));
}
// ... etc
```

Trade-off: SeaORM's JOIN ergonomics for multi-related entities get awkward. The raw SQL might still win for readability. Consider it; skip if no clear win.

- [ ] Approved
- [ ] Needs discussion
- [x] Skip — keep raw SQL for multi-table JOINs.

---

## 5. Consistency & Style

### 5.1 Raw SQL vs. SeaORM query builder

**Observation:** Both styles in use. Raw SQL for JOINs (via `find_by_statement`), builder for single-table. Pragmatic, but undocumented.

**Recommended fix:** Add a convention note to DESIGN.md: "Use the builder for single-table ops; drop to raw SQL only for complex JOINs where the builder is clumsy."

- [x] Approved — document convention: "SeaORM builder for single-table ops; raw SQL via `find_by_statement` for multi-table JOINs."
- [ ] Needs discussion
- [ ] Skip

---

### 5.2 Transaction usage

**Observation:** Most multi-write operations use transactions. Some edge cases don't:
- `routes/drink_types.rs::create_drink_type` — single insert, no txn needed, correct.
- `routes/users.rs::update_user` — single update, no txn needed, correct.

No issues found. Leaving this for completeness.

- [x] Approved (no changes needed) — current usage is correct. Rule: transaction only when multiple writes must succeed/fail together.

---

### 5.3 Route-layer logic inconsistency

**Observation:**
- `routes/sessions.rs`, `routes/runs.rs` — thin pass-throughs (~5 lines per handler). Good.
- `routes/auth.rs` — mostly orchestrates service calls but contains some validation and response-building logic.
- `routes/users.rs::update_user` — 127 lines of business logic. Outlier.

**Recommended fix:** Cap what routes do at: input extraction, delegate to service, map result to response. All validation / DB logic in services. This plus #2.7 (`services/users.rs`) addresses it.

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

## 6. Testing Gaps

### 6.1 Route layer is untested

**Observation:** All tests are at the service layer. Routes (especially the logic-heavy `routes/auth.rs` and `routes/users.rs`) have no direct coverage.

**Recommended fix:** After the route-thinning refactors land (#2.7, #5.3), route layers are so thin they don't need direct tests. Until then, add integration tests via Axum's `tower::ServiceExt::oneshot` for at least the auth endpoints.

- [x] Approved — narrowed scope: integration tests only for `routes/auth.rs` endpoints and `middleware/auth.rs` extractor. Other routes are thin pass-throughs after refactor; no route-level tests needed there.
- [ ] Needs discussion
- [ ] Skip

---

### 6.2 Middleware extractor is untested

**File:** `middleware/auth.rs`

`AuthUser` and `AdminUser` extractors have no direct tests. Mostly covered transitively by any test that hits an authenticated endpoint, but edge cases (missing header, malformed bearer, refresh token used as access) aren't exercised.

**Recommended fix:** Add a `tests/auth_middleware.rs` integration test file. ~5 test cases.

- [x] Approved
- [ ] Needs discussion
- [ ] Skip

---

## 7. Things That Are Good (Keep Doing)

No action needed — flagging these so we don't lose them in refactoring.

- `services/auth.rs` is exemplary: small, focused, well-tested.
- `drink_type_id.rs` is a nice small module with complete test coverage.
- `impl_into_simple_item!` macro in `game_data.rs` — clean macro use, saves repetition.
- Inline comments on tricky bits (ThreadRng `!Send` scoping, deserialize_optional_field rationale) — very helpful.
- `AppError` design — clean, idiomatic, easy to propagate via `?`.
- Login timing-attack mitigation (dummy verify on missing user) — security-conscious.
- JWT `token_type` field — prevents refresh-as-access abuse.
- `refresh_token_version` for revocation — correct JWT revocation pattern.
- `PRAGMA foreign_keys = ON` correctly set per connection.
- `ActiveParticipantRow` extraction already done between `check_not_in_any_session` and `get_active_session_id`.

---

## Implementation Strategy

If most of this is approved, I'd suggest splitting into three PRs for digestibility:

**PR 1: "Low-level primitives"** — Sections 1.2 (enums), 2.1 (load_active_session), 2.2 (require_participant), 2.3 (touch_session), 2.6 (test helpers), 1.4 (timestamp helpers). Pure additions; existing call sites unchanged. Foundation for the others.

**PR 2: "Apply primitives, collapse duplication"** — Rewrite `join_session`, `leave_session`, `next_track`, `skip_turn`, `create_run`, `delete_run` to use the primitives from PR 1. Apply 2.4 (FK helpers), 2.5 (pick_random_track), 3.1 (decompose get_session_detail), 3.2 (decompose create_run), 3.3 (

---

## Document history

- 2026-05-05 — Audit content (authored 2026-04-15) migrated into `docs/designs/` as part of PR 1 (docs restructure foundation, commit `31f90bd`).
- 2026-05-09 — Added §1.5 "Divergence noted" subsection recording that `leave_session::BadRequest` is a deliberate departure from the rule, with a cross-reference to the inline rationale at [`backend/src/services/sessions.rs:859-863`](../../backend/src/services/sessions.rs#L859-L863). Per Issue [#81](https://github.com/brendanbyrne/beerio-kart/issues/81).
- 2026-05-09 — Recorded §4.2 adoption status: the four ID newtypes (`UserId`, `SessionId`, `RunId`, `SessionRaceId`) are now plumbed through service signatures, middleware (`AuthUser` / `AdminUser`), route adapters, response DTOs, and test helpers. Per Issue [#82](https://github.com/brendanbyrne/beerio-kart/issues/82).
- 2026-05-09 — Recorded §3.2 adoption status: `create_run` decomposed into a 4-line orchestrator over `validate_run_request` and `insert_run`. Per Issue [#83](https://github.com/brendanbyrne/beerio-kart/issues/83).
