# Testing — cross-cutting assertion rules

The language-specific testing policy lives with each stack: Rust patterns in
[`rust.md`](./rust.md) § 7, the TypeScript umbrella + Vitest patterns in
[`typescript.md`](./typescript.md) § 12, and the React Testing Library + MSW
patterns in [`react.md`](./react.md) § 13. This file holds the few rules that
apply to **both** stacks — the ones a test most often gets *almost* right: it
passes, but it verifies less than its name promises.

The one-line test of a test, from Google's Alex Eagle: a test has no value if
**a correct or incorrect program is equally likely to pass it.**

## 1. Assert the *specific* failure, not just "it failed"

**Rule:** A test named for a specific failure must pin that failure — the exact
error variant / `code` (backend) or the exact rejected outcome (frontend) — not
just that *some* error happened. `assert!(result.is_err())` and a bare
`assert_status(409)` are too loose: they pass if the code fails at a *different*
gate, for a *different* reason than the test name claims.

**Why:** We maintain a machine-readable [`ErrorCode`](../../backend/src/error.rs)
registry ([`api-contract.md`](../api-contract.md) § 7) precisely so failures are
distinguishable. A multi-gate function (`create_run`, `join_session`, …) has
several failure modes that share a status; a status-only or `is_err`-only
assertion can't tell "rejected the closed session" from "rejected because the
user wasn't a participant." A test that can't tell those apart doesn't guard the
behavior its name describes.

**Example (Rust):**

```rust
// Don't — passes if create_run fails at ANY gate.
let result = create_run(&db, &user, req).await;
assert!(result.is_err());

// Do — pin the registry code (and the message where the code is generic).
let err = delete_run(&db, &run.id, &other).await.unwrap_err();
assert_eq!(err.code(), ErrorCode::Forbidden);
// `create_run` returns a non-Debug `RunDetail`, so match instead of unwrap_err:
match create_run(&db, &user, req).await {
    Err(e) => {
        assert_eq!(e.code(), ErrorCode::Conflict);
        assert!(e.to_string().contains("Already submitted"), "got: {e}");
    }
    Ok(_) => panic!("expected Conflict, got Ok"),
}
```

**Example (integration / wire):**

```rust
// Don't — status-only: invalid_request_body and bad_request are both 400.
res.assert_status(StatusCode::BAD_REQUEST);

// Do — pin the body `code` too.
res.assert_status(StatusCode::BAD_REQUEST);
let body: Value = res.json();
assert_eq!(body["code"], "bad_request");
```

Single-failure-mode functions (one bound, one parse) are the exception: there
`r.unwrap_err()` is enough, because there is only one reason to fail.
[`error.rs`](../../backend/src/error.rs), [`extract.rs`](../../backend/src/extract.rs),
and [`db_timeout.rs`](../../backend/tests/db_timeout.rs) are the model — they
assert variant **+** code **+** body.

**Source:** clippy [`assertions_on_result_states`][clippy-res]; A. Eagle,
[Change-Detector Tests Considered Harmful][eagle]; Issue [#217][issue].

## 2. Assert the observable outcome, not the choreography

**Rule:** Don't make a spy on an internal interaction the *sole* assertion.
`expect(invalidateQueries).toHaveBeenCalledWith(...)` proves a call was made, not
that anything refreshed; it stays green even if invalidation stops causing a
refetch. Drive the real effect and assert what the user would see — the new
screen rendered, the button flipped, the refreshed data appeared.

**Why:** A change-detector test mirrors the implementation, so it can't fail
independently of it. Asserting the outcome (navigation occurred, the Join button
became Leave, the rotated cookie still refreshes) catches the regression the
interaction was a proxy for.

**Example (frontend):**

```tsx
// Don't — passes even if invalidation no longer refreshes.
expect(invalidate).toHaveBeenCalledWith({ queryKey: ['session', 's1'] });

// Do — flip the (MSW) server state on the mutation, then assert the refetched
// UI. Keep the spy only as a secondary check of *which* keys, never the sole one.
await user.click(screen.getByRole('button', { name: /next track/i }));
expect(await screen.findByRole('heading', { name: 'Rainbow Road' })).toBeInTheDocument();
```

**Source:** A. Eagle, [Change-Detector Tests Considered Harmful][eagle]; M. Fowler,
[Mocks Aren't Stubs][fowler]; eslint-plugin-vitest / eslint-plugin-testing-library.

## 3. Pin the wire format; don't rely on a self-round-trip

**Rule:** `assert_eq!(deserialize(serialize(x)), x)` is invariant under a
*symmetric* representation change — a typo present in both directions survives,
and the actual on-disk / on-the-wire bytes are never pinned. For any value whose
serialized form is a contract (a DB-stored enum string, a JSON discriminant
tag), assert the **literal** alongside the round-trip.

**Why:** `SessionStatus::Active` is written to `sessions.status` on the live path;
a `string_value` typo that's symmetric (write `"actve"`, parse `"actve"`)
round-trips cleanly but corrupts the column. The literal pin
(`assert_eq!(SessionStatus::Active.to_value(), "active")`) is what catches it.

**Source:** serde [unit-testing][serde] (`assert_tokens` pins the wire form);
Issue [#217][issue].

## Tooling

These rules are partly enforced so they can't silently reappear:

- **Backend:** `#![warn(clippy::assertions_on_result_states)]` in
  [`lib.rs`](../../backend/src/lib.rs) flags `assert!(r.is_ok()/is_err())`. It is
  in clippy's `restriction` group (opt-in) and only fires on compiled code, so
  the lefthook `rust-clippy` hook runs `cargo clippy --all-targets` to reach the
  `#[cfg(test)]` modules. Status-only integration assertions (rule 1, wire) and
  the wire-literal pins (rule 3) are review-enforced — no lint covers them.
- **Frontend:** `@vitest/eslint-plugin` `recommended` + `eslint-plugin-testing-library`
  flat React config, scoped to test files in [`eslint.config.js`](../../frontend/eslint.config.js).
  Together they lock in `valid-expect`, `valid-expect-in-promise`,
  `expect-expect`, `no-focused-tests`, `no-disabled-tests`, and the async-query
  hygiene rules. Two rules that fire on existing valid patterns
  (`vitest/no-conditional-expect`, `testing-library/no-node-access`) start at
  `warn` per that file's preamble. Rule 2 (outcome over choreography) is
  review-enforced.

## Document history

- 2026-05-31 — Created (#217) as the cross-cutting companion to `rust.md` § 7 /
  `typescript.md` § 12 / `react.md` § 13. Captures the three recurring
  test-assertion gaps a backend+frontend audit surfaced — assert the specific
  error/`code`, assert the observable outcome (not a spy), pin the wire literal —
  and records the lints adopted to hold them (`clippy::assertions_on_result_states`
  + lefthook `--all-targets`; vitest/testing-library eslint presets). Audit
  findings live in the issue; promote the working draft to `research/` if a
  durable in-repo link is wanted.

[clippy-res]: https://rust-lang.github.io/rust-clippy/master/index.html#assertions_on_result_states
[eagle]: https://testing.googleblog.com/2015/01/testing-on-toilet-change-detector-tests.html
[fowler]: https://martinfowler.com/articles/mocksArentStubs.html
[serde]: https://serde.rs/unit-testing.html
[issue]: https://github.com/brendanbyrne/beerio-kart/issues/217
