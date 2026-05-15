# SeaORM entity codegen strategy — design record (2026-05-02)

> **Status: complete.** Archived 2026-05-15. Implemented via PR-X1 and PR-X2 (see [`compliance-plan.md`](./compliance-plan.md) Stream J). The hand-written-entities convention is live in [`../../coding-standards/seaorm.md`](../../coding-standards/seaorm.md) § 6.

## 1. Context

PR-E3 (`phase-e/drop-sessions-created-by`) surfaced a workflow bug. After running `just entities` to regenerate `backend/src/entities/sessions.rs`, four other entity files came back dirty in two distinct ways:

**Group (1) — codegen-clobbered hand-corrections:**

- `entities/session_participants.rs` gets `#[sea_orm(column_type = "Text", unique)]` on `user_id`. The schema actually has a *partial* unique index — `CREATE UNIQUE INDEX … ON session_participants(user_id) WHERE left_at IS NULL` — codegen erases the WHERE clause and treats it as full uniqueness.
- `entities/users.rs` gets `#[sea_orm(has_one = "super::session_participants::Entity")]`. This *is* runtime-impacting: `find_related` returns `Option<Model>` instead of `Vec<Model>`, breaking historical-row loading. The cardinality bug stems from the same root cause: codegen sees the FK column as fully unique and infers 1:1.
- `entities/session_race_participations.rs` loses its hand-written doc-comment header.

**Group (2) — legitimate new codegen unlocked by PR-E3.** Removing the second FK from `sessions` → `users` resolved an ambiguity that previously caused codegen to skip several relations. The new output adds `users.has_many sessions`, a many-to-many between `users` and `session_races` via `session_race_participations`, plus the corresponding `Related` impls. **These are correct and should be kept.** PR-E3 reverted them wholesale; they need to be re-added on that branch as a separate cleanup before merge.

This record is about Group (1). Group (2) is a one-off cleanup on PR-E3, independent of the design decision below.

The previous workaround was to embed corrections as comments inside the generated files. Anyone running `just entities` blew them away with no automated check. PR-E3 was the second cycle to lose this work — the trigger for this design record.

- [x] Approved — context accurate

## 2. Verification of root cause

The bug originates upstream in `sea-schema`'s SQLite introspector. SQLite's `PRAGMA index_list` and `PRAGMA index_info` queries don't expose the WHERE clause of partial indexes, and sea-schema does not parse `sqlite_master.sql` to recover it. ([SQLite partial indexes](https://www.sqlite.org/partialindex.html))

This is industry-wide on SQLite, not specific to sea-schema. SQLAlchemy has the identical bug ([sqlalchemy #8804](https://github.com/sqlalchemy/sqlalchemy/issues/8804)). PostgreSQL handles this correctly via `pg_index.indpred` ([Postgres docs](https://www.postgresql.org/docs/current/catalog-pg-index.html)).

There is an existing upstream issue tracking the propagation of this through to entity codegen: [`SeaQL/sea-orm#2666`](https://github.com/SeaQL/sea-orm/issues/2666) ("Entity generation misinterprets partial unique index as fully unique"). It was closed in November 2025 with sea-schema PR #150, which added an `is_partial` flag for **PostgreSQL** discovery only. **The fix does not parse the predicate, does not propagate to entity codegen output, and does not touch SQLite.** So while the issue is closed, our specific bug remains. Documented in detail in [`docs/research/seaorm_2_0_migration.md`](../../research/seaorm_2_0_migration.md).

- [x] Approved — verification

## 3. The architectural reframing

The previous workaround discussion treated this as a tooling quirk to manage. That framing was wrong. The real issue is that **we've been using codegen-from-introspection for a workflow it isn't designed for.**

The dependency chain we currently use:

> Migration code → Database → Entity files

The migration code (which we hand-write) contains complete schema information, including the WHERE clause of the partial index. Running migrations builds a database. Then `sea-orm-cli generate entity` introspects that database and produces entity files. **The information loss happens at the third arrow** — the introspector reads the DB via SQLite PRAGMAs and can't recover everything the migration knew.

This round-trip through the database makes sense for one specific workflow: **"I have a database I didn't create, and I want entity types to talk to it."** Adopting an ORM on top of a legacy schema, integrating with someone else's DB, prototyping with raw SQL. For that workflow, going DB → entity is correct.

For *our* workflow — greenfield project, we own the schema, we wrote the migration — that round-trip is unnecessary. We have a perfectly good source of truth (the migration). Going DB → entity throws away information we already have, and the introspection limitations are entirely a consequence of that wasted round-trip.

Most modern "schema as code" implementations don't round-trip through a database at runtime for development workflow. Either the schema file is the source (Prisma, Ent), or hand-authored model classes are the source (SQLAlchemy, TypeORM, Diesel `schema.rs`-as-source), or entity-first ORM patterns where migrations derive from entities (SeaORM 2.0).

- [x] Approved — reframing

## 4. SeaORM 2.0 entity-first investigation

We considered whether SeaORM 2.0's entity-first workflow would solve the partial-index problem. A separate research session investigated this against primary sources; the full evaluation is at **[`docs/research/seaorm_2_0_migration.md`](../../research/seaorm_2_0_migration.md)**. Summary of findings:

**SeaORM 2.0 is not stable yet.** As of 2026-05-03 (verified directly via the crates.io API):

- Latest stable: `1.1.20` (released 2026-03-31).
- Latest 2.0 release candidate: `2.0.0-rc.38` (released 2026-04-09).
- ~38 RCs over ~7 months (rc.1 ≈ September 2025).
- The 2026-01-12 SeaORM blog post titled "SeaORM 2.0 Migration Guide" is an **API freeze announcement**, not a stable release announcement. (My earlier framing in this record had this wrong.)
- Recent RC release notes (rc.34, rc.35, rc.38) show the team still fixing core entity-first / schema-sync bugs. Several open issues for entity-first features remain (#2983, #2953, #2889, PR #3015).

**Entity-first DSL does not support partial unique indexes.** Verified by reading `sea-orm-macros/src/derives/entity_model.rs` directly — the macro accepts `column_type`, `auto_increment`, `default_value`, `nullable`, `indexed`, `unique`, `unique_key`, `extra`, etc. There is no `where`, `partial`, `filter`, `condition`, or `index_where` attribute. The closest related ergonomics request ([sea-query #872](https://github.com/SeaQL/sea-query/issues/872), "Index::create().filter(...)") was closed `not_planned` in February 2023.

**Sidecar migration alongside entity-first is not safe.** `sea_query::IndexCreateStatement` *can* express partial indexes via `.and_where(...)` at the schema-construction layer — so a hand-written migration can declare it. But `schema-sync`'s documented behavior is to **drop indexes it cannot reconcile to an entity declaration**. Issue #2812 (closed Nov 2025) is the receipt: a multi-column unique index was created on first run and dropped on the second sync. The rc.34/rc.35 fixes addressed entity-declared cases but not the "preserve unknown index" case. There's no documented opt-out flag.

**Generated-column workaround exists but is not recommended.** SQLite supports `GENERATED ALWAYS AS (CASE WHEN left_at IS NULL THEN user_id ELSE NULL END)` plus a regular UNIQUE INDEX as a way to encode partial uniqueness. SeaORM 2.0's `extra = "..."` escape hatch can pass that through. Three reasons to avoid it: (1) it's a raw-SQL escape hatch, not type-checked, fragile under renames; (2) sea-schema's introspector likely doesn't surface generation expressions, so schema-sync's diff is unpredictable; (3) it changes the data model for ORM-fit reasons. Full discussion in the migration eval doc.

**Conclusion:** entity-first is not a near-term path for us. We may revisit if 2.0 ships stable, if a `preserve_unknown_indexes` flag lands, if a `#[sea_orm(generated = "...")]` attribute is added, or if PR #3015 (entity-first migrations) lands — see § 10.

- [x] Approved — entity-first investigation

## 5. Decision

**Adopt hand-written entities. Migration code is the source of truth. Codegen is reduced to a bootstrap-only tool.**

Concretely:

- Existing entity files become committed source code, hand-edited going forward. Strip the `@generated by sea-orm-codegen` headers — they're misleading.
- `just entities` is renamed `just entities-bootstrap` (or similar), with help-text that says: "Use once when scaffolding a new table. Hand-edit afterward. Do not re-run on existing entities — it will overwrite hand-corrections."
- New table workflow: write migration → run bootstrap → move output into the entities directory → hand-edit → commit.
- Existing-table changes: edit migration and entity together in the same PR (consistent with the existing atomic-PR rule already in CLAUDE.md).

We stay on SeaORM 1.x for now. The 2.0 upgrade is a separate decision (independent of this one), is not gated on this work, and per the migration-eval doc should wait until 2.0 ships stable anyway.

- [x] Approved — decision

## 6. Why this is the right answer (and what we considered instead)

Three reasons hand-written wins:

1. **It addresses the root cause, not the symptom.** The architectural reframing in § 3 makes clear that the smell came from misusing a tool. Hand-written entities are the natural shape for greenfield projects that own their schema.

2. **It handles every codegen quirk uniformly.** The partial-index bug is one instance; multi-FK ambiguity is another (we hit that one in PR-E3 too). Hand-written entities don't have either bug. Future SQLite features that codegen mishandles cost us nothing.

3. **The cost is bounded and one-time.** ~14 entities, ~30–50 lines each, hand-converted once. Per-table cost going forward: ~25 minutes hand-write vs. ~5 minutes codegen-and-verify. Small delta for our schema's churn rate.

**Considered alternative — keep codegen + post-process script.** The migration-eval doc recommends "stay schema-first with hand-corrected entities," interpreted as: keep running codegen, automate the two known corrections via a small post-processing script that runs after `sea-orm-cli generate entity`. This is roughly equivalent to my earlier "Option B-prime" approach.

We chose hand-written entities over the post-process script for two reasons:

1. **The smell argument.** Post-codegen patching keeps a workflow loop where the codegen output is "almost right, with a documented list of things to fix." Maintaining that list as new partial indexes appear (we expect 2-3 more — see `seaorm.md` § 11 discussion of the recurring pattern) is ongoing work that grows over time. Hand-writing once and owning the result eliminates the list.
2. **It treats codegen as authoritative when it isn't.** Once the architectural reframing in § 3 lands ("schema-as-code shouldn't round-trip through the database"), continuing to run codegen as a routine workflow step contradicts the principle even if a post-script papers over the visible defects.

The post-process-script approach remains a fallback if hand-written entities turn out to be more painful in practice than expected. Both are coherent choices; we're picking the architecturally cleaner one.

- [x] Approved — rationale

## 7. Verification approach

Without codegen acting as an indirect drift-checker, we replace it with one specific test that runs in CI:

```rust
// tests/entity_schema_drift.rs (or similar)
#[tokio::test]
async fn each_entity_can_load_from_a_fresh_migrated_db() {
    let db = Database::connect("sqlite::memory:?cache=shared").await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    // For every entity, force SeaORM to issue a SELECT covering all declared columns.
    // If a column is missing from the migration, or the type mismatches, this fails.
    users::Entity::find().limit(0).all(&db).await.unwrap();
    sessions::Entity::find().limit(0).all(&db).await.unwrap();
    runs::Entity::find().limit(0).all(&db).await.unwrap();
    // ... one line per entity ...
}
```

This catches:

- A column added to the migration but missing from the entity (entity's SELECT will fail to bind).
- A column type changed in the migration without a matching entity update (decode error at runtime).
- An entity declaring a column that doesn't exist in the migration (SELECT references a missing column).

It does *not* catch relation-cardinality mismatches or hand-corrections forgotten on a specific column attribute. Those rely on review and the atomic-PR rule.

The test is ~30–50 lines. Cost is one entry per entity — easy to maintain.

- [x] Approved — verification approach

## 8. `docs/coding-standards/seaorm.md` updates

Section 6 (Entity organization) needs reworking. Today it says "treat the generated `entities/*.rs` files as build output. No hand-edits." That rule inverts after this change.

Replacement language for § 6:

- **Rule:** Entities are committed source code, hand-edited as the schema evolves. Codegen is a one-shot scaffolding tool, not a routine workflow step.
- **Rule:** When adding a new table, write the migration first, run `just entities-bootstrap` to scaffold the entity, then own that file going forward.
- **Rule:** When changing an existing table, edit the migration and the entity in the same PR. Run the schema-drift test to catch column/type mismatches.
- **Rule:** Do not re-run codegen on existing entity files. It will clobber hand-corrections (the partial-index attribute fix and the `has_many` cardinality fix on `users` ↔ `session_participants` are both currently necessary; future schemas will likely have similar cases).

§ 11 (Relations) becomes simpler — codegen ambiguity (the multi-FK case currently noted as a footnote) becomes a non-issue when entities are hand-authored. The rule still applies as guidance for *what to write*; the warning about codegen-getting-it-wrong becomes hypothetical.

- [x] Approved — `seaorm.md` updates

## 9. Implementation sequencing

- **PR-E3 cleanup** (independent of this record): re-add the legitimate Group (2) relations on the existing PR-E3 branch before merge. Owed work.

- **PR-X1 — Convert to hand-written entities.**
  - Re-apply the two known hand-corrections (`session_participants.user_id` partial-index attribute fix, `users` ↔ `session_participants` relation as `has_many`).
  - Strip `@generated by sea-orm-codegen` headers from all entity files.
  - Rename `just entities` to `just entities-bootstrap` with appropriate help text.
  - Update `docs/coding-standards/seaorm.md` § 6 (and § 11 footnote) per § 8 above.
  - Effort: M.

- **PR-X2 — Add schema-drift verification test.**
  - One test file with the per-entity `find().limit(0).all()` pattern.
  - Wire into the CI test job (already runs `cargo test`).
  - Effort: S.
  - Depends on: PR-X1.

- **No PR-X3 (upstream issue).** We considered filing a sea-schema issue for SQLite partial-index introspection. Skipping for now — #2666 already exists (closed without addressing SQLite), and we no longer rely on the introspection path for our own workflow. If we hit it again later (e.g., during a Postgres migration evaluation, where the PG side of #2666's fix might apply) we can revisit.

Compliance plan update: add PR-X1 and PR-X2 as a new "Phase J — Codegen workflow" section in `docs/compliance-plan.md`. The existing entry presuming `just entities` as a routine step (in the compliance plan and in places like CLAUDE.md) gets updated as part of PR-X1.

- [x] Approved — sequencing

## 10. Long-term signpost

Conditions under which we'd revisit this decision (cribbed from the migration-eval doc's "Things to watch" list):

- **SeaORM 2.0 actually ships stable.** Removes the moving-target argument against entity-first.
- **A `preserve_unknown_indexes` flag for `schema-sync`.** Would let a sidecar migration coexist with entity-first sync without index deletion. Non-breaking, plausible post-stable addition.
- **A `#[sea_orm(generated = "...")]` attribute** exposing `sea_query::ColumnDef::generated()`. Would make the generated-column trick a clean, type-checked solution.
- **PR #3015 (entity-first migrations) lands.** Diff-to-migration codegen would replace schema-sync's destructive-vs-non-destructive tension with a reviewable migration generator.
- **sea-schema gains SQLite WHERE-clause parsing.** Would fix the original schema-first codegen bug at the root.
- **Schema growth past ~30 tables** with frequent additions. Per-table hand-write cost starts to dominate.
- **Crossing to PostgreSQL.** Postgres exposes partial-index predicates via `pg_index.indpred`, so codegen-from-introspection becomes lossless on the PG side.

If none of those triggers fire, hand-written entities stay in place. Today's schema (~14 tables, infrequent additions, prelaunch) makes the trade clearly worth it.

- [x] Approved — signpost

---

## Sign-off summary

When all ten checkboxes above are checked, the design is approved and Claude Code can proceed with PR-X1 / PR-X2. The PR-E3 Group (2) cleanup happens regardless, on the existing branch.

Reference document: [`docs/research/seaorm_2_0_migration.md`](../../research/seaorm_2_0_migration.md) — full primary-source evaluation.

- [x] **All sections approved — clear to implement**
