//! Subprocess integration test for WindowOp wire messages.
//!
//! In headless/mock modes the renderer cannot open real windows
//! (no display server), so window ops dispatched into the iced
//! daemon are silently dropped after the typed wire decode. That
//! still leaves a load-bearing path under test: the wire message
//! must decode cleanly, the dispatcher must route the typed
//! WindowOp variant without crashing, and subsequent messages on
//! the same session must continue to flow.
//!
//! These tests exercise that path against a real renderer binary
//! by sending a sequence of WindowOp variants (resize, move,
//! close) and asserting:
//!
//! - The subprocess produces no `session_error` event.
//! - A subsequent reset request still gets a `reset_response`,
//!   proving the session loop kept running through every dispatch.
//!
//! For closer coverage of `window_resized`/`window_closed` events
//! with a real iced daemon, see `crates/plushie/tests/multi_window_test.rs`,
//! which drives the SDK-level event surface through TestSession.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const RECV_TIMEOUT: Duration = Duration::from_secs(10);

fn plushie_binary() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("plushie-renderer");
    path.to_string_lossy().to_string()
}

struct LineReceiver {
    rx: mpsc::Receiver<serde_json::Value>,
    _handle: std::thread::JoinHandle<()>,
}

impl LineReceiver {
    fn new(stdout: ChildStdout) -> Self {
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        let val: serde_json::Value = match serde_json::from_str(trimmed) {
                            Ok(v) => v,
                            Err(e) => panic!("JSON parse error: {e}\nraw: {line:?}"),
                        };
                        if tx.send(val).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            rx,
            _handle: handle,
        }
    }

    fn recv(&self) -> serde_json::Value {
        self.rx
            .recv_timeout(RECV_TIMEOUT)
            .expect("subprocess did not respond within 10s")
    }
}

fn send(stdin: &mut ChildStdin, msg: &serde_json::Value) {
    let line = serde_json::to_string(msg).unwrap();
    writeln!(stdin, "{line}").unwrap();
    stdin.flush().unwrap();
}

fn spawn_renderer() -> Option<(Child, ChildStdin, LineReceiver)> {
    let binary = plushie_binary();
    if !std::path::Path::new(&binary).exists() {
        eprintln!(
            "skipping window_ops_subprocess: renderer binary not found at {binary}; \
             build it with `cargo build -p plushie-renderer`"
        );
        return None;
    }
    let mut child = Command::new(&binary)
        .args(["--mock", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn plushie-renderer");
    let stdin = child.stdin.take().unwrap();
    let stdout = LineReceiver::new(child.stdout.take().unwrap());
    Some((child, stdin, stdout))
}

/// Drain events from stdout until the predicate returns true. Panics
/// if a `session_error` shows up first; that's the failure mode the
/// test is guarding against.
fn drain_until(stdout: &LineReceiver, mut pred: impl FnMut(&serde_json::Value) -> bool) {
    loop {
        let msg = stdout.recv();
        if msg.get("type").and_then(|v| v.as_str()) == Some("event")
            && msg.get("family").and_then(|v| v.as_str()) == Some("session_error")
        {
            panic!("subprocess emitted a session_error during WindowOp dispatch: {msg}");
        }
        if pred(&msg) {
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn window_ops_dispatch_through_subprocess_without_error() {
    // Sequence: settings (handshake) -> snapshot (so a tree exists)
    // -> resize, move, close window ops -> reset (sync barrier).
    // The reset response is the load-bearing assertion: the session
    // loop survived every preceding WindowOp.
    let Some((mut child, mut stdin, stdout)) = spawn_renderer() else {
        return;
    };

    send(
        &mut stdin,
        &serde_json::json!({
            "session": "s1",
            "type": "settings",
            "settings": {"protocol_version": 1},
        }),
    );
    let hello = stdout.recv();
    assert_eq!(hello["type"], "hello");

    send(
        &mut stdin,
        &serde_json::json!({
            "session": "s1",
            "type": "snapshot",
            "tree": {
                "id": "main",
                "type": "window",
                "props": {},
                "children": [],
            },
        }),
    );

    // Send each WindowOp variant the action plan calls out. Mock
    // mode silently drops the dispatched typed op (no daemon to
    // execute against), but the wire decode and session loop still
    // run; that's the surface under test.
    for op in [
        serde_json::json!({
            "session": "s1",
            "type": "window_op",
            "op": "resize",
            "window_id": "main",
            "payload": {"width": 800.0, "height": 600.0},
        }),
        serde_json::json!({
            "session": "s1",
            "type": "window_op",
            "op": "move",
            "window_id": "main",
            "payload": {"x": 50.0, "y": 75.0},
        }),
        serde_json::json!({
            "session": "s1",
            "type": "window_op",
            "op": "close",
            "window_id": "main",
            "payload": {},
        }),
    ] {
        send(&mut stdin, &op);
    }

    // Sync barrier: the reset response only arrives after every
    // preceding message has been processed (the session loop is
    // strictly serial per session).
    send(
        &mut stdin,
        &serde_json::json!({
            "session": "s1",
            "type": "reset",
            "id": "r1",
        }),
    );

    drain_until(&stdout, |msg| {
        msg["type"] == "reset_response" && msg["id"] == "r1"
    });

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn partial_window_close_in_one_session_does_not_disturb_another() {
    // With max-sessions > 1, closing a window in one session must
    // leave another session free to keep responding. This is the
    // "partial-close" scenario the action plan asks for. Because
    // mock mode doesn't open real windows, the test verifies the
    // session-level isolation: a WindowOp::Close under one session
    // ID followed by activity on a different session ID stays
    // interleaved.
    let binary = plushie_binary();
    if !std::path::Path::new(&binary).exists() {
        eprintln!("skipping partial_window_close test: renderer binary not found");
        return;
    }
    let mut child = Command::new(&binary)
        .args(["--mock", "--json", "--max-sessions", "4"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn plushie-renderer");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = LineReceiver::new(child.stdout.take().unwrap());

    // Session s1 setup.
    send(
        &mut stdin,
        &serde_json::json!({
            "session": "s1",
            "type": "settings",
            "settings": {"protocol_version": 1},
        }),
    );
    let hello = stdout.recv();
    assert_eq!(hello["type"], "hello");

    // Both sessions get a snapshot so they exist on the dispatcher.
    for sid in ["s1", "s2"] {
        send(
            &mut stdin,
            &serde_json::json!({
                "session": sid,
                "type": "snapshot",
                "tree": {
                    "id": format!("{sid}-window"),
                    "type": "window",
                    "props": {},
                    "children": [],
                },
            }),
        );
    }

    // Close the window in s1; s2 untouched.
    send(
        &mut stdin,
        &serde_json::json!({
            "session": "s1",
            "type": "window_op",
            "op": "close",
            "window_id": "s1-window",
            "payload": {},
        }),
    );

    // Issue a query against s2. The response must come back tagged
    // for s2 even though s1 just closed its window.
    send(
        &mut stdin,
        &serde_json::json!({
            "session": "s2",
            "type": "query",
            "id": "q1",
            "target": "tree",
            "selector": {},
        }),
    );

    // Drain until the s2 query response arrives, asserting no
    // session_error from either session.
    drain_until(&stdout, |msg| {
        msg["type"] == "query_response" && msg["session"] == "s2" && msg["id"] == "q1"
    });

    drop(stdin);
    let _ = child.wait();
}
