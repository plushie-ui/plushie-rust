//! Wire-end convergence test for the consolidated native effect handler.
//!
//! In-process convergence (the trait impl path used by the SDK in
//! direct mode and by the renderer daemon for sync effects matches the
//! free-function path used by the headless dispatcher) is asserted in
//! `plushie_renderer_lib::effects::native::tests`. This file exercises
//! the wire path: a real `plushie-renderer` subprocess receiving an
//! `effect` message and emitting an `effect_response`.
//!
//! Mock mode is the right vehicle: it short-circuits async effects to
//! `unsupported` deterministically (no display server required) and
//! routes sync effects through the same shared dispatcher. The test
//! pins the response envelope (`type`, `id`, `status`) so a regression
//! in either the wire codec or the consolidated handler shape would
//! surface here.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const RECV_TIMEOUT: Duration = Duration::from_secs(10);

fn send(stdin: &mut impl Write, msg: &serde_json::Value) {
    let line = serde_json::to_string(msg).unwrap();
    writeln!(stdin, "{line}").unwrap();
    stdin.flush().unwrap();
}

struct LineReceiver {
    rx: mpsc::Receiver<serde_json::Value>,
    _handle: std::thread::JoinHandle<()>,
}

impl LineReceiver {
    fn new(stdout: std::process::ChildStdout) -> Self {
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
                        match serde_json::from_str(trimmed) {
                            Ok(val) => {
                                if tx.send(val).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                panic!("failed to parse JSON from subprocess: {e}\nraw: {line:?}");
                            }
                        }
                    }
                    Err(e) => panic!("read_line failed: {e}"),
                }
            }
        });
        Self {
            rx,
            _handle: handle,
        }
    }

    fn recv(&self) -> serde_json::Value {
        match self.rx.recv_timeout(RECV_TIMEOUT) {
            Ok(val) => val,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("recv timed out after {RECV_TIMEOUT:?}");
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!("subprocess stdout closed unexpectedly");
            }
        }
    }

    /// Receive the next message that matches `predicate`, draining and
    /// discarding intermediate messages (e.g. status events).
    fn recv_first<F: Fn(&serde_json::Value) -> bool>(&self, predicate: F) -> serde_json::Value {
        loop {
            let msg = self.recv();
            if predicate(&msg) {
                return msg;
            }
        }
    }
}

fn plushie_binary() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("plushie-renderer");
    path.to_string_lossy().to_string()
}

struct StderrCapture {
    buffer: Arc<Mutex<Vec<u8>>>,
    _handle: std::thread::JoinHandle<()>,
}

impl StderrCapture {
    fn spawn(stderr: ChildStderr) -> Self {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let buf_for_thread = Arc::clone(&buffer);
        let handle = std::thread::spawn(move || {
            let mut reader = stderr;
            let mut chunk = [0u8; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Ok(mut guard) = buf_for_thread.lock() {
                            guard.extend_from_slice(&chunk[..n]);
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            buffer,
            _handle: handle,
        }
    }

    fn snapshot(&self) -> String {
        let guard = self.buffer.lock().unwrap();
        String::from_utf8_lossy(&guard).into_owned()
    }
}

impl Drop for StderrCapture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            let text = self.snapshot();
            if !text.is_empty() {
                eprintln!("---- captured renderer stderr ----\n{text}\n---- end ----");
            }
        }
    }
}

fn spawn_renderer(args: &[&str]) -> (Child, StderrCapture) {
    let mut child = Command::new(plushie_binary())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn plushie-renderer");
    let stderr = child.stderr.take().expect("stderr should be piped");
    let capture = StderrCapture::spawn(stderr);
    (child, capture)
}

/// File dialog requests in mock mode are deterministic: the dispatcher
/// short-circuits all async effects to `unsupported` regardless of OS.
/// Both the SDK direct-mode path and the wire path land in the same
/// shared module to make that decision; the wire side is what this
/// test pins.
#[test]
fn file_open_in_mock_mode_returns_unsupported() {
    let (mut child, _stderr) = spawn_renderer(&["--mock", "--json"]);

    let mut stdin = child.stdin.take().unwrap();
    let stdout = LineReceiver::new(child.stdout.take().unwrap());

    send(
        &mut stdin,
        &serde_json::json!({"session": "s1", "type": "settings", "settings": {"protocol_version": 1}}),
    );
    let _hello = stdout.recv_first(|m| m["type"] == "hello");

    send(
        &mut stdin,
        &serde_json::json!({
            "session": "s1",
            "type": "effect",
            "id": "open-1",
            "kind": "file_open",
            "payload": {"title": "ignored in mock mode"},
        }),
    );

    let resp = stdout.recv_first(|m| m["type"] == "effect_response");
    assert_eq!(resp["type"], "effect_response");
    assert_eq!(resp["id"], "open-1");
    assert_eq!(resp["status"], "unsupported");
    assert_eq!(resp["session"], "s1");

    drop(stdin);
    child.wait().unwrap();
}

/// The same envelope shape (`type`, `id`, `status`, `session`) must
/// arrive for every async file-dialog kind in mock mode. A divergence
/// here would mean the wire path's dispatch table drifted from the
/// in-process one in `plushie_renderer_lib::effects::native`.
#[test]
fn every_file_dialog_kind_returns_unsupported_in_mock_mode() {
    let (mut child, _stderr) = spawn_renderer(&["--mock", "--json"]);

    let mut stdin = child.stdin.take().unwrap();
    let stdout = LineReceiver::new(child.stdout.take().unwrap());

    send(
        &mut stdin,
        &serde_json::json!({"session": "s1", "type": "settings", "settings": {"protocol_version": 1}}),
    );
    let _hello = stdout.recv_first(|m| m["type"] == "hello");

    let kinds = [
        "file_open",
        "file_open_multiple",
        "file_save",
        "directory_select",
        "directory_select_multiple",
    ];
    for kind in kinds {
        let id = format!("eff-{kind}");
        send(
            &mut stdin,
            &serde_json::json!({
                "session": "s1",
                "type": "effect",
                "id": id,
                "kind": kind,
                "payload": {},
            }),
        );

        let resp = stdout.recv_first(|m| m["type"] == "effect_response");
        assert_eq!(resp["id"], id, "id mismatch for {kind}");
        assert_eq!(
            resp["status"], "unsupported",
            "expected unsupported status for {kind} in mock mode"
        );
        assert_eq!(resp["session"], "s1");
    }

    drop(stdin);
    child.wait().unwrap();
}

/// Unknown effect kinds are rejected at the validation stage in the
/// engine, before the handler dispatcher runs. The wire envelope
/// (`type`, `id`, `error` text) must still pin down so a regression
/// here would surface a divergence between the engine's pre-handler
/// rejection and the shared dispatcher's post-handler unsupported.
#[test]
fn unknown_effect_kind_returns_error_in_mock_mode() {
    let (mut child, _stderr) = spawn_renderer(&["--mock", "--json"]);

    let mut stdin = child.stdin.take().unwrap();
    let stdout = LineReceiver::new(child.stdout.take().unwrap());

    send(
        &mut stdin,
        &serde_json::json!({"session": "s1", "type": "settings", "settings": {"protocol_version": 1}}),
    );
    let _hello = stdout.recv_first(|m| m["type"] == "hello");

    send(
        &mut stdin,
        &serde_json::json!({
            "session": "s1",
            "type": "effect",
            "id": "bogus-1",
            "kind": "teleport_sandwich",
            "payload": {},
        }),
    );

    let resp = stdout.recv_first(|m| m["type"] == "effect_response");
    assert_eq!(resp["id"], "bogus-1");
    assert_eq!(resp["status"], "error");
    let err = resp["error"]
        .as_str()
        .expect("error field must be a string");
    assert!(
        err.contains("teleport_sandwich"),
        "error should name the unknown kind, got {err:?}"
    );

    drop(stdin);
    child.wait().unwrap();
}
