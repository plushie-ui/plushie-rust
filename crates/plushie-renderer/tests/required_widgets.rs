//! Integration test: verify Settings-handshake validation of the
//! `required_widgets` list.
//!
//! Spawns `plushie-renderer --mock --json` and sends a Settings
//! message listing a mix of built-in and unknown widget type names.
//! The renderer must emit a `required_widgets_missing` diagnostic
//! naming the unknown types and omit the diagnostic entirely when
//! all names resolve.

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
                            Err(e) => panic!("bad JSON from subprocess: {e}\nraw: {line:?}"),
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

    fn recv(&self) -> Option<serde_json::Value> {
        match self.rx.recv_timeout(RECV_TIMEOUT) {
            Ok(v) => Some(v),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => None,
        }
    }

    /// Pull messages until we get a `diagnostic` matching the given
    /// kind, or we hit a short no-more-messages window (None).
    fn find_diagnostic(&self, kind: &str, deadline: Duration) -> Option<serde_json::Value> {
        let start = std::time::Instant::now();
        while start.elapsed() < deadline {
            match self.rx.recv_timeout(Duration::from_millis(250)) {
                Ok(msg) => {
                    if msg.get("type").and_then(|v| v.as_str()) == Some("diagnostic")
                        && msg
                            .get("diagnostic")
                            .and_then(|d| d.get("kind"))
                            .and_then(|k| k.as_str())
                            == Some(kind)
                    {
                        return Some(msg);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            }
        }
        None
    }

    /// Drain all messages that arrive within the given window and
    /// return any `diagnostic` entries seen. Used for the negative
    /// path (expecting no required_widgets_missing).
    fn drain_diagnostics(&self, window: Duration) -> Vec<serde_json::Value> {
        let mut seen = Vec::new();
        let start = std::time::Instant::now();
        while start.elapsed() < window {
            match self.rx.recv_timeout(Duration::from_millis(100)) {
                Ok(msg) => {
                    if msg.get("type").and_then(|v| v.as_str()) == Some("diagnostic") {
                        seen.push(msg);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        seen
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
                        if let Ok(mut g) = buf_for_thread.lock() {
                            g.extend_from_slice(&chunk[..n]);
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
        let g = self.buffer.lock().unwrap();
        String::from_utf8_lossy(&g).into_owned()
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
        .expect("spawn plushie-renderer");
    let stderr = child.stderr.take().expect("stderr piped");
    let capture = StderrCapture::spawn(stderr);
    (child, capture)
}

#[test]
fn settings_required_widgets_missing_names_are_reported() {
    let (mut child, _stderr) = spawn_renderer(&["--mock", "--json"]);
    let mut stdin = child.stdin.take().unwrap();
    let stdout = LineReceiver::new(child.stdout.take().unwrap());

    // "button" is a built-in widget, "never_registered_widget" is not.
    // The validator should emit a RequiredWidgetsMissing diagnostic
    // listing only the unknown name.
    send(
        &mut stdin,
        &serde_json::json!({
            "session": "s1",
            "type": "settings",
            "settings": {
                "protocol_version": 1,
                "required_widgets": ["button", "never_registered_widget"]
            }
        }),
    );

    // Hello arrives first.
    let hello = stdout.recv().expect("hello message");
    assert_eq!(hello["type"], "hello");

    let diag = stdout
        .find_diagnostic("required_widgets_missing", Duration::from_secs(5))
        .expect("renderer should emit required_widgets_missing diagnostic");

    let missing = diag["diagnostic"]["missing"]
        .as_array()
        .expect("missing should be an array");
    let names: Vec<&str> = missing.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(
        names,
        vec!["never_registered_widget"],
        "only the unknown name should appear in missing; got: {names:?}"
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn settings_required_widgets_all_known_emits_no_diagnostic() {
    let (mut child, _stderr) = spawn_renderer(&["--mock", "--json"]);
    let mut stdin = child.stdin.take().unwrap();
    let stdout = LineReceiver::new(child.stdout.take().unwrap());

    // All names are built-in widgets; the validator should stay quiet.
    send(
        &mut stdin,
        &serde_json::json!({
            "session": "s1",
            "type": "settings",
            "settings": {
                "protocol_version": 1,
                "required_widgets": ["button", "text", "column"]
            }
        }),
    );

    let hello = stdout.recv().expect("hello message");
    assert_eq!(hello["type"], "hello");

    // No required_widgets_missing should appear in a reasonable window.
    let diags = stdout.drain_diagnostics(Duration::from_millis(500));
    let offending: Vec<_> = diags
        .iter()
        .filter(|d| d["diagnostic"]["kind"].as_str() == Some("required_widgets_missing"))
        .collect();
    assert!(
        offending.is_empty(),
        "renderer should not emit required_widgets_missing when all names are known; saw: {offending:?}"
    );

    drop(stdin);
    let _ = child.wait();
}
