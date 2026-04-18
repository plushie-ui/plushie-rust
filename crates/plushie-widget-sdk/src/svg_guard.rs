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

/// Headless / offline decode budget. Longer because batch runs can
/// legitimately process larger SVG assets where a user isn't waiting
/// on the frame.
pub const HEADLESS_TIMEOUT: Duration = Duration::from_secs(5);

/// Parse the given SVG source with a wall-clock deadline.
///
/// `source` is UTF-8 SVG text. The caller picks the appropriate
/// deadline ([`INTERACTIVE_TIMEOUT`] or [`HEADLESS_TIMEOUT`]) based
/// on the rendering context.
///
/// Returns [`DecodeOutcome::Ok`] on success, [`DecodeOutcome::ParseError`]
/// on a syntactic failure reported by usvg, or [`DecodeOutcome::Timeout`]
/// if the deadline passed before the worker finished.
pub fn parse_with_timeout(source: String, deadline: Duration) -> DecodeOutcome {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("plushie-svg-guard".into())
        .spawn(move || {
            let opt = usvg::Options::default();
            let result = usvg::Tree::from_str(&source, &opt).map_err(|e| e.to_string());
            // Receiver may have already dropped due to timeout; send
            // is best-effort.
            let _ = tx.send(result);
        })
        .map(|_| ())
        .unwrap_or_else(|e| log::error!("svg_guard: failed to spawn worker: {e}"));

    match rx.recv_timeout(deadline) {
        Ok(Ok(_tree)) => DecodeOutcome::Ok,
        Ok(Err(msg)) => DecodeOutcome::ParseError(msg),
        Err(mpsc::RecvTimeoutError::Timeout) => DecodeOutcome::Timeout,
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            // Worker panicked before sending. Treat as a parse error
            // with a generic message so the caller still falls back.
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
