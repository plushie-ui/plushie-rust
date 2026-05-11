//! Windowed automation runner: spawns a real renderer subprocess and
//! drives it over the wire protocol.
//!
//! The windowed backend mirrors Elixir's `session_pool/windowed.ex`
//! shape: the runner owns the renderer subprocess, performs the
//! protocol handshake, sends the current tree snapshot so the user
//! sees the app, and then replays each script instruction against an
//! in-process [`TestSession`](crate::test::TestSession). After every
//! instruction the refreshed tree is sent to the renderer so the
//! window stays in sync with the MVU state.
//!
//! The choice to reuse `TestSession` for MVU simulation (rather than
//! routing events through a live `run_wire` loop) is deliberate: it
//! keeps the script's semantics identical across backends. The only
//! behavioural delta vs the mock/headless paths is that `wait`
//! actually sleeps and the user sees the frames on screen.
//!
//! The module itself is gated on the `wire` feature at its
//! declaration site in [`crate::automation`].

use std::time::Duration;

use crate::App;
use crate::automation::file::{Instruction, PlushieFile};
use crate::automation::runner::{self, RunResult};
use crate::runner::bridge::{Bridge, Codec, Incoming};
use crate::runner::wire_discovery;
use crate::test::TestSession;
use crate::{Error, Result as PlushieResult};

/// How long to wait for outgoing tree-snapshot acks or renderer
/// events to drain between instructions. Kept small so the script
/// paces off its own `wait` instructions rather than the drain loop.
const DRAIN_POLL: Duration = Duration::from_millis(5);

/// Wall-clock pause after the final instruction so the renderer has
/// a beat to flush the last frame before the bridge's `Drop` asks it
/// to exit. Intentionally short; the user-visible `wait` instructions
/// are responsible for any longer holds.
const FINAL_FLUSH_PAUSE: Duration = Duration::from_millis(100);

/// Drive a windowed script end to end.
///
/// Steps, in order:
///
/// 1. Locate the renderer binary via
///    [`wire_discovery::discover_renderer`].
/// 2. Spawn the renderer, send Settings, read the hello message,
///    confirm the codec, start the reader thread.
/// 3. Initialise a [`TestSession`] and send its current tree to the
///    renderer as the first snapshot so the user sees the initial
///    state before any instructions run.
/// 4. Execute instructions via the shared runner, refreshing the
///    renderer's tree after each one. `wait` instructions sleep for
///    their declared duration (windowed scripts are wall-clock paced
///    so the user can follow along).
/// 5. Tear the subprocess down via the bridge's `Drop`. The grace
///    period in `Bridge::Drop` lets the renderer exit cleanly before
///    SIGKILL fires.
///
/// # Errors
///
/// - [`Error::BinaryNotFound`] if no renderer is discoverable.
/// - [`Error::Spawn`] if the subprocess fails to start.
/// - [`Error::ProtocolVersionMismatch`] if the handshake rejects
///   the SDK's protocol version.
/// - [`Error::WireEncode`] / [`Error::WireDecode`] on framing errors.
/// - [`Error::Startup`] summarising failing instructions, if any.
pub fn run_windowed<A: App>(file: &PlushieFile) -> PlushieResult {
    let binary = wire_discovery::discover_renderer()?;
    run_windowed_with_renderer::<A>(&binary, file)
}

/// Drive a windowed script against an explicit renderer binary.
///
/// Same behaviour as [`run_windowed`] except the renderer path is
/// supplied directly, bypassing the discovery chain. Useful for
/// integration tests that want to wrap a custom launcher and for
/// apps that ship a bespoke renderer alongside their release.
///
/// # Errors
///
/// Same surface as [`run_windowed`].
pub fn run_windowed_with_renderer<A: App>(binary: &str, file: &PlushieFile) -> PlushieResult {
    log::info!("automation windowed: using renderer at {binary}");

    let mut bridge = Bridge::spawn(binary).map_err(|e| Error::spawn(binary.to_string(), e))?;

    // Settings exchange and hello. The handshake shape mirrors
    // `run_session_single` in runner/wire.rs; we keep it inline here
    // because automation has no App MVU loop to bootstrap, no
    // subscription manager, and no effect tracker. A shared helper
    // would need awkward feature gates for every extra concern.
    let settings = build_automation_settings::<A>();
    bridge.send_settings(&settings)?;

    let hello = bridge
        .receive()
        .map_err(|e| Error::WireDecode(format!("hello: {e}")))?;
    verify_protocol_version(&hello)?;
    if let Some(codec) = hello.get("codec").and_then(|v| v.as_str()) {
        let codec = match codec {
            "msgpack" => Codec::MsgPack,
            _ => Codec::Json,
        };
        bridge.set_codec(codec);
    }
    bridge.start_reader()?;

    // Seed the TestSession and push the initial tree so the window
    // shows the app's first frame before any instructions run.
    let mut session = TestSession::<A>::start().allow_diagnostics();
    send_current_tree(&mut bridge, &session)?;

    let mut passed = 0usize;
    let mut failures: Vec<(usize, String)> = Vec::new();

    for (line_no, instruction) in &file.instructions {
        // `Wait` is wall-clock in windowed mode so the user can see
        // frames between interactions. The shared executor treats it
        // as a no-op; we intercept here.
        if let Instruction::Wait(ms) = instruction {
            std::thread::sleep(Duration::from_millis(*ms));
            passed += 1;
            continue;
        }
        match execute_once(&mut session, instruction) {
            Ok(()) => {
                passed += 1;
            }
            Err(msg) => {
                failures.push((*line_no, msg));
            }
        }
        if let Err(e) = send_current_tree(&mut bridge, &session) {
            failures.push((*line_no, format!("windowed tree refresh failed: {e}")));
            break;
        }
    }

    // Give the compositor a beat to flush the final frame before
    // Bridge::Drop asks the renderer to exit.
    std::thread::sleep(FINAL_FLUSH_PAUSE);

    let result = RunResult { passed, failures };
    if result.is_ok() {
        Ok(())
    } else {
        Err(Error::Startup(format!(
            "{} instruction(s) failed",
            result.failures.len()
        )))
    }
}

/// Delegate a single instruction to the shared executor.
///
fn execute_once<A: App>(
    session: &mut TestSession<A>,
    instruction: &Instruction,
) -> Result<(), String> {
    runner::execute_instruction(session, instruction)
}

fn verify_protocol_version(hello: &serde_json::Value) -> Result<(), Error> {
    let expected = plushie_core::protocol::PROTOCOL_VERSION;
    let got = hello_protocol_version(hello);
    if got == Some(expected) {
        Ok(())
    } else {
        Err(Error::ProtocolVersionMismatch { expected, got })
    }
}

fn hello_protocol_version(hello: &serde_json::Value) -> Option<u32> {
    hello
        .get("protocol_version")
        .or_else(|| hello.get("protocol"))
        .and_then(plushie_core::protocol::json_protocol_version)
}

fn send_current_tree<A: App>(bridge: &mut Bridge, session: &TestSession<A>) -> PlushieResult {
    let snapshot = serde_json::to_value(session.tree())
        .map_err(|e| Error::WireEncode(format!("tree: {e}")))?;
    bridge.send_snapshot(&snapshot)?;

    loop {
        match bridge.recv_timeout(Some(DRAIN_POLL)) {
            Incoming::Message(_) => {}
            Incoming::Timeout => break,
            Incoming::Error(e) => return Err(Error::WireDecode(format!("drain: {e}"))),
        }
    }
    Ok(())
}

fn build_automation_settings<A: App>() -> serde_json::Value {
    // Use the canonical wire shape so this stays in lockstep with the
    // production wire-mode Settings handshake.
    let mut json = A::settings().to_wire_json();
    if let serde_json::Value::Object(ref mut map) = json {
        map.insert(
            "protocol_version".to_string(),
            serde_json::json!(plushie_core::protocol::PROTOCOL_VERSION),
        );
    }
    json
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_version_wins_over_legacy_protocol() {
        let expected = plushie_core::protocol::PROTOCOL_VERSION;
        let hello = serde_json::json!({
            "protocol_version": expected,
            "protocol": expected + 1,
        });

        assert_eq!(hello_protocol_version(&hello), Some(expected));
        assert!(verify_protocol_version(&hello).is_ok());
    }

    #[test]
    fn legacy_protocol_is_fallback() {
        let expected = plushie_core::protocol::PROTOCOL_VERSION;
        let hello = serde_json::json!({
            "protocol": expected,
        });

        assert_eq!(hello_protocol_version(&hello), Some(expected));
        assert!(verify_protocol_version(&hello).is_ok());
    }

    #[test]
    fn out_of_range_protocol_is_rejected() {
        let hello = serde_json::json!({
            "protocol_version": u64::from(u32::MAX) + 1,
        });

        assert_eq!(hello_protocol_version(&hello), None);
        assert!(verify_protocol_version(&hello).is_err());
    }

    #[test]
    fn non_integer_protocol_is_rejected() {
        let hello = serde_json::json!({
            "protocol_version": 1.5,
        });

        assert_eq!(hello_protocol_version(&hello), None);
        assert!(verify_protocol_version(&hello).is_err());
    }

    #[test]
    fn u32_max_protocol_is_accepted() {
        let hello = serde_json::json!({
            "protocol_version": u32::MAX,
        });

        assert_eq!(hello_protocol_version(&hello), Some(u32::MAX));
    }
}
