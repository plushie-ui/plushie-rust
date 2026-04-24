//! Web output writer.
//!
//! Wraps a JavaScript callback function. When events are emitted,
//! the encoded bytes are converted to a string and passed to the
//! JS callback.

use std::io::{self, Write};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

/// Output writer used by the WASM entry point.
///
/// On wasm32 this forwards encoded bytes to a JavaScript callback. On
/// native targets it is an inert placeholder used only for type-checking
/// docs and tests for this crate.
#[cfg_attr(not(target_arch = "wasm32"), derive(Default))]
pub struct WebOutputWriter {
    #[cfg(target_arch = "wasm32")]
    callback: js_sys::Function,
}

// SAFETY: On wasm32 this writer stores a js_sys::Function, which wraps a
// JsValue. JsValue is not thread-safe. The renderer-lib sink requires Send
// because native renderer sinks can be shared across threads, but this WASM
// entry point is only sound when the module's linear memory is not shared.
// try_new checks wasm_bindgen::memory().buffer() and rejects SharedArrayBuffer
// before the callback is stored in the global sink. That prevents this value
// from being moved across wasm threads by a shared-memory module. Real wasm
// thread support needs a different output path.
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
unsafe impl Send for WebOutputWriter {}

impl WebOutputWriter {
    #[cfg(target_arch = "wasm32")]
    pub fn try_new(callback: js_sys::Function) -> Result<Self, JsValue> {
        let memory: js_sys::WebAssembly::Memory = wasm_bindgen::memory().unchecked_into();
        if memory
            .buffer()
            .is_instance_of::<js_sys::SharedArrayBuffer>()
        {
            return Err(JsValue::from_str(
                "plushie-renderer-wasm does not support shared-memory wasm builds; \
                 the JavaScript callback output path is not thread-safe",
            ));
        }

        Ok(Self { callback })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_new(_callback: js_sys::Function) -> Result<Self, JsValue> {
        Ok(Self {})
    }
}

impl Write for WebOutputWriter {
    #[cfg(target_arch = "wasm32")]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let json = String::from_utf8_lossy(buf);
        let js_str = JsValue::from_str(&json);
        self.callback.call1(&JsValue::NULL, &js_str).map_err(|e| {
            let msg = e
                .as_string()
                .unwrap_or_else(|| "unknown JS error".to_string());
            io::Error::other(format!("JS callback failed: {msg}"))
        })?;
        Ok(buf.len())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("WebOutputWriter is only usable on wasm32"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
