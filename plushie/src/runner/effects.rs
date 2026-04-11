//! Direct mode effect handlers (file dialogs, clipboard, notifications).
//!
//! Provides the [`DirectEffectHandler`] which implements the renderer-lib's
//! [`EffectHandler`](plushie_renderer_lib::EffectHandler) trait for
//! in-process effect execution.

#[cfg(feature = "direct")]
use serde_json::{Value, json};

/// Effect handler for direct mode. Executes effects in-process using
/// rfd (file dialogs), arboard (clipboard), and notify-rust (notifications).
#[cfg(feature = "direct")]
pub(crate) struct DirectEffectHandler;

#[cfg(feature = "direct")]
impl plushie_renderer_lib::EffectHandler for DirectEffectHandler {
    fn handle_sync(
        &self,
        id: &str,
        request: &plushie_core::ops::EffectRequest,
    ) -> Option<plushie_widget_sdk::protocol::EffectResponse> {
        use plushie_widget_sdk::protocol::EffectResponse;
        let (kind, payload) = plushie_core::ops::effect_request_to_wire(request);
        let (status, result) = dispatch_sync(kind, &payload);
        Some(match status.as_str() {
            "ok" => EffectResponse::ok(id.to_string(), result),
            "cancelled" => EffectResponse::cancelled(id.to_string()),
            _ => EffectResponse::error(id.to_string(), status),
        })
    }

    fn handle_async(
        &self,
        id: String,
        request: plushie_core::ops::EffectRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = plushie_widget_sdk::protocol::EffectResponse> + Send>> {
        use plushie_widget_sdk::protocol::EffectResponse;
        let (kind, payload) = plushie_core::ops::effect_request_to_wire(&request);
        let kind = kind.to_string();
        Box::pin(async move {
            let (status, result) = dispatch_async(&kind, &payload).await;
            match status.as_str() {
                "ok" => EffectResponse::ok(id, result),
                "cancelled" => EffectResponse::cancelled(id),
                _ => EffectResponse::error(id, status),
            }
        })
    }

    fn is_async(&self, request: &plushie_core::ops::EffectRequest) -> bool {
        use plushie_core::ops::EffectRequest;
        matches!(request,
            EffectRequest::FileOpen(_)
            | EffectRequest::FileOpenMultiple(_)
            | EffectRequest::FileSave(_)
            | EffectRequest::DirectorySelect(_)
            | EffectRequest::DirectorySelectMultiple(_)
        )
    }
}

// ---------------------------------------------------------------------------
// Sync dispatch
// ---------------------------------------------------------------------------

#[cfg(feature = "direct")]
fn dispatch_sync(kind: &str, payload: &Value) -> (String, Value) {
    match kind {
        "clipboard_read" => clipboard_read(),
        "clipboard_write" => clipboard_write(payload),
        "clipboard_read_html" => clipboard_read_html(),
        "clipboard_write_html" => clipboard_write_html(payload),
        "clipboard_clear" => clipboard_clear(),
        "clipboard_read_primary" => clipboard_read_primary(),
        "clipboard_write_primary" => clipboard_write_primary(payload),
        "notification" => notification(payload),
        // File dialogs: fall back to sync rfd::FileDialog.
        "file_open" => file_dialog_sync(payload, "Open File", DialogKind::OpenFile),
        "file_open_multiple" => file_dialog_sync(payload, "Open Files", DialogKind::OpenMultiple),
        "file_save" => file_dialog_sync(payload, "Save File", DialogKind::Save),
        "directory_select" => file_dialog_sync(payload, "Select Directory", DialogKind::PickFolder),
        "directory_select_multiple" => file_dialog_sync(payload, "Select Directories", DialogKind::PickFolders),
        _ => ("unsupported".to_string(), json!(null)),
    }
}

// ---------------------------------------------------------------------------
// Async dispatch (file dialogs only)
// ---------------------------------------------------------------------------

#[cfg(feature = "direct")]
async fn dispatch_async(kind: &str, payload: &Value) -> (String, Value) {
    match kind {
        "file_open" => file_dialog_async(payload, "Open File", DialogKind::OpenFile).await,
        "file_open_multiple" => file_dialog_async(payload, "Open Files", DialogKind::OpenMultiple).await,
        "file_save" => file_dialog_async(payload, "Save File", DialogKind::Save).await,
        "directory_select" => file_dialog_async(payload, "Select Directory", DialogKind::PickFolder).await,
        "directory_select_multiple" => file_dialog_async(payload, "Select Directories", DialogKind::PickFolders).await,
        _ => ("unsupported".to_string(), json!(null)),
    }
}

// ---------------------------------------------------------------------------
// File dialogs (rfd)
// ---------------------------------------------------------------------------

#[cfg(feature = "direct")]
enum DialogKind { OpenFile, OpenMultiple, Save, PickFolder, PickFolders }

#[cfg(feature = "direct")]
fn apply_params(dialog: rfd::FileDialog, payload: &Value, default_title: &str) -> rfd::FileDialog {
    let title = payload.get("title").and_then(|v| v.as_str()).unwrap_or(default_title);
    let mut d = dialog.set_title(title);
    if let Some(dir) = payload.get("directory").and_then(|v| v.as_str()) {
        d = d.set_directory(dir);
    }
    if let Some(name) = payload.get("default_name").and_then(|v| v.as_str()) {
        d = d.set_file_name(name);
    }
    if let Some(filters) = payload.get("filters").and_then(|v| v.as_array()) {
        for filter in filters {
            if let Some(pair) = filter.as_array()
                && pair.len() >= 2
                && let (Some(name), Some(ext)) = (pair[0].as_str(), pair[1].as_str())
            {
                let extensions: Vec<&str> = ext.split(';').map(|e| e.trim().trim_start_matches("*.")).collect();
                d = d.add_filter(name, &extensions);
            }
        }
    }
    d
}

#[cfg(feature = "direct")]
fn apply_params_async(dialog: rfd::AsyncFileDialog, payload: &Value, default_title: &str) -> rfd::AsyncFileDialog {
    let title = payload.get("title").and_then(|v| v.as_str()).unwrap_or(default_title);
    let mut d = dialog.set_title(title);
    if let Some(dir) = payload.get("directory").and_then(|v| v.as_str()) {
        d = d.set_directory(dir);
    }
    if let Some(name) = payload.get("default_name").and_then(|v| v.as_str()) {
        d = d.set_file_name(name);
    }
    if let Some(filters) = payload.get("filters").and_then(|v| v.as_array()) {
        for filter in filters {
            if let Some(pair) = filter.as_array()
                && pair.len() >= 2
                && let (Some(name), Some(ext)) = (pair[0].as_str(), pair[1].as_str())
            {
                let extensions: Vec<&str> = ext.split(';').map(|e| e.trim().trim_start_matches("*.")).collect();
                d = d.add_filter(name, &extensions);
            }
        }
    }
    d
}

#[cfg(feature = "direct")]
fn path_to_string(path: &std::path::Path) -> String {
    path.to_str().map(|s| s.to_string()).unwrap_or_else(|| {
        log::warn!("file path contains non-UTF-8 bytes: {}", path.display());
        path.to_string_lossy().into_owned()
    })
}

#[cfg(feature = "direct")]
fn file_dialog_sync(payload: &Value, title: &str, kind: DialogKind) -> (String, Value) {
    let d = apply_params(rfd::FileDialog::new(), payload, title);
    match kind {
        DialogKind::OpenFile => match d.pick_file() {
            Some(p) => ("ok".into(), json!({"path": path_to_string(&p)})),
            None => ("cancelled".into(), json!(null)),
        },
        DialogKind::OpenMultiple => match d.pick_files() {
            Some(ps) => ("ok".into(), json!({"paths": ps.iter().map(|p| path_to_string(p)).collect::<Vec<_>>()})),
            None => ("cancelled".into(), json!(null)),
        },
        DialogKind::Save => match d.save_file() {
            Some(p) => ("ok".into(), json!({"path": path_to_string(&p)})),
            None => ("cancelled".into(), json!(null)),
        },
        DialogKind::PickFolder => match d.pick_folder() {
            Some(p) => ("ok".into(), json!({"path": path_to_string(&p)})),
            None => ("cancelled".into(), json!(null)),
        },
        DialogKind::PickFolders => match d.pick_folders() {
            Some(ps) => ("ok".into(), json!({"paths": ps.iter().map(|p| path_to_string(p)).collect::<Vec<_>>()})),
            None => ("cancelled".into(), json!(null)),
        },
    }
}

#[cfg(feature = "direct")]
async fn file_dialog_async(payload: &Value, title: &str, kind: DialogKind) -> (String, Value) {
    let d = apply_params_async(rfd::AsyncFileDialog::new(), payload, title);
    match kind {
        DialogKind::OpenFile => match d.pick_file().await {
            Some(h) => ("ok".into(), json!({"path": path_to_string(h.path())})),
            None => ("cancelled".into(), json!(null)),
        },
        DialogKind::OpenMultiple => match d.pick_files().await {
            Some(hs) => ("ok".into(), json!({"paths": hs.iter().map(|h| path_to_string(h.path())).collect::<Vec<_>>()})),
            None => ("cancelled".into(), json!(null)),
        },
        DialogKind::Save => match d.save_file().await {
            Some(h) => ("ok".into(), json!({"path": path_to_string(h.path())})),
            None => ("cancelled".into(), json!(null)),
        },
        DialogKind::PickFolder => match d.pick_folder().await {
            Some(h) => ("ok".into(), json!({"path": path_to_string(h.path())})),
            None => ("cancelled".into(), json!(null)),
        },
        DialogKind::PickFolders => match d.pick_folders().await {
            Some(hs) => ("ok".into(), json!({"paths": hs.iter().map(|h| path_to_string(h.path())).collect::<Vec<_>>()})),
            None => ("cancelled".into(), json!(null)),
        },
    }
}

// ---------------------------------------------------------------------------
// Clipboard (arboard)
// ---------------------------------------------------------------------------

#[cfg(feature = "direct")]
fn with_clipboard(f: impl FnOnce(&mut arboard::Clipboard) -> (String, Value)) -> (String, Value) {
    use std::sync::Mutex;
    static CLIPBOARD: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

    let mut guard = CLIPBOARD.lock().unwrap_or_else(|p| p.into_inner());
    let clipboard = match guard.as_mut() {
        Some(c) => c,
        None => match arboard::Clipboard::new() {
            Ok(c) => { *guard = Some(c); guard.as_mut().unwrap() }
            Err(e) => return ("error".into(), json!(format!("clipboard init failed: {e}"))),
        },
    };
    f(clipboard)
}

#[cfg(feature = "direct")]
fn clipboard_read() -> (String, Value) {
    with_clipboard(|c| match c.get_text() {
        Ok(text) => ("ok".into(), json!({"text": text})),
        Err(e) => ("error".into(), json!(format!("clipboard read failed: {e}"))),
    })
}

#[cfg(feature = "direct")]
fn clipboard_write(payload: &Value) -> (String, Value) {
    let Some(text) = payload.get("text").and_then(|v| v.as_str()) else {
        return ("error".into(), json!("missing required field: text"));
    };
    let text = text.to_string();
    with_clipboard(|c| match c.set_text(text) {
        Ok(()) => ("ok".into(), json!(null)),
        Err(e) => ("error".into(), json!(format!("clipboard write failed: {e}"))),
    })
}

#[cfg(feature = "direct")]
fn clipboard_read_html() -> (String, Value) {
    with_clipboard(|c| match c.get().html() {
        Ok(html) => ("ok".into(), json!({"html": html})),
        Err(e) => ("error".into(), json!(format!("clipboard read html failed: {e}"))),
    })
}

#[cfg(feature = "direct")]
fn clipboard_write_html(payload: &Value) -> (String, Value) {
    let Some(html) = payload.get("html").and_then(|v| v.as_str()) else {
        return ("error".into(), json!("missing required field: html"));
    };
    let html = html.to_string();
    let alt = payload.get("alt_text").and_then(|v| v.as_str()).map(|s| s.to_string());
    with_clipboard(|c| match c.set_html(&html, alt.as_ref()) {
        Ok(()) => ("ok".into(), json!(null)),
        Err(e) => ("error".into(), json!(format!("clipboard write html failed: {e}"))),
    })
}

#[cfg(feature = "direct")]
fn clipboard_clear() -> (String, Value) {
    with_clipboard(|c| match c.clear() {
        Ok(()) => ("ok".into(), json!(null)),
        Err(e) => ("error".into(), json!(format!("clipboard clear failed: {e}"))),
    })
}

#[cfg(all(feature = "direct", target_os = "linux"))]
fn clipboard_read_primary() -> (String, Value) {
    use arboard::{GetExtLinux, LinuxClipboardKind};
    with_clipboard(|c| match c.get().clipboard(LinuxClipboardKind::Primary).text() {
        Ok(text) => ("ok".into(), json!({"text": text})),
        Err(e) => ("error".into(), json!(format!("primary clipboard read failed: {e}"))),
    })
}

#[cfg(all(feature = "direct", target_os = "linux"))]
fn clipboard_write_primary(payload: &Value) -> (String, Value) {
    use arboard::{SetExtLinux, LinuxClipboardKind};
    let Some(text) = payload.get("text").and_then(|v| v.as_str()) else {
        return ("error".into(), json!("missing required field: text"));
    };
    let text = text.to_string();
    with_clipboard(|c| match c.set().clipboard(LinuxClipboardKind::Primary).text(text) {
        Ok(()) => ("ok".into(), json!(null)),
        Err(e) => ("error".into(), json!(format!("primary clipboard write failed: {e}"))),
    })
}

#[cfg(all(feature = "direct", not(target_os = "linux")))]
fn clipboard_read_primary() -> (String, Value) { clipboard_read() }

#[cfg(all(feature = "direct", not(target_os = "linux")))]
fn clipboard_write_primary(payload: &Value) -> (String, Value) { clipboard_write(payload) }

// ---------------------------------------------------------------------------
// Notifications (notify-rust)
// ---------------------------------------------------------------------------

#[cfg(feature = "direct")]
fn notification(payload: &Value) -> (String, Value) {
    let title = payload.get("title").and_then(|v| v.as_str()).unwrap_or("Plushie");
    let body = payload.get("body").and_then(|v| v.as_str()).unwrap_or("");

    let mut n = notify_rust::Notification::new();
    n.summary(title).body(body);

    if let Some(icon) = payload.get("icon").and_then(|v| v.as_str()) { n.icon(icon); }
    if let Some(ms) = payload.get("timeout").and_then(|v| v.as_u64()) {
        n.timeout(notify_rust::Timeout::Milliseconds(ms.min(u32::MAX as u64) as u32));
    }
    #[cfg(target_os = "linux")]
    if let Some(urgency) = payload.get("urgency").and_then(|v| v.as_str()) {
        n.urgency(match urgency {
            "low" => notify_rust::Urgency::Low,
            "critical" => notify_rust::Urgency::Critical,
            _ => notify_rust::Urgency::Normal,
        });
    }
    if let Some(sound) = payload.get("sound").and_then(|v| v.as_str()) { n.sound_name(sound); }

    match n.show() {
        Ok(_) => ("ok".into(), json!(null)),
        Err(e) => ("error".into(), json!(format!("notification failed: {e}"))),
    }
}
