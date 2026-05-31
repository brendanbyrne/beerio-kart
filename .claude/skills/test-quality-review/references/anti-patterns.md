# Anti-pattern reference — tests that don't test what they claim

Detailed taxonomy, grep signatures, false-positive traps, and regression-prevention lint rules for the `test-quality-review` skill. Read the section you need; you don't have to read it all.

## Contents

- [Three root mechanisms](#three-root-mechanisms)
- [Mechanism 1 — no real check runs](#mechanism-1--no-real-check-runs)
- [Mechanism 2 — the check can't fail](#mechanism-2--the-check-cant-fail)
- [Mechanism 3 — the check just restates the code](#mechanism-3--the-check-just-restates-the-code)
- [Project rule — pin the ErrorCode](#project-rule--pin-the-errorcode)
- [False-positive traps](#false-positive-traps)
- [Lint rules to prevent regressions](#lint-rules-to-prevent-regressions)
- [Sources](#sources)

## Three root mechanisms

Every "green but doesn't verify its claim" test fails by one of three mechanisms. The sharpest one-line test, from Google's Alex Eagle: a test has no value if *"a correct or incorrect program is equally likely to pass"* it.

Empirical motivation (why this is worth checking, especially for AI-written tests): a Codex baseline produced **31% of tests with no assertions and 37% with no call to the focal method**; LLM oracles tend to encode the code's *actual* (buggy) behavior rather than the *expected* one; coding agents add mocks to tests markedly more than humans (36% vs 26% of test commits). See [Sources](#sources).

## Mechanism 1 — no real check runs

| Anti-pattern | What it looks like | Grep signature |
|---|---|---|
| **Empty test** | `#[test]`/`it()` with no executable statements; passes anyway | test fn/block whose body is `{}` or only comments |
| **Assertion-free test** | calls the SUT but asserts nothing; only fails on panic/throw. "100% coverage with zero assertions" | test body with zero `assert*` / `expect(` / custom-assert calls (trace helpers!) |
| **Conditional test logic** | `assert` nested in `if`/`match`/loop that may never run → green having checked nothing | `assert`/`expect` inside `if`/`catch`/`for` with no `else`/`panic` fallback |
| **Async assertion never runs** | missing `await`/`return` so the test ends before the assertion; a later rejection lands after green | `.then(`/`waitFor(`/`findBy`/`.resolves`/`.rejects` not `await`ed |
| **Spawned work not joined (Rust)** | `tokio::spawn` whose `JoinHandle` is dropped; a future built but never `.await`ed (futures are lazy) | `tokio::spawn(` in a test with no later `.await` on the handle |

## Mechanism 2 — the check can't fail

| Anti-pattern | What it looks like | Grep signature |
|---|---|---|
| **Over-broad / weak assertion** | `toBeTruthy` / `toBeDefined` / `expect.anything()` / `not.toThrow()` — passes for a huge class of wrong values | those matchers as the *only* assertion |
| **Variant-only Result assertion (Rust)** | `assert!(r.is_ok())` / `assert!(r.is_err())` discards the value *and* the error — a wrong-but-`Ok` value passes, the real error is hidden | `assert!(.*\.is_ok())`, `assert!(.*\.is_err())`, `…is_some()`, `…is_none()` |
| **Status-only API assertion** | asserts HTTP 400/404/409 but not the error-body `code` → passes if the endpoint failed *for a different reason* | `assert_status(` with no `body["code"]` / `assert_json` nearby |
| **Redundant / vacuous assertion** | `assert!(true)`, `assert_eq!(x, x)` — independent of the SUT | `assert!(true)`, `assert_eq!((\w+), \1)` |
| **Expected value recomputed with the SUT's own logic** | the test re-derives the expected value the same way the code does → shares any bug, can't fail | expected built from the same formula/helper as the SUT |
| **Happy-path-only** | every fixture is valid; no error/edge/boundary case | *absence* of `is_err`/`rejects`/`throws`/boundary inputs in a module's tests |

## Mechanism 3 — the check just restates the code

| Anti-pattern | What it looks like | Grep signature |
|---|---|---|
| **Change-detector / tautological test** | a "mirror of the implementation": mock expectations line up 1:1 with production steps, true by construction | mostly `verify(…)` / `toHaveBeenCalled` with few/no output asserts |
| **Testing the mock** | stub a dep with `mockReturnValue(X)`, then assert the result equals `X` (the unit only forwards it); or `toHaveBeenCalled` as the *only* assertion | `mockReturnValue(x)` … `expect(...).toBe(x)`; sole-assertion `toHaveBeenCalled` |
| **Over-mocking internal collaborators** | mocking in-process collaborators (not external boundaries); "your code works *if* your mocks behave like the real ones" | `vi.mock('../internalHelper')` for a pure in-process module |
| **Mocking / spying the SUT itself** | partial-mock or `spyOn` a method *on the object under test* → "contaminated test subject" | `vi.spyOn(SUT, 'methodUnderTest')` |
| **Self-round-trip serialization** | `assert_eq!(deserialize(serialize(x)), x)` is invariant under representation change — a symmetric bug survives and the *wire format* is never pinned | `from_str(&…to_string`, `to_value()`↔`try_from_value` with no literal |
| **Asserting on Debug/Display or implementation detail** | pins `format!("{:?}", x)` or internal structure, not behavior | `assert_eq!(format!("{:?}"`, `.to_string(), "` |
| **Name claims more than the body verifies** | `it('rotates the token')` that only checks a token *exists* | (read each test's name against its asserts) |

## Project rule — pin the ErrorCode

This codebase's `error::Error` carries a machine-readable `ErrorCode` **enum** per failure (`ErrorCode::SessionClosed`, `ErrorCode::UsernameTaken`, `ErrorCode::TokenExpired`, …), which serializes to a stable lowercase string in the JSON body (`session_closed`, `username_taken`, `token_expired`, `invalid_request_body` vs generic `bad_request`, …). So assert the **variant** in service-layer tests and the serialized **`body["code"]` string** in integration tests:

- **Service-layer unit tests** named for a specific rejection must assert the **variant** (and message substring where the name implies a code), e.g. `assert!(matches!(result, Err(Error::Conflict { .. })))` — not bare `result.is_err()`.
- **Integration tests** must assert `body["code"]`, not just the HTTP status, since distinct codes share a status (two 400s: `invalid_request_body` vs `bad_request`; three 409s; two 404s).
- **Exempt:** functions with a *single* failure mode (pure newtype validators like `Username::try_from`, a raw FK-violation insert). For those, `is_err()` is adequate — don't flag it. The good in-repo templates are `backend/src/error.rs`, `backend/src/extract.rs`, `backend/tests/db_timeout.rs`.

## False-positive traps

Do **not** flag these — they look bad to a regex but are fine:

1. **Custom assertion helpers.** `axum-test`'s `res.assert_status(StatusCode::X)` panics on mismatch — a test using it is *not* assertion-free. Same for a local `assert_bad_request(...)` that matches `Err(BadRequest)` and checks the message. Trace the helper before flagging.
2. **Joined spawns.** `tokio::spawn` is fine when the handle is later `.await`ed / joined (`for h in handles { h.await.unwrap() }`).
3. **Complemented round-trips.** A serde round-trip proptest is fine when a sibling test pins the actual wire bytes (`..._serializes_as_bare_string`). Flag the round-trip only when nothing pins the literal form.
4. **MSW-at-the-fetch-boundary (frontend).** Mocking HTTP and exercising the real client/hook/page through it is the *correct* fidelity boundary, not over-mocking.
5. **`vi.spyOn(console, 'error')`** to silence/verify logging is fine. A `vi.spyOn(queryClient, 'invalidateQueries')` that is the *sole* assertion is the smell (Mechanism 3).
6. **`assert!(x.is_err())` on a single-failure-mode function** (see the project rule's exemption).

## Lint rules to prevent regressions

- **Rust:** `#![warn(clippy::assertions_on_result_states)]` flags `assert!(r.is_ok()/is_err())` (it's in clippy's `restriction` group → must be opted in). `eq_op` and `assertions_on_constants` are already default-warn and catch `assert_eq!(x, x)` / `assert!(true)`.
- **Frontend:** `eslint-plugin-vitest` `recommended` (turns on `valid-expect`, `valid-expect-in-promise`, `no-conditional-expect`, `no-standalone-expect`, `expect-expect`, `no-focused-tests`, `no-disabled-tests`) + `eslint-plugin-testing-library` React config (`await-async-utils`, `await-async-queries`). What lint can't catch: snapshot rubber-stamping, weak-but-allowed matchers, and over-mocking judgment — those need this skill.

## Sources

Test-smell canon: Meszaros, *xUnit Test Patterns* (Assertion Roulette, Conditional Test Logic, Eager Test); Fowler, ["Assertion Free Testing"](https://martinfowler.com/bliki/AssertionFreeTesting.html) and ["Mocks Aren't Stubs"](https://martinfowler.com/articles/mocksArentStubs.html); Google Testing Blog, ["Change-Detector Tests Considered Harmful"](https://testing.googleblog.com/2015/01/testing-on-toilet-change-detector-tests.html), ["Don't Overuse Mocks"](https://testing.googleblog.com/2013/05/testing-on-toilet-dont-overuse-mocks.html), ["Test Behavior, Not Implementation"](https://testing.googleblog.com/2013/08/testing-on-toilet-test-behavior-not.html); [testsmells.org](https://testsmells.org/pages/testsmells.html); Khorikov, *Unit Testing: Principles, Practices, and Patterns*.

LLM-specific evidence: Yuan et al., FSE 2024 ([arXiv 2305.04207](https://arxiv.org/abs/2305.04207)); Schäfer et al., TSE 2024 / TestPilot ([arXiv 2302.06527](https://arxiv.org/pdf/2302.06527)); Codex no-assertion/no-focal-call figures ([arXiv 2412.14308](https://arxiv.org/abs/2412.14308)); oracle-captures-actual-behavior ([arXiv 2410.21136](https://arxiv.org/pdf/2410.21136)); coverage≠mutation ([arXiv 2506.02954](https://arxiv.org/html/2506.02954)); over-mocking by agents, MSR 2026 ([arXiv 2602.00409](https://arxiv.org/abs/2602.00409)).

Rust / Vitest specifics: The Rust Book ch. 11; clippy lint index (`assertions_on_result_states`, `eq_op`, `assertions_on_constants`); matklad, ["How to Test"](https://matklad.github.io/2021/05/31/how-to-test.html); official Jest/Vitest docs; `eslint-plugin-jest` / `eslint-plugin-vitest` / `eslint-plugin-testing-library` rule docs; Kent C. Dodds, ["Effective Snapshot Testing"](https://kentcdodds.com/blog/effective-snapshot-testing).

Full project-specific findings that motivated this skill: `docs/drafts/unit_test_audit.md` (and GitHub Issue #217).
