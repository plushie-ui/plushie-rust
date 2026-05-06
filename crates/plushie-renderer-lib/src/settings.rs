//! Shared settings parsing logic for plushie renderer startup.
//!
//! Both native and WASM entry points need to extract iced settings,
//! font data, and validation flags from the host's Settings JSON message.
//! This module centralizes that parsing so each platform only handles
//! platform-specific concerns (file I/O, environment variables, etc.).

use serde_json::Value;

/// Parse iced-level settings from the host's Settings JSON.
///
/// Extracts `antialiasing`, `vsync`, `default_text_size`, and
/// `default_font` from the settings object and returns a configured
/// `iced::Settings`. Fields that are absent or have invalid types
/// fall back to iced defaults (antialiasing off, vsync on).
///
/// Called early in the startup flow, before the iced daemon launches.
pub fn parse_iced_settings(settings: &Value) -> iced::Settings {
    let antialiasing = settings
        .get("antialiasing")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let vsync = settings
        .get("vsync")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let default_text_size = settings
        .get("default_text_size")
        .and_then(|v| v.as_f64())
        .map(|s| iced::Pixels(s as f32));
    let default_font = settings.get("default_font").map(|v| {
        let family = v.get("family").and_then(|f| f.as_str());
        if family == Some("monospace") {
            iced::Font::MONOSPACE
        } else {
            iced::Font::DEFAULT
        }
    });

    let mut iced_settings = iced::Settings {
        antialiasing,
        vsync,
        ..Default::default()
    };
    if let Some(size) = default_text_size {
        iced_settings.default_text_size = size;
    }
    if let Some(font) = default_font {
        iced_settings.default_font = font;
    }
    iced_settings
}

/// Validate the `required_widgets` list against the renderer's
/// registered widgets.
///
/// Looks at the `required_widgets` array in the Settings JSON and
/// emits a [`plushie_core::diagnostic::Diagnostic::RequiredWidgetsMissing`] diagnostic listing
/// any type names the renderer does not know about. Both the built-in
/// widget set and the `native_widgets` argument contribute to the
/// "known" pool. Non-fatal: the caller keeps running regardless.
pub fn validate_required_widgets(settings: &Value, native_widgets: &[&str]) {
    let Some(required) = settings.get("required_widgets").and_then(|v| v.as_array()) else {
        return;
    };
    if required.is_empty() {
        return;
    }
    let builtin = plushie_widget_sdk::runtime::IcedWidgetSet::type_names();
    let mut known: std::collections::HashSet<&str> = builtin.iter().map(|s| s.as_ref()).collect();
    known.extend(native_widgets.iter().copied());
    let mut missing = Vec::new();
    for item in required {
        if let Some(name) = item.as_str()
            && !known.contains(name)
        {
            missing.push(name.to_string());
        }
    }
    if !missing.is_empty() {
        plushie_widget_sdk::diagnostics::warn(plushie_core::Diagnostic::RequiredWidgetsMissing {
            missing,
        });
    }
}

/// Enable prop validation if the host requested it.
///
/// Checks for `validate_props: true` in the settings JSON and, if
/// present, enables debug-mode prop validation globally via
/// `plushie_widget_sdk::runtime::set_validate_props`. The flag is backed by
/// a `OnceLock` and can only be set once per process lifetime.
///
/// Called during startup, after the Settings message is parsed.
/// Returns `true` if the settings asked for validation; tests use
/// the return value to verify the parse without poking the
/// process-wide OnceLock.
pub fn apply_validate_props(settings: &Value) -> bool {
    let requested = settings
        .get("validate_props")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if requested {
        plushie_widget_sdk::runtime::set_validate_props(true);
        log::info!("prop validation enabled via settings");
    }
    requested
}

/// Decode font data from a JSON value.
///
/// Supports two wire formats:
///
/// - **String**: base64-encoded binary data. This is the standard JSON
///   wire format used when the host sends font bytes as a string field.
/// - **Array of numbers**: raw byte values (0-255). This is the format
///   that results when MessagePack binary data passes through the
///   codec's `rmpv -> serde_json` conversion path.
///
/// Returns `None` if the value is neither a string nor an array, if
/// base64 decoding fails, or if an array element is not a valid u8.
pub fn decode_font_data(value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::String(s) => {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.decode(s).ok()
        }
        Value::Array(arr) => {
            let bytes: Vec<u8> = arr
                .iter()
                .filter_map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
                .collect();
            if bytes.len() == arr.len() {
                Some(bytes)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Parse inline font data from the `fonts` array in settings.
///
/// Each element in the `fonts` array can be:
///
/// - A plain string representing a file path. These are skipped here
///   and handled by platform-specific code (native reads from disk,
///   WASM fetches over HTTP, etc.).
/// - An object with a `data` field containing inline font bytes,
///   decoded via [`decode_font_data`].
///
/// Returns a `Vec` of decoded font byte buffers, ready to be passed
/// to `iced::font::load`. Called during startup after the Settings
/// message is parsed, before the iced daemon launches.
///
/// The returned list respects
/// [`crate::constants::MAX_LOADED_FONTS`]: any inputs beyond the
/// remaining font-slot budget are dropped and a `font_cap_exceeded`
/// warning is logged with the excess count. Each font that makes it
/// through increments the shared
/// [`crate::constants::LOADED_FONT_COUNT`] atomic so later calls
/// (both dynamic `load_font` ops and subsequent `parse_inline_fonts`
/// passes) share the budget.
pub fn parse_inline_fonts(settings: &Value) -> Vec<Vec<u8>> {
    use std::sync::atomic::Ordering;

    let Some(fonts) = settings.get("fonts").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut decoded = Vec::new();
    for font_val in fonts {
        if let Some(obj) = font_val.as_object()
            && let Some(data_val) = obj.get("data")
        {
            match decode_font_data(data_val) {
                Some(bytes) if bytes.is_empty() => {
                    log::warn!("fonts: empty inline font data, skipping");
                }
                Some(bytes) => {
                    log::info!("loaded inline font ({} bytes)", bytes.len());
                    decoded.push(bytes);
                }
                None => {
                    log::warn!("fonts: failed to decode inline font data");
                }
            }
        }
        // Plain strings are file paths, handled by platform-specific code
    }

    // Enforce the process-wide cap. Reserving budget atomically via
    // fetch_add race-loop ensures concurrent callers (unlikely at
    // startup but possible in multi-session setups) don't oversubscribe.
    let requested = decoded.len() as u32;
    if requested == 0 {
        return decoded;
    }
    let max = crate::constants::MAX_LOADED_FONTS;
    let counter = &crate::constants::LOADED_FONT_COUNT;
    let mut current = counter.load(Ordering::Relaxed);
    let granted = loop {
        let budget = max.saturating_sub(current);
        let grant = requested.min(budget);
        if grant == 0 {
            break 0;
        }
        match counter.compare_exchange(
            current,
            current + grant,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break grant,
            Err(actual) => current = actual,
        }
    };
    if granted < requested {
        let dropped = requested - granted;
        plushie_widget_sdk::diagnostics::warn(plushie_core::Diagnostic::FontCapExceeded {
            max,
            requested,
            granted,
            dropped,
        });
        decoded.truncate(granted as usize);
    }
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::Ordering;

    /// Serialise tests that mutate the shared font counter. Without
    /// this they race when cargo test runs them in parallel.
    static FONT_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Reset the shared font counter so ordering between tests in
    /// this module doesn't matter. Each test saturates the counter
    /// independently.
    fn reset_font_counter() {
        crate::constants::LOADED_FONT_COUNT.store(0, Ordering::Relaxed);
    }

    #[test]
    fn inline_fonts_under_cap_pass_through() {
        let _guard = FONT_TEST_LOCK.lock().unwrap();
        reset_font_counter();
        // Build 3 tiny inline fonts. Each is just a non-empty blob;
        // parse_inline_fonts doesn't validate the format.
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"font-bytes");
        let settings = serde_json::json!({
            "fonts": [
                {"data": b64.clone()},
                {"data": b64.clone()},
                {"data": b64},
            ]
        });
        let out = parse_inline_fonts(&settings);
        assert_eq!(out.len(), 3);
        assert_eq!(
            crate::constants::LOADED_FONT_COUNT.load(Ordering::Relaxed),
            3
        );
    }

    #[test]
    fn inline_fonts_past_cap_are_dropped_with_diagnostic() {
        let _guard = FONT_TEST_LOCK.lock().unwrap();
        reset_font_counter();
        // Pre-saturate the counter to MAX_LOADED_FONTS - 2 so only
        // 2 fonts fit.
        let max = crate::constants::MAX_LOADED_FONTS;
        crate::constants::LOADED_FONT_COUNT.store(max - 2, Ordering::Relaxed);

        let b64 = base64::engine::general_purpose::STANDARD.encode(b"font-bytes");
        let mut fonts = Vec::new();
        for _ in 0..10 {
            fonts.push(serde_json::json!({"data": b64.clone()}));
        }
        let settings = serde_json::json!({"fonts": fonts});

        let out = parse_inline_fonts(&settings);
        assert_eq!(out.len(), 2, "only 2 slots remained under the cap");
        assert_eq!(
            crate::constants::LOADED_FONT_COUNT.load(Ordering::Relaxed),
            max
        );
    }

    use base64::Engine;

    #[test]
    fn apply_validate_props_returns_false_when_unset() {
        let settings = serde_json::json!({});
        assert!(!apply_validate_props(&settings));
    }

    #[test]
    fn apply_validate_props_returns_false_when_explicitly_false() {
        let settings = serde_json::json!({"validate_props": false});
        assert!(!apply_validate_props(&settings));
    }

    #[test]
    fn apply_validate_props_returns_true_when_requested() {
        let settings = serde_json::json!({"validate_props": true});
        // The OnceLock is process-wide; apply_validate_props returning
        // true is the observable behaviour we care about here. The
        // side effect on the lock is exercised in startup-mode tests
        // that run in their own subprocess.
        assert!(apply_validate_props(&settings));
    }
}
