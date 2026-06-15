# Rust → TypeScript type-sync evaluation

> **Status:** Research note. Not a decision. Informs `coding-standards/typescript.md` § 11.
> **Date:** 2026-05-16.
> **Question:** Should Beerio Kart adopt a tool to generate TypeScript types from Rust DTOs, and if so which one?
> **Recommendation:** No tool today. When DTO count crosses ~30 types or we adopt OpenAPI for any other reason, adopt `schemars` + `json-schema-to-typescript` (the only path that natively understands our `nutype` newtypes). Detailed reasoning below.

## Why this question came up

`frontend/src/api/types.ts` mirrors the Rust DTOs by hand. Brendan asked specifically about 1Password's `typeshare`, with the broader question of whether automated Rust→TS type-sync would prevent backend↔frontend drift.

The constraint that drove the recommendation: **the backend uses `nutype` for every domain newtype** (`UserId`, `Username`, `RaceTimeMs`, etc., per `coding-standards/rust.md` § 2). `nutype` is a proc-macro that *replaces* the annotated struct during expansion. Tools that parse Rust source pre-expansion can't see the result.

## The contenders

### typeshare (`1Password/typeshare`)

- **Status.** ~2.9k stars, 1.13.x line current, slow-but-active maintenance (most 2025 commits are bugfixes). Actively maintained; the LibHunt "discontinued" claim is wrong — confused with the SaaS at `typeshare.co`.
- **Mechanism.** Annotation-based. Add `#[typeshare]` to types; the `typeshare-cli` binary parses Rust source via `syn` and emits a single `.ts` file (or multiple, with the 1.10+ multi-file generator).
- **Coverage.** Generics work. Lifetimes stripped. All serde tag modes (`tag`, `tag`+`content`, `untagged`) supported. `rename_all`, `transparent`, optional fields, built-in mappings for `Uuid`/`chrono`. Discriminated unions emit cleanly.
- **`nutype` blocker (critical).** `typeshare` parses source via `syn` and cannot expand macros. It sees `#[nutype(...)] pub struct UserId(Uuid);` as a tuple struct with attributes it doesn't understand — it can't see the `#[serde(transparent)]` that `nutype` injects in its expansion. Workaround: parallel `#[cfg(not(typeshare))]` stubs or per-field `#[typeshare(serialized_as = "string")]` annotations. Either is significant friction in a codebase where IDs are branded by default. Upstream confirmation: [issue #124](https://github.com/1Password/typeshare/issues/124).
- **CI workflow.** Run CLI, `git diff --exit-code` the output file. Well-understood.
- **Generated TS.** Plain `type` / `interface`. Discriminated unions match serde. No branded types. Rustdoc preserved as JSDoc.
- **Drawbacks for us.** The macro-blindness is the dealbreaker. Multi-language is unused weight. Every type needs explicit `#[typeshare]` — easy to forget on new endpoints.

Sources: [github.com/1Password/typeshare](https://github.com/1Password/typeshare), [annotations docs](https://github.com/1Password/typeshare/blob/main/docs/src/usage/annotations.md), [typeshare/issues/124](https://github.com/1Password/typeshare/issues/124).

### ts-rs (`Aleph-Alpha/ts-rs`)

- **Status.** ~1.8k stars, 11.x current (released 2025). Maintenance has effectively shifted to community contributors (`escritorio-gustavo` and others); Aleph-Alpha's name is on the org but the maintenance is community-driven. Healthy issue throughput.
- **Mechanism.** Derive-macro. `#[derive(TS)] #[ts(export)] struct Foo { ... }` — the derive emits a `#[test]` that writes `bindings/Foo.ts` (or `$TS_RS_EXPORT_DIR/Foo.ts`) when you `cargo test`. One file per type.
- **Coverage.** Most complete serde-compat of the three. Generics with bounds, lifetimes stripped, all tag modes, `flatten`, `skip`, `skip_serializing`, `skip_serializing_if`, `default`. Optional fields. `#[serde(transparent)]` via `#[ts(type = "...")]`. Built-in mappings via feature flags. 11.0 made `bigint` handling configurable.
- **`nutype` blocker (same).** Same proc-macro composition issue: `#[derive(TS)]` can't coexist with `#[nutype]` because `nutype` rewrites the struct first. Workarounds: `nutype`'s `derive_unchecked(TS)` escape hatch (added in nutype 0.6.2 — bypasses nutype's validation guarantees, and the `TS` derive expects a struct shape that `nutype`'s expansion may not provide), hand-implement the `TS` trait (~5 lines per newtype), or `#[ts(as = "String")]` on every *field* that uses a newtype. None are great.
- **CI workflow.** `cargo test export_bindings`, then `git diff --exit-code bindings/`. The `cargo test` mechanism is a clever hack but it inflates `cargo test` output and means binding generation costs a full test compile.
- **Generated TS.** Per-type `.ts` files with explicit `import` lines for cross-references. Discriminated unions match serde. No branded types. JSDoc preserved.
- **Drawbacks for us.** Same `nutype` blocker. Per-type file design is noisy. Generation-via-`cargo test` surprises contributors.

Sources: [github.com/Aleph-Alpha/ts-rs](https://github.com/Aleph-Alpha/ts-rs), [docs.rs/ts-rs](https://docs.rs/ts-rs/latest/ts_rs/), [ts-rs/issues/49 (newtype wrappers)](https://github.com/Aleph-Alpha/ts-rs/issues/49).

### specta (`specta-rs/specta`)

- **Status.** ~583 stars, **2.0.0 still in release-candidate phase** as of May 2026 (`2.0.0-rc.24` current). Development resumed under Flight Science funding starting January 2025, ongoing into May 2026 ([issue #475](https://github.com/specta-rs/specta/issues/475)). Org moved from `oscartbeaumont/specta` to `specta-rs/specta`. **Sibling project rspc (the killer Axum-integration story) had its development stepped back from in 2024** — [discussion](https://github.com/specta-rs/rspc/discussions/351). Specta itself is healthy but the rspc story is unclear.
- **Mechanism.** Derive-based with a runtime exporter. `#[derive(Type)]` on types, plus an exporter call (`Typescript::default().export_to(...)`). Types register into a `TypeCollection`; the TS exporter consumes the collection.
- **Coverage.** Most expressive of the three. Generics, lifetimes, all serde tag modes, `rename_all`, `transparent`, deep nesting. Built-ins for `uuid`/`chrono`/`time`/`url`/`bigdecimal`. **Branded types are an experimental feature in the 2.0 RC line** — `branded!` macro plus `Typescript::branded_type_impl` config, designed for lifting Rust newtypes into TS branded primitives. Genuinely the closest match to what we want — but experimental and may shift before 2.0 final.
- **`nutype` blocker (same).** Same proc-macro composition issue. Same workaround menu as `ts-rs`: `nutype`'s `derive_unchecked(Type)`, hand-implement `Type`, or `#[specta(as = String)]` per-field.
- **CI workflow.** Run export binary (`cargo run --example export-types`), `git diff --exit-code`. Same shape as typeshare's, less surprising than ts-rs's `cargo test` ritual.
- **Generated TS.** Best of the three. Multi-file or namespaced output. Discriminated unions match serde. Branded types available (experimental). JSDoc preserved.
- **Drawbacks for us.** Pre-1.0 (RC churn). The rspc story raised eyebrows; you're an off-the-main-road specta user if you're not on Tauri/rspc. Branded types are experimental — you'd be an early adopter.

Sources: [github.com/specta-rs/specta](https://github.com/specta-rs/specta), [specta.dev](https://specta.dev/), [issue #475](https://github.com/specta-rs/specta/issues/475), [rspc/discussions/351](https://github.com/specta-rs/rspc/discussions/351).

## Alternatives that go through an intermediate format

### utoipa + openapi-typescript

- **What.** [`utoipa`](https://github.com/juhaku/utoipa) generates an OpenAPI spec from Axum handlers (via `utoipa-axum`); [`openapi-typescript`](https://github.com/openapi-ts/openapi-typescript) generates a TS types file from the spec.
- **`nutype` compatibility.** **Native.** `nutype` has a `utoipa` feature flag — `#[nutype]` types implement `utoipa::ToSchema` directly with no workaround.
- **Drawback.** Commits us to OpenAPI, which we have explicitly punted on. OpenAPI is also itself a contract document — it would compete with `docs/api-contract.md`.
- **Upside.** The OpenAPI doc is a useful artifact in its own right: spec-driven mock servers, public API docs if we ever want them, third-party client SDK generation.

### schemars + json-schema-to-typescript

- **What.** Derive `JsonSchema` on Rust types via the [`schemars`](https://github.com/GREsau/schemars) crate; emit JSON Schema; run [`json-schema-to-typescript`](https://www.npmjs.com/package/json-schema-to-typescript) on the frontend.
- **`nutype` compatibility.** **Native.** `nutype` has a `schemars08` feature flag. `#[nutype]` types produce correct JSON Schemas with the right `transparent` shape.
- **Drawback.** Two tools, two ecosystems, output TS is less aesthetic than ts-rs/specta (`json-schema-to-typescript` produces `interface` declarations and sometimes verbose union shapes). No commitment to OpenAPI, but JSON Schema is itself a contract — if we adopt it we should decide whether `docs/api-contract.md` is the source of truth or a derived artifact.
- **Upside.** Smallest commitment of any of the "use a tool" options. Doesn't lock in OpenAPI. `nutype`-native.

### Stay hand-rolled

- **What.** Continue maintaining `frontend/src/api/types.ts` by hand.
- **`nutype` compatibility.** Trivially compatible — we write the brands by hand (per `typescript.md` § 3).
- **Drawback.** Drift risk. Every new endpoint requires touching two files. Easy to forget.
- **Upside.** Zero build complexity, zero macro debugging, zero agent-coordination friction. Each side of the contract is legible to both Cowork and Claude Code without indirection.

## Recommendation

**Stay hand-rolled for now. When you cross the adoption threshold, use `schemars` + `json-schema-to-typescript`** (or `utoipa` if a separate OpenAPI decision lands first).

The four reasons:

1. **All three Rust-direct tools fail on `nutype`.** typeshare, ts-rs, and specta all parse Rust source pre-macro-expansion. `nutype` rewrites structs *during* expansion. There's no clean composition — every branded type needs a workaround. `nutype`'s `schemars08` feature flag is the only sanctioned path where the macros compose, and that path routes through JSON Schema.
2. **We're not getting the killer feature of any of these tools.** typeshare's multi-language is unused. ts-rs's per-type files are noise. specta's tightest integration (rspc) is dormant. None of these tools is *built for* our situation; we'd be using each in its plainest mode.
3. **The two-assistant workflow penalizes invisible state.** Codegen artifacts that need a `cargo test` or CLI run to refresh are exactly the kind of thing that drifts when Cowork edits Rust and the next Claude Code session assumes the TS side is current. Hand-rolled keeps both sides legible to both assistants. A CI drift check is necessary regardless of tool choice.
4. **We explicitly haven't decided on OpenAPI.** That's exactly when *not* to let a tool decide for us. Hand-rolling preserves the optionality. If we later adopt OpenAPI for any reason, `utoipa` becomes the obvious pick and gives us types as a side effect.

### Adoption threshold

Revisit when either is true:

- DTO count in `backend/src/dtos/` (or equivalent) crosses ~30 types and hand-sync is costing measurable PR time.
- A separate decision lands to adopt OpenAPI (public API docs, third-party SDKs, mock servers, etc.). At that point, `utoipa` becomes the obvious adoption — types via openapi-typescript are a side effect.

### Cheap drift detection without a tool

**Implemented — PR-H1 / Issue [#185](https://github.com/brendanbyrne/beerio-kart/issues/185).** A CI check fails the PR if a backend wire-contract file changes without `frontend/src/api/types.ts` also changing — exactly the `git diff --name-only origin/main...HEAD` grep sketched here. There's no single `backend/src/dtos/` directory (the DTOs are spread across `backend/src/routes/`, `backend/src/domain/enums.rs`, and several `backend/src/services/` modules), so the check watches an explicit path list rather than one folder. It lives in [`.github/workflows/dto-drift.yml`](../../.github/workflows/dto-drift.yml) + [`.github/scripts/dto-drift-check.sh`](../../.github/scripts/dto-drift-check.sh) and is documented as a rule in [`../coding-standards/typescript.md`](../coding-standards/typescript.md) § 11. False positives (a DTO rename that doesn't affect the wire shape) are acceptable — the contributor adds a one-line update and ships.

## Comparison table

| | typeshare | ts-rs | specta | utoipa+oapi-ts | schemars+jst | Hand-rolled |
|---|---|---|---|---|---|---|
| **Maturity** | 1.13.x stable | 11.x stable | 2.0-rc.24 | utoipa 5.x; oapi-ts 7.x | both stable | n/a |
| **`nutype` native** | No | No | No | **Yes** | **Yes** | n/a |
| **Generation trigger** | CLI | `cargo test` | `cargo run --example` | build script or `cargo run` | build script | hand |
| **Output shape** | Single .ts | One file per type | Single or namespaced | One bundle | One bundle | One file |
| **Branded types** | No | No | Yes (experimental) | No | No | **Yes (hand)** |
| **Serde tag modes** | Full | Full | Full | Full (via serde→OpenAPI) | Full (via serde→JSON Schema) | n/a |
| **JSDoc preserved** | Yes | Yes | Yes | Yes (description fields) | Yes (description fields) | n/a |
| **Commits us to** | nothing | nothing | nothing | OpenAPI | JSON Schema | nothing |
| **Bundle cost (frontend)** | 0 (.ts only) | 0 | 0 | 0 (build-time) | 0 (build-time) | 0 |
| **Two-assistant friction** | Medium | Medium-high | Medium | Low (yaml is greppable) | Low | None |
| **Recommended?** | No | No | No | **If adopting OpenAPI** | **Yes when threshold hit** | **Yes today** |

## Addendum (2026-05-21) — Zod adoption refines target and threshold

This note was written before [`coding-standards/typescript.md`](../coding-standards/typescript.md) § 8 committed the frontend to Zod for runtime API-boundary validation (PR-B2, Issue [#191](https://github.com/brendanbyrne/beerio-kart/issues/191)). Three things shift as a result.

**The threshold codegen target is `schemars + json-schema-to-zod`, not `schemars + json-schema-to-typescript`.** The original target emits static `type`/`interface` declarations. With Zod in place, those declarations don't replace anything — they'd sit alongside hand-written Zod schemas, giving us three artifacts per DTO (Rust struct + generated TS + hand Zod) instead of two. The Zod-targeting tool ([`json-schema-to-zod`](https://www.npmjs.com/package/json-schema-to-zod)) emits the runtime schemas directly; types fall out as `z.infer<typeof Schema>`. That's one generated artifact per DTO and no hand-written types.

**A thin brand-mint overlay survives codegen regardless.** JSON Schema cannot natively express TS phantom-type brands (per [`typescript.md`](../coding-standards/typescript.md) § 3). The brand-mint logic — branded ID types, their constructors, and their matching Zod schemas — lives in [`frontend/src/api/brand.ts`](../../frontend/src/api/brand.ts). Generated Zod for an ID field would emit a plain `z.string()` / `z.number()`; the overlay replaces those with the corresponding `*IdSchema`. Two mechanics are viable at adoption: (a) annotate brandable fields in Rust via a `schemars` extension (e.g., `#[schemars(extend("x-brand" = "UserId"))]`) and post-process the generated output to swap primitives for brand schemas, or (b) keep generated DTO schemas thin and wrap them with hand-written `.extend({ id: SessionIdSchema, ... })` overlays. Both keep the overlay small — it's just the ID promotions, not the full DTO shapes — and it only changes when a new domain ID is introduced. The shapes themselves, which are what actually drift, become a generated artifact.

**The threshold metric should change.** The "~30 DTOs" heuristic above implicitly priced the hand-rolled cost as maintaining `frontend/src/api/types.ts`. The real cost now is maintaining Zod schemas, which is more code per DTO (transforms, brand mints, nested objects, literal-union enums) and more sensitive to backend change — every Rust DTO edit forces a schema edit. Reframe the trigger from "DTO count" to "Zod-maintenance friction": adopt codegen when a Rust DTO change is routinely forcing a parallel Zod schema edit, or when at least one drift bug has shipped that a generated schema would have caught. DTO count remains a useful proxy (the ratio of Zod-effort-saved to codegen-setup-effort grows with type count) but it's no longer the metric of record.

**Net of the addendum.** The hand-roll-for-now recommendation stands; what changes is the at-threshold adoption shape: `schemars + json-schema-to-zod + a thin brand-mint overlay`, not `schemars + json-schema-to-typescript`. In the comparison table above, the `schemars + jst` column is superseded — read it as `schemars + json-schema-to-zod`, with the "Recommended?" cell still "**Yes when threshold hit**" but applied to the runtime-schema target rather than the static-type target. The `nutype`-native row, the build-script generation trigger, the JSON-Schema commitment, and the two-assistant-friction grade are all unchanged between the two targets.

The consolidated decision (current state, options considered, adoption trigger) lives in [ADR 0039](../decisions/0039-api-client-generation.md). This research note is the tool evaluation it draws from.

## Open questions to revisit at the threshold

- Should `docs/api-contract.md` become a derived artifact (from OpenAPI or JSON Schema) instead of a hand-maintained doc?
- Where do error-code unions live? `ErrorCode` is exhaustively listed in `api-contract.md` § 7; either path generates it from Rust if we make the enum `#[derive(JsonSchema)]` or `ToSchema`.
- Branded types after codegen: do we wrap the generated `type UserId = string` in a phantom-symbol brand at the frontend boundary, or accept structural typing for IDs? (Recommendation: still brand at the frontend boundary; the codegen gives us shapes, the brand layer gives us the safety the structural type system can't.)

## Document history

- 2026-05-16 — Initial creation. Sourced from research conducted same day comparing typeshare (1Password), ts-rs (Aleph-Alpha), specta (specta-rs), utoipa, and schemars against our `nutype`-heavy backend. Reaches a "not yet, but here's the path when you're ready" recommendation. Companion to `coding-standards/typescript.md` § 11.
- 2026-05-21 — Added an addendum prompted by the frontend audit's commitment to Zod for runtime API-boundary validation (`typescript.md` § 8, PR-B2 / Issue [#191](https://github.com/brendanbyrne/beerio-kart/issues/191)). Reframed the at-threshold codegen target from `schemars + json-schema-to-typescript` (static types) to `schemars + json-schema-to-zod` (runtime schemas) so the generated artifact replaces the hand-written Zod schemas rather than sitting alongside them. Documented the brand-mint overlay caveat — JSON Schema can't express TS phantom-type brands, so `brand.ts` survives codegen as a thin overlay that swaps generated `z.string()` / `z.number()` for the matching `*IdSchema`. Reframed the threshold metric from "DTO count" to "Zod-maintenance friction" with DTO count as proxy. Original hand-roll-for-now recommendation preserved. `coding-standards/typescript.md` § 11 still references this note and is now slightly stale (mentions the old `json-schema-to-typescript` target).
- 2026-06-15 — § Cheap drift detection without a tool: marked implemented (PR-H1 / Issue [#185](https://github.com/brendanbyrne/beerio-kart/issues/185)). The sketched `git diff --name-only` check now exists as `.github/workflows/dto-drift.yml` + `.github/scripts/dto-drift-check.sh`, watching an explicit path list because the DTOs aren't in one `backend/src/dtos/` folder (they span `routes/`, `domain/enums.rs`, and several `services/` modules); documented as a rule in `coding-standards/typescript.md` § 11.
