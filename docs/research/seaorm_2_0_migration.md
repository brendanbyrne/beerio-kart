# SeaORM 2.0 Evaluation — Partial Unique Indexes on SQLite

This document captures the May 2026 evaluation of whether to migrate from
SeaORM's schema-first workflow to the entity-first workflow introduced in
SeaORM 2.0, given Beerio Kart's use of partial unique indexes on SQLite.

## Context

Beerio Kart uses one partial unique index in its current schema:

```sql
CREATE UNIQUE INDEX session_participants_one_active_per_user
  ON session_participants(user_id) WHERE left_at IS NULL;
```

This expresses "at most one active participation per user" while allowing
unbounded historical (left) rows. We expect 2-3 more such indexes over the
project's lifetime.

**The schema-first blocker.** `sea-schema`'s SQLite introspector cannot read
the `WHERE` clause from `sqlite_master.sql` text in a way that propagates to
codegen output. Each `sea-orm-cli generate entity` run produces a wrong
`#[sea_orm(unique)]` attribute on the column and a wrong `has_one`
cardinality on the inverse `Relation`. The entity must be hand-corrected
after every codegen.

**The question.** Does SeaORM 2.0's entity-first workflow solve this, or
does it have the same problem?

## SeaORM 2.0 status (as of 2026-05-03)

**SeaORM 2.0 is not yet stable.** Verified directly against
crates.io's API:

- `max_stable_version: 1.1.20` (released 2026-03-31)
- `max_version: 2.0.0-rc.38` (released 2026-04-09)
- `newest_version: 2.0.0-rc.38`
- ~38 release candidates over ~7 months (rc.1 ≈ Sept 2025)

The 2026-01-12 SeaORM blog post that uses the title "SeaORM 2.0" is an
**API freeze announcement**, not a stable release announcement. Verbatim:

> Over the past few months, we've rolled out a series of SeaORM 2.0
> releases packed with new capabilities. We've stablized our API surface
> now. Other than dependency upgrades (sqlx 0.9), there won't be more
> major breaking changes.

API freeze means new attributes can still be added (they're additive,
non-breaking), but the team is signalling that porting code to 2.0 is
now safe. Recent RC release notes (rc.34, rc.35, rc.38) show the team
is still fixing core entity-first / schema-sync bugs:

- rc.34 — "Don't create index if column is already unique (entity first
  workflow)" (#2950)
- rc.35 — "Fix unique column in schema sync" (#2971)
- rc.38 — "Fix schema sync not discovering tables in non-default
  schemas" (#3016)

## Q1: Can entity-first express a partial unique index? — **No.**

I read `sea-orm-macros/src/derives/entity_model.rs` directly and
enumerated every accepted `#[sea_orm(...)]` attribute on
`DeriveEntityModel`:

- Field-level: `column_type`, `auto_increment`, `comment`,
  `default_value`, `default_expr`, `column_name`, `enum_name`,
  `select_as`, `save_as`, `ignore`, `primary_key`, `nullable`,
  `indexed`, `unique`, `unique_key`, `renamed_from`, `extra`
- Struct-level: `comment`, `table_name`, `schema_name`, `table_iden`,
  `model_ex`, `rename_all`

There is no `where`, `partial`, `filter`, `condition`, or `index_where`
attribute. `unique_key = "name"` only groups columns into a plain
composite unique constraint — no predicate.

The official entity-first doc page never shows a `WHERE` clause, and a
related ergonomics request (#872, "Index::create().filter(...)") was
closed `not_planned` in February 2023.

## Q2: Mixing entity-first with a sidecar migration — **Possible, but risky.**

The lower layer supports it. `sea_query::IndexCreateStatement` has a
`r#where` field with `.and_where(...)` / `.cond_where(...)` builders,
and emits valid `WHERE ...` clauses for SQLite. Verified in
`sea-query/src/index/create.rs`. So a hand-written migration can
declare the partial index in pure Rust.

**The problem is what `sync()` does next boot.** From the entity-first
docs (footnote):

> schema sync would not attempt to do any destructive actions, so
> meaning no DROP on tables, columns and foreign keys. **Dropping
> index is an exception here.**

That is the failure mode. Schema-sync drops indexes it cannot reconcile
to an entity declaration. Issue **#2812** (closed Nov 2025, fixed in
rc.34/rc.35) is the receipt: a multi-column unique index, declared
entirely via `unique_key`, was created on first run and **dropped on
the second sync**. That was the entity-declared case; the rc.34/rc.35
fixes do not address the "preserve unknown index" case.

There is no documented opt-out flag (e.g., `preserve_unknown_indexes`).
On SQLite specifically, the introspector reads `sqlite_master.sql`
text but does not parse the predicate, so a partial index will look
"extra" to the diff and get DROPped.

Practical workarounds: gate `schema-sync` to first-boot only, or
re-create the partial index outside the sync path on every startup.
Both are uglier than just using a normal migration system.

## Q3: Production maturity — **RC series, still ironing out core bugs.**

- 38 RCs over ~7 months. No `2.x.y` stable patch series.
- `1.1.20` is the latest stable.
- GitHub search returns 30 results for `schema-sync`, 175 for
  `entity-first`. Activity concentrated in 2026 RCs, not stabilized.

Notable issues:

- **#2812** (closed, fixed rc.34/rc.35) — Multi-column unique index
  dropped by schema-sync. Direct precedent for the partial-index
  concern.
- **#2666** (closed Nov 2025) — Entity generation misinterprets partial
  unique index as fully unique (the schema-first blocker). The fix
  (sea-schema PR #150) only added an `is_partial` flag for
  **PostgreSQL** discovery; it does not parse the predicate, does not
  propagate to entity codegen output, and does not touch SQLite.
- **#2983** (open, Mar 2026) — "entity first: allow dropping columns and
  tables." Confirms current schema-sync silently ignores DROP-column
  actions.
- **#2953** (open, Feb 2026) — "schema sync does not sync existing
  foreign key actions."
- **#2889** (open, Jan 2026) + **PR #3015** (open) — "entity-first
  migrations." Diff-to-migration codegen is requested but in-progress.

No independent community write-ups (DEV.to, /r/rust, This Week in
Rust) about teams running entity-first in production were located.

## Q4: SQLite-specific schema-sync caveats

No authoritative per-scenario documentation exists for SQLite. The
following is stitched from the entity-first doc page, recent RC
release notes, and #2983.

- **Rename a column.** Supported via `#[sea_orm(renamed_from =
  "old_name")]`. Emits `ALTER TABLE ... RENAME COLUMN`, which SQLite
  has supported since 3.25.0 (2018).
- **Change a column's type.** Not documented. SQLite has no `ALTER
  COLUMN ... TYPE`; the standard fix is a table rebuild (CREATE NEW +
  INSERT + DROP + RENAME). No evidence schema-sync performs the
  rebuild — expect silent no-op or failure. Use a hand-written
  migration.
- **Add a `NOT NULL` column without default to a non-empty table.**
  SQLite rejects this at the engine level. Schema-sync emits the
  `ALTER` and SQLite returns an error. Either give the column a
  default or do a rebuild migration.
- **Remove a foreign key.** Per the entity-first doc footnote,
  schema-sync **will not** drop FK constraints. On SQLite, removing an
  FK requires a table rebuild anyway, which schema-sync does not
  implement. The FK quietly remains.

General SQLite caveat: `ALTER TABLE` is much narrower than on
PostgreSQL or MySQL, and schema-sync's "non-destructive only" stance
compounds that. The set of changes sync can do for you on SQLite is a
strict subset of "things SQLite can do safely with one statement."

## Workaround investigated: generated-column trick

There is a SQL technique for converting a partial unique into a
regular unique by using a generated column:

```sql
ALTER TABLE session_participants ADD COLUMN active_user_id
  GENERATED ALWAYS AS (CASE WHEN left_at IS NULL THEN user_id ELSE NULL END);
CREATE UNIQUE INDEX ... ON session_participants(active_user_id);
```

SQLite treats NULLs as distinct in unique constraints, so historical
rows (NULL `active_user_id`) all coexist while at most one non-NULL
value per `user_id` is allowed. SQLite has supported generated columns
since 3.31 (2020).

In SeaORM 2.0 entity-first, this could be expressed via the `extra`
escape hatch:

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "session_participants")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub left_at: Option<DateTimeUtc>,

    #[sea_orm(
        unique,
        nullable,
        extra = "GENERATED ALWAYS AS (CASE WHEN left_at IS NULL THEN user_id ELSE NULL END) STORED"
    )]
    pub active_user_id: Option<i32>,
}
```

`extra` is verified to pass strings through to `sea_query::ColumnDef::extra()`,
which appends arbitrary DDL text to a column definition.

**Three reasons this is not recommended:**

1. **It's an escape hatch, not a DSL.** `sea_query::ColumnDef` has a
   first-class `.generated(expr, stored)` builder, but the SeaORM
   entity macro doesn't expose a `#[sea_orm(generated = "...")]`
   attribute that maps to it. The `extra` approach is a raw SQL
   string: not type-checked, breaks if columns are renamed, and
   database-specific.
2. **Schema-sync will probably misbehave.** sea-schema's introspector
   likely doesn't surface the generation expression on round-trip
   discovery, so the diff between entity and live DB will look
   mismatched. Best case: sync no-ops. Worst case: it tries to rebuild
   the table or drop the unique index. Untested.
3. **It changes the data model for ORM-fit reasons.** Anyone reading
   the table will see two `user_id`-shaped columns and need an
   explanatory comment.

The clean upstream fix would be a `#[sea_orm(generated = "...")]`
attribute exposing `sea_query::ColumnDef::generated()`. That is the
issue worth opening if we want this properly supported.

## Decision: stay schema-first

**Recommendation: stay on schema-first with hand-corrected entities.**

Reasoning:

1. **The DSL doesn't support partial indexes**, and the workaround
   (sidecar migration) collides with sync's documented index-drop
   behavior. We'd be trading a known annoyance (post-codegen patch)
   for a less predictable one (sync silently wiping our partial index
   on a future cold start).
2. **2.0 isn't stable yet.** Adopting rc.38 to fix one schema-first
   irritant means a moving target. Every recent RC has been fixing
   core entity-first / schema-sync bugs.
3. **The existing blocker is cheap to script around.** After
   `sea-orm-cli generate entity`, delete the bogus
   `#[sea_orm(unique)]` on `user_id` and change the inverse
   `Relation` to `has_many`. A small post-codegen script (or a clearly
   commented manual step) is far cheaper than fighting sync semantics.

## Why entity-first probably never gets partial-index DSL support

The pattern across SeaQL's libraries is that the *entity* layer is for
type/relation mapping and the *schema construction* layer (sea_query)
is where DDL flexibility lives. The capability exists at the lower
layer (`sea_query::IndexCreateStatement::and_where`); it's just not
surfaced in the entity DSL. There is no clean Rust syntax for
"WHERE left_at IS NULL" in a derive macro — the options are all bad
(string predicate loses type safety; full predicate DSL is huge surface
area; closures don't fit derive macros). The team has likely decided
"leave that to migrations."

Issue #872 closing as `not_planned` reinforces this read.

## Things to watch

These would change the calculus enough to revisit the decision:

- **2.0 actually ships stable.** Removes the moving-target argument.
- **A `preserve_unknown_indexes` flag** for schema-sync. Would let a
  sidecar migration coexist with entity-first sync without index
  deletion. Non-breaking, plausible post-stable addition.
- **A `#[sea_orm(generated = "...")]` attribute** exposing
  `sea_query::ColumnDef::generated()`. Would make the generated-column
  trick a clean, type-checked solution.
- **#3015 ("entity-first migrations") lands.** Diff-to-migration
  codegen would replace schema-sync's destructive-vs-non-destructive
  tension with a reviewable migration generator.
- **sea-schema gains SQLite WHERE-clause parsing** (parallel to the PG
  `is_partial` work in PR #150). Would fix the original schema-first
  codegen bug at the root.

## Sources

- [Entity-first docs](https://www.sea-ql.org/SeaORM/docs/generate-entity/entity-first/)
- [SeaORM what's new](https://www.sea-ql.org/SeaORM/docs/introduction/whats-new/)
- [SeaORM 2.0 API freeze blog post — 2026-01-12](https://www.sea-ql.org/blog/2026-01-12-sea-orm-2.0/)
- [crates.io API for sea-orm](https://crates.io/api/v1/crates/sea-orm)
- [SeaQL/sea-orm releases](https://github.com/SeaQL/sea-orm/releases)
- [sea-orm-macros entity_model.rs](https://github.com/SeaQL/sea-orm/blob/master/sea-orm-macros/src/derives/entity_model.rs)
- [sea-query table/column.rs (extra, generated)](https://github.com/SeaQL/sea-query/blob/master/src/table/column.rs)
- [sea-query index/create.rs (and_where)](https://github.com/SeaQL/sea-query/blob/master/src/index/create.rs)
- Issues: [#872](https://github.com/SeaQL/sea-orm/issues/872), [#2666](https://github.com/SeaQL/sea-orm/issues/2666), [#2812](https://github.com/SeaQL/sea-orm/issues/2812), [#2889](https://github.com/SeaQL/sea-orm/issues/2889), [#2953](https://github.com/SeaQL/sea-orm/issues/2953), [#2983](https://github.com/SeaQL/sea-orm/issues/2983), [#3015](https://github.com/SeaQL/sea-orm/pull/3015)

## Document history

- 2026-05-03 — Initial document. Captures the SeaORM 2.0 entity-first evaluation and the decision to stay on schema-first with hand-corrected entities for tables that have partial unique indexes.
