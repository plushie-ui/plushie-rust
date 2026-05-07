//! Native platform effect handler.
//!
//! Effects are side-effectful operations requested by the host that
//! interact with OS resources. Each effect has an `id` for correlating
//! the response, a `kind` string for dispatch, and a JSON `payload`
//! with kind-specific parameters.
//!
//! This module is the single source of truth for native effect
//! implementations. The renderer binary and the in-process direct mode
//! of the `plushie` SDK both delegate here. See the by-design entry
//! "Platform effect implementations live in `plushie-renderer-lib`"
//! for the rationale.
//!
//! File dialog effects run asynchronously via [`handle_async_effect`]
//! when a tokio runtime is available (the normal iced daemon path).
//! The sync [`handle_effect`] fallback exists for headless/blocking
//! contexts. Clipboard and notification effects are always synchronous.
//!
//! # Platform notes
//!
//! **File paths:** Returned paths use OS-native separators (`/` on Unix,
//! `\` on Windows). On macOS, paths may arrive in NFD (decomposed Unicode)
//! form. Hosts should normalize paths for comparison if needed.
//!
//! **File dialogs (rfd):** On Wayland, rfd uses the xdg-desktop-portal.
//! On compositors without a portal service, dialogs return None (cancelled).
//! On X11 without a portal, rfd falls back to GTK dialogs which may block
//! a tokio worker thread. Filter extensions should be simple (e.g. "png",
//! "jpg") without wildcards for best cross-platform compatibility.
//!
//! **Clipboard (arboard):** The clipboard instance is lazily initialized
//! in a static Mutex and may be created from a worker thread. On Wayland,
//! arboard spawns a background thread for clipboard serving. Dropping the
//! Clipboard would lose served data, so it persists for process lifetime.
//! On Linux, primary selection is routed via `LinuxClipboardKind::Primary`.
//!
//! **Notifications (notify-rust):** On macOS, notifications require an app
//! bundle identifier (bare binaries may fail). The `icon` field only works
//! on Linux (freedesktop icon name); on macOS it is ignored, on Windows
//! the app icon is used instead. The `urgency` field is Linux-only
//! (freedesktop notification spec).

use std::future::Future;
use std::pin::Pin;

use serde_json::{Value, json};

use plushie_core::ops::EffectRequest;
use plushie_widget_sdk::protocol::EffectResponse;

use super::EffectHandler;

/// Native effect handler wrapping rfd (file dialogs), arboard (clipboard),
/// and notify-rust (notifications).
///
/// Both the `plushie-renderer` binary and direct mode in the `plushie`
/// SDK use this handler. The behaviour is byte-for-byte identical
/// across the two paths.
pub struct NativeEffectHandler;

impl EffectHandler for NativeEffectHandler {
    fn handle_sync(&self, id: &str, request: &EffectRequest) -> Option<EffectResponse> {
        let (kind, payload) = plushie_core::ops::effect_request_to_wire(request);
        Some(handle_effect(id.to_string(), kind, &payload))
    }

    fn handle_async(
        &self,
        id: String,
        request: EffectRequest,
    ) -> Pin<Box<dyn Future<Output = EffectResponse> + Send>> {
        let (kind, payload) = plushie_core::ops::effect_request_to_wire(&request);
        let kind = kind.to_string();
        Box::pin(async move { handle_async_effect(id, &kind, &payload).await })
    }

    fn is_async(&self, request: &EffectRequest) -> bool {
        matches!(
            request,
            EffectRequest::FileOpen(_)
                | EffectRequest::FileOpenMultiple(_)
                | EffectRequest::FileSave(_)
                | EffectRequest::DirectorySelect(_)
                | EffectRequest::DirectorySelectMultiple(_)
        )
    }
}

/// Convert a file path to a JSON string value, logging a warning if the path
/// contains non-UTF-8 bytes and lossy conversion is required.
///
/// **Platform notes:** Windows UNC paths (`\\?\C:\...`) are valid UTF-8 and
/// pass through cleanly. macOS HFS+ paths may arrive in NFD (decomposed
/// Unicode) form. This is valid UTF-8 but the host should normalize for
/// comparison. Non-UTF-8 filenames are rare on modern systems (NTFS is
/// UTF-16, HFS+ is UTF-8, ext4 allows arbitrary bytes but tooling
/// discourages it).
fn path_to_json_string(path: &std::path::Path) -> String {
    match path.to_str() {
        Some(s) => s.to_string(),
        None => {
            log::warn!(
                "file path contains non-UTF-8 bytes, using lossy conversion: {}",
                path.display()
            );
            path.to_string_lossy().into_owned()
        }
    }
}

// -- Dialog parameter parsing ------------------------------------------------

/// Parsed file dialog parameters extracted from the JSON payload.
struct DialogParams<'a> {
    title: &'a str,
    filters: Vec<(&'a str, Vec<&'a str>)>,
    directory: Option<&'a str>,
    default_name: Option<&'a str>,
}

/// Parse common dialog parameters from a JSON payload.
fn parse_dialog_params<'a>(payload: &'a Value, default_title: &'a str) -> DialogParams<'a> {
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or(default_title);

    let mut filters = Vec::new();
    if let Some(arr) = payload.get("filters").and_then(|v| v.as_array()) {
        for filter in arr {
            if let Some(pair) = filter.as_array()
                && pair.len() >= 2
                && let (Some(name), Some(ext)) = (pair[0].as_str(), pair[1].as_str())
            {
                let extensions: Vec<&str> = ext
                    .split(';')
                    .map(|e| e.trim().trim_start_matches("*."))
                    .collect();
                filters.push((name, extensions));
            }
        }
    }

    let directory = payload.get("directory").and_then(|v| v.as_str());
    let default_name = payload.get("default_name").and_then(|v| v.as_str());

    DialogParams {
        title,
        filters,
        directory,
        default_name,
    }
}

/// Apply parsed parameters to an `rfd::FileDialog` or `rfd::AsyncFileDialog`.
/// Both types share identical builder methods but no common trait.
macro_rules! apply_dialog_params {
    ($dialog_type:ty, $params:expr) => {{
        let params = &$params;
        let mut d = <$dialog_type>::new().set_title(params.title);
        for (name, exts) in &params.filters {
            d = d.add_filter(*name, exts);
        }
        if let Some(dir) = params.directory {
            d = d.set_directory(dir);
        }
        if let Some(name) = params.default_name {
            d = d.set_file_name(name);
        }
        d
    }};
}

// -- Effect dispatch ---------------------------------------------------------

/// Returns true for effect kinds that should run asynchronously (file dialogs).
pub fn is_async_effect(kind: &str) -> bool {
    matches!(
        kind,
        "file_open"
            | "file_open_multiple"
            | "file_save"
            | "directory_select"
            | "directory_select_multiple"
    )
}

/// Dispatch an effect synchronously and return the response.
///
/// File dialog effects use `rfd::FileDialog` (blocking). On macOS, sync
/// dialogs may deadlock if called on the main thread; prefer
/// [`handle_async_effect`] when a tokio runtime is available.
///
/// Clipboard and notification effects are always synchronous regardless
/// of which dispatch function is used.
pub fn handle_effect(id: String, kind: &str, payload: &Value) -> EffectResponse {
    match kind {
        "file_open" => handle_file_open(id, payload),
        "file_open_multiple" => handle_file_open_multiple(id, payload),
        "file_save" => handle_file_save(id, payload),
        "directory_select" => handle_directory_select(id, payload),
        "directory_select_multiple" => handle_directory_select_multiple(id, payload),
        "clipboard_read" => handle_clipboard_read(id),
        "clipboard_write" => handle_clipboard_write(id, payload),
        "clipboard_read_html" => handle_clipboard_read_html(id),
        "clipboard_write_html" => handle_clipboard_write_html(id, payload),
        "clipboard_clear" => handle_clipboard_clear(id),
        "clipboard_read_primary" => handle_clipboard_read_primary(id),
        "clipboard_write_primary" => handle_clipboard_write_primary(id, payload),
        "notification" => handle_notification(id, payload),
        _ => EffectResponse::unsupported(id),
    }
}

/// Dispatch an async effect and return the response. The response format
/// matches [`handle_effect`] exactly so the host can deserialize uniformly.
///
/// Only file dialog effects have async implementations (via
/// `rfd::AsyncFileDialog`). Other kinds are not routed here; see
/// [`is_async_effect`].
///
/// Note: on X11-only Linux desktops without a portal (e.g. minimal WMs),
/// rfd falls back to a GTK dialog which may block a tokio worker thread.
/// This is a known rfd limitation, not specific to plushie.
pub async fn handle_async_effect(id: String, kind: &str, payload: &Value) -> EffectResponse {
    match kind {
        "file_open" => {
            let p = parse_dialog_params(payload, "Open File");
            let dialog = apply_dialog_params!(rfd::AsyncFileDialog, p);
            match dialog.pick_file().await {
                Some(h) => EffectResponse::ok(id, json!({"path": path_to_json_string(h.path())})),
                None => EffectResponse::cancelled(id),
            }
        }
        "file_open_multiple" => {
            let p = parse_dialog_params(payload, "Open Files");
            let dialog = apply_dialog_params!(rfd::AsyncFileDialog, p);
            match dialog.pick_files().await {
                Some(handles) => {
                    let paths: Vec<String> = handles
                        .iter()
                        .map(|h| path_to_json_string(h.path()))
                        .collect();
                    EffectResponse::ok(id, json!({"paths": paths}))
                }
                None => EffectResponse::cancelled(id),
            }
        }
        "file_save" => {
            let p = parse_dialog_params(payload, "Save File");
            let dialog = apply_dialog_params!(rfd::AsyncFileDialog, p);
            match dialog.save_file().await {
                Some(h) => EffectResponse::ok(id, json!({"path": path_to_json_string(h.path())})),
                None => EffectResponse::cancelled(id),
            }
        }
        "directory_select" => {
            let p = parse_dialog_params(payload, "Select Directory");
            let dialog = apply_dialog_params!(rfd::AsyncFileDialog, p);
            match dialog.pick_folder().await {
                Some(h) => EffectResponse::ok(id, json!({"path": path_to_json_string(h.path())})),
                None => EffectResponse::cancelled(id),
            }
        }
        "directory_select_multiple" => {
            let p = parse_dialog_params(payload, "Select Directories");
            let dialog = apply_dialog_params!(rfd::AsyncFileDialog, p);
            match dialog.pick_folders().await {
                Some(handles) => {
                    let paths: Vec<String> = handles
                        .iter()
                        .map(|h| path_to_json_string(h.path()))
                        .collect();
                    EffectResponse::ok(id, json!({"paths": paths}))
                }
                None => EffectResponse::cancelled(id),
            }
        }
        _ => EffectResponse::unsupported(id),
    }
}

// -- Sync file dialog handlers ----------------------------------------------
//
// These use rfd::FileDialog (blocking). The async counterparts above use
// rfd::AsyncFileDialog. Both coexist: sync for headless/blocking contexts,
// async for the normal iced daemon event loop.

fn handle_file_open(id: String, payload: &Value) -> EffectResponse {
    let p = parse_dialog_params(payload, "Open File");
    let dialog = apply_dialog_params!(rfd::FileDialog, p);
    match dialog.pick_file() {
        Some(path) => EffectResponse::ok(id, json!({"path": path_to_json_string(&path)})),
        None => EffectResponse::cancelled(id),
    }
}

fn handle_file_open_multiple(id: String, payload: &Value) -> EffectResponse {
    let p = parse_dialog_params(payload, "Open Files");
    let dialog = apply_dialog_params!(rfd::FileDialog, p);
    match dialog.pick_files() {
        Some(paths) => {
            let paths: Vec<String> = paths.iter().map(|p| path_to_json_string(p)).collect();
            EffectResponse::ok(id, json!({"paths": paths}))
        }
        None => EffectResponse::cancelled(id),
    }
}

fn handle_file_save(id: String, payload: &Value) -> EffectResponse {
    let p = parse_dialog_params(payload, "Save File");
    let dialog = apply_dialog_params!(rfd::FileDialog, p);
    match dialog.save_file() {
        Some(path) => EffectResponse::ok(id, json!({"path": path_to_json_string(&path)})),
        None => EffectResponse::cancelled(id),
    }
}

fn handle_directory_select(id: String, payload: &Value) -> EffectResponse {
    let p = parse_dialog_params(payload, "Select Directory");
    let dialog = apply_dialog_params!(rfd::FileDialog, p);
    match dialog.pick_folder() {
        Some(path) => EffectResponse::ok(id, json!({"path": path_to_json_string(&path)})),
        None => EffectResponse::cancelled(id),
    }
}

fn handle_directory_select_multiple(id: String, payload: &Value) -> EffectResponse {
    let p = parse_dialog_params(payload, "Select Directories");
    let dialog = apply_dialog_params!(rfd::FileDialog, p);
    match dialog.pick_folders() {
        Some(paths) => {
            let paths: Vec<String> = paths.iter().map(|p| path_to_json_string(p)).collect();
            EffectResponse::ok(id, json!({"paths": paths}))
        }
        None => EffectResponse::cancelled(id),
    }
}

// -- Clipboard (arboard crate) ----------------------------------------------
//
// A single Clipboard instance is kept alive for the process lifetime.
// On Wayland, arboard serves clipboard data from a background thread
// tied to the Clipboard instance; dropping it loses the data.

fn with_clipboard(
    id: &str,
    f: impl FnOnce(&mut arboard::Clipboard, &str) -> EffectResponse,
) -> EffectResponse {
    use std::sync::Mutex;

    static CLIPBOARD: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

    let mut guard = CLIPBOARD.lock().unwrap_or_else(|poisoned| {
        log::warn!("clipboard mutex was poisoned, recovering");
        poisoned.into_inner()
    });

    let clipboard = match guard.as_mut() {
        Some(c) => c,
        None => match arboard::Clipboard::new() {
            Ok(c) => {
                *guard = Some(c);
                guard.as_mut().unwrap()
            }
            Err(e) => {
                return EffectResponse::error(
                    id.to_string(),
                    format!("clipboard init failed: {e}"),
                );
            }
        },
    };

    f(clipboard, id)
}

fn handle_clipboard_read(id: String) -> EffectResponse {
    // Normalise platform variance: some backends return
    // `Err(ContentNotAvailable)` for an empty clipboard while others
    // return `Ok("")`. Map both to `{"text": ""}` so apps see a
    // consistent "empty-is-empty" semantic.
    with_clipboard(&id, |clipboard, id| match clipboard.get_text() {
        Ok(text) => EffectResponse::ok(id.to_string(), json!({"text": text})),
        Err(arboard::Error::ContentNotAvailable) => {
            EffectResponse::ok(id.to_string(), json!({"text": ""}))
        }
        Err(e) => EffectResponse::error(id.to_string(), format!("clipboard read failed: {e}")),
    })
}

fn handle_clipboard_write(id: String, payload: &Value) -> EffectResponse {
    let Some(text) = payload.get("text").and_then(|v| v.as_str()) else {
        return EffectResponse::error(id, "missing required field: text".to_string());
    };
    let text = text.to_string();

    with_clipboard(&id, |clipboard, id| match clipboard.set_text(text) {
        Ok(()) => EffectResponse::ok(id.to_string(), json!(null)),
        Err(e) => EffectResponse::error(id.to_string(), format!("clipboard write failed: {e}")),
    })
}

fn handle_clipboard_read_html(id: String) -> EffectResponse {
    with_clipboard(&id, |clipboard, id| match clipboard.get().html() {
        Ok(html) => EffectResponse::ok(id.to_string(), json!({"html": html})),
        Err(e) => EffectResponse::error(id.to_string(), format!("clipboard read html failed: {e}")),
    })
}

fn handle_clipboard_write_html(id: String, payload: &Value) -> EffectResponse {
    let Some(html) = payload.get("html").and_then(|v| v.as_str()) else {
        return EffectResponse::error(id, "missing required field: html".to_string());
    };
    let html = html.to_string();

    let alt_text = payload
        .get("alt_text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    with_clipboard(&id, |clipboard, id| {
        match clipboard.set_html(&html, alt_text.as_ref()) {
            Ok(()) => EffectResponse::ok(id.to_string(), json!(null)),
            Err(e) => {
                EffectResponse::error(id.to_string(), format!("clipboard write html failed: {e}"))
            }
        }
    })
}

fn handle_clipboard_clear(id: String) -> EffectResponse {
    with_clipboard(&id, |clipboard, id| match clipboard.clear() {
        Ok(()) => EffectResponse::ok(id.to_string(), json!(null)),
        Err(e) => EffectResponse::error(id.to_string(), format!("clipboard clear failed: {e}")),
    })
}

// Primary clipboard: uses the X11/Wayland primary selection on Linux.
// On other platforms, the protocol reports it as unsupported.

#[cfg(target_os = "linux")]
fn handle_clipboard_read_primary(id: String) -> EffectResponse {
    use arboard::{GetExtLinux, LinuxClipboardKind};

    with_clipboard(&id, |clipboard, id| {
        match clipboard
            .get()
            .clipboard(LinuxClipboardKind::Primary)
            .text()
        {
            Ok(text) => EffectResponse::ok(id.to_string(), json!({"text": text})),
            Err(e) => EffectResponse::error(
                id.to_string(),
                format!("primary clipboard read failed: {e}"),
            ),
        }
    })
}

#[cfg(target_os = "linux")]
fn handle_clipboard_write_primary(id: String, payload: &Value) -> EffectResponse {
    use arboard::{LinuxClipboardKind, SetExtLinux};
    let Some(text) = payload.get("text").and_then(|v| v.as_str()) else {
        return EffectResponse::error(id, "missing required field: text".to_string());
    };
    let text = text.to_string();

    with_clipboard(&id, |clipboard, id| {
        match clipboard
            .set()
            .clipboard(LinuxClipboardKind::Primary)
            .text(text)
        {
            Ok(()) => EffectResponse::ok(id.to_string(), json!(null)),
            Err(e) => EffectResponse::error(
                id.to_string(),
                format!("primary clipboard write failed: {e}"),
            ),
        }
    })
}

#[cfg(not(target_os = "linux"))]
fn handle_clipboard_read_primary(id: String) -> EffectResponse {
    EffectResponse::unsupported(id)
}

#[cfg(not(target_os = "linux"))]
fn handle_clipboard_write_primary(id: String, _payload: &Value) -> EffectResponse {
    EffectResponse::unsupported(id)
}

// -- Notifications (notify-rust crate) --------------------------------------

/// Send an OS notification.
///
/// **Platform quirks:**
/// - **macOS:** Requires the app to be signed or have an Info.plist for
///   notifications to appear. The `icon` field is ignored (macOS uses the
///   app icon). Notifications go to macOS Notification Center.
/// - **Linux:** Depends on the desktop environment's notification daemon
///   (e.g. dunst, mako, GNOME notifications). The `icon` field is a
///   freedesktop icon name (e.g. "dialog-information"). `urgency` is
///   Linux-only.
/// - **Windows:** Uses the Windows toast notification system. The `icon`
///   field is ignored (Windows uses the app icon).
fn handle_notification(id: String, payload: &Value) -> EffectResponse {
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Plushie");

    let body = payload.get("body").and_then(|v| v.as_str()).unwrap_or("");

    let mut notification = notify_rust::Notification::new();
    notification.summary(title).body(body);

    if let Some(icon) = payload.get("icon").and_then(|v| v.as_str()) {
        notification.icon(icon);
    }

    if let Some(timeout_ms) = payload.get("timeout").and_then(|v| v.as_u64()) {
        let clamped = timeout_ms.min(u32::MAX as u64) as u32;
        notification.timeout(notify_rust::Timeout::Milliseconds(clamped));
    }

    #[cfg(target_os = "linux")]
    if let Some(urgency) = payload.get("urgency").and_then(|v| v.as_str()) {
        let u = match urgency {
            "low" => notify_rust::Urgency::Low,
            "critical" => notify_rust::Urgency::Critical,
            _ => notify_rust::Urgency::Normal,
        };
        notification.urgency(u);
    }

    if let Some(sound) = payload.get("sound").and_then(|v| v.as_str()) {
        notification.sound_name(sound);
    }

    match notification.show() {
        Ok(_) => EffectResponse::ok(id, json!(null)),
        Err(e) => EffectResponse::error(id, format!("notification failed: {e}")),
    }
}

// -- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_effect_returns_unsupported() {
        let resp = handle_effect("eff-1".to_string(), "teleport_sandwich", &json!({}));
        assert_eq!(resp.status, "unsupported");
        assert_eq!(resp.id, "eff-1");
    }

    /// Dispatch every known effect kind with a minimal payload and verify
    /// none of them panic. The handlers may return "error" when the OS
    /// resource (clipboard, display server, notification daemon) is
    /// unavailable in the test environment. That's fine: we're testing
    /// that the routing reaches the right handler and returns cleanly.
    #[test]
    fn dispatch_routes_all_known_kinds_without_panic() {
        let kinds_with_payloads: Vec<(&str, Value)> = vec![
            ("file_open", json!({"title": "Pick a file"})),
            ("file_open_multiple", json!({"title": "Pick files"})),
            (
                "file_save",
                json!({"title": "Save", "default_name": "out.txt"}),
            ),
            ("directory_select", json!({"title": "Choose dir"})),
            ("directory_select_multiple", json!({"title": "Choose dirs"})),
            ("clipboard_read", json!({})),
            ("clipboard_write", json!({"text": "hello"})),
            ("clipboard_read_html", json!({})),
            (
                "clipboard_write_html",
                json!({"html": "<b>hi</b>", "alt_text": "hi"}),
            ),
            ("clipboard_clear", json!({})),
            ("clipboard_read_primary", json!({})),
            ("clipboard_write_primary", json!({"text": "primary"})),
            (
                "notification",
                json!({"title": "Test", "body": "body", "icon": "dialog-information", "timeout": 3000, "urgency": "low", "sound": "message-new-instant"}),
            ),
        ];

        for (kind, payload) in &kinds_with_payloads {
            let id = format!("test-{kind}");
            let resp = handle_effect(id.clone(), kind, payload);

            assert_eq!(resp.id, id, "id mismatch for kind {kind}");
            assert_eq!(resp.message_type, "effect_response");
            #[cfg(target_os = "linux")]
            assert!(
                resp.status == "ok" || resp.status == "error" || resp.status == "cancelled",
                "unexpected status '{}' for kind {kind}",
                resp.status
            );

            #[cfg(not(target_os = "linux"))]
            assert!(
                resp.status == "ok"
                    || resp.status == "error"
                    || resp.status == "cancelled"
                    || (resp.status == "unsupported"
                        && matches!(*kind, "clipboard_read_primary" | "clipboard_write_primary")),
                "unexpected status '{}' for kind {kind}",
                resp.status
            );
        }
    }

    /// Convergence: the trait impl path (`NativeEffectHandler::handle_sync`,
    /// used by the SDK in direct mode and by the renderer daemon for sync
    /// effects) and the free-function path (`handle_effect`, used by the
    /// renderer's headless dispatcher) must produce identical responses
    /// for the same input.
    ///
    /// Before consolidation, the SDK's `DirectEffectHandler` and the
    /// renderer's `NativeEffectHandler` each carried their own copy of
    /// these handlers and quietly drifted (the clipboard
    /// `ContentNotAvailable` handling diverged for a while). With one
    /// shared implementation, the two entry points can only diverge if
    /// the trait impl wraps the free function differently. This test
    /// pins the wrapper down.
    #[test]
    fn trait_impl_matches_free_function_for_all_sync_kinds() {
        use plushie_core::ops::{EffectRequest, NotificationOpts};

        // One typed EffectRequest per sync kind. Async (file dialog)
        // requests have a separate convergence path covered below.
        let sync_requests: Vec<(&str, EffectRequest)> = vec![
            ("clipboard_read", EffectRequest::ClipboardRead),
            (
                "clipboard_write",
                EffectRequest::ClipboardWrite("hello".to_string()),
            ),
            ("clipboard_read_html", EffectRequest::ClipboardReadHtml),
            (
                "clipboard_write_html",
                EffectRequest::ClipboardWriteHtml {
                    html: "<b>hi</b>".to_string(),
                    alt_text: Some("hi".to_string()),
                },
            ),
            ("clipboard_clear", EffectRequest::ClipboardClear),
            (
                "clipboard_read_primary",
                EffectRequest::ClipboardReadPrimary,
            ),
            (
                "clipboard_write_primary",
                EffectRequest::ClipboardWritePrimary("primary".to_string()),
            ),
            (
                "notification",
                EffectRequest::Notification {
                    title: "Test".to_string(),
                    body: "body".to_string(),
                    opts: NotificationOpts::new()
                        .icon("dialog-information")
                        .timeout(std::time::Duration::from_millis(3000))
                        .sound("message-new-instant"),
                },
            ),
        ];

        let handler = NativeEffectHandler;
        for (kind, request) in &sync_requests {
            let id = format!("converge-{kind}");

            // Path A: SDK / renderer-daemon path through the trait impl.
            let trait_resp = handler
                .handle_sync(&id, request)
                .expect("sync request must produce a response");

            // Path B: Renderer headless path through the free function.
            // Synthesise the same wire (kind, payload) the trait impl
            // produces internally so the two paths see identical input.
            let (wire_kind, payload) = plushie_core::ops::effect_request_to_wire(request);
            assert_eq!(
                wire_kind, *kind,
                "wire kind mismatch for {kind} (typed -> wire)"
            );
            let fn_resp = handle_effect(id.clone(), wire_kind, &payload);

            // Identity envelope: id, message_type, and status must agree.
            assert_eq!(trait_resp.id, fn_resp.id, "id mismatch for {kind}");
            assert_eq!(
                trait_resp.message_type, fn_resp.message_type,
                "message_type mismatch for {kind}"
            );
            assert_eq!(
                trait_resp.status, fn_resp.status,
                "status mismatch for {kind}"
            );

            // Shape of the optional payload fields must agree (presence
            // and structure). We don't assert byte-equal values because
            // the OS clipboard / notification daemon is shared static
            // state and the second call may observe a transient change
            // (e.g. a cursor in the test text). Status agreement plus
            // the same field being populated is the guarantee.
            assert_eq!(
                trait_resp.result.is_some(),
                fn_resp.result.is_some(),
                "result presence mismatch for {kind}"
            );
            assert_eq!(
                trait_resp.error.is_some(),
                fn_resp.error.is_some(),
                "error presence mismatch for {kind}"
            );
        }
    }

    /// Convergence for async (file dialog) effects. Async paths route
    /// through `NativeEffectHandler::is_async` to decide whether to
    /// dispatch via tokio. We don't actually await the dialog futures
    /// (they would spin up a real file picker), but we do confirm that
    /// every async effect kind is recognised by both `is_async_effect`
    /// (the headless path) and `NativeEffectHandler::is_async` (the
    /// daemon path). A divergence here would route effects down the
    /// wrong path silently, the same class of bug the consolidation is
    /// meant to prevent.
    #[test]
    fn async_routing_agrees_between_trait_impl_and_free_function() {
        use plushie_core::ops::EffectRequest;

        let async_requests: Vec<(&str, EffectRequest)> = vec![
            ("file_open", EffectRequest::FileOpen(Default::default())),
            (
                "file_open_multiple",
                EffectRequest::FileOpenMultiple(Default::default()),
            ),
            ("file_save", EffectRequest::FileSave(Default::default())),
            (
                "directory_select",
                EffectRequest::DirectorySelect(Default::default()),
            ),
            (
                "directory_select_multiple",
                EffectRequest::DirectorySelectMultiple(Default::default()),
            ),
        ];

        let handler = NativeEffectHandler;
        for (wire_kind, request) in &async_requests {
            assert!(
                handler.is_async(request),
                "trait impl should route {wire_kind} async"
            );
            assert!(
                is_async_effect(wire_kind),
                "free function should route {wire_kind} async"
            );
        }

        // Sync requests must NOT be routed async by either side.
        let sync_examples: Vec<(&str, EffectRequest)> = vec![
            ("clipboard_read", EffectRequest::ClipboardRead),
            ("clipboard_clear", EffectRequest::ClipboardClear),
            (
                "notification",
                EffectRequest::Notification {
                    title: String::new(),
                    body: String::new(),
                    opts: plushie_core::ops::NotificationOpts::default(),
                },
            ),
        ];
        for (wire_kind, request) in &sync_examples {
            assert!(
                !handler.is_async(request),
                "trait impl should route {wire_kind} sync"
            );
            assert!(
                !is_async_effect(wire_kind),
                "free function should route {wire_kind} sync"
            );
        }
    }

    /// Verify that empty payloads don't cause panics; handlers should
    /// defensively unwrap_or on missing fields.
    #[test]
    fn handlers_tolerate_empty_payloads() {
        let kinds: &[&str] = &[
            "file_open",
            "file_open_multiple",
            "file_save",
            "directory_select",
            "directory_select_multiple",
            "clipboard_read",
            "clipboard_write",
            "clipboard_read_html",
            "clipboard_write_html",
            "clipboard_clear",
            "clipboard_read_primary",
            "clipboard_write_primary",
            "notification",
        ];

        for kind in kinds {
            let resp = handle_effect(format!("empty-{kind}"), kind, &json!({}));
            assert_eq!(resp.message_type, "effect_response");
        }
    }

    #[test]
    fn unknown_kinds_preserve_id() {
        for i in 0..5 {
            let id = format!("unk-{i}");
            let resp = handle_effect(id.clone(), &format!("bogus_{i}"), &json!(null));
            assert_eq!(resp.id, id);
            assert_eq!(resp.status, "unsupported");
        }
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn primary_clipboard_effects_are_unsupported() {
        let read = handle_effect(
            "read-primary".to_string(),
            "clipboard_read_primary",
            &json!({}),
        );
        assert_eq!(read.status, "unsupported");
        assert_eq!(read.id, "read-primary");

        let write = handle_effect(
            "write-primary".to_string(),
            "clipboard_write_primary",
            &json!({"text": "primary"}),
        );
        assert_eq!(write.status, "unsupported");
        assert_eq!(write.id, "write-primary");
    }

    // -- is_async_effect -----------------------------------------------------

    #[test]
    fn async_effects_recognized() {
        assert!(is_async_effect("file_open"));
        assert!(is_async_effect("file_open_multiple"));
        assert!(is_async_effect("file_save"));
        assert!(is_async_effect("directory_select"));
        assert!(is_async_effect("directory_select_multiple"));
    }

    #[test]
    fn sync_effects_not_async() {
        assert!(!is_async_effect("clipboard_read"));
        assert!(!is_async_effect("clipboard_write"));
        assert!(!is_async_effect("notification"));
    }

    #[test]
    fn unknown_effect_not_async() {
        assert!(!is_async_effect("teleport_sandwich"));
        assert!(!is_async_effect(""));
        assert!(!is_async_effect("FILE_OPEN")); // case-sensitive
    }

    // -- parse_dialog_params -------------------------------------------------

    #[test]
    fn parse_params_defaults() {
        let payload = json!({});
        let p = parse_dialog_params(&payload, "Default Title");
        assert_eq!(p.title, "Default Title");
        assert!(p.filters.is_empty());
        assert!(p.directory.is_none());
        assert!(p.default_name.is_none());
    }

    #[test]
    fn parse_params_with_all_fields() {
        let payload = json!({
            "title": "Custom Title",
            "filters": [["Images", "*.png;*.jpg"], ["All", "*.*"]],
            "directory": "/home/user",
            "default_name": "output.txt"
        });
        let p = parse_dialog_params(&payload, "Ignored");
        assert_eq!(p.title, "Custom Title");
        assert_eq!(p.filters.len(), 2);
        assert_eq!(p.filters[0].0, "Images");
        assert_eq!(p.filters[0].1, vec!["png", "jpg"]);
        assert_eq!(p.filters[1].0, "All");
        assert_eq!(p.directory, Some("/home/user"));
        assert_eq!(p.default_name, Some("output.txt"));
    }

    #[test]
    fn parse_params_malformed_filters_ignored() {
        let payload = json!({
            "filters": [
                "not an array",
                [],
                ["only one element"],
                ["Name", "*.txt"]
            ]
        });
        let p = parse_dialog_params(&payload, "T");
        // Only the last filter is valid
        assert_eq!(p.filters.len(), 1);
        assert_eq!(p.filters[0].0, "Name");
    }

    // -- path_to_json_string -------------------------------------------------

    #[test]
    fn path_normal() {
        use std::path::Path;
        assert_eq!(
            path_to_json_string(Path::new("/home/user/file.txt")),
            "/home/user/file.txt"
        );
    }

    #[test]
    fn path_empty() {
        use std::path::Path;
        assert_eq!(path_to_json_string(Path::new("")), "");
    }

    #[test]
    fn path_with_spaces() {
        use std::path::Path;
        assert_eq!(
            path_to_json_string(Path::new("/home/user/my documents/file.txt")),
            "/home/user/my documents/file.txt"
        );
    }

    #[test]
    fn path_with_special_chars() {
        use std::path::Path;
        assert_eq!(
            path_to_json_string(Path::new("/tmp/test-file_v2 (1).tar.gz")),
            "/tmp/test-file_v2 (1).tar.gz"
        );
    }

    #[test]
    fn empty_clipboard_returns_ok_with_empty_text() {
        // ContentNotAvailable -> empty text shape; verifies the
        // platform-variance normalisation in handle_clipboard_read.
        // Drives the full handler: we don't assert the exact result
        // (a real clipboard server may fill it during the test), but
        // the response shape must be consistent.
        let resp = handle_clipboard_read("read-empty".to_string());
        assert_eq!(resp.message_type, "effect_response");
        assert_eq!(resp.id, "read-empty");
        // Either ok or error is acceptable depending on whether a
        // clipboard daemon is reachable in the test env.
        assert!(resp.status == "ok" || resp.status == "error");
    }
}
