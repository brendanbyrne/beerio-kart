---
status: accepted
date: 2026-05-15
deciders: [Brendan]
source: ad-hoc
---

# 0036 — Error code rollout shape

## Context and problem statement

`api-contract.md` § 3 has long specified that every error response carries
both an `error` (human-readable) and a `code` (stable machine-readable)
field. § 8 maintains a forward-looking registry of those codes. The
implementation, however, only ever emitted `{ "error": "..." }` — the
`code` field was deferred from launch and never wired.

By 2026-05, two pressures forced a decision:

1. **Frontend evolution.** The frontend started needing to distinguish
   different 409s ("session is closed, start a new one" vs. "username
   taken, pick another") to render the right copy and recovery action.
   Free-text matching was the only available path and was brittle.
2. **Issue #146** (typed-Path-extractor 400s) needed a custom rejection
   handler that produced the documented envelope. Wiring just the
   extractor's `code` would have meant one path emitted `code` while every
   other 4xx/5xx response did not — partial rollout that creates
   inconsistency without enabling the actual switchboard the field exists
   for.

So the question is: when wiring the `code` field across the codebase, how
should `error::Error` carry it, and how should call sites declare it?

## Decision drivers

- Type system enforces consistency between HTTP status, registry code, and
  user-facing message.
- Call sites read naturally — domain meaning visible at the construction
  point, not buried in a separate argument.
- Long-tail bespoke errors don't force registry decisions for every
  one-off validation message.
- Codes-as-stable-contract: once the frontend pattern-matches on a code,
  rename is breaking. The shape needs to surface that gravity.
- Future-proofing for added codes without breaking external matchers
  (`#[non_exhaustive]`).

## Considered options

- **Option A. Static `code: &'static str` on every variant.** Add a
  `code: &'static str` field to each `Error` variant. Simple. Frees the
  registry from any type relationship — codes are strings on the wire and
  strings in the code. Cost: no type-level check that a code corresponds
  to a registry row; typos and renames slip through review. Loses the
  "rename is breaking" signal entirely.
- **Option B. `ErrorCode` enum, every variant carries it.** Define an
  `ErrorCode` enum (variant per registry row), `#[serde(rename_all =
  "snake_case")]` so it serializes as the registry strings. Add a `code:
  ErrorCode` field to every `Error` variant. Single API — `Error::
  bad_request(msg, ErrorCode::BadRequest)`. Cost: every construction
  site has to pass a code argument, even for the long tail where the
  generic code is the right answer. Pairing of (variant, code) is not
  type-enforced — `Error::bad_request(msg, ErrorCode::UsernameTaken)`
  type-checks but is wrong (Conflict variant should hold UsernameTaken).
- **Option C. `ErrorCode` enum, hybrid helper API.** Same `ErrorCode`
  enum + variant field as Option B, but the helper API splits:
  - **Generic helpers** for variants that span multiple codes default
    to the generic code: `Error::bad_request(msg)` →
    `ErrorCode::BadRequest`, `Error::conflict(msg)` →
    `ErrorCode::Conflict`, `Error::forbidden(msg)` →
    `ErrorCode::Forbidden`.
  - **Per-code helpers** for named domain codes:
    `Error::lap_times_mismatch(msg)`, `Error::username_taken(msg)`,
    `Error::session_closed(msg)`, `Error::invalid_credentials()`,
    `Error::admin_required()`, etc. Each emits the (variant, code) pair
    that matches the registry — typo-resistant by construction.
- **Option D. Variants for every code.** Make every registry row its own
  `Error` variant (`Error::UsernameTaken(msg)`, `Error::SessionClosed
  (msg)`, etc.). No `code` field anywhere — the variant is the code.
  Cost: 19+ variants today, more later. Pattern-matching on `Error` gets
  unwieldy. Every `match e` needs a wildcard for `non_exhaustive` anyway,
  so the variant-per-code doesn't buy meaningful type safety over Option
  C's hybrid.

## Decision outcome

Chosen: **Option C — `ErrorCode` enum + hybrid helper API.**

Rationale: option D is the cleanest in spirit but explodes the variant
list and forces a registry decision for every bespoke 400 / 409. Option
B forces a code argument at every construction site even when the
generic code is the right answer, creating call-site bloat and a
pairing-mismatch foot-gun. Option A loses type safety on the codes,
which is exactly the thing the field exists to provide.

Option C threads the needle:

- `ErrorCode` enum lives once, mirrors the registry. `#[non_exhaustive]`
  so future codes don't break external matchers. `#[serde(rename_all =
  "snake_case")]` so the on-the-wire shape matches the registry's string
  values without a separate mapping table.
- Variants that span multiple codes (`BadRequest`, `Conflict`,
  `Unauthorized`, `Forbidden`) carry the code in a `code: ErrorCode`
  field. `Error::code()` returns the code for any variant
  (variant-pinned or carried).
- Per-code helpers (`Error::username_taken`, etc.) are the readable
  path for domain meanings — readers see the code at construction.
- Generic helpers (`Error::bad_request`, `Error::conflict`,
  `Error::forbidden`) keep their signatures and default to the generic
  codes for bespoke long-tail errors. Adding a new bespoke error doesn't
  force a registry decision.
- 401 has no generic — `Unauthorized` always picks
  `invalid_credentials()`, `token_expired()`, or `token_invalid(msg)`.

### Positive consequences

- Frontend gets a stable wire contract for every 4xx/5xx response with
  one rollout.
- Per-code helpers are typo-resistant: `lap_times_mismatch()` can't
  produce the wrong code; the function name names the code.
- Generic helpers preserve the long-tail ergonomics — adding a one-off
  400 doesn't force a registry change.
- The `code` field becomes the natural place to discriminate variants
  that share an HTTP status (the four-403 case in middleware/auth
  surfaces this cleanly: `forbidden` vs. `admin_required`).
- The token-expired-vs-invalid case in `middleware/auth.rs` becomes
  discriminable: the middleware now checks `jsonwebtoken
  ::errors::ErrorKind::ExpiredSignature` and emits `token_expired()`
  for that case specifically, where it previously emitted the same
  `Unauthorized("Invalid or expired token")` for both.

### Negative consequences / trade-offs

- 422 `unprocessable` stays registered-but-unimplemented. JsonRejection
  data-validation failures fold into 400 `invalid_request_body` for
  now; splitting 400/422 is a follow-up if it earns its keep. Decision
  taken to avoid scope creep — no service-layer path currently
  emits 422.
- `out_of_order_submission` is registered but unused today. The
  pending-races validation in `services/runs/submission.rs::create_run`
  doesn't distinguish "submitting current race blocked by older
  pending" from "submitting pending race out of order" — the check is
  the same. Emits `pending_races_first` for both. If the distinction
  earns its keep, the check can be split later.
- Pairing of (variant, code) is not type-enforced for the four
  multi-code variants — you *could* construct
  `Error::Conflict { code: ErrorCode::BadRequest, ... }` via the
  struct-literal path. The per-code helpers prevent this for the
  common case; the struct-literal form is used only inside
  `From<DbErr>` where the pairing is obvious. A future ergonomic
  refinement could split `ErrorCode` into per-status-class subtypes
  (e.g., `BadRequestCode`, `ConflictCode`) — out of scope today.
- Codes are forever once consumed by a client. The registry sweep
  during this PR is the moment to pick names carefully — every name
  here lives in the public contract.

## Links

- Source: ad-hoc (decision made in chat 2026-05-15 during scope
  analysis of #146; bundled into the broader #157 rollout).
- Related ADRs: 0035 (race-anchored session lifetime, the most recent
  ADR).
- Implementing PR: [#158](https://github.com/brendanbyrne/beerio-kart/pull/158).
- Closed Issues: [#146](https://github.com/brendanbyrne/beerio-kart/issues/146)
  (typed-Path-extractor 400s, subsumed by #157), [#157](https://github.com/brendanbyrne/beerio-kart/issues/157).
- Standards refs: [`api-contract.md` § 3](../api-contract.md#3-error-response-contract),
  [§ 8](../api-contract.md#8-error-code-registry),
  [`design.md` Error response pattern](../design.md#error-response-pattern).
