//! Graceful-shutdown wiring for the HTTP server and background tasks.
//!
//! See `coding-standards/tokio.md` § 5 (`TaskTracker` for long-lived tasks),
//! § 8 (background task shape), and § 13 (the canonical
//! signal → cancel → wait sequence).

use std::{future::Future, panic::AssertUnwindSafe, time::Duration};

use futures_util::FutureExt;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

/// Build the shutdown-signal future for `axum::serve(...).with_graceful_shutdown(...)`.
///
/// Resolves when Ctrl-C **or** SIGTERM (Unix) is received, then calls
/// [`CancellationToken::cancel`] so every tracked background task can wind
/// down via its own `cancel.cancelled()` branch.
///
/// SIGTERM-handler installation can fail; that's surfaced as an `io::Error`
/// from this constructor rather than from inside the returned future.
/// `with_graceful_shutdown` only accepts `impl Future<Output = ()>`, so
/// failing fast at install time keeps `main`'s error path normal.
///
/// # Errors
///
/// Returns the OS error from `tokio::signal::unix::signal` on Unix if the
/// SIGTERM handler cannot be installed (typically a missing capability or a
/// resource limit). On non-Unix platforms this is currently infallible.
pub fn signal(
    cancel: CancellationToken,
) -> std::io::Result<impl std::future::Future<Output = ()> + Send + 'static> {
    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    Ok(async move {
        #[cfg(unix)]
        tokio::select! {
            res = tokio::signal::ctrl_c() => match res {
                Ok(()) => tracing::info!("ctrl-c received, shutting down"),
                Err(e) => tracing::error!(?e, "ctrl-c handler error, shutting down anyway"),
            },
            _ = sigterm.recv() => tracing::info!("sigterm received, shutting down"),
        }
        #[cfg(not(unix))]
        match tokio::signal::ctrl_c().await {
            Ok(()) => tracing::info!("ctrl-c received, shutting down"),
            Err(e) => tracing::error!(?e, "ctrl-c handler error, shutting down anyway"),
        }
        cancel.cancel();
    })
}

/// Wait for all [`TaskTracker`]-spawned tasks to wind down, with a hard timeout.
///
/// Call this *after* `axum::serve(...).with_graceful_shutdown(...)` returns —
/// the cancellation token has been triggered by then, so each tracked
/// background task should observe its `cancel.cancelled()` branch and exit.
/// The caller is responsible for `tracker.close()`-ing before calling this;
/// `wait()` doesn't close so it stays composable.
pub async fn wait(tracker: TaskTracker, timeout: Duration) {
    if tokio::time::timeout(timeout, tracker.wait()).await.is_ok() {
        tracing::info!("clean shutdown");
    } else {
        tracing::warn!(
            timeout_secs = timeout.as_secs(),
            "shutdown timed out, abandoning tasks"
        );
    }
}

/// Wrap a background task with entry/exit logs and panic capture.
///
/// Per `coding-standards/tokio.md` § 5 ("spawn a wrapper that logs panics
/// and errors") and § 8 ("Always log on background-task entry and exit,
/// with the task name"). Detached tasks lose panics silently — Tokio
/// catches the panic and stores it in the dropped `JoinHandle`. Wrapping
/// every `tracker.spawn` with this helper turns a 3 a.m. mystery into a
/// log line.
///
/// `name` is the task identifier emitted as the `task = ...` field on
/// every log; `fut` is the task body. The wrapper is intentionally
/// future-only (not spawn-and-handle) so the caller can compose it with
/// `tracker.spawn(supervised(...))` without dragging `TaskTracker` into
/// this helper's signature.
pub async fn supervised<F>(name: &'static str, fut: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tracing::info!(task = name, "started");
    if AssertUnwindSafe(fut).catch_unwind().await.is_ok() {
        tracing::info!(task = name, "exited cleanly");
    } else {
        tracing::error!(task = name, "task panicked");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;

    #[tokio::test]
    async fn test_wait_returns_immediately_for_closed_empty_tracker() {
        let tracker = TaskTracker::new();
        tracker.close();
        let start = tokio::time::Instant::now();
        // 1s is well above any real schedule jitter and well under the
        // production 20s budget — distinguishes "returned" from "timed out".
        wait(tracker, Duration::from_secs(1)).await;
        assert!(start.elapsed() < Duration::from_millis(500));
    }

    #[tokio::test]
    async fn test_wait_returns_after_tracked_task_observes_cancel() {
        let tracker = TaskTracker::new();
        let cancel = CancellationToken::new();
        let exited = Arc::new(AtomicBool::new(false));

        tracker.spawn({
            let cancel = cancel.clone();
            let exited = exited.clone();
            async move {
                cancel.cancelled().await;
                exited.store(true, Ordering::SeqCst);
            }
        });
        tracker.close();

        cancel.cancel();
        wait(tracker, Duration::from_secs(1)).await;
        assert!(exited.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_supervised_runs_clean_future_to_completion() {
        let ran = Arc::new(AtomicBool::new(false));
        let ran_inner = ran.clone();
        supervised("test-clean", async move {
            ran_inner.store(true, Ordering::SeqCst);
        })
        .await;
        assert!(ran.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_supervised_catches_panic_and_returns() {
        // If `supervised` failed to catch, this test would itself panic and
        // be reported as failed. Reaching the assertion proves the panic
        // was contained — the log path is exercised by hand via smoke tests.
        supervised("test-panic", async move {
            panic!("intentional panic for supervised() test");
        })
        .await;
    }

    #[tokio::test]
    async fn test_wait_times_out_for_stubborn_task() {
        let tracker = TaskTracker::new();
        // Task that ignores cancellation and outlives the budget.
        let handle = tracker.spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        tracker.close();

        let start = tokio::time::Instant::now();
        wait(tracker, Duration::from_millis(50)).await;
        // Timeout fired — we didn't wait for the stubborn task.
        assert!(start.elapsed() < Duration::from_secs(1));
        handle.abort();
    }
}
