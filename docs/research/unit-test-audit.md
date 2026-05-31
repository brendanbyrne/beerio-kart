# Unit Test Audit — "Do our tests test what they say they do?"

**Author:** Cowork (design/research/review)
**Date:** 2026-05-31 (promoted from `docs/drafts/` to `docs/research/` 2026-05-31).
**Scope:** Every Rust test in `backend/` (≈370 test fns across 30 files) and every TypeScript/Vitest test in `frontend/` (≈145 tests across 29 files).
**Tracking:** Findings actioned under [Issue #217 — Tighten test assertions: pin specific errors/codes in rejection tests](https://github.com/brendanbyrne/beerio-kart/issues/217), implemented in PR [#225](https://github.com/brendanbyrne/beerio-kart/pull/225). This is the durable record of the audit's taxonomy, findings, and cleared false-positives; [`../coding-standards/testing.md`](../coding-standards/testing.md) distils the recurring patterns into the enforced cross-cutting rules.

---

## TL;DR

This is a **high-quality, human-curated suite** that largely avoids the failure modes the research literature attributes to LLM-written tests. There is **no broken-on-its-face test that silently verifies nothing** except one deliberate no-op (`test_lazy_check_assertion`).

The findings collapse into **one pervasive habit** plus a few one-offs:

> **The dominant pattern (~50 of ~63 flagged tests):** a test asserts that an operation *fails* (`assert!(result.is_err())`) or *returns an HTTP status* (`assert_status(409)`) but never pins **which** error — even though this codebase has invested heavily in a machine-readable `ErrorCode` registry precisely so failures are distinguishable. These tests pass if the code fails *for the wrong reason*. They are not worthless (most still catch the headline regression), but they are **looser than their names promise**.

About 12% of tests are flagged; **1 is High severity, ~52 are Medium (almost all the one pattern), ~10 are Low.** None represents a security hole. The recommended response is one coordinated tightening pass plus turning on a few lint rules that make the pattern impossible to reintroduce.

The audit also *cleared* several things that look alarming at a glance — documented in [§5](#5-false-positive-traps-we-checked-and-cleared) so we don't re-litigate them.

---

## Part 1 — How LLMs write tests that don't test what they claim

This section is the research deliverable: a cited taxonomy used as the audit checklist. The anti-patterns sort into **three root mechanisms** by which a green test fails to verify its claim (framing adapted from the canonical test-smell literature). The single sharpest one-line test, from Google's Alex Eagle, is: a test has no value if **"a correct or incorrect program is equally likely to pass"** it.[^eagle]

### Why this is worth auditing for: the empirical picture

LLM-generated tests fail in two classes. **Loud failures** (don't compile, don't run, flaky) are annoying but self-evident. **Silent failures** (assertion-free, weak assertions, oracles that encode the buggy implementation, over-mocking) produce *green suites that don't verify intended behavior* — the dangerous class. The evidence:

| Finding | Figure | Source |
|---|---|---|
| ChatGPT unit tests that passed execution | only **24.8%**; 57.9% had compile errors, 17.3% runtime failures "mostly result from incorrect assertions" | Yuan et al., **FSE 2024** (peer-reviewed)[^fse] |
| LLM tests "often lack assertions, or only contain very generic assertions (e.g. that a variable is not null)" | median **61.4%** of generated tests were "non-trivial" (≥1 meaningful assertion) | Schäfer et al., **IEEE TSE 2024** (TestPilot)[^tse] |
| Codex-generated tests with **no assertions** / **no call to the focal method** | **31%** no-assertion, **37%** never invoke the code they "test" | arXiv 2412.14308[^codex] |
| LLM oracles capture the code's *actual* (buggy) behavior, not the *expected* behavior | classification accuracy "drops when the given code is buggy" | Konstantinou et al., arXiv 2410.21136[^oracle] |
| Coverage ≠ fault detection for LLM suites | cases of "100% coverage but only 4% mutation score" | arXiv 2506.02954[^mutation] |
| Coding **agents add mocks** to tests more than humans | **36%** of agent test-commits add mocks vs **26%** for humans | Hora et al., **MSR 2026**[^mock] |
| Copilot-generated tests with ≥1 test smell | **~47%** (Assertion Roulette & Magic Number most common) | arXiv 2410.10628 / companion[^smells] |

Caveats carried forward: the strongest correctness numbers are GPT-3.5-era (frontier models compile better now); benchmark scores overstate real-world quality due to data leakage; several supporting papers are arXiv preprints (flagged above where so). The *failure modes*, however, are durable and corroborated across peer-reviewed venues (FSE/TSE/AST/MSR) and the pre-LLM test-smell canon (Meszaros, Fowler, Google, van Deursen, Tufano).

### Mechanism 1 — No real check runs

| Anti-pattern | What it looks like | Grep signature | Source |
|---|---|---|---|
| **Empty Test** | `#[test]`/`it()` with no executable statements; framework reports pass | `#[test]`/`it(` followed by `{}` or only comments | testsmells.org[^ts] |
| **Assertion-Free / "Unknown" Test** | calls the SUT but asserts nothing; only fails on panic/throw. "You can have 100% coverage with zero assertions" | test body with zero `assert*`/`expect(`/`verify(` (account for custom assert helpers!) | Fowler[^fowler-af], testsmells.org |
| **Conditional Test Logic** | `assert` nested in `if`/`match`/loop that may never run → green having checked nothing | `expect(`/`assert` inside `if`/`catch`/`for` with no else/panic fallback | Meszaros[^mesz-ctl] |
| **Async assertion never runs** | missing `await`/`return` on a promise so the test ends before the assertion; rejection lands after green | `.then(`/`waitFor(`/`findBy`/`.resolves`/`.rejects` not `await`ed | Jest docs[^jest-async]; eslint `valid-expect-in-promise`, `await-async-utils` |
| **Spawned work not joined (Rust)** | `tokio::spawn` whose `JoinHandle` is dropped; a future built but not `.await`ed (futures are lazy) | `tokio::spawn(` in a test with no later `.await` on the handle | Tokio testing[^tokio]; matklad[^matklad] |

### Mechanism 2 — The check exists but cannot fail

| Anti-pattern | What it looks like | Grep signature | Source |
|---|---|---|---|
| **Over-broad / weak assertion** | `toBeTruthy`/`toBeDefined`/`expect.anything()`/`not.toThrow()` — passes for a huge class of wrong values | those matchers as the *only* assertion | AppSignal[^appsignal]; eslint `no-restricted-matchers` |
| **Variant-only Result assertion (Rust)** | `assert!(r.is_ok())` / `assert!(r.is_err())` discards the value *and* the error — a wrong-but-`Ok` value passes, and the real error is hidden | `assert!(.*\.is_ok())`, `assert!(.*\.is_err())` | clippy `assertions_on_result_states`[^clippy-res]; burntsushi[^bs] |
| **Status-only API assertion** | asserts HTTP 400/404/409 but not the error-body `code` → passes if the endpoint failed *for a different reason* | `assert_status(` with no `body["code"]`/`assert_json` nearby | (project-specific; see error-code registry) |
| **Redundant / vacuous assertion** | `assert!(true)`, `assert_eq!(x, x)` — independent of the SUT | `assert!(true)`, `assert_eq!((\w+), \1)` | clippy `assertions_on_constants`, `eq_op`[^clippy-eq]; testsmells.org |
| **Expected value recomputed with the SUT's own logic** | the test re-derives the expected value the same way the code does → shares any bug, can't fail | expected built from the same formula/helper as the SUT | Meszaros "Production Logic in Test" |
| **Happy-path-only** | every fixture is valid; no error/edge/boundary case | *absence* of `is_err`/`rejects`/`throws`/boundary inputs in a module's tests | Open test-smell catalog |

### Mechanism 3 — The check just restates the code

| Anti-pattern | What it looks like | Grep signature | Source |
|---|---|---|---|
| **Change-detector / tautological test** | a "mirror of the implementation": mock expectations line up 1:1 with the production steps, so it's true by construction. "A correct or incorrect program is equally likely to pass" | mostly `verify(...)`/`toHaveBeenCalled` with few/no output asserts | Google (Eagle)[^eagle]; Pereira[^ttdd] |
| **Testing the mock** | stub a dep with `mockReturnValue(X)`, then assert the result equals `X` (the unit just forwards it); or `toHaveBeenCalled` as the *only* assertion | `mockReturnValue(x)` … `expect(...).toBe(x)`; sole-assertion `toHaveBeenCalled` | Khorikov[^khorikov]; Google "Don't Overuse Mocks"[^google-mocks] |
| **Over-mocking internal collaborators** | mocking in-process collaborators (not external boundaries) couples the test to internals; "your code works *if* your mocks behave like the real ones" | `vi.mock('../internalHelper')` for a pure in-process module | Google[^google-mocks]; Fowler "Mocks Aren't Stubs"[^fowler-mock] |
| **Mocking / spying the SUT itself** | partial-mock or `spyOn` a method *on the object under test* → "contaminated test subject" | `vi.spyOn(SUT, 'methodUnderTest')` | testdouble wiki; Khorikov |
| **Self-round-trip serialization** | `assert_eq!(deserialize(serialize(x)), x)` is invariant under representation change — a symmetric bug survives, and the *wire format* is never pinned | `from_str(&...to_string`, `to_value()`↔`try_from_value` with no literal | serde docs[^serde]; proptest write-ups |
| **Asserting on Debug/Display or implementation detail** | pins `format!("{:?}", x)` or internal structure, not behavior; breaks on refactor, misses logic bugs | `assert_eq!(format!("{:?}"`, `.to_string(), "` | Google "Test Behavior Not Implementation"[^google-behavior] |
| **Name claims more than the body verifies** | `it('rotates the token')` that only checks a token *exists* | (read each test's name against its asserts) | — |

---

## Part 2 — Audit method

Per the agreed approach, this was a **risk-prioritized deep dive**: the service layer, integration tests, and domain validation were scrutinized hardest; trivial tests (types, brand, enums, getters) got a lighter pass.

1. **Mechanical sweep** of both trees for every grep signature above.
2. **Trace-the-helper** pass — custom assertion helpers (`axum-test`'s `res.assert_status(...)`, the local `assert_bad_request`) were read to confirm they actually assert before judging the tests that use them. This is the step a naive audit skips, and it overturned ~33 false "assertion-free" hits.
3. **Full per-file read** of the high-risk areas (4 parallel reviewers), each test's *name/comment* compared against what its body *actually verifies*, classified by the taxonomy and severity.
4. **Independent verification** of every High-severity claim against source (this is what downgraded the auth token-type/expiry tests from High to Medium — see [§5](#5-false-positive-traps-we-checked-and-cleared)).

**Severity scale**

- **High** — a broken implementation of the *claimed behavior* would still pass (verifies ~nothing).
- **Medium** — verifies the behavior but more loosely than the name implies (passes if the code fails/behaves differently in a way the name implies it guards), or tests choreography instead of outcome.
- **Low** — stylistic, redundant, or the claim is substantially covered elsewhere.

---

## Part 3 — Findings

### High (1)

| # | Location | Test | Problem | Fix |
|---|---|---|---|---|
| H1 | `backend/src/services/sessions/races.rs:1021` | `test_lazy_check_assertion` | **Empty test.** Body is `// No-op test by design — this comment IS the assertion.` Name + docstring imply it verifies the lazy-pending invariant; it asserts nothing and is green unconditionally. | Delete it (the invariant *is* covered by `test_pending_excludes_closed_session` and siblings), or convert the doc-comment into a real assertion. Move the explanatory prose to a module doc-comment, not a `#[test]`. |

> Risk note: H1 is the textbook "test that doesn't test what it says," but its *practical* risk is low because sibling tests cover the invariant. Its real cost is that it inflates the test count and misleads a reader into thinking the invariant has a dedicated guard. Flagship example for this audit, not a fire.

### Medium — Pattern A: rejection asserted without pinning the error (backend)

The codebase's `error::Error` carries a machine-readable `code` per failure (`session_closed`, `pending_races_first`, `username_taken`, `forbidden`, `token_expired`, `invalid_request_body` vs generic `bad_request`, etc.). These tests are named for a **specific** failure but assert only the coarse outcome, so they'd stay green if the function failed at a *different* gate / for a *different* reason.

**A1 — Service-layer `assert!(result.is_err())` on multi-gate functions.** Each of these functions (`create_run`, `delete_run`, `join_session`, `create_session`, `next_track`, `skip_turn`) has several independent failure modes with different codes.

| Location | Test | Claims | Verifies |
|---|---|---|---|
| `runs/submission.rs:491` | `test_create_run_fails_if_not_participant` | Forbidden (not a participant) | only `is_err()` |
| `runs/submission.rs:502` | `test_create_run_fails_if_duplicate_submission` | Conflict (already submitted) | only `is_err()` |
| `runs/submission.rs:516` | `test_create_run_fails_if_session_closed` | Conflict `session_closed` | only `is_err()` |
| `runs/submission.rs:532–568` | `..._invalid_track_time`, `..._lap_sum_mismatch`, `..._lap_time_exceeding_max` | BadRequest (specific validation) | only `is_err()` |
| `runs/submission.rs:580/592` | `..._invalid_character_id`, `..._invalid_drink_type_id` | BadRequest from the FK check (comment stresses "FK, not type-parse") | only `is_err()` |
| `runs/submission.rs:622/640` | `test_delete_run_fails_for_non_owner`, `..._if_session_closed` | Forbidden / Conflict | only `is_err()` |
| `sessions/lifecycle.rs:904/917/1044/1061` | join/create guards (closed, double-join, one-active-session) | specific Conflict | only `is_err()` |
| `sessions/lifecycle.rs:980` | `test_create_with_invalid_ruleset_returns_error` | BadRequest (parse) | only `is_err()` |
| `sessions/races.rs:521/534/607` | `next_track` host gate, closed-session, `skip_turn` no-race | Forbidden / Conflict / BadRequest | only `is_err()` |

*Fix pattern:* `assert!(matches!(result, Err(Error::Conflict { .. })))` (or the right variant), and assert the `client` message substring where the name implies a specific code. **Not flagged** (correctly): the eight `validate_time_fields_*` tests and the raw-FK-violation insert — those functions have a single failure mode, so `is_err()` is adequate.

**A2 — Integration tests asserting HTTP status only, not `body["code"]`.** Distinct codes share a status (e.g. `invalid_request_body` vs `bad_request` are both 400; `session_closed`/`pending_races_first`/generic are all 409; `not_found` vs `invalid_path_param` are both 404), so status-only can be "right for the wrong reason."

| File | Status-only error tests (examples) |
|---|---|
| `tests/api_integration.rs` | `..._invalid_fk_returns_400` (483), `..._all_or_nothing` (442), `..._empty_name_returns_400` (669), `..._nonexistent_user_returns_404` (426), `..._nonexistent_drink_type_returns_404` (706), `..._other_users_profile_returns_403` (501) |
| `tests/auth_integration.rs` | the 4 register-validation 400s (409–446); the middleware-rejection 401s (313, 335, 357) |
| `tests/session_integration.rs` | join-closed-409 (361), invalid-ruleset-400 (389), join-twice-409 (402), nonexistent-session-404 (475), skip-unknown-race-404 (627), skip-already-submitted-409 (638), skip-after-leaving-403 (816) |
| `tests/auth_middleware.rs` | refresh-as-access-401 (109), empty-token-401 (100), no-bearer-401 (90), non-admin-403 (138), no-admin-configured-403 (165) |

*Fix pattern:* add `let body: Value = res.json(); assert_eq!(body["code"], "session_closed");` (etc.). **Priority within A2:** the auth token-type/expiry tests (`auth_middleware.rs:109`, `auth_integration.rs:313/335/357`) are security-adjacent — tighten these first. (They are *not* High: the tokens carry a valid signature + `token_type:"access"`, so removing the check under test flips the response to 200, which the status assertion *does* catch. The residual gap is only that they don't prove the 401 is `token_expired`/`token_invalid` specifically.)

### Medium — Pattern B: testing choreography, not outcome (frontend)

| Location | Test | Claims | Verifies | Fix |
|---|---|---|---|---|
| `frontend/src/pages/Home.test.tsx:44` | invalidates membership + session-list queries after creating a session | the UI refreshes | only that `queryClient.invalidateQueries` was *called* with the keys (spy is the sole assertion) | assert a user-visible effect (nav tab enables / new session appears) |
| `frontend/src/pages/Session.test.tsx:124` | invalidates keys on join | join is reflected | spy-only | assert button flips to "Leave" / participant appears |
| `frontend/src/pages/Session.test.tsx:149` | invalidates detail on next-track & skip | detail refreshes | next-track/skip halves are spy-only (the *leave* half correctly asserts navigation) | assert the refetched detail/UI changed |
| `frontend/src/pages/Session.test.tsx:190` | invalidates detail after a run is submitted | detail refreshes | spy-only (RunEntrySheet is stubbed, so no real submit) | assert the detail visibly updates post-submit |

These are textbook change-detector tests: they pin the *interaction* (`invalidateQueries` was called) rather than the *outcome* (the data actually refreshed). They'd pass even if invalidation no longer caused a refresh.

### Medium — other (backend)

| Location | Test | Problem | Fix |
|---|---|---|---|
| `backend/src/domain/enums.rs:161` | `test_session_status_string_values_round_trip_through_sea_orm` | round-trip is invariant under representation change — never pins that the **DB-stored string** is literally `"active"`/`"closed"`. A symmetric `string_value` typo survives. (`status` is on the live write path.) | add `assert_eq!(SessionStatus::Active.to_value(), "active".into())` |
| `backend/src/domain/enums.rs:215` | `..._session_ruleset_string_values_round_trip...` | same; `ruleset` is written by `create_session` | pin the literal `to_value()` for `round_robin`/`least_played` |
| `backend/tests/auth_integration.rs:146` | `..._returns_new_access_token_and_rotated_cookie` | "rotated" is unverified — checks the new cookie *exists*, never that its value **changed** (a server re-issuing the identical cookie would pass) | assert `new_cookie != original_cookie` |

### Low (~10)

| Location | Test | Note |
|---|---|---|
| `backend/src/domain/ids.rs:237` | `test_distinct_id_types_do_not_unify` | Name claims a compile-time property; body is a runtime tautology (`assert_eq!` of cross-constructed inner UUIDs). Move to a `trybuild`/`compile_fail` test or rename to "construction smoke test." |
| `backend/src/domain/numeric.rs:286` | `test_assert_lap_sum_round_trip_on_valid_construction` (proptest) | Builds `total = l1+l2+l3` then asserts the SUT agrees — expected value recomputed with the SUT's own arithmetic, so it can't fail by construction. Add a perturbed-total `err` case or drop as redundant. |
| `backend/src/domain/enums.rs:240/259` | drink-category / run-flag-reason round-trips | serde spelling *is* pinned; sea_orm `to_value()` literal is not (partially mitigated). |
| `backend/src/services/notifications.rs:253` | `..._round_trips_through_json` | Pure round-trip; the sibling `..._json_carries_kind_tag` already pins the wire shape. Consider merging. |
| `backend/src/shutdown.rs:149` | `test_supervised_catches_panic_and_returns` | No explicit assertion (reaching the end is the implicit check — acceptable); the error-log branch isn't verified. Add `tracing_test` + `logs_contain("task panicked")`. |
| `backend/tests/auth_middleware.rs:138/165` | non-admin-403 / no-admin-configured-403 | Byte-identical assertions → neither proves its *distinct* cause. Pin `body["code"]`. |
| `backend/tests/auth_integration.rs:313 & 335` | refresh-as-access pair | Functionally identical (same token, request, single status assert). One is redundant. |
| `frontend/src/components/SubmitButton.test.tsx:19` | "falls back to children when no pendingLabel" | Misleading title: renders outside a form where `pending` is always false, so the pending fallback it names is never exercised (duplicates the line-12 test). Render in a form with a held request. |
| `frontend/src/App.test.tsx:71` | "attaches a route-scoped errorElement to every top-level route" | `toBeDefined()` passes for any truthy value; doesn't confirm it's a `RouteErrorFallback`, ignores nested routes. Assert element identity. |

---

## Part 4 — Coverage summary (per area)

Counts are flagged-tests / total-audited; "—" means clean. (Totals are as-of-audit; rstest/`it.each` cases counted per-declaration.)

**Backend — services & domain**

| File | Flagged / Total | File | Flagged / Total |
|---|---|---|---|
| `services/runs/submission.rs` | 10 / 30 | `domain/strings.rs` | — / 41 |
| `services/sessions/lifecycle.rs` | 5 / 31 | `domain/numeric.rs` | 1 / 16 (Low) |
| `services/sessions/races.rs` | 4 / 26 (**1 High**) | `domain/ids.rs` | 1 / 12 (Low) |
| `services/runs/read.rs` | — / 4 | `domain/enums.rs` | 4 / 8 (2 Med, 2 Low) |
| `services/sessions/detail.rs` | — / 4 | `error.rs` | — / 22 ✅ exemplary |
| `services/session_context.rs` | — / 7 | `extract.rs` | — / 7 ✅ |
| `services/auth.rs` | — / 12 | `db.rs` | — / 3 |
| `services/users.rs` | — / 9 | `timeout.rs` | — / 5 ✅ |
| `services/notifications.rs` | 1 / 8 (Low) | `middleware/limits.rs` | — / 2 ✅ |
| `services/helpers.rs` | — / 15 | `shutdown.rs` | 1 / 5 (Low) |
| `drink_type_id.rs` | — / 4 | `entities/users_behavior.rs` | — / 2 |

**Backend — integration (`tests/`)**

| File | Flagged / Total |
|---|---|
| `tests/api_integration.rs` | 7 / 26 (Med/Low — Pattern A2) |
| `tests/auth_integration.rs` | 10 / 21 (Med/Low — A2 + rotation) |
| `tests/session_integration.rs` | 7 / 17 (Med — A2) |
| `tests/auth_middleware.rs` | 6 / 8 (Med/Low — A2) |
| `tests/db_timeout.rs` | — / 1 ✅ exemplary (asserts variant + budget + code + body) |
| `tests/middleware_limits.rs` | — / 1 (bare 413 has no JSON envelope — status-only is correct) |
| `tests/schema_drift.rs` | — / 1 (`.unwrap()`-panics-on-failure is a valid mechanism) |

**Frontend** — 29 files, ~145 tests, **6 flagged** (4 Med = the `invalidateQueries`-spy cluster; 2 Low). All other files clean, including `App.routing.test.tsx` (mocks pages appropriately for a routing test and asserts the right marker renders). No snapshots, no `.skip`/`.only`, no bare automocks, clean `await` hygiene throughout.

---

## Part 5 — False-positive traps we checked and cleared

So these don't get re-flagged later:

1. **33 "assertion-free" backend tests** — false alarm. They assert via `axum-test`'s `res.assert_status(StatusCode::X)` (panics on mismatch) and the local `assert_bad_request` helper (matches `Err(BadRequest)` *and* checks the message). A regex counting only `assert!`/`assert_eq!` misses these.
2. **The auth concurrency test** (`auth.rs:348`) — the `tokio::spawn` handles **are** awaited (`for h in handles { elapsed.push(h.await.unwrap()); }`) and the observer is aborted. No leaked-task footgun.
3. **`ids.rs` / `strings.rs` serde round-trip proptests** — *not* flagged, because each is complemented by an explicit wire-form test (`..._serializes_as_bare_uuid_string` / `..._serializes_as_bare_string`) that pins the actual bytes. (The `enums.rs` sea_orm round-trips *were* flagged because no such literal pin exists for the DB string.)
4. **The auth token-type/expiry tests** — downgraded High → Medium after reading the source: the test tokens carry valid signatures, so the regression they guard (check removed) flips the status to 200, which `assert_status(401)` catches. The gap is only code-precision.
5. **MSW-at-the-fetch-boundary frontend tests** — *not* over-mocking. Mocking HTTP and exercising the real client/hook/page code through it is the correct fidelity boundary.

---

## Part 6 — Recommendations

**Tighten (one coordinated pass — see the handoff):**

1. Fix **H1** (delete/convert the empty test).
2. Address **Pattern A** — assert the `Error` variant (service tests) and `body["code"]` (integration tests), starting with the security-adjacent auth tests. This is mechanical and high-value: it's the difference between "fails somehow" and "fails for the reason we claim."
3. Address **Pattern B** — make the four `invalidateQueries`-spy tests assert a user-visible refresh.
4. Sweep the **Lows** opportunistically.

**Prevent reintroduction (lint — most of this is free):**

- **Rust:** enable `#![warn(clippy::assertions_on_result_states)]` in the backend crate — it flags exactly `assert!(r.is_ok())`/`assert!(r.is_err())` and tells you to `unwrap()`/match instead. (`assertions_on_constants`, `eq_op` are already default-warn.) Note `clippy::assertions_on_result_states` is in the `restriction` group, so it must be opted in.[^clippy-res]
- **Frontend:** adopt **`eslint-plugin-vitest` `recommended`** — turns on `valid-expect`, `valid-expect-in-promise`, `no-conditional-expect`, `no-standalone-expect`, `expect-expect`, `no-focused-tests`, `no-disabled-tests` in one move, plus `eslint-plugin-testing-library`'s React config for `await-async-utils`/`await-async-queries`. Consider opt-in `no-restricted-matchers` to discourage `toBeDefined`/`toBeTruthy` as sole assertions.[^eslint]
- **Convention:** a one-line testing standard — *"a test named for a specific failure must assert the specific `code`/variant, not just `is_err()`/the HTTP status."* Worth a short `docs/coding-standards/testing.md` note since it's the single recurring gap.

**Keep doing (what's already right):**

- `error.rs`, `extract.rs`, `timeout.rs`, `db_timeout.rs`, `middleware_limits.rs` are the model: they assert variant **+** code **+** body content. Use them as the template for the Pattern-A fixes.
- The frontend's MSW-boundary discipline, real-content assertions, and clean async hygiene are exactly the practices the research says LLM suites most often lack. This suite reads as human-curated, not machine-dumped.

---

## Sources

[^eagle]: A. Eagle, "Change-Detector Tests Considered Harmful," Google Testing Blog, 2015. https://testing.googleblog.com/2015/01/testing-on-toilet-change-detector-tests.html
[^fse]: Z. Yuan et al., "No More Manual Tests? Evaluating and Improving ChatGPT for Unit Test Generation," FSE 2024. https://arxiv.org/abs/2305.04207
[^tse]: M. Schäfer, S. Nadi, A. Eghbali, F. Tip, "An Empirical Evaluation of Using LLMs for Automated Unit Test Generation," IEEE TSE 2024 (TestPilot). https://arxiv.org/pdf/2302.06527
[^codex]: "RL from Automatic Feedback for High-Quality Unit Test Generation," arXiv 2412.14308 (31% no-assertion / 37% no-focal-call for the Codex baseline). https://arxiv.org/abs/2412.14308
[^oracle]: S. Konstantinou, R. Degiovanni, M. Papadakis, "Do LLMs generate test oracles that capture the actual or the expected program behaviour?", arXiv 2410.21136. https://arxiv.org/pdf/2410.21136
[^mutation]: "Towards More Effective Fault Detection in LLM-Based Unit Test Generation," arXiv 2506.02954. https://arxiv.org/html/2506.02954
[^mock]: Hora et al., "Are Coding Agents Generating Over-Mocked Tests? An Empirical Study," MSR 2026. https://arxiv.org/abs/2602.00409
[^smells]: "On the Diffusion of Test Smells in LLM-Generated Unit Tests," arXiv 2410.10628. https://arxiv.org/abs/2410.10628 — and the Copilot/Python ~47% test-smell study (companion). 
[^ts]: testsmells.org — operational catalog with mechanical detectors (Empty Test, Unknown Test, Redundant Assertion, Conditional Test Logic, Assertion Roulette, Eager Test, Mystery Guest). https://testsmells.org/pages/testsmells.html
[^fowler-af]: M. Fowler, "Assertion Free Testing." https://martinfowler.com/bliki/AssertionFreeTesting.html
[^mesz-ctl]: G. Meszaros, *xUnit Test Patterns*, "Conditional Test Logic." http://xunitpatterns.com/Conditional%20Test%20Logic.html
[^jest-async]: Jest, "Testing Asynchronous Code" (the "be sure to return/await the promise" caution). https://jestjs.io/docs/asynchronous
[^tokio]: Tokio, "Unit Testing" + issue tokio#3963 (`#[tokio::test]` drops the future outside the runtime). https://tokio.rs/tokio/topics/testing
[^matklad]: matklad, "How to Test" (behavior-not-code; fire-and-forget is "fundamentally untestable"; don't hide tests behind cfg). https://matklad.github.io/2021/05/31/how-to-test.html
[^appsignal]: "Avoiding False Positives in Node.js Tests" (the `toBeTruthy` forgot-to-call-the-function example). https://blog.appsignal.com/2024/11/20/avoiding-false-positives-in-nodejs-tests.html
[^clippy-res]: clippy `assertions_on_result_states` — "Checks for `assert!(r.is_ok())`/`assert!(r.is_err())` … Use `r.unwrap()`/`r.unwrap_err()`." (restriction group, opt-in). https://rust-lang.github.io/rust-clippy/master/index.html#assertions_on_result_states
[^bs]: A. Gallant (burntsushi), "Using unwrap() in Rust is Okay" (in tests, a panic is the failure signal). https://burntsushi.net/unwrap/
[^clippy-eq]: clippy `assertions_on_constants` and `eq_op` (default-warn). https://rust-lang.github.io/rust-clippy/master/index.html#eq_op
[^ttdd]: F. Pereira, "Tautological Test Driven Development (Anti Pattern)." http://fabiopereira.me/blog/2010/05/27/ttdd-tautological-test-driven-development-anti-pattern/
[^khorikov]: V. Khorikov, "When to Mock" + *Unit Testing: Principles, Practices, and Patterns*, ch. 11 ("never assert interactions with stubs"). https://enterprisecraftsmanship.com/posts/when-to-mock/
[^google-mocks]: Google Testing Blog, "Don't Overuse Mocks" ("the only assurance you get is that your code will work if your mocks behave exactly like your real implementations"). https://testing.googleblog.com/2013/05/testing-on-toilet-dont-overuse-mocks.html
[^fowler-mock]: M. Fowler, "Mocks Aren't Stubs" (state vs behavior verification; mockist tests "run green but mask inherent errors"). https://martinfowler.com/articles/mocksArentStubs.html
[^serde]: serde, "Unit testing" (`serde_test::assert_tokens` pins the wire form rather than self-round-tripping). https://serde.rs/unit-testing.html
[^google-behavior]: A. Trenk, "Test Behavior, Not Implementation," Google Testing Blog. https://testing.googleblog.com/2013/08/testing-on-toilet-test-behavior-not.html
[^eslint]: eslint-plugin-jest / eslint-plugin-vitest rule docs (valid-expect, valid-expect-in-promise, no-conditional-expect, no-standalone-expect, expect-expect, no-focused-tests, no-disabled-tests) and eslint-plugin-testing-library (await-async-utils, await-async-queries). https://github.com/jest-community/eslint-plugin-jest/tree/main/docs/rules

---

## Document history

- 2026-05-31 — Promoted from `docs/drafts/unit_test_audit.md` (gitignored) to give the audit a durable in-repo home that `coding-standards/testing.md` and Issue #217 can link — `drafts/` is `--exclude-path`'d from the lychee link-check, so the references only resolve once it lives here. Findings are unchanged and were actioned in PR #225; only the header was reframed (draft/handoff framing → tracking + promotion note) and one dead citation repaired (`arxiv.org/pdf/2412.14308` → `/abs/`; the `/pdf/` form 404s).
