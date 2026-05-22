---
status: accepted
date: 2026-05-21
deciders: [Brendan]
source: ad-hoc
---

# 0039 — API client generation: hand-rolled with Zod, codegen at threshold

## Context and problem statement

The initial `api-contract.md` (2026-05-02) declared as a firm decision that the backend would emit an OpenAPI 3.x spec via `utoipa` and the frontend would consume a generated TypeScript client (`openapi-fetch`), with hand-rolling explicitly rejected as "untenable past 15 endpoints." That decision was made before the frontend existed.

When the frontend was actually built, it was hand-rolled — per-endpoint `fetch` wrappers in `frontend/src/api/`. The `utoipa` decoration was never added to the backend. PR-B2 (Issue [#191](https://github.com/brendanbyrne/beerio-kart/issues/191)) then added a third layer: hand-written Zod schemas at the API boundary, per [`coding-standards/typescript.md`](../coding-standards/typescript.md) § 8.

By 2026-05-16, the research note [`docs/research/rust-to-ts-codegen.md`](../research/rust-to-ts-codegen.md) had evaluated five Rust→TS type-sync tools (typeshare, ts-rs, specta, schemars, utoipa) against the constraint that the backend uses `nutype` for every domain newtype. The note recommended staying hand-rolled, with the at-threshold adoption being `schemars + json-schema-to-zod` (per the 2026-05-21 addendum). This contradicted § 2 of `api-contract.md`, which still read as a firm utoipa decision.

This ADR distills the actual decision so `api-contract.md` can be cleaned up (§ 2 deleted, §§ 3–11 renumbered to §§ 2–10) and so the research note has an authoritative pointer.

## Decision drivers

- **`nutype` macro composition.** Every domain newtype on the backend (`UserId`, `Username`, `RaceTimeMs`, etc., per [`coding-standards/rust.md`](../coding-standards/rust.md) § 2) is a `nutype`-generated struct. Tools that parse Rust source pre-macro-expansion (typeshare, ts-rs, specta) cannot see the expanded shape and require per-newtype workarounds — significant friction in a codebase where IDs are branded by default.
- **Runtime validation is required regardless of codegen.** `await res.json()` returns `any`. Even a perfectly-generated TS type doesn't replace the need for a runtime parser at the wire boundary. Zod (per `typescript.md` § 8) parses and infers in lockstep, so the schema *is* the type definition.
- **Two-assistant workflow penalizes invisible state.** Codegen artifacts that need a `cargo test` or CLI run to refresh are exactly the kind of thing that drifts when Cowork edits Rust and the next Claude Code session assumes the TS side is current. Hand-rolled keeps both sides legible to both assistants.
- **OpenAPI is not (yet) needed for its own sake.** No standing decision to adopt OpenAPI for public docs, third-party SDK generation, or mock-server tooling. Adopting `utoipa` purely to derive types is a heavy commitment for the wrong reason.
- **Empirical evidence.** § 2's "untenable past 15 endpoints" claim has been falsified — `api-contract.md` § 1 lists ~40 endpoints and the hand-rolled frontend is working. The pain has not materialized at the predicted threshold.

## Considered options

- **Option A: Stay hand-rolled with Zod-as-source-of-truth (current state).** Per-endpoint `fetch` wrappers in `frontend/src/api/`; one Zod schema per response DTO with TS types derived via `z.infer<typeof Schema>`; branded IDs minted in the schema's `.transform()` (see [`frontend/src/api/brand.ts`](../../frontend/src/api/brand.ts)). Drift detection via the CI check named in `research/rust-to-ts-codegen.md` § "Cheap drift detection" (PR-H1 scope).
- **Option B: Adopt `schemars + json-schema-to-zod + brand-mint overlay` now.** Decorate Rust DTOs with `#[derive(JsonSchema)]`; emit JSON Schema; generate Zod schemas via [`json-schema-to-zod`](https://www.npmjs.com/package/json-schema-to-zod); the existing `brand.ts` overlay swaps generated `z.string()` / `z.number()` for branded `*IdSchema` on ID fields. Generated Zod replaces hand-written Zod; the brand layer is small (~25 lines today), stable, and only grows when a new domain ID is introduced.
- **Option C: Adopt `utoipa + openapi-fetch` as § 2 originally proposed.** Decorate Rust handlers with `#[utoipa::path]`; expose `/api/v1/openapi.json`; generate a typed `openapi-fetch` client. Static-type-only — still need a runtime validator on top.
- **Option D: Adopt `utoipa + openapi-zod-client` (or `orval`).** OpenAPI as the intermediate format, but generate Zod schemas (not just types) so the runtime-validation layer is also generated. Equivalent to Option B's outcome but via a different intermediate format.

## Decision outcome

Chosen: **Option A — stay hand-rolled with Zod-as-source-of-truth — with Option B (`schemars + json-schema-to-zod + brand-mint overlay`) as the adoption target when the trigger fires.** Options C and D are reserved for the case where a separate OpenAPI decision lands first (public docs, third-party SDKs, mock servers); if that happens, `utoipa + openapi-zod-client` is preferred over `utoipa + openapi-fetch` because the Zod commitment in § 8 makes runtime-schema-emitting tools strictly better than static-type-only ones.

**Adoption trigger:** reframe the threshold from "DTO count" to **Zod-maintenance friction**. Adopt codegen when (a) a Rust DTO change is routinely forcing a parallel Zod schema edit, or (b) at least one drift bug has shipped that a generated schema would have caught. The DTO-count heuristic (~30) remains a useful proxy — the ratio of Zod-effort-saved to codegen-setup-effort grows with type count — but it's a proxy, not the metric of record.

### Positive consequences

- The wire format is described once in `api-contract.md` (the wire-format doc) and once in the Zod schemas (the runtime/type source of truth). No third generated artifact to keep in sync.
- `nutype` blocker doesn't apply — hand-written Zod schemas don't depend on Rust source parsing.
- The brand-mint layer in `frontend/src/api/brand.ts` is already in place and survives any future codegen adoption.
- Optionality preserved: if the OpenAPI question comes up for a non-codegen reason, Options C and D are still available with no decision debt to undo.
- The single point of truth for "how does the frontend talk to the backend" is now this ADR, not a section of the wire-format doc that conflated tooling with contract.

### Negative consequences / trade-offs

- Schema drift between Rust DTOs and Zod schemas is a real risk. Mitigated by the runtime-loud failure mode (`response_shape_mismatch` per typescript.md § 8) and the planned CI drift-check (PR-H1 scope).
- Every new endpoint requires touching both the Rust DTO and the Zod schema. Cost is small per-endpoint but grows linearly with DTO count.
- "Untenable past 15" was empirically wrong, but a future "untenable past N" could be right. The reframed Zod-maintenance-friction trigger is intentionally a lived signal rather than a number to forecast — that's a deliberate choice and may feel imprecise compared to a numeric threshold.

## Links

- Source: ad-hoc (Cowork session 2026-05-21, triggered by Claude Code's handoff during PR-B2)
- Related ADRs: [0023](0023-hand-written-seaorm-entities.md) (hand-written SeaORM entities — analogous "skip the codegen, hand-write the artifact" call on the backend side), [0036](0036-error-code-rollout.md) (defines the `ErrorCode` registry the Zod `ApiError` union mirrors)
- Research: [`docs/research/rust-to-ts-codegen.md`](../research/rust-to-ts-codegen.md) (full tool evaluation + 2026-05-21 addendum on Zod target)
- Standards refs: [`coding-standards/typescript.md`](../coding-standards/typescript.md) § 8 (runtime validation), § 11 (backend interop)
- Implementing PRs: hand-rolled client across `frontend/src/api/` (Cup Mushroom + Cup Flower work), PR-B1 ([#171](https://github.com/brendanbyrne/beerio-kart/issues/171), branded IDs), PR-B2 ([#191](https://github.com/brendanbyrne/beerio-kart/issues/191), Zod runtime validation; PR [#199](https://github.com/brendanbyrne/beerio-kart/pull/199))
- Superseded text: `api-contract.md` § 2 "API client generation" (deleted in this PR; §§ 3–11 renumbered to §§ 2–10)
