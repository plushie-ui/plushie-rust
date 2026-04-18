//! Execution backends for plushie apps.
//!
//! `plushie::run::<A>()` dispatches to whichever backend is compiled in:
//!
//! - **Direct** (`direct` feature, default): Renders in-process using
//!   iced. No subprocess, no serialization.
//! - **Wire** (`wire` feature): Spawns a renderer binary and
//!   communicates over stdin/stdout. The binary path is discovered via
//!   the four-step chain: `PLUSHIE_BINARY_PATH`, custom build output
//!   (`target/plushie-renderer/`), downloaded stock binary
//!   (`target/plushie/bin/`), then `PATH`. Use
//!   [`crate::run_with_renderer`] to supply an explicit path.

#[cfg(feature = "direct")]
pub mod direct;

#[cfg(feature = "direct")]
mod effects;

#[cfg(any(feature = "direct", feature = "wire"))]
pub(crate) mod event_bridge;

#[cfg(any(feature = "direct", feature = "wire"))]
pub(crate) mod effect_tracker;

#[cfg(feature = "direct")]
mod queue_sink;

#[cfg(feature = "wire")]
pub mod bridge;

#[cfg(feature = "wire")]
pub(crate) mod env;

#[cfg(feature = "wire")]
pub mod socket;

#[cfg(feature = "wire")]
pub mod wire;

// Wire-binary discovery is used by `plushie::run` mode detection
// (socket / binary-path / mode-flag / feature default) and by the
// curated `run_spawn` entry point. Available whenever the `wire`
// feature is compiled in.
#[cfg(feature = "wire")]
pub(crate) mod wire_discovery;

// ---------------------------------------------------------------------------
// Shared helpers (direct + wire)
// ---------------------------------------------------------------------------

/// Platform-aware cooperative sleep for runner `Task::perform` futures.
///
/// Keeps the future cooperative so iced's executor can park it rather
/// than blocking a worker thread. `std::thread::sleep` inside an async
/// block would pin whichever executor thread drove the future.
///
/// Native builds use `tokio::time::sleep`. WASM builds would route
/// through `wasmtimer::tokio::sleep`, but the `plushie` crate has
/// native-only dependencies (rfd, arboard) so wasm32 isn't supported
/// here today. The cfg is future-proofed; if wasm32 ever lands, wire
/// in wasmtimer the same way `plushie-renderer-lib::emitter` does.
#[cfg(feature = "direct")]
#[allow(dead_code)]
pub(crate) async fn platform_sleep(duration: std::time::Duration) {
    tokio::time::sleep(duration).await;
}

/// Extract a human-readable message from a `catch_unwind` payload.
///
/// Matches the `&'static str` / `String` downcast pattern used in
/// `plushie-renderer/src/headless.rs` panic recovery.
#[cfg(any(feature = "direct", feature = "wire"))]
#[allow(dead_code)]
pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&'static str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
        .unwrap_or("(non-string panic)")
}

/// Run a user-supplied future and convert panics into structured errors.
///
/// Without this guard, a panic inside the user's async task unwinds the
/// executor worker, drops the result sender, and leaves the app waiting
/// forever for a result that never arrives. Wrap every user future fed
/// into `runtime.spawn` or `Task::perform` with this helper so panics
/// surface as `Err(json!({"error": "panic", "message": ...}))` and
/// the MVU loop sees an `AsyncEvent(Err(..))` for the tag.
#[cfg(any(feature = "direct", feature = "wire"))]
#[allow(dead_code)]
pub(crate) async fn run_task_with_panic_guard<F>(
    tag: &str,
    future: F,
) -> Result<serde_json::Value, serde_json::Value>
where
    F: std::future::Future<Output = Result<serde_json::Value, serde_json::Value>>,
{
    use futures::FutureExt;
    match std::panic::AssertUnwindSafe(future).catch_unwind().await {
        Ok(result) => result,
        Err(payload) => {
            let msg = panic_message(&*payload);
            log::error!("async task `{tag}` panicked: {msg}");
            Err(serde_json::json!({ "error": "panic", "message": msg }))
        }
    }
}

#[cfg(all(test, feature = "direct"))]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    // ---------------------------------------------------------------------------
    // platform_sleep is cooperative
    //
    // Direct-mode SendAfter must not block the executor worker.
    // `std::thread::sleep` inside a Task::perform future would
    // serialise concurrent delays; `platform_sleep`
    // (tokio::time::sleep) lets multiple delays run in parallel.
    //
    // With a two-worker runtime and four 100 ms sleeps, total wall
    // time must be much closer to 100 ms than 400 ms. Pick a generous
    // ceiling (250 ms) to avoid flakiness under loaded CI.
    // ---------------------------------------------------------------------------

    #[test]
    fn platform_sleep_runs_concurrently() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("runtime build failed");

        rt.block_on(async {
            let start = Instant::now();
            let mut handles = Vec::new();
            for _ in 0..4 {
                handles.push(tokio::spawn(async {
                    platform_sleep(Duration::from_millis(100)).await;
                }));
            }
            for h in handles {
                h.await.expect("task panicked");
            }
            let elapsed = start.elapsed();
            assert!(
                elapsed < Duration::from_millis(250),
                "platform_sleep should be cooperative; 4x100ms elapsed = {elapsed:?}, \
                 expected < 250ms but got serialised"
            );
        });
    }

    // ---------------------------------------------------------------------------
    // run_task_with_panic_guard surfaces panics as Err(...)
    //
    // Without the guard, a panic in a user future unwinds the
    // executor worker and drops the result channel. The app then
    // waits forever for an AsyncEvent that never arrives.
    // ---------------------------------------------------------------------------

    #[test]
    fn panic_guard_returns_err_payload() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime build failed");

        let future: Pin<Box<dyn Future<Output = Result<serde_json::Value, serde_json::Value>>>> =
            Box::pin(async { panic!("boom") });
        let result = rt.block_on(run_task_with_panic_guard("t", future));
        let err = result.expect_err("panic must surface as Err");
        let obj = err.as_object().expect("panic payload must be an object");
        assert_eq!(obj.get("error").and_then(|v| v.as_str()), Some("panic"));
        let msg = obj.get("message").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            msg.contains("boom"),
            "message {msg:?} should contain panic payload"
        );
    }

    #[test]
    fn panic_guard_passes_ok_through() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime build failed");

        let future: Pin<Box<dyn Future<Output = Result<serde_json::Value, serde_json::Value>>>> =
            Box::pin(async { Ok(serde_json::json!(42)) });
        let result = rt.block_on(run_task_with_panic_guard("t", future));
        assert_eq!(result, Ok(serde_json::json!(42)));
    }

    #[test]
    fn panic_guard_passes_err_through() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime build failed");

        let future: Pin<Box<dyn Future<Output = Result<serde_json::Value, serde_json::Value>>>> =
            Box::pin(async { Err(serde_json::json!({"kind": "not_found"})) });
        let result = rt.block_on(run_task_with_panic_guard("t", future));
        assert_eq!(result, Err(serde_json::json!({"kind": "not_found"})));
    }

    // ---------------------------------------------------------------------------
    // Panicking future doesn't hang the runtime
    //
    // Even with multiple concurrent tasks where one panics, the rest
    // must still complete. This pins the lifecycle invariant that
    // panics never leak into the wait-on-result loop.
    // ---------------------------------------------------------------------------

    #[test]
    fn concurrent_panics_do_not_stall_other_tasks() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("runtime build failed");

        let counter = Arc::new(AtomicUsize::new(0));
        rt.block_on(async {
            let mut handles = Vec::new();

            for i in 0..4 {
                let counter = counter.clone();
                handles.push(tokio::spawn(async move {
                    let future: Pin<
                        Box<
                            dyn Future<Output = Result<serde_json::Value, serde_json::Value>>
                                + Send,
                        >,
                    > = if i == 1 {
                        Box::pin(async { panic!("user code exploded") })
                    } else {
                        Box::pin(async move {
                            counter.fetch_add(1, Ordering::SeqCst);
                            Ok(serde_json::json!(i))
                        })
                    };
                    run_task_with_panic_guard(&format!("task-{i}"), future).await
                }));
            }

            for h in handles {
                // Every spawn must return a Result (not panic out of the
                // task). The JoinHandle only errors if the spawn itself
                // panicked, which the guard prevents.
                let _ = h.await.expect("spawn panicked through the guard");
            }
        });

        assert_eq!(
            counter.load(Ordering::SeqCst),
            3,
            "three non-panicking tasks must all have completed"
        );
    }
}
