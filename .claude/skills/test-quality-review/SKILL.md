---
name: test-quality-review
description: >
  Audits unit tests to verify they actually test what their names claim — not
  merely that they pass. Flags assertion-free tests, weak or over-broad
  assertions, tautological / over-mocked ("testing the mock") tests, rejection
  tests that don't pin the specific error, serialization round-trips that don't
  pin the wire format, and tests whose name promises more than the body verifies.
  Covers Rust (#[test] / #[tokio::test]) and TypeScript (Vitest / React Testing
  Library). Use when reviewing or writing tests, when the user asks to check test
  quality or "do these tests actually test what they say", and whenever a diff
  adds or changes test files — run it even if the user doesn't explicitly ask for
  a test audit.
allowed-tools:
  - Bash(bash:*)        # runs scripts/scan_tests.sh (shells out to grep + python3)
  - Bash(grep:*)
  - Bash(git diff:*)    # scope to changed test files
  - Bash(git ls-files:*)
  - Read
  - Grep
  - Glob
---

# Test Quality Review

Verify that tests test **what their names claim** — not merely that they pass. A green test that asserts nothing, asserts something that can't fail, or just restates the implementation gives false confidence. For each test, compare what its name/comment *claims* against what its body *actually verifies*, and flag the gaps.

This is the "do these tests test what they say?" layer. It complements the `code-review` skill (correctness / security / standards); here the subject under review is the **tests themselves**.

## Method

1. **Scope.** Reviewing a diff? Target the added/changed test files (`git diff --name-only`, filtered to `*.rs` and `*.test.ts(x)` / `*.spec.ts(x)`). Otherwise use the paths the user gives.
2. **Run the mechanical scan from the repo root** to surface candidates fast:
   ```bash
   # whole test trees (defaults: backend/src backend/tests frontend/src)
   bash .claude/skills/test-quality-review/scripts/scan_tests.sh
   # …or scope to a diff's changed test files:
   bash .claude/skills/test-quality-review/scripts/scan_tests.sh \
     $(git diff --name-only --diff-filter=AM | grep -E '\.(rs|tsx?)$')
   ```
   It flags the high-signal smells (variant-only `is_err()` asserts, `should_panic` without `expected`, `assert!(true)`, `#[ignore]`, no-assertion `#[test]` bodies; bare `vi.mock`, weak matchers, sole-`toHaveBeenCalled`, un-awaited `waitFor`/`findBy`, `.skip`/`.only`, snapshots). Needs `python3`.
3. **Trace before you judge.** A scan hit is a **candidate, not a verdict** — read the test in context first. The common false positives are in [references/anti-patterns.md](references/anti-patterns.md) § False-positive traps (custom assert helpers like `axum-test`'s `res.assert_status(...)` *do* assert; MSW-at-the-fetch-boundary is *not* over-mocking; a round-trip with a sibling test that pins the bytes is covered). **Accuracy over volume** — never flag a test that's actually fine.
4. **Classify the gap** against the taxonomy in [references/anti-patterns.md](references/anti-patterns.md): the three mechanisms are (1) no real check runs, (2) the check can't fail, (3) the check just restates the code — plus *name-claims-more-than-verified* and *async-assertion-never-runs*.
5. **Apply the project rule.** This codebase has a machine-readable `ErrorCode` registry, so a test named for a *specific* failure that asserts only `result.is_err()` (service layer) or only the HTTP status (integration) is **too weak** — it passes if the code fails for the wrong reason. Require the specific variant / `code`. Single-failure-mode validators are exempt (see the reference).
6. **Assign severity and write findings.**

## Severity

- **High** — a broken implementation of the *claimed* behavior would still pass (verifies ~nothing).
- **Medium** — verifies the behavior but more loosely than the name implies, or tests choreography (a mock was called) instead of the outcome.
- **Low** — stylistic or redundant. (If the claim is fully covered by a sibling test, *clear* it — naming the sibling — rather than flagging it Low.)

## Output

One line per flagged test:

```
FILE:LINE | test name | [mechanism] | CLAIMS: <what the name implies> | VERIFIES: <what the asserts check> | SEVERITY | FIX: <one line>
```

Example:

```
backend/src/services/sessions/lifecycle.rs:904 | test_cannot_join_closed_session | [Mechanism 2 / status-only] | CLAIMS: joining a closed session is rejected as session_closed | VERIFIES: only result.is_err() | Medium | FIX: assert!(matches!(result, Err(Error::Conflict { code: ErrorCode::SessionClosed, .. })))
CLEARED: domain/ids.rs:233 deserialize-failure is_err() — single-failure-mode newtype validator; is_err() is adequate per the project rule.
```

Then a short summary: per-file `flagged / total`, the systemic patterns, and the highest-value fixes first. Show the corrected assertion. Note what you **cleared** and why (the false positives you ruled out) so the review reads as evidence-based, not a complaint list.

## References & scripts

- [references/anti-patterns.md](references/anti-patterns.md) — the cited anti-pattern taxonomy, grep signatures, the false-positive traps, and the lint rules that prevent regressions. **Read it** when classifying.
- `scripts/scan_tests.sh` — **run** it (from the repo root, full path above) to surface candidates (deterministic grep battery; needs `python3`). Read it only to extend the patterns.
