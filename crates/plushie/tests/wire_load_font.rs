//! Wire-mode integration tests for the typed `LoadFont` message.
//!
//! `LoadFont` is a typed binary message: the JSON wire path encodes
//! the font bytes as a base64 string; the MessagePack wire path uses
//! native binary, which costs ~33% less than the base64 form for
//! large fonts. Both code paths run through the renderer's typed
//! dispatch; this test exercises both via a real renderer subprocess
//! to catch shape regressions on either side.
//!
//! Mock mode is used because `IncomingMessage::LoadFont` reaches the
//! same `apply()` site there as in headless and windowed modes; the
//! wire shape and the dispatch contract are what's under test.
//!
//! ## How the test confirms the load
//!
//! `LoadFont` itself does not produce a wire event on success (only a
//! debug log). The renderer processes messages serially per session,
//! so we send a `RegisterEffectStub` immediately after the `LoadFont`
//! and wait for the `effect_stub_register_ack`. If the LoadFont had
//! crashed or produced a `session_error`, the ack would never arrive
//! (we'd see the error event first) or the session would have torn
//! down. Surviving to the ack is the load-bearing assertion.

#![cfg(feature = "wire")]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use base64::Engine;

const RECV_TIMEOUT: Duration = Duration::from_secs(10);

/// Minimal valid TrueType font header bytes.
///
/// fontdb sniffs the format from the bytes themselves; we don't need a
/// fully renderable font for this test, just bytes the loader accepts.
/// This is the magic number plus enough table directory data to look
/// like a TTF on cursory inspection. The renderer's `load_font`
/// pipeline calls into iced's font system, which forwards to fontdb;
/// invalid font data is logged but does not produce a wire-level
/// error, so any byte sequence that survives `decode_font_data`
/// suffices to exercise the wire path.
fn fake_font_bytes() -> Vec<u8> {
    // Just a small payload that round-trips both the base64 and
    // native-binary paths. The renderer's font system silently
    // ignores unparseable bytes, which is fine: we're testing the
    // wire codec and the dispatch path, not the font parser.
    (0u8..=63u8).collect()
}

fn plushie_binary() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("plushie-renderer");
    path.to_string_lossy().to_string()
}

// ---------------------------------------------------------------------------
// JSON-mode subprocess harness
// ---------------------------------------------------------------------------

struct JsonReceiver {
    rx: mpsc::Receiver<serde_json::Value>,
    _handle: std::thread::JoinHandle<()>,
}

impl JsonReceiver {
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

fn send_json(stdin: &mut ChildStdin, msg: &serde_json::Value) {
    let line = serde_json::to_string(msg).unwrap();
    writeln!(stdin, "{line}").unwrap();
    stdin.flush().unwrap();
}

fn spawn_renderer_json() -> Option<(Child, ChildStdin, JsonReceiver)> {
    let binary = plushie_binary();
    if !std::path::Path::new(&binary).exists() {
        eprintln!(
            "skipping wire_load_font: renderer binary not found at {binary}; \
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
    let stdout = JsonReceiver::new(child.stdout.take().unwrap());
    Some((child, stdin, stdout))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn json_load_font_processes_via_typed_message() {
    let Some((mut child, mut stdin, stdout)) = spawn_renderer_json() else {
        return;
    };

    // Settings handshake; the renderer responds with hello.
    send_json(
        &mut stdin,
        &serde_json::json!({
            "session": "s1",
            "type": "settings",
            "settings": {"protocol_version": 1},
        }),
    );
    let hello = stdout.recv();
    assert_eq!(hello["type"], "hello");

    // The wire shape under test: a typed LoadFont message with the
    // font bytes packed as a base64 string under payload.data. This
    // mirrors what `Bridge::send_load_font` writes in JSON mode.
    let bytes = fake_font_bytes();
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    send_json(
        &mut stdin,
        &serde_json::json!({
            "session": "s1",
            "type": "load_font",
            "payload": {
                "family": "Wire-Test-Sans",
                "data": b64,
            },
        }),
    );

    // Sync barrier: register an effect stub. The renderer processes
    // messages serially per session, so the ack arrives only if the
    // preceding LoadFont was applied without crashing.
    send_json(
        &mut stdin,
        &serde_json::json!({
            "session": "s1",
            "type": "register_effect_stub",
            "kind": "file_open",
            "response": {"status": "ok", "result": {"path": "/tmp/x"}},
        }),
    );

    // Drain events until the ack arrives; assert no session_error
    // appears between the LoadFont and the ack.
    loop {
        let msg = stdout.recv();
        match msg["type"].as_str() {
            Some("effect_stub_register_ack") => {
                assert_eq!(msg["kind"], "file_open");
                assert_eq!(msg["status"], "registered");
                break;
            }
            Some("event") if msg["family"] == "session_error" => {
                panic!("LoadFont path emitted a session_error: {msg}");
            }
            // Status / log-style events are fine to ignore; they don't
            // break the serialization invariant.
            _ => continue,
        }
    }

    drop(stdin);
    child.wait_timeout_or_kill(Duration::from_secs(5));
}

#[test]
fn msgpack_load_font_processes_via_typed_message() {
    // The MsgPack path matters because it uses native binary
    // (rmpv::Value::Binary) instead of base64. A regression here
    // (e.g. swapping to base64 inside msgpack frames) would silently
    // bloat the wire and break parity with the typed encode side.
    let binary = plushie_binary();
    if !std::path::Path::new(&binary).exists() {
        eprintln!("skipping wire_load_font (msgpack): renderer binary not found at {binary}");
        return;
    }
    let mut child = Command::new(&binary)
        .args(["--mock"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn plushie-renderer");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Helper: encode a single rmpv message with a 4-byte BE length prefix.
    fn write_frame(stdin: &mut ChildStdin, value: &rmpv::Value) {
        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, value).expect("encode");
        let len = u32::try_from(buf.len()).expect("frame fits");
        stdin.write_all(&len.to_be_bytes()).unwrap();
        stdin.write_all(&buf).unwrap();
        stdin.flush().unwrap();
    }

    fn read_frame(reader: &mut BufReader<ChildStdout>) -> Option<rmpv::Value> {
        use std::io::Read;
        let mut len_buf = [0u8; 4];
        if reader.read_exact(&mut len_buf).is_err() {
            return None;
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf).ok()?;
        rmpv::decode::read_value(&mut &buf[..]).ok()
    }

    use rmpv::Value as V;

    // Settings handshake.
    let settings_msg = V::Map(vec![
        (V::String("session".into()), V::String("s1".into())),
        (V::String("type".into()), V::String("settings".into())),
        (
            V::String("settings".into()),
            V::Map(vec![(
                V::String("protocol_version".into()),
                V::Integer(1.into()),
            )]),
        ),
    ]);
    write_frame(&mut stdin, &settings_msg);

    let hello = read_frame(&mut stdout).expect("hello");
    let hello_type = match &hello {
        V::Map(entries) => entries
            .iter()
            .find(|(k, _)| matches!(k, V::String(s) if s.as_str() == Some("type")))
            .and_then(|(_, v)| match v {
                V::String(s) => s.as_str().map(str::to_string),
                _ => None,
            }),
        _ => None,
    };
    assert_eq!(hello_type.as_deref(), Some("hello"));

    // LoadFont with native msgpack binary; this is the wire shape
    // `Bridge::send_load_font` writes in MsgPack mode.
    let bytes = fake_font_bytes();
    let load_font = V::Map(vec![
        (V::String("session".into()), V::String("s1".into())),
        (V::String("type".into()), V::String("load_font".into())),
        (
            V::String("payload".into()),
            V::Map(vec![
                (
                    V::String("family".into()),
                    V::String("Wire-Test-Sans".into()),
                ),
                (V::String("data".into()), V::Binary(bytes)),
            ]),
        ),
    ]);
    write_frame(&mut stdin, &load_font);

    // Sync barrier: same approach as the JSON test.
    let stub = V::Map(vec![
        (V::String("session".into()), V::String("s1".into())),
        (
            V::String("type".into()),
            V::String("register_effect_stub".into()),
        ),
        (V::String("kind".into()), V::String("file_open".into())),
        (
            V::String("response".into()),
            V::Map(vec![
                (V::String("status".into()), V::String("ok".into())),
                (
                    V::String("result".into()),
                    V::Map(vec![(V::String("path".into()), V::String("/tmp/x".into()))]),
                ),
            ]),
        ),
    ]);
    write_frame(&mut stdin, &stub);

    let deadline = std::time::Instant::now() + RECV_TIMEOUT;
    loop {
        if std::time::Instant::now() > deadline {
            panic!("timed out waiting for effect_stub_register_ack");
        }
        let Some(msg) = read_frame(&mut stdout) else {
            panic!("renderer closed stdout before ack arrived");
        };
        let map = match &msg {
            V::Map(m) => m,
            _ => continue,
        };
        let msg_type = map
            .iter()
            .find(|(k, _)| matches!(k, V::String(s) if s.as_str() == Some("type")))
            .and_then(|(_, v)| match v {
                V::String(s) => s.as_str().map(str::to_string),
                _ => None,
            });
        match msg_type.as_deref() {
            Some("effect_stub_register_ack") => {
                let status = map
                    .iter()
                    .find(|(k, _)| matches!(k, V::String(s) if s.as_str() == Some("status")))
                    .and_then(|(_, v)| match v {
                        V::String(s) => s.as_str().map(str::to_string),
                        _ => None,
                    });
                assert_eq!(status.as_deref(), Some("registered"));
                break;
            }
            Some("event") => {
                // Surface session_error if it appears.
                let family = map
                    .iter()
                    .find(|(k, _)| matches!(k, V::String(s) if s.as_str() == Some("family")))
                    .and_then(|(_, v)| match v {
                        V::String(s) => s.as_str().map(str::to_string),
                        _ => None,
                    });
                if family.as_deref() == Some("session_error") {
                    panic!("LoadFont (msgpack) path emitted a session_error: {map:?}");
                }
            }
            _ => {}
        }
    }

    drop(stdin);
    child.wait_timeout_or_kill(Duration::from_secs(5));
}

// ---------------------------------------------------------------------------
// Cleanup helper
// ---------------------------------------------------------------------------
//
// std::process::Child has no built-in wait-with-timeout, but the
// existing renderer tests use the same drop-then-wait idiom with a
// best-effort kill if the child is still alive. Rather than pull in
// the `wait-timeout` crate just for this file, this helper trait
// inlines the same pattern.
trait WaitTimeoutOrKill {
    fn wait_timeout_or_kill(&mut self, timeout: Duration);
}

impl WaitTimeoutOrKill for Child {
    fn wait_timeout_or_kill(&mut self, timeout: Duration) {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            match self.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => break,
            }
        }
        let _ = self.kill();
        let _ = self.wait();
    }
}
