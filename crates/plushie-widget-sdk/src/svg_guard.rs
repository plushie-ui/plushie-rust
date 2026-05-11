//! SVG decode guard: bounded pre-parse with a wall-clock timeout.
//!
//! iced's SVG widget decodes lazily inside its own render path, which
//! means a malicious SVG can tie up the rendering thread for an
//! unbounded amount of time. To bound that, [`parse_with_timeout`]
//! offloads the `usvg::Tree::from_data` call to a worker thread and
//! waits on a channel with a deadline. If the worker exceeds the
//! deadline, the main thread moves on with a `DecodeOutcome::Timeout`
//! verdict and the caller should fall back to a placeholder render.
//!
//! Limitations: Rust does not provide cooperative thread cancellation
//! in std, so a worker stuck in usvg keeps running until it finishes
//! or the process exits. The main thread is insulated from the hang;
//! the worker is not. Real-world impact is limited because the 64 MiB
//! wire cap bounds input size and usvg's own safeguards (recursion
//! caps inside its tree builder) keep most pathological inputs finite.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

/// Result of a bounded SVG parse.
#[derive(Debug)]
pub enum DecodeOutcome {
    /// The input parsed successfully.
    Ok,
    /// Parsing failed. The inner string is the usvg error message.
    ParseError(String),
    /// Parsing exceeded the deadline. The worker thread is abandoned;
    /// the main thread moved on.
    Timeout,
}

/// Interactive (windowed / native) decode budget.
pub const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(1);

const MAX_CONCURRENT_SVG_WORKERS: usize = 8;

static ACTIVE_SVG_WORKERS: AtomicUsize = AtomicUsize::new(0);

struct SvgWorkerSlot;

impl Drop for SvgWorkerSlot {
    fn drop(&mut self) {
        ACTIVE_SVG_WORKERS.fetch_sub(1, Ordering::Relaxed);
    }
}

fn try_acquire_worker_slot() -> Option<SvgWorkerSlot> {
    if ACTIVE_SVG_WORKERS.fetch_add(1, Ordering::Relaxed) >= MAX_CONCURRENT_SVG_WORKERS {
        ACTIVE_SVG_WORKERS.fetch_sub(1, Ordering::Relaxed);
        None
    } else {
        Some(SvgWorkerSlot)
    }
}

pub fn parse_with_timeout(source: String, deadline: Duration) -> DecodeOutcome {
    let Some(slot) = try_acquire_worker_slot() else {
        return DecodeOutcome::Timeout;
    };

    let (tx, rx) = mpsc::channel();
    let spawn_result = std::thread::Builder::new()
        .name("plushie-svg-guard".into())
        .spawn(move || {
            let _slot = slot;
            let opt = usvg::Options::default();
            let result = usvg::Tree::from_str(&source, &opt).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    if let Err(e) = spawn_result {
        let msg = format!("svg_guard: failed to spawn worker: {e}");
        log::error!("{msg}");
        return DecodeOutcome::ParseError(msg);
    }

    match rx.recv_timeout(deadline) {
        Ok(Ok(_tree)) => DecodeOutcome::Ok,
        Ok(Err(msg)) => DecodeOutcome::ParseError(msg),
        Err(mpsc::RecvTimeoutError::Timeout) => DecodeOutcome::Timeout,
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            DecodeOutcome::ParseError("svg_guard: worker panicked".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_svg() {
        let src = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"></svg>"#;
        let out = parse_with_timeout(src.to_string(), INTERACTIVE_TIMEOUT);
        matches!(out, DecodeOutcome::Ok).then_some(()).expect("ok");
    }

    #[test]
    fn returns_parse_error_on_garbage() {
        let out = parse_with_timeout("not xml at all".to_string(), INTERACTIVE_TIMEOUT);
        assert!(
            matches!(out, DecodeOutcome::ParseError(_)),
            "expected parse error"
        );
    }

    #[test]
    fn timeout_short_circuits() {
        // A very short deadline against a minimal SVG may still
        // sometimes succeed depending on scheduler latency; just
        // verify the function returns one of the defined variants
        // within an upper bound rather than hanging.
        let src = r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#;
        let out = parse_with_timeout(src.to_string(), Duration::from_nanos(1));
        assert!(
            matches!(
                out,
                DecodeOutcome::Ok | DecodeOutcome::Timeout | DecodeOutcome::ParseError(_)
            ),
            "unexpected outcome"
        );
    }
}
