# Async Rust & Tokio — Coding Standards

> **Scope.** Async Rust and Tokio practice for the `beerio-kart` backend. Stack: Axum 0.8, Tokio multi-thread, Rust edition 2024.
> **Format.** *Rule / Why / Example / Source*.
> **Companions.** `rust.md`, `seaorm.md`, `../api-contract.md`, `../compliance-plan.md`.

The rules are ordered roughly by how much damage breaking them causes. The first four sections — runtime choice, blocking, sync primitives across `.await`, and channels — catch the most production bugs in async Rust.

## Index

1. [Runtime choice](#1-runtime-choice)
2. [Blocking operations](#2-blocking-operations)
3. [Synchronization primitives across `.await`](#3-synchronization-primitives-across-await)
4. [Channels](#4-channels)
5. [Spawning and structured concurrency](#5-spawning-and-structured-concurrency)
6. [Cancellation](#6-cancellation)
7. [`select!` pitfalls](#7-select-pitfalls)
8. [Background tasks](#8-background-tasks)
9. [Send / Sync / `'static`](#9-send--sync--static)
10. [`#[tracing::instrument]` and async](#10-tracinginstrument-and-async)
11. [Common async pitfalls](#11-common-async-pitfalls)
12. [Backpressure and resource limits](#12-backpressure-and-resource-limits)
13. [Shutdown](#13-shutdown)

---

## 1. Runtime choice

The runtime is the engine that polls futures. Picking the wrong flavor, or constructing it badly, makes every other rule less effective.

- **Rule:** Use `#[tokio::main]` (multi-thread by default) for the binary, and never construct a second runtime inside a task.
  - **Why:** A multi-thread runtime is correct for an HTTP server: work-stealing balances request load across cores, and it's the only flavor on which `block_in_place` is legal. Nested runtimes panic — `block_on` panics if called inside an async context.
  - **Example:**
    ```rust
    // Do.
    #[tokio::main]
    async fn main() -> anyhow::Result<()> {
        let app = build_router();
        let listener = TcpListener::bind("0.0.0.0:3000").await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
    // Don't — nested runtime in a handler.
    async fn handler() {
        let rt = tokio::runtime::Runtime::new().unwrap(); // panics or starves
        rt.block_on(some_future());
    }
    ```
  - **Source:** <https://docs.rs/tokio/latest/tokio/attr.main.html>

- **Rule:** Every `async fn` reachable from `tokio::spawn` must be `Send + 'static`. (See § 9 for what to do about `'static`.)
  - **Why:** The multi-thread runtime moves tasks between worker threads at any `.await`. Anything held across an `.await` (locals, captures, `MutexGuard`s) becomes part of the future's state machine and must be `Send`. The compiler error "future cannot be sent between threads safely" usually means an `Rc`, `RefCell`, `*mut T`, or `std::sync::MutexGuard` is alive across an `.await`.
  - **Source:** <https://docs.rs/tokio/latest/tokio/task/fn.spawn.html>

- **Rule:** If you genuinely need `!Send` state, use `LocalSet` with `spawn_local` — not `tokio::spawn`.
  - **Source:** <https://docs.rs/tokio/latest/tokio/task/struct.LocalSet.html>

- **Rule:** Don't reconfigure the runtime with `Builder` unless you have a measured reason. If you do, set `worker_threads` and `max_blocking_threads` explicitly.

## 2. Blocking operations

> **Definition (Alice Ryhl):** "Blocking" is any operation that takes "substantially longer than nanoseconds" without an `.await`. Rule of thumb: if a call can run for **more than ~10–100 µs** between awaits, treat it as blocking.

A single blocking call on a worker thread starves every other task scheduled on it until it returns. With 8 workers and 8 unlucky requests, the entire server stalls.

- **Rule:** Run Argon2 hashing and verification inside `tokio::task::spawn_blocking`.
  - **Why:** Argon2 is deliberately CPU- and memory-hard — typical parameters take 50–500 ms per hash. Two-to-four orders of magnitude over the 100 µs budget. On an async worker, every login freezes the server for the duration.
  - **Example:**
    ```rust
    pub async fn hash_password(pw: String) -> Result<String, AppError> {
        tokio::task::spawn_blocking(move || {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(pw.as_bytes(), &salt)
                .map(|h| h.to_string())
                .map_err(AppError::from)
        })
        .await
        .map_err(|_| AppError::Internal("argon2 task panicked".into()))?
    }
    ```
  - **Source:** <https://ryhl.io/blog/async-what-is-blocking/>, <https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html>

- **Rule:** Prefer `spawn_blocking` over `block_in_place` by default.
  - **Why:** `block_in_place` runs blocking code on the current worker after handing other tasks off to a fresh worker — only on the multi-thread runtime, and any other code in the same task is suspended for the whole call. `spawn_blocking` moves the work to the dedicated blocking pool (default 512 threads).
  - **Source:** <https://docs.rs/tokio/latest/tokio/task/fn.block_in_place.html>

- **Rule:** For long pure-CPU loops that never `.await`, insert `tokio::task::yield_now().await` (or move the whole loop to `spawn_blocking`).
  - **Why:** Tokio's coop budget hands each task ~128 resource operations per tick. CPU-only loops never touch the budget, so the runtime cannot preempt them. A `yield_now()` per chunk keeps tail latency in line.
  - **Source:** <https://tokio.rs/blog/2020-04-preemption>

- **Rule:** Use `tokio::fs` for convenience, not for performance.
  - **Why:** Every `tokio::fs` call is a `spawn_blocking` under the hood. API-shape async, not faster than `std::fs`. For batched reads/writes, one `spawn_blocking` around a `std::fs` block is more efficient.
  - **Source:** <https://docs.rs/tokio/latest/tokio/fs/index.html>

- **Rule:** Mark "needs offload" sites with a comment so reviewers can spot them. (`// CPU-bound: spawn_blocking`)

## 3. Synchronization primitives across `.await`

This is the single largest source of mysterious deadlocks in async Rust. Read this section twice.

- **Rule:** Never hold a `std::sync::Mutex`, `parking_lot::Mutex`, or `RefCell` borrow across an `.await`.
  - **Why:** A blocking guard captured in the future's state machine pins the lock for the entire time the task is parked — across IO, scheduling, every other task contending for the same lock. On the multi-thread runtime the future may resume on a different thread, which makes a re-lock from the same logical task explode unpredictably. Clippy's `await_holding_lock` (and `await_holding_refcell_ref`) catches the obvious cases.
  - **Example:**
    ```rust
    // Don't — guard outlives the .await.
    let g = state.lock().unwrap();
    let val = some_async_lookup(&g.key).await; // BAD
    drop(g);
    // Do — drop the guard before .await.
    let key = {
        let g = state.lock().unwrap();
        g.key.clone()
    }; // guard dropped here
    let val = some_async_lookup(&key).await;
    ```
  - **Source:** <https://rust-lang.github.io/rust-clippy/master/index.html#await_holding_lock>

- **Rule:** Default to `std::sync::Mutex` (or `parking_lot::Mutex`) for protecting plain data; reach for `tokio::sync::Mutex` only when the lock must span an `.await`.
  - **Why:** A blocking mutex's fast path is a single CAS. `tokio::sync::Mutex` allocates a wait queue and is observably slower under contention. Tokio's own docs say: "It is ok and often preferred to use the ordinary Mutex from the standard library in asynchronous code."
  - **Source:** <https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html#which-kind-of-mutex-should-you-use>

- **Rule:** When you must lock briefly inside async code, wrap it in a non-async helper method on the shared type.
  - **Why:** Encapsulating `lock()` / mutate / `drop` inside a synchronous `&self` method makes it impossible to accidentally hold the guard across an `.await`.
  - **Example:**
    ```rust
    impl Counters {
        pub fn record_hit(&self) { self.inner.lock().unwrap().hits += 1; }
    }
    ```

- **Rule:** Apply the same care to `RwLock`. `tokio::sync::RwLock::{read,write}` are not cancel-safe.

- **Rule:** Enable `clippy::await_holding_lock` and `clippy::await_holding_refcell_ref` at deny level (already in the lint block in `rust.md` § 8).

## 4. Channels

Tokio's `tokio::sync` module gives four channel types. Picking the wrong one quietly destroys backpressure.

- **Rule:** Use bounded `mpsc::channel(capacity)`. Never `mpsc::unbounded_channel` in production code paths.
  - **Why:** Unbounded channels delete backpressure: if the consumer falls behind, the producer keeps allocating until OOM. Bounded channels make the producer `.await` when full, which is the load signal you actually want.
  - **Example:**
    ```rust
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Event>(64);
    tx.send(event).await?; // yields under backpressure
    // Or: prefer-to-drop-on-overload
    match tx.try_send(event) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => metrics::dropped_events.inc(),
        Err(TrySendError::Closed(_)) => return Err(AppError::Internal("rx closed".into())),
    }
    ```
  - **Source:** <https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html>

- **Rule:** Pick the channel type that matches cardinality. `oneshot` for request/response, `broadcast` for fan-out where every subscriber sees every message and lag is tolerated, `watch` for a single-value latest-only state (config reload, current leader, shutdown signal).
  - **Source:** <https://tokio.rs/tokio/tutorial/channels>

- **Rule:** Treat `send` errors and `recv` returning `None` as the canonical shutdown signal — don't ignore them.
  - **Example:**
    ```rust
    while let Some(msg) = rx.recv().await { handle(msg).await; }
    // recv returned None — exit cleanly.
    ```

- **Rule:** `broadcast` consumers must handle `RecvError::Lagged`.
  - **Source:** <https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html>

## 5. Spawning and structured concurrency

- **Rule:** Treat every `tokio::spawn` call site as a deliberate "this task is detached" decision. If the lifetime should match the caller's, use `JoinSet`.
  - **Example:**
    ```rust
    let mut set = tokio::task::JoinSet::new();
    for input in inputs { set.spawn(process(input)); }
    while let Some(res) = set.join_next().await {
        let value = res??; // outer ? = JoinError, inner ? = task error
        handle(value);
    }
    // Dropping `set` aborts unfinished tasks.
    ```
  - **Source:** <https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html>

- **Rule:** For long-lived background tasks that survive the spawner but participate in shutdown, use `tokio_util::task::TaskTracker` + `CancellationToken`.
  - **Why:** `TaskTracker` doesn't abort on drop — its job is to let main `await` for everything to wind down. `CancellationToken` is how you tell tasks to wind down. Together they implement "tell, then wait" graceful shutdown.
  - **Source:** <https://docs.rs/tokio-util/latest/tokio_util/task/task_tracker/struct.TaskTracker.html>

- **Rule:** Use `tokio::join!` / `try_join!` for "wait for N concurrent things in one task"; use `select!` for "race N things, take the first."
  - **Why:** `join!` and `try_join!` keep the futures on the same task — no `Send + 'static` overhead, no spawn — so they can borrow from the surrounding scope. The canonical pattern for "concurrent borrows" without giving up `'static` (see § 9).

- **Rule:** When you must detach a task, spawn a wrapper that logs panics and errors.
  - **Why:** A panicking spawned task that nobody awaits is silent — Tokio catches the panic and stores it in the dropped `JoinHandle`. You find out at 3 a.m. that the cleanup task hasn't run for two weeks.
  - **Example:**
    ```rust
    fn spawn_supervised<F, T>(name: &'static str, fut: F) -> JoinHandle<()>
    where F: Future<Output = anyhow::Result<T>> + Send + 'static, T: Send + 'static {
        tokio::spawn(async move {
            match AssertUnwindSafe(fut).catch_unwind().await {
                Ok(Ok(_)) => tracing::info!(task = name, "exited cleanly"),
                Ok(Err(e)) => tracing::error!(task = name, ?e, "task failed"),
                Err(_) => tracing::error!(task = name, "task panicked"),
            }
        })
    }
    ```

## 6. Cancellation

Drop equals cancellation in async Rust.

- **Rule:** A future is *cancel-safe* iff dropping it before completion is a no-op semantically. Audit any future used inside `tokio::select!` for this property.
  - **Source:** <https://docs.rs/tokio/latest/tokio/macro.select.html#cancellation-safety>

- **Rule:** Know which Tokio operations are *not* cancel-safe.
  - Several operations lose state on drop because they use a fairness queue: `tokio::sync::Mutex::lock`, `RwLock::{read,write}`, `Semaphore::acquire`, `Notify::notified`. Partial writes (`AsyncWriteExt::write_all`) are not cancel-safe — bytes already written stay written. `mpsc::Receiver::recv` *is* cancel-safe.

- **Rule:** Use `CancellationToken` for cooperative cancellation, not ad-hoc bool flags.
  - **Why:** A token is cheap to clone, supports `.cancelled().await` (cancel-safe), and supports parent/child trees.
  - **Example:**
    ```rust
    async fn worker(cancel: CancellationToken, mut rx: mpsc::Receiver<Job>) {
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break,
                maybe_job = rx.recv() => match maybe_job {
                    Some(job) => process(job).await,
                    None => break,
                },
            }
        }
    }
    ```

- **Rule:** When cancellation safety is hard, extract the un-safe step into its own task and communicate via cancel-safe channels.

## 7. `select!` pitfalls

- **Rule:** Inside a loop, pin long-lived futures *outside* the macro so they survive across iterations.
  - **Example:**
    ```rust
    // Don't — `do_work()` restarts every iteration.
    loop {
        tokio::select! {
            _ = do_work() => break,
            msg = rx.recv() => handle(msg),
        }
    }
    // Do — pin once, re-poll the same future.
    let work = do_work();
    tokio::pin!(work);
    loop {
        tokio::select! {
            _ = &mut work => break,
            msg = rx.recv() => handle(msg),
        }
    }
    ```

- **Rule:** Default branch order is random. Use `biased;` only when you have a reason and document it.
  - **Example:**
    ```rust
    tokio::select! {
        biased; // shutdown wins ties — explicit priority
        _ = cancel.cancelled() => return,
        msg = rx.recv() => handle(msg).await,
    }
    ```

- **Rule:** Don't `select!` over an `async` block that takes a Tokio sync lock.

- **Rule:** Prefer 2–3 branches per `select!`. Refactor larger ones into actors / state machines.

## 8. Background tasks

The 5-minute "close stale sessions" task is the canonical example for this project.

- **Rule:** Background tasks live behind a `loop { select! }` over their work-trigger and a `CancellationToken`.
  - **Example:**
    ```rust
    pub async fn session_cleanup_loop(db: DbConn, cancel: CancellationToken) {
        let mut tick = tokio::time::interval(Duration::from_secs(5 * 60));
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break,
                _ = tick.tick() => {
                    if let Err(e) = close_stale_sessions(&db).await {
                        tracing::error!(?e, "session cleanup failed");
                    }
                }
            }
        }
        tracing::info!("session cleanup task exited");
    }
    ```

- **Rule:** Configure `interval` with `MissedTickBehavior::Skip` (or `Delay`).
  - **Why:** Default is `Burst`, which fires N times rapidly to "catch up." For idempotent cleanup, that's wasted work.

- **Rule:** Spawn background tasks via `TaskTracker::spawn` and don't store the `JoinHandle` in handlers.

- **Rule:** Always log on background-task entry and exit, with the task name.

## 9. Send / Sync / `'static`

- **Rule:** Do not hold non-`Send` types across `.await`. Usual suspects: `Rc`, `RefCell`'s `Ref`/`RefMut`, raw pointers, `MutexGuard` from a `!Send` mutex.

- **Rule:** Shared state across tasks must be `Send + Sync`, wrapped in `Arc`.
  - **Why:** Axum's `State<T>` requires `T: Clone + Send + Sync + 'static`. Anything in app state inherits these bounds.

- **Rule:** When a `Send` error points at a guard or borrow, scope it instead of `drop()`-ing it. Block scopes document the critical section visually.

- **Rule:** `'static` is the price of admission for `tokio::spawn`. Don't reach for `async-scoped` or similar crates as a workaround — they have known soundness caveats around `mem::forget` and require `unsafe` to use safely. Share state via `Arc`, clone into the task.
  - **Why:** Scoped *synchronous* threads (`std::thread::scope`) work because the scope function guarantees join before return; you can't `mem::forget` a function call. Scoped *async* tasks return a value (a scope handle), which can be forgotten — leaving spawned futures still holding borrows into a stack frame that's about to disappear. The trilemma is: "tasks-can-borrow + cancel-on-drop + safe-against-mem::forget" — you can't have all three in safe Rust today.
  - **Workaround:** For "concurrent borrows in one task" (the most common case people reach for scoped tasks for), use `tokio::try_join!` or `JoinSet::spawn_local` inside a `LocalSet`. Both keep work on the same task, no spawn involved, ordinary borrows work.
  - **Example:**
    ```rust
    // Concurrent borrows without spawning — `&db` is fine across all three.
    async fn fan_out(db: &DbConn) -> Result<(), AppError> {
        let (a, b, c) = tokio::try_join!(
            query_a(db),
            query_b(db),
            query_c(db),
        )?;
        Ok(())
    }
    ```
  - **Source:** <https://without.boats/blog/the-scoped-task-trilemma/>

## 10. `#[tracing::instrument]` and async

- **Rule:** Annotate every public `async fn` in services and handlers with `#[tracing::instrument]`, and `skip` non-`Debug` or large arguments.
  - **Example:**
    ```rust
    #[tracing::instrument(skip(db, password), fields(user_id = %user_id))]
    pub async fn verify_login(
        db: &impl ConnectionTrait,
        user_id: UserId,
        password: SecretString,
    ) -> Result<(), AppError> { /* logs inside carry user_id automatically */ }
    ```

- **Rule:** When you spawn a task that should inherit the current span, attach it explicitly with `.in_current_span()` or `.instrument(span)`.
  - **Why:** `tokio::spawn` does not propagate spans automatically.
  - **Example:** `tokio::spawn(do_work().in_current_span());`

- **Rule:** Don't `instrument` trivial helpers in tight loops.

## 11. Common async pitfalls

- **Rule:** Every `async fn` call must be `.await`-ed (or explicitly stored as a future). The compiler issues `unused_must_use`; do not silence it.

- **Rule:** Recursive `async fn` requires boxing. Use `Box::pin(self.recurse(args)).await` for one-offs; reach for `async-recursion` when boxing every site is ugly.

- **Rule:** Native `async fn` in traits is the default since Rust 1.75. Use it for static dispatch (the common case). Reach for `trait-variant::make` when you need to add a `Send` bound on the returned future for spawning. Fall back to `#[async_trait]` only when you genuinely need `dyn Trait`.
  - **Why:** Native is the future; `#[async_trait]` boxes the returned future and adds an allocation per call. The known limitation of native is that you can't yet write generic bounds requiring the future be `Send`; `trait-variant` works around that for the common spawn case.
  - **Source:** <https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/>, <https://docs.rs/trait-variant>

- **Rule:** If you're fighting the borrow checker across `.await`, the answer is almost never `Box`. Reduce what's held across the suspension point — pull the borrow's lifetime in to before the await, clone, or restructure.

## 12. Backpressure and resource limits

- **Rule:** Wrap every external call (DB, HTTP) in `tokio::time::timeout`. Service-layer functions don't always run in HTTP-request context (background tasks, future CLI tools), so per-call timeouts give site-specific budgets that Tower middleware can't replicate. Defense in depth: pool acquire timeout + per-call timeout + request-level timeout, all three.
  - **Why:** Without a timeout, a stuck call holds a connection, a permit, and the calling task forever. SeaORM's transaction methods will wait indefinitely if SQLite is locked. Per-call also lets auth verification have a 1s ceiling where a stats aggregation gets 5s — Tower's `TimeoutLayer` is uniform.
  - **Example:**
    ```rust
    let row = tokio::time::timeout(
        Duration::from_secs(2),
        users::Entity::find_by_id(id.to_string()).one(db),
    ).await
        .map_err(|_| AppError::Internal("db timeout".into()))??;
    ```
  - **Note on boilerplate:** If `tokio::time::timeout(Duration::from_secs(2), ...)` becomes pervasive, introduce a thin helper macro `db_timeout!` or function wrapper to keep call sites tidy without losing the per-call budget knob.

- **Rule:** Cap concurrent expensive operations with a `tokio::sync::Semaphore`.
  - **Why:** Argon2 hashing on `spawn_blocking` is bounded only by the blocking pool size (default 512). Login storms can exhaust that for unrelated traffic. Front login with a semaphore (e.g., 16 concurrent hashes).
  - **Example:**
    ```rust
    let permit = state.argon2_limit.acquire().await?;
    let hash = tokio::task::spawn_blocking(move || verify(pw, hash)).await??;
    drop(permit);
    ```

- **Rule:** Use Tower middleware for request-level limits: `tower::timeout::TimeoutLayer`, `tower::limit::ConcurrencyLimitLayer`, `tower_http::limit::RequestBodyLimitLayer`.

- **Rule:** For rate limiting, use `tower-governor`, not `tower::limit::RateLimitLayer`.
  - **Why:** `RateLimitLayer` produces a service that isn't `Clone`, which Axum requires. Wrapping it in `BufferLayer` defeats the purpose.
  - **Source:** <https://github.com/benwis/tower-governor>

- **Rule:** Set a connection-acquire timeout on the SeaORM connection pool. (See `seaorm.md` § 8.)

## 13. Shutdown

- **Rule:** Implement shutdown as: (1) signal source — typically `tokio::signal::ctrl_c()`; (2) propagation — `CancellationToken` shared across all tasks; (3) wait — `TaskTracker::wait()` with a hard timeout.
  - **Example:**
    ```rust
    let cancel = CancellationToken::new();
    let tracker = TaskTracker::new();
    tracker.spawn(session_cleanup_loop(db.clone(), cancel.clone()));
    tracker.close();

    let listener = TcpListener::bind(addr).await?;
    let shutdown = {
        let cancel = cancel.clone();
        async move {
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("ctrl-c received, shutting down");
            cancel.cancel();
        }
    };
    axum::serve(listener, app).with_graceful_shutdown(shutdown).await?;

    match tokio::time::timeout(Duration::from_secs(20), tracker.wait()).await {
        Ok(()) => tracing::info!("clean shutdown"),
        Err(_) => tracing::warn!("shutdown timed out, abandoning tasks"),
    }
    ```
  - **Source:** <https://tokio.rs/tokio/topics/shutdown>

- **Rule:** Handle SIGTERM in addition to SIGINT on Linux deployments.
  - **Why:** Container orchestrators (Docker, k8s, systemd) send SIGTERM, not SIGINT.
  - **Example:**
    ```rust
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(SignalKind::terminate())
            .expect("install SIGTERM handler").recv().await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate => {}
    }
    ```

- **Rule:** Document, per task, what "in-flight work" means on shutdown. A request mid-`.await` when shutdown timeout hits gets its future dropped. For each task that mutates external state, write down whether the work is idempotent or transactional.

---

## Quick lint checklist (CI gate)

Cross-referenced from `rust.md` § 8:

```toml
[lints.clippy]
await_holding_lock = "deny"
await_holding_refcell_ref = "deny"
unused_async = "warn"
mem_forget = "deny"
```

These make the most catastrophic async bugs (lock held across await, RefCell ref held across await) fail CI rather than ship.

---

## Reading list

Anyone modifying async code in this repo should have read at least the first three:

1. Alice Ryhl, "Async: What is blocking?" — <https://ryhl.io/blog/async-what-is-blocking/>
2. Tokio docs, `tokio::select!` — Cancellation safety section — <https://docs.rs/tokio/latest/tokio/macro.select.html#cancellation-safety>
3. Tokio tutorial, Shared State — <https://tokio.rs/tokio/tutorial/shared-state>
4. Tokio topic, Graceful Shutdown — <https://tokio.rs/tokio/topics/shutdown>
5. Without Boats, "The Scoped Task Trilemma" — <https://without.boats/blog/the-scoped-task-trilemma/>
6. Tokio blog, "Reducing tail latencies with automatic cooperative task yielding" — <https://tokio.rs/blog/2020-04-preemption>

---

## Document history

- 2026-05-02 — Initial draft as part of `docs/rust-coding-standards.md`.
- 2026-05-02 — Split into `docs/coding-standards/tokio.md`. Added `'static` discussion + scoped-task-trilemma reference in § 9. Held § 12 timeout rule strict (per project's "no corner cutting" stance). Took position on async traits in § 11 (native + trait-variant for spawn).
