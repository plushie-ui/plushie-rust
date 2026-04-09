//! Wire mode runner: subprocess renderer via stdin/stdout.
//!
//! Spawns the plushie renderer binary as a child process and
//! communicates over the plushie wire protocol (MessagePack or JSON
//! framing over stdin/stdout).

use crate::App;

/// Run the app in wire mode.
///
/// Spawns the renderer binary at `binary_path` and communicates
/// over stdin/stdout using the plushie wire protocol.
pub fn run_wire<A: App>(_binary_path: &str) -> crate::Result {
    let _ = std::any::type_name::<A>();

    // TODO: Implement wire mode runner (Batch 5)
    // 1. Spawn renderer binary as child process
    // 2. Negotiate codec (msgpack/json)
    // 3. Send Settings + initial Snapshot
    // 4. Enter event loop: receive events, call update, diff tree, send patches
    // 5. Handle bridge restart/recovery

    Err("Wire mode runner is under construction".into())
}
