# Rust — General Coding Standards

> **Scope.** General Rust patterns for the `beerio-kart` backend. Edition 2024, Rust 1.85+. Not specific to SeaORM or Tokio — those have their own files in this directory.
> **Format.** Each rule: *Rule / Why / Example / Source*.
> **Companions.** `seaorm.md`, `tokio.md`, `../api-contract.md`. Archived: `../designs/archive/compliance-plan.md`.

## Index

1. [Error handling](#1-error-handling)
2. [Type-driven design](#2-type-driven-design)
3. [Module organization & visibility](#3-module-organization--visibility)
4. [Ownership patterns](#4-ownership-patterns)
5. [Iterators & functional style](#5-iterators--functional-style)
6. [Documentation](#6-documentation)
7. [Testing](#7-testing)
8. [Clippy & lints](#8-clippy--lints)
9. [Edition 2024](#9-edition-2024)
10. [Logging with tracing](#10-logging-with-tracing)
11. [Misc idioms](#11-misc-idioms)
12. [Anti-patterns](#12-anti-patterns)
13. [File length](#13-file-length)
14. [Serde conventions](#14-serde-conventions)
15. [Cargo.toml hygiene](#15-cargotoml-hygiene)
16. [rustfmt and editorconfig](#16-rustfmt-and-editorconfig)
17. [Config and environment handling](#17-config-and-environment-handling)
18. [Feature flags](#18-feature-flags)

---

## 1. Error handling

The repo's `AppError` is squarely typed-enum territory — the response layer maps variants to HTTP status codes, so callers must be able to discriminate. The relevant tools are `thiserror` (typed enums for libraries / service code) and `anyhow` (opaque wrapper with context, for binaries / glue). We use `thiserror`.

- **Rule:** `AppError` is a `thiserror`-derived enum. Don't use `anyhow::Error` in any signature reachable from a route handler.
  - **Why:** The route layer needs to distinguish 404 from 409 from 500. `anyhow::Error` is opaque; the discriminator is gone. `thiserror` for libraries (and the service layer is one), `anyhow` for top-level binary glue.
  - **Example:**
    ```rust
    #[derive(thiserror::Error, Debug)]
    #[non_exhaustive]
    pub enum AppError {
        #[error("{0}")] NotFound(String),
        #[error("{0}")] Conflict(String),
        #[error(transparent)] Db(#[from] sea_orm::DbErr),
        // ...
    }
    ```
  - **Source:** <https://docs.rs/thiserror>, <https://docs.rs/anyhow>, <https://www.lpalmieri.com/posts/error-handling-rust/>

- **Rule:** Mark `AppError` `#[non_exhaustive]`.
  - **Why:** Adding a variant later (`Timeout`, `RateLimited`) won't break match arms in tests and call sites if they have a `_ => ...` fallback. With `#[non_exhaustive]` the compiler enforces the fallback.
  - **Source:** <https://rust-lang.github.io/rfcs/2008-non-exhaustive.html>

- **Rule:** Use `?` and `From` for "this failure is exactly that failure"; use `map_err` (or a constructor like `AppError::not_found`) when the call site adds meaning.
  - **Why:** A blanket `From<DbErr> for AppError::Internal` collapses every database error into a 500. That hides 404s (`DbErr::RecordNotFound`) and 409s (UNIQUE / FK violations). Keep `?` for genuinely-internal infrastructure errors; use `map_err` to translate at sites where the meaning is domain-level. (See `seaorm.md` § 7 for the recommended `From<DbErr>` impl.)
  - **Example:**
    ```rust
    let user = users::Entity::find_by_id(id).one(db).await?
        .ok_or_else(|| AppError::NotFound("user".into()))?;
    ```

- **Rule:** When `Internal` collapses many calls into one variant, attach context. Either an `Internal { source, context: &'static str }` shape or a single `Internal(anyhow::Error)` variant with `.context(...)` at service boundaries.
  - **Why:** `Internal("Database error: <DbErr Display>")` tells you *what type* failed, never *what we were doing*. Either a static context per call site or anyhow's chain restores the frame so a 500 is debuggable from one log line.
  - **Source:** <https://docs.rs/anyhow/latest/anyhow/trait.Context.html>

- **Rule:** Log the full error chain at the boundary (`IntoResponse` impl), not at every `?`.
  - **Why:** Logging at every `?` produces five lines per error. Logging once at the boundary, walking `error.source()`, gives one line with the entire chain.
  - **Source:** <https://www.iroh.computer/blog/error-handling-in-iroh>

- **Rule:** Error-message strings we *construct* start with a capital letter and have no trailing punctuation. This applies to `AppError` variant payloads (`AppError::BadRequest("Username already taken")`), `anyhow` static contexts (`.context("Failed to build Set-Cookie header")`), and synthetic `anyhow!` messages (`anyhow::anyhow!("Cup not found for cup_id {id}")`). Function names and other Rust identifiers are baked-in lowercase — keep their case.
  - **Why:** Consistency between user-facing messages (4xx body `error` field) and log-visible messages (5xx chain log). The capital-leading-word convention reads as English. We deliberately diverge from anyhow's docs convention (lowercase, no punctuation) for project-wide uniformity — the user-facing 4xx variants already commit to capital-first across 30+ existing call sites, and a mixed convention (capital for client-facing, lowercase for internal) would be invisible inconsistency that drifts on every PR.
  - **Scope:** The rule applies to strings *we construct*. Pass-through messages from external crates (e.g., `DbErr::RecordNotFound(msg)` flowing into `AppError::NotFound(msg)`) are not rewritten — we don't own those.

## 2. Type-driven design

Newtype every value with semantic meaning. The compiler then catches "wrong-id-in-this-arg" bugs at compile time, validation runs once at the type boundary, and signatures become self-documenting. Use the [`nutype`](https://docs.rs/nutype) crate for the boilerplate.

The principle is *parse, don't validate*: turn raw strings/integers from the wire into a strongly-typed value at the entry point, and after that, the rest of the code can trust the value because the type proves it.

**Integration approach with SeaORM.** Codegen produces `String`/`i32` for entity columns. We convert at the entity↔service boundary — entities stay primitive, the domain layer uses newtypes. This is the practical hybrid; see `seaorm.md` § 6 for the entity side.

- **Rule:** Newtype every domain identifier — `UserId`, `RunId`, `SessionId`, `TrackId`, `CharacterId`, etc. Don't pass raw `Uuid` or `i32` between modules.
  - **Why:** `fn record_run(user: UserId, session: SessionId)` makes the wrong-order bug compile-time. Raw `Uuid` arguments have no such protection.
  - **Example:**
    ```rust
    #[nutype(
        derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display,
               AsRef, Serialize, Deserialize, FromStr),
    )]
    pub struct UserId(uuid::Uuid);
    ```
  - **Source:** <https://docs.rs/nutype>, <https://rust-unofficial.github.io/patterns/patterns/behavioural/newtype.html>

- **Rule:** Apply *parse, don't validate* to anything with a constructor invariant: `Username`, `EmailAddress`, `DrinkTypeName`, `RaceTimeMs`, `LapTimeMs`. Use `nutype` to encode validation rules and produce the `TryFrom`/`FromStr` impls.
  - **Why:** Re-validating "must be 1–30 chars" at every call site drifts. Encoding it in the type validates once, at the boundary; everything downstream can trust the value.
  - **Example:**
    ```rust
    #[nutype(
        sanitize(trim),
        validate(len_char_min = 1, len_char_max = 30),
        derive(Debug, Clone, AsRef, Display, Serialize, Deserialize, TryFrom),
    )]
    pub struct Username(String);
    ```
  - **Source:** <https://docs.rs/nutype/latest/nutype/>, <https://entropicdrift.com/blog/refined-types-parse-dont-validate/>

- **Rule:** Always add `#[serde(transparent)]` (or its `nutype` equivalent — included by `Serialize`/`Deserialize` derives in `nutype`'s default config) to newtypes that cross the wire.
  - **Why:** Without it, `UserId(uuid)` serializes as `{"0": "abc-..."}` instead of the bare UUID string, breaking the API contract.
  - **Source:** <https://serde.rs/container-attrs.html#transparent>

- **Rule:** Use stdlib refinement types when they fit: `NonZeroI32` for "must be positive" race times, `Duration` for time intervals.
  - **Why:** Free, layout-optimized (`Option<NonZeroI32>` is 4 bytes), self-documenting.
  - **Source:** <https://doc.rust-lang.org/std/num/type.NonZeroI32.html>

- **Rule:** Add `#[must_use]` to constructors and to types representing a guard (e.g., `TransactionGuard`).
  - **Why:** `Result` is `#[must_use]` automatically; custom types aren't. A guard whose return value gets dropped silently leaks the resource.
  - **Source:** <https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-must_use-attribute>

- **Rule:** Replace stringly-typed enums (`status: String`, `ruleset: String`) with real enums that derive `sea_orm::DeriveActiveEnum`.
  - **Why:** The DB stores them as TEXT, but everything in Rust should be `enum Ruleset { Random, Default, LeastPlayed, RoundRobin }`. The compiler then guarantees every match is exhaustive and rules out typos.
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/generate-entity/enumeration/>

- **Rule:** Don't newtype primitives whose context already disambiguates them — booleans (`disqualified`, `alcoholic`), timestamps (`DateTime<Utc>` is already a strong type), short-lived counters in a single function.
  - **Why:** Newtypes earn their cost when there's a real risk of confusion. `bool` parameters with descriptive names don't have that risk; over-newtyping turns into noise.

## 3. Module organization & visibility

- **Rule:** Use the `foo.rs` + `foo/bar.rs` style for hand-written code. No new `mod.rs` files. Codegen output is exempt — leave SeaORM's `entities/mod.rs` alone.
  - **Why:** The 2018 edition explicitly recommends the new style; editor tabs all named `mod.rs` are unreadable. Codegen tools (sea-orm-cli, prost, tonic) all produce `mod.rs`; renaming post-codegen is a hack with no industry precedent. Accept the inconsistency at the codegen boundary.
  - **Source:** <https://doc.rust-lang.org/edition-guide/rust-2018/path-changes.html>

- **Rule:** Default to `pub(crate)` for cross-module items. Use bare `pub` only when the item must cross a crate boundary (between `migration` and the main backend, primarily).
  - **Why:** `pub(crate)` documents intent and lets the compiler help on refactors. With `warnings = "deny"` already set, `dead_code` flags truly unused items.
  - **Example:**
    ```rust
    pub(crate) fn hash_password(plain: &str) -> Result<String, AppError> { /* ... */ }
    pub fn version() -> &'static str { env!("CARGO_PKG_VERSION") }
    ```
  - **Source:** <https://kobzol.github.io/rust/2025/04/23/two-ways-of-interpreting-visibility-in-rust.html>

- **Rule:** Re-export the public surface from `lib.rs` so external code never imports through internal paths.
  - **Why:** Direct imports of internal paths bind callers to file layout. A re-export block at the top of `lib.rs` decouples them.
  - **Source:** <https://doc.rust-lang.org/rustdoc/write-documentation/re-exports.html>

## 4. Ownership patterns

- **Rule:** Take `&str` and `&[T]` in arguments by default. Take owned `String` / `Vec<T>` only when the function actually stores them.
  - **Why:** `&str` accepts `&String`, `&str`, and string literals via deref coercion — no caller-side conversion. Taking `String` forces a clone at every borrow site.
  - **Example:**
    ```rust
    // Inspect-only:
    fn validate_username(s: &str) -> Result<Username, AppError> { /* ... */ }
    // Stored:
    fn into_user(name: String, hash: String) -> users::ActiveModel { /* ... */ }
    ```
  - **Source:** <https://www.philipdaniels.com/blog/2019/rust-api-design/>

- **Rule:** Treat `.clone()` as a code smell that wants a one-line justification. Banned for "make the borrow checker happy" in handlers and services.
  - **Why:** Most "borrow-checker clones" mean lifetimes weren't thought through. In async code, large clones add up — DTOs cloned per request matter.
  - **Source:** <https://rust-unofficial.github.io/patterns/anti_patterns/borrow_clone.html>

- **Rule:** When sharing immutable state across handlers, wrap once in `Arc` at construction. Use `Arc::clone(&x)` rather than `x.clone()` to make intent explicit.
  - **Why:** `Arc::clone` distinguishes "bump refcount" from "deep clone." A reader scanning the code sees the difference.
  - **Source:** <https://doc.rust-lang.org/std/sync/struct.Arc.html>

- **Rule:** Prefer `impl Trait` in argument position (`impl Iterator<Item = ...>`) for "any iterator / future"; reach for named generics only when the same type appears twice in the signature.
  - **Source:** <https://doc.rust-lang.org/reference/types/impl-trait.html>

## 5. Iterators & functional style

- **Rule:** Iterator chains beat imperative loops when the chain is ≤ 3 stages with clear intent. Switch to `for` when control flow inside closures gets non-trivial.
  - **Why:** Iterators are zero-cost and self-documenting for transforms; forcing a loop into `.scan()` and `.fold()` to avoid mutation is the failure mode.
  - **Source:** <https://www.lurklurk.org/effective-rust/iterators.html>

- **Rule:** Collect `impl Iterator<Item = Result<T, E>>` directly into `Result<Vec<T>, E>` with `.collect::<Result<Vec<_>, _>>()`. Don't loop and push.
  - **Why:** Canonical idiom; short-circuits on the first error.
  - **Example:**
    ```rust
    let ids: Vec<RunId> = raw.iter()
        .map(|s| s.parse::<Uuid>().map(RunId::new))
        .collect::<Result<_, _>>()?;
    ```
  - **Source:** <https://doc.rust-lang.org/rust-by-example/error/iter_result.html>

- **Rule:** Don't `collect()` if you immediately re-iterate. `.iter().collect::<Vec<_>>().iter()` is a pointless allocation.
  - **Source:** <https://rust-lang.github.io/rust-clippy/master/index.html#needless_collect>

## 6. Documentation

- **Rule:** Every `pub` and cross-module `pub(crate)` item gets a `///` doc comment whose first line is one sentence ending in a period.
  - **Why:** rustdoc renders the first sentence in summary tables; a real sentence makes the index readable.
  - **Source:** <https://rust-lang.github.io/api-guidelines/documentation.html>

- **Rule:** Document fallible functions with `# Errors`; document panicking functions with `# Panics`.
  - **Example:**
    ```rust
    /// Looks up a user by id.
    ///
    /// # Errors
    /// - [`AppError::NotFound`] if no user has that id.
    /// - [`AppError::Internal`] on database failure.
    pub async fn find_user(db: &impl ConnectionTrait, id: UserId)
        -> Result<User, AppError> { /* ... */ }
    ```
  - **Source:** <https://rust-lang.github.io/rfcs/1574-more-api-documentation-conventions.html>

- **Rule:** Use intra-doc links — `` [`AppError`] `` not `[AppError](crate::error::AppError)`. They're checked by rustdoc and survive refactors.
  - **Source:** <https://rust-lang.github.io/rfcs/1946-intra-rustdoc-links.html>

- **Rule:** Add a crate-level `//!` doc to `lib.rs` and `migration/lib.rs`. Three short paragraphs max — what the crate does, what the entry points are.
  - **Source:** Rust API Guidelines C-CRATE-DOC.

- **Rule:** When a PR modifies a file, re-review the doc comments on every public item in that file. Update wording, examples, and `# Errors` sections to match the new behavior.
  - **Why:** Stale docs are worse than no docs — they actively mislead. The cost of re-reviewing is small if you do it as part of the PR; the cost of fixing stale docs months later is much higher.

## 7. Testing

- **Rule:** Tests trace to requirements, not to lines. Before writing a test, name the behavior or invariant it verifies. High line coverage is a *byproduct* of thoroughly verifying requirements, not the goal.
  - **Why:** A test whose only purpose is to traverse a code path without asserting anything meaningful is worse than no test — it gives false confidence. Tests that exist to "hit coverage" rot fastest.

- **Rule:** Inline `#[cfg(test)] mod tests` for unit tests; `tests/` for end-to-end HTTP integration tests against fresh in-memory SQLite. One file per logical area (`tests/auth.rs`, `tests/runs.rs`).
  - **Why:** Inline tests can reach private items; integration tests live in their own crate and exercise the public API. Keeping them separate keeps clear what's being tested.
  - **Source:** <https://doc.rust-lang.org/book/ch11-03-test-organization.html>

- **Rule:** Test names are sentences: `test_login_with_wrong_password_returns_401`, not `test_login2`. The test name *is* the behavioral spec — read the test list and the requirement coverage should be visible.
  - **Source:** Existing project convention (CLAUDE.md).

- **Rule:** Use `rstest` for table-driven cases when the same logic runs over half-a-dozen inputs. Use a plain `for` loop for two or three.
  - **Example:**
    ```rust
    #[rstest]
    #[case("ab", false)]
    #[case("ok_user", true)]
    #[case(&"x".repeat(31), false)]
    fn username_validates(#[case] input: &str, #[case] ok: bool) {
        assert_eq!(Username::try_from(input).is_ok(), ok);
    }
    ```
  - **Source:** <https://docs.rs/rstest>

- **Rule:** Reach for `proptest` when you have an algebraic invariant — round-trip serde, lap times always sum to total time, monotonic timestamps. Skip for "load user by id"-style example tests.
  - **Source:** <https://docs.rs/proptest>

- **Rule:** Reach for `insta` for snapshot tests of HTTP response bodies in integration tests; otherwise stick to `assert_eq!`.
  - **Why:** Snapshot diffs are reviewable in PRs; updates are one `cargo insta accept`.
  - **Source:** <https://docs.rs/insta>

- **Rule:** Always pass `expected = "..."` to `#[should_panic]`. Bare `#[should_panic]` will pass on the wrong panic.
  - **Source:** <https://doc.rust-lang.org/book/ch11-01-writing-tests.html>

- **Rule:** `unwrap()` and `expect()` are fine in tests — don't bend test code into `?` shapes when a panic is what you want.
  - **Source:** <https://burntsushi.net/unwrap/>

- **Rule:** If a benchmark is needed, use `criterion`. Microbenchmarks are not the project's primary testing strategy — they exist to verify a specific perf hypothesis, not for coverage.
  - **Source:** <https://docs.rs/criterion>

## 8. Clippy & lints

- **Rule:** Configure lints once at the workspace level, then `lints.workspace = true` in each crate.
  - **Why:** Single source of truth; per-crate overrides can't drift.
  - **Example (workspace `Cargo.toml`):**
    ```toml
    [workspace.lints.rust]
    warnings = "deny"
    unsafe_code = "forbid"
    rust_2018_idioms = "warn"

    [workspace.lints.clippy]
    pedantic = { level = "warn", priority = -1 }
    nursery  = { level = "warn", priority = -1 }
    cargo    = { level = "warn", priority = -1 }
    # Tame the noisy pedantic ones:
    module_name_repetitions   = "allow"
    must_use_candidate        = "allow"
    missing_errors_doc        = "allow"   # raise to "warn" once docs catch up
    missing_panics_doc        = "allow"
    cast_precision_loss       = "allow"
    cast_possible_truncation  = "allow"
    # High-value extras:
    dbg_macro          = "deny"
    print_stdout       = "deny"
    print_stderr       = "deny"
    todo               = "warn"
    unimplemented      = "warn"
    expect_used        = "warn"
    unwrap_used        = "warn"
    panic              = "warn"
    # Async (catastrophic if missed) — see tokio.md § 3:
    await_holding_lock        = "deny"
    await_holding_refcell_ref = "deny"
    unused_async              = "warn"
    ```
  - **Source:** <https://rust-lang.github.io/rfcs/3389-manifest-lint.html>

- **Rule:** `unsafe_code = "forbid"` at the workspace level. We have no FFI; if we ever do, it gets its own crate that explicitly allows it.
  - **Why:** `forbid` (vs `deny`) means no descendant module can override with `#[allow]`. Free for a pure-safe API server.
  - **Source:** <https://doc.rust-lang.org/rustc/lints/levels.html>

- **Rule:** `unwrap_used` and `expect_used` are `warn`-level globally and allowed in test code. The form depends on where the test code lives:
  - **In `lib.rs` / non-test source files** with `#[cfg(test)] mod tests { ... }` blocks: use `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]` at the top of the file. The `cfg_attr(test, ...)` gating is load-bearing — it suppresses the lints only when the file is being compiled as a test target.
  - **In integration test files under `tests/`**: use bare `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` at the top of each file. Files under `tests/` are *only* compiled under `cargo test` — `cfg(test)` is unconditionally true there, so the `cfg_attr` gating is redundant and arguably misleading (it implies the file might compile outside test mode).
  - **Why:** Forbidding outright pushes people to `.expect("INFALLIBLE")` which is no better. A warn nudges thinking; tests legitimately want to panic.
  - **Source:** <https://matklad.github.io/2022/07/10/almost-rules.html>

- **Rule:** The lint config and clearing the warnings it surfaces are landed via separate PRs (one to add the config with `#[allow]`s where needed, then one PR per lint group to clear). See the compliance plan for the sequence.

## 9. Edition 2024

The project targets edition 2024 (Rust 1.85+). Edition 2024 is the latest stable as of May 2026; the next stable edition is not expected before 2027.

- **Rule:** With RPIT lifetime capture rules, `fn make_iter<'a>(xs: &'a [u32]) -> impl Iterator<Item = u32>` now captures `'a` automatically. Don't write the `Captures<'a>` trick. If you need to *not* capture a lifetime, use `+ use<>`.
  - **Source:** <https://doc.rust-lang.org/edition-guide/rust-2024/rpit-lifetime-capture.html>

- **Rule:** Use `let` chains in `if`/`while` conditions when they collapse multiple `if let` levels. Cap at two clauses — three+ becomes unreadable.
  - **Source:** <https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/>

- **Rule:** Use `async` closures (`async |req| { ... }`) only when you need higher-ranked async signatures previously impossible with `Fn` returning `Future`. Don't prefer them stylistically.
  - **Source:** <https://rust-lang.github.io/rfcs/3668-async-closures.html>

## 10. Logging with tracing

- **Rule:** Annotate every public service-layer fn with `#[tracing::instrument(skip(db, password), fields(user_id = %id))]`. Skip the connection, skip secrets, surface IDs.
  - **Why:** Auto-recording `db` adds noise; auto-recording `password` is a security incident.
  - **Source:** <https://docs.rs/tracing/latest/tracing/attr.instrument.html>

- **Rule:** Use structured fields (`info!(user_id = %id, "login succeeded")`), never format strings (`info!("login succeeded for {}", id)`).
  - **Why:** Structured fields are separate JSON keys in production logs; format strings are opaque messages that can't be filtered or aggregated.

- **Rule:** Level guidance:
  - `error!` — operator action required (DB unreachable, JWT key missing).
  - `warn!` — anomaly, request was handled (rate limit hit, retry succeeded).
  - `info!` — significant request lifecycle (login, run recorded, session closed).
  - `debug!` — investigation detail (entity loaded, validation passed).
  - `trace!` — extremely verbose; off in prod.

- **Rule:** Never log: passwords (plain or hashed), full JWT tokens (log the `jti` claim if needed), free-text user content (drink notes, race notes — Mario Kart trash talk is user content with PII risk).
  - **Why:** Passwords end up in support tickets; JWTs grant access; free text contains PII you don't expect.

- **Rule:** Span fields on pre-auth handlers (`/login`, `/register`, `/refresh`) take attacker-controlled input. Recording them is correct — `username` is the only correlator before `user_id` exists, and dropping it makes pre-auth incidents un-investigable — but be aware they're a cardinality-attack surface.
  - **Why:** Log aggregators (Datadog, Honeycomb, Loki) meter indexed-field cardinality. A credential-stuffing probe pouring thousands of distinct usernames into `/login` per hour quietly raises the log bill and buries legitimate users in noise. Today the rate-limit middleware ([#132](https://github.com/brendanbyrne/beerio-kart/issues/132)) caps this at the door and there's no aggregator to bill against, so no action needed.
  - **Mitigation if it bites:** drop the field on failed-auth events specifically (not the whole handler) by emitting a separate `auth_failure` event without `username`, sampling failures, or recording `username_hash = sha256(username)` for cardinality-bounded grouping. Don't blanket-drop — successful logins still want the field for investigability.

## 11. Misc idioms

- **Rule:** `unwrap()` policy:
  - **Tests, `build.rs`, examples:** unrestricted.
  - **`main.rs` startup:** prefer `fn main() -> anyhow::Result<()>` with `?` and `.context("...")` over `.expect(...)`/`.unwrap()`. Both abort on misconfiguration, but the anyhow form yields an error chain via `Display`/`source()` (richer than a panic message) and stays clippy-clean without per-file `#[allow(clippy::expect_used)]`. This is the binary-glue half of § 1's `anyhow` (binary) / `thiserror` (library) split. `expect("static config invariant: ...")` remains acceptable for genuinely-infallible call sites where wiring through `Result` would be all noise (rare in practice).
  - **Handlers, services, library code:** banned. Use `?` or explicit `match`.
  - **Source:** <https://burntsushi.net/unwrap/>

- **Rule:** Prefer `expect("description of why this can't fail")` over `unwrap()` when the panic is intentional. The string is for the maintainer reading the panic.
  - **Source:** <https://www.thecodedmessage.com/posts/2022-07-14-programming-unwrap/>

- **Rule:** `Default` impls only for *meaningful* defaults (empty `Vec`, zero counter). Don't `derive(Default)` on a struct where every field needs to be set explicitly.
  - **Why:** A `Default` that produces an invalid value is worse than no `Default` — the type system can't help you remember to override.

- **Rule:** Prefer generics (`fn record_run(repo: &impl RunRepository, ...)`) to `Box<dyn Trait>`. Use `dyn` only for heterogeneous collections, runtime type selection, or stable ABI boundaries.
  - **Why:** Generics monomorphize and inline; `dyn` adds indirection and allocation. The compiler proves more about generic types.

- **Rule:** Use `let ... else` for early-exit on `Option`/`Result` instead of `if let { } else { return Err(...) }`.
  - **Example:**
    ```rust
    let Some(user) = repo.find(id).await? else {
        return Err(AppError::NotFound("user".into()));
    };
    ```
  - **Source:** <https://rust-lang.github.io/rfcs/3137-let-else.html>

## 12. Anti-patterns

- **Anti-pattern:** Check then `unwrap`. Use `if let`.
- **Anti-pattern:** `Vec<Box<dyn Error>>` in a project error type. If you need to aggregate, define `AppError::Multi(Vec<AppError>)`.
- **Anti-pattern:** `.collect::<Vec<_>>().iter()` mid-chain. Drop the collect.
- **Anti-pattern:** Stringly-typed enums (`role: String`). Make a real enum with `serde(rename_all = "snake_case")`.
- **Anti-pattern:** Re-validating an invariant at every call. Use a newtype with `TryFrom`.
- **Anti-pattern:** Business logic inside `From` impls. `From` should be near-zero cost; non-trivial conversion belongs in a named constructor.
- **Anti-pattern:** `clone()` "to make it compile." Fix the design — `&` parameter, an `Arc`, or a structural change.
- **Anti-pattern:** Mutable global state via `lazy_static`/`OnceCell` for things that aren't truly global. Plumb explicitly.

## 13. File length

- **Rule:** A `.rs` file's *non-test* code stays under ~500 lines. Past that, split by concern. Tests in `#[cfg(test)] mod tests` do not count toward the limit.
  - **Why:** Long non-test files usually mean multiple concerns are mashed together — refactoring becomes more expensive the longer the file lives. Test bloat is fine; tests are linear and live next to what they test.
  - **Example (split):** `services/sessions.rs` → `services/sessions/lifecycle.rs` (create / join / leave / host transfer / close-stale), `services/sessions/detail.rs` (the polling read), `services/sessions/races.rs` (per-race orchestration), and `services/sessions/types.rs` (shared DTOs and constants at the bottom of the submodule dependency stack), with `services/sessions.rs` as the module root that declares the submodules and re-exports their public surface.

    The `types.rs` layer is what keeps the dependency graph acyclic when two submodules share a DTO or constant — without it the read aggregator and the race mutations form a `detail ↔ races` cycle on the shared race DTO (and a separate `races → lifecycle` edge on a shared constant closes the 3-cycle — see `services/sessions/types.rs` for the full picture). See PR [#150](https://github.com/brendanbyrne/beerio-kart/pull/150) for the landed shape.
  - **Source:** General refactoring guidance; project-specific call.

- **Rule:** Splits go *by concern*, not by line count. "Half the file at the 500-line mark" is the wrong way; "the rule-application logic and the lifecycle logic don't share state, split them" is the right way.

## 14. Serde conventions

- **Rule:** Wire format is `snake_case` everywhere. Add `#[serde(rename_all = "snake_case")]` to every struct that crosses the wire — request DTOs, response DTOs, event payloads.
  - **Why:** The API contract (see `../api-contract.md`) commits to snake_case. The Rust default is field-name verbatim, which produces inconsistent casing once a field happens to be a multi-word name.
  - **Example:**
    ```rust
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub struct CreateRunRequest {
        pub track_id: TrackId,
        pub time_ms: RaceTimeMs,
        pub disqualified: bool,
    }
    ```
  - **Source:** <https://serde.rs/container-attrs.html#rename_all>

- **Rule:** Separate request DTOs from response DTOs from entity types. Don't `#[derive(Serialize, Deserialize)]` on SeaORM entities and ship them directly.
  - **Why:** Entities are the DB shape, not the API shape. Direct serialization leaks every column (including ones you'd never want exposed, e.g., `password_hash`), and binds the wire format to schema decisions. Define explicit DTOs.

- **Rule:** Newtypes that cross the wire need transparent serialization. With `nutype`'s `Serialize`/`Deserialize` derives this is automatic; for hand-rolled newtypes, add `#[serde(transparent)]` on the struct.
  - **Why:** Without transparent, `UserId(uuid)` serializes as `{"0": "abc"}` instead of `"abc"`. That breaks the API contract silently.
  - **Source:** <https://serde.rs/container-attrs.html#transparent>

- **Rule:** For optional response fields, use `Option<T>` with `#[serde(skip_serializing_if = "Option::is_none")]`. Don't ship `null` fields when there's nothing to say.
  - **Why:** Cleaner JSON for the frontend; smaller payloads; "absent" and "explicitly null" stay distinct.

- **Rule:** For optional input fields, prefer `#[serde(default)]` over `Option<T>` when there is a sensible default. Use `Option<T>` when "not provided" is semantically distinct from "default."
  - **Why:** `default` collapses missing-field and explicit-default into the same code path; `Option<T>` lets the handler distinguish them. Pick on intent, not convenience.

- **Rule:** Use `#[serde(deny_unknown_fields)]` on request DTOs.
  - **Why:** Catches frontend typos that would otherwise be silently dropped. A request with `{"trakc_id": ...}` should fail loudly, not succeed with a missing `track_id` value.
  - **Source:** <https://serde.rs/container-attrs.html#deny_unknown_fields>

## 15. Cargo.toml hygiene

- **Rule:** Pin to the minor version where stability matters: `tokio = "1.40"`, not `tokio = "1"`. For utility crates with strong semver discipline (serde, anyhow), the major-only pin is fine.
  - **Why:** A `1.x` pin accepts every minor release; a `1.40` pin accepts only `>=1.40, <2.0`. The minor-pin is conservative against accidental breaking changes (which do happen even within semver-minor in some crates).
  - **Source:** <https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html>

- **Rule:** Prefer fewer dependencies. Each new dep is a supply-chain surface, an audit obligation, and a build-time cost.
  - **Why:** A 50-dep project takes seconds to compile; a 500-dep project takes minutes. Compile time is engineering velocity. Every dep's transitive closure is your closure.

- **Rule:** Group features explicitly, not via `default-features`. Any third-party feature flag should be turned on by name in the `[dependencies]` block.
  - **Example:**
    ```toml
    sea-orm = { version = "1", default-features = false, features = [
        "sqlx-sqlite", "runtime-tokio-rustls", "macros",
    ] }
    ```
  - **Why:** `default-features = true` (the default) silently pulls in whatever the crate decides to default. Being explicit makes upgrades safer.

- **Rule:** Run `cargo update` periodically (weekly is fine) and `cargo audit` in CI. Open issues for any advisory.
  - **Source:** <https://github.com/rustsec/rustsec/tree/main/cargo-audit>

- **Rule:** Don't depend on yanked crates or unmaintained crates. RustSec advisories cover this; `cargo audit` will flag.

- **Rule:** Use `[workspace]` to share dependency versions and lints across crates. Add new crates to `members` explicitly.
  - **Source:** <https://doc.rust-lang.org/cargo/reference/workspaces.html>

## 16. rustfmt and editorconfig

- **Rule:** Commit a `rustfmt.toml` to the repo root. Use defaults plus an explicit `max_width = 100` and `imports_granularity = "Crate"`.
  - **Why:** Default rustfmt evolves over editions; pinning the config makes formatter behavior reproducible. `max_width = 100` is the de-facto Rust community standard.
  - **Example (`rustfmt.toml`):**
    ```toml
    edition = "2024"
    max_width = 100
    imports_granularity = "Crate"
    group_imports = "StdExternalCrate"
    use_field_init_shorthand = true
    ```
  - **Source:** <https://rust-lang.github.io/rustfmt/>

- **Rule:** Run `cargo fmt` before every commit. CI runs `cargo fmt --check` and fails if anything is unformatted.

- **Rule:** Commit a `.editorconfig` covering non-Rust files (Markdown, TOML, YAML). LF line endings, UTF-8, two-space indent for YAML/JSON, four for everything else.
  - **Source:** <https://editorconfig.org/>

## 17. Config and environment handling

- **Rule:** Configuration is a single typed `Config` struct, loaded once at startup. No `env::var(...)` reads scattered through service code.
  - **Why:** A typed `Config` documents what's configurable in one place, fails fast on missing required values at startup, and is mockable in tests. Scattered `env::var` reads are testability poison and surface-area sprawl.
  - **Example:**
    ```rust
    pub struct Config {
        pub database_url: String,
        pub jwt_secret: SecretString,
        pub access_token_ttl: Duration,
        pub refresh_token_ttl: Duration,
        pub upload_dir: PathBuf,
        pub admin_user_id: Option<UserId>,
    }
    impl Config {
        pub fn from_env() -> Result<Self, ConfigError> { /* ... */ }
    }
    ```

- **Rule:** Required values fail fast at startup with a clear error message naming the missing variable. Optional values get a documented default.
  - **Why:** A server that starts with a missing `JWT_SECRET` and only fails on the first login is much harder to diagnose than one that refuses to start.

- **Rule:** `dotenvy` is dev-only. Gate it on `cfg(debug_assertions)` or an explicit env flag; production loads from the orchestration layer (Docker compose env, k8s secrets), never from a `.env` file in the container.
  - **Why:** `.env` files in production are an anti-pattern — they undermine secret-rotation tooling and tend to leak via container images.
  - **Source:** <https://docs.rs/dotenvy>

- **Rule:** Secrets (JWT key, DB password) live in a wrapper type that doesn't `Debug`/`Display` the value. Use `secrecy::SecretString` or equivalent.
  - **Why:** A `SecretString` accidentally printed via `tracing` shows `[REDACTED]`, not the value. Without it, one stray `info!("config = {:?}", config)` ships your JWT key to the log aggregator.
  - **Source:** <https://docs.rs/secrecy>

## 18. Feature flags

- **Rule:** Every `#[cfg(feature = "...")]` site has a comment naming the feature and explaining what it gates.
  - **Why:** A stray `#[cfg(feature = "foo")]` with no context is a maintenance hazard — nobody knows whether it's safe to delete or what activates it. The comment is the documentation.
  - **Example:**
    ```rust
    // Feature: ocr-stub. Compiled in only when the OCR pipeline is included
    // (Phase 7+). Real implementation lives in services/ocr.rs.
    #[cfg(feature = "ocr-stub")]
    pub fn extract_time(_image: &[u8]) -> Option<RaceTimeMs> { None }
    ```

- **Rule:** Don't rely on default features in your own crate's `Cargo.toml`. Set `default = []` and turn features on by name where used.
  - **Why:** Hidden defaults make feature interactions confusing. Explicit feature lists are auditable.

- **Rule:** Test all reasonable feature combinations in CI. For a small project, "all features off" + "all features on" + "default features" is usually enough.
  - **Source:** <https://doc.rust-lang.org/cargo/reference/features.html>

---

## Reading list

Anyone modifying Rust code in this repo should have read at least the first three:

1. Rust API Guidelines — <https://rust-lang.github.io/api-guidelines/>
2. Luca Palmieri, "Error Handling in Rust" — <https://www.lpalmieri.com/posts/error-handling-rust/>
3. burntsushi, "Using `unwrap()` in Rust is Okay" — <https://burntsushi.net/unwrap/>
4. Effective Rust — <https://www.lurklurk.org/effective-rust/>
5. nutype docs — <https://docs.rs/nutype>
6. Embark Studios, lints.rs — <https://github.com/EmbarkStudios/rust-ecosystem/blob/main/lints.rs>

## Document history

- 2026-05-02 — Initial draft as part of `docs/rust-coding-standards.md`.
- 2026-05-02 — Split into `docs/coding-standards/rust.md`. Added: file length, serde, Cargo.toml hygiene, rustfmt, config/env, feature flags. Tightened type-driven design to commit to `nutype` + entity-boundary conversion. Added "tests trace to requirements, not lines" rule to testing. Added "re-review docs on PR" rule to documentation.
- 2026-05-04 — Clarified § 8 unwrap/expect rule for test modules vs integration test files: lib code uses `cfg_attr(test, allow(...))`, files under `tests/` use bare `#![allow(...)]` since `cfg(test)` is unconditionally true there. Surfaced during PR-A1 (#24) review.
- 2026-05-08 — Repaired the broken Rust stdlib doc URL on the `NonZeroI32` rule: `struct.NonZeroI32.html` → `type.NonZeroI32.html` (the type became a type alias for `NonZero<i32>` in Rust 1.79, so the old struct page no longer exists). PR 5 of the docs restructure (plan deviation — surfaced when lychee `fail: true` flipped on).
- 2026-05-09 — Added § 1 rule on capital-first error messages with no trailing punctuation. Codifies the convention already in force across the 30+ `AppError::{BadRequest,Unauthorized,Forbidden,NotFound,Conflict}` call sites and extends it to anyhow contexts and `anyhow!` synthetic messages (a deliberate divergence from anyhow's lowercase-context docs convention). Surfaced during PR #107 (PR-C2) review.
- 2026-05-10 — Updated § 11's `main.rs` startup sub-rule to prefer `fn main() -> anyhow::Result<()>` + `?` + `.context(...)` over `.expect(...)`/`.unwrap()`. The previous wording predated PR-A1's lint config and PR-C2's anyhow foundation; with both in place, the anyhow form is clippy-clean without per-file allows and yields a richer error chain. `expect(...)` retained as the rare-case fallback for genuinely-infallible sites. Surfaced during PR #108 (PR-H1+ a) review; tracking Issue [#109](https://github.com/brendanbyrne/beerio-kart/issues/109).
- 2026-05-13 — § 10: added a rule covering cardinality of attacker-controlled span fields on pre-auth handlers (`/login`, `/register`, `/refresh`). Recording `username` etc. is correct — it's the only correlator before `user_id` exists — but flagged the log-aggregator cost trap and the standard mitigation (per-event field-drop on failed-auth, not handler-wide). Surfaced as a 🔵 Suggestion during PR #144 (PR-F5) review.
- 2026-05-14 — § 13: refreshed the example-split to match the shape that actually landed in PR [#150](https://github.com/brendanbyrne/beerio-kart/pull/150) (PR-G4). The prior example named two submodules that don't exist (`rulesets.rs`, `cleanup.rs`) and omitted the four that do (`lifecycle.rs`, `detail.rs`, `races.rs`, `types.rs`). The revision also calls out the `types.rs` layer as the cycle-breaker — without a shared-DTO sibling, the read aggregator and the race mutations form a `detail ↔ races` cycle on the shared race DTO. Surfaced as a 🔵 Suggestion during PR #150 review.
- 2026-05-15 — Updated the Companions list: dropped the `../compliance-plan.md` companion (now archived at `../designs/archive/compliance-plan.md`) and called it out explicitly on its own line as an archived companion. Companion to PR [#160](https://github.com/brendanbyrne/beerio-kart/pull/160) / Issue [#159](https://github.com/brendanbyrne/beerio-kart/issues/159).
