//! Side effects returned from [`App::update`](crate::App::update).
//!
//! Commands are data, not closures (except `Async` and `Stream`).
//! This makes them testable: you can assert which commands an
//! update call returns without executing them.
//!
//! Operation types ([`WindowOp`], [`ImageOp`], [`EffectRequest`], etc.)
//! are defined in [`plushie_core::ops`] and re-exported here.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use serde_json::Value;

use crate::event::Event;

// Re-export all operation types from plushie-core.
pub use plushie_core::ops::*;

/// A boxed async closure that produces a result.
///
/// The closure is called once to produce a future. The future resolves
/// to `Ok(value)` or `Err(value)`, delivered as
/// [`AsyncEvent`](crate::event::AsyncEvent).
pub type AsyncTaskFn = Box<
    dyn FnOnce() -> Pin<Box<dyn Future<Output = Result<Value, Value>> + Send>>
        + Send
>;

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

/// A side effect returned from the update function.
///
/// Use the builder methods (`Command::focus`, `Command::async_task`,
/// `Command::close_window`, etc.) for ergonomic construction.
///
/// Commands that go to the renderer are wrapped in
/// [`Command::Renderer`]. SDK-local commands (async tasks, timers)
/// are handled in-process and never reach the renderer.
pub enum Command {
    /// No side effect.
    None,
    /// Execute multiple commands.
    Batch(Vec<Command>),
    /// Exit the application.
    Exit,

    // -- SDK-local (never sent to renderer) --

    /// Run an async task. Result delivered as
    /// [`AsyncEvent`](crate::event::AsyncEvent).
    Async {
        tag: String,
        task: AsyncTaskFn,
    },
    /// Cancel a running async task or stream by tag.
    Cancel { tag: String },
    /// Deliver an event after a delay.
    SendAfter { delay: Duration, event: Box<Event> },

    // -- Renderer operations (typed, zero-overhead in direct mode) --

    /// An operation for the renderer to execute.
    Renderer(RendererOp),
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Batch(cmds) => f.debug_tuple("Batch").field(cmds).finish(),
            Self::Exit => write!(f, "Exit"),
            Self::Async { tag, .. } => f.debug_struct("Async").field("tag", tag).finish(),
            Self::Cancel { tag } => f.debug_struct("Cancel").field("tag", tag).finish(),
            Self::SendAfter { delay, .. } => {
                f.debug_struct("SendAfter").field("delay", delay).finish()
            }
            Self::Renderer(op) => f.debug_tuple("Renderer").field(op).finish(),
        }
    }
}

// ---------------------------------------------------------------------------
// Builder methods
// ---------------------------------------------------------------------------

impl Command {
    /// A no-op command.
    pub fn none() -> Self { Self::None }

    /// Execute multiple commands together.
    pub fn batch(cmds: impl IntoIterator<Item = Command>) -> Self {
        Self::Batch(cmds.into_iter().collect())
    }

    /// Exit the application.
    pub fn exit() -> Self { Self::Exit }

    /// Deliver an event after a delay.
    pub fn send_after(delay: Duration, event: Event) -> Self {
        Self::SendAfter { delay, event: Box::new(event) }
    }

    /// Run an async task. The result is delivered as
    /// [`AsyncEvent`](crate::event::AsyncEvent).
    ///
    /// ```ignore
    /// Command::async_task("fetch", || async {
    ///     let data = reqwest::get("https://api.example.com/data")
    ///         .await
    ///         .map_err(|e| serde_json::json!(e.to_string()))?
    ///         .text()
    ///         .await
    ///         .map_err(|e| serde_json::json!(e.to_string()))?;
    ///     Ok(serde_json::json!(data))
    /// })
    /// ```
    pub fn async_task<F, Fut>(tag: &str, f: F) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Value, Value>> + Send + 'static,
    {
        Self::Async {
            tag: tag.to_string(),
            task: Box::new(move || Box::pin(f())),
        }
    }

    /// Cancel a running async task by tag.
    pub fn cancel(tag: &str) -> Self {
        Self::Cancel { tag: tag.to_string() }
    }

    // -- Focus --

    /// Move keyboard focus to the widget with the given ID.
    pub fn focus(id: &str) -> Self {
        Self::Renderer(RendererOp::Focus(id.to_string()))
    }

    /// Move keyboard focus to the next focusable widget.
    pub fn focus_next() -> Self { Self::Renderer(RendererOp::FocusNext) }

    /// Move keyboard focus to the previous focusable widget.
    pub fn focus_previous() -> Self { Self::Renderer(RendererOp::FocusPrevious) }

    // -- Scroll --

    /// Scroll a scrollable widget to an absolute position.
    pub fn scroll_to(target: &str, x: f32, y: f32) -> Self {
        Self::Renderer(RendererOp::ScrollTo {
            target: target.to_string(), x, y,
        })
    }

    // -- Window --

    /// Close the window with the given ID.
    pub fn close_window(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::Close(id.to_string())))
    }

    /// Resize a window to the given dimensions in logical pixels.
    pub fn resize_window(id: &str, width: f32, height: f32) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::Resize {
            window_id: id.to_string(), width, height,
        }))
    }

    /// Move a window to the given position in logical pixels.
    pub fn move_window(id: &str, x: f32, y: f32) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::Move {
            window_id: id.to_string(), x, y,
        }))
    }

    // -- Effects --

    /// Open a file-open dialog.
    pub fn file_open(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::FileOpen(Default::default()),
        })
    }

    /// Open a file-open dialog with options.
    pub fn file_open_with(tag: &str, opts: FileDialogOpts) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::FileOpen(opts),
        })
    }

    /// Open a multi-file selection dialog.
    pub fn file_open_multiple(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::FileOpenMultiple(Default::default()),
        })
    }

    /// Open a multi-file selection dialog with options.
    pub fn file_open_multiple_with(tag: &str, opts: FileDialogOpts) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::FileOpenMultiple(opts),
        })
    }

    /// Open a file-save dialog.
    pub fn file_save(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::FileSave(Default::default()),
        })
    }

    /// Open a file-save dialog with options.
    pub fn file_save_with(tag: &str, opts: FileDialogOpts) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::FileSave(opts),
        })
    }

    /// Open a single-directory selection dialog.
    pub fn directory_select(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::DirectorySelect(Default::default()),
        })
    }

    /// Open a single-directory selection dialog with options.
    pub fn directory_select_with(tag: &str, opts: FileDialogOpts) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::DirectorySelect(opts),
        })
    }

    /// Open a multi-directory selection dialog.
    pub fn directory_select_multiple(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::DirectorySelectMultiple(Default::default()),
        })
    }

    /// Open a multi-directory selection dialog with options.
    pub fn directory_select_multiple_with(tag: &str, opts: FileDialogOpts) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::DirectorySelectMultiple(opts),
        })
    }

    /// Read text from the system clipboard.
    pub fn clipboard_read(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::ClipboardRead,
        })
    }

    /// Write text to the system clipboard.
    pub fn clipboard_write(tag: &str, text: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::ClipboardWrite(text.to_string()),
        })
    }

    /// Read HTML content from the system clipboard.
    pub fn clipboard_read_html(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::ClipboardReadHtml,
        })
    }

    /// Write HTML content to the system clipboard.
    pub fn clipboard_write_html(tag: &str, html: &str, alt_text: Option<&str>) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::ClipboardWriteHtml {
                html: html.to_string(),
                alt_text: alt_text.map(|s| s.to_string()),
            },
        })
    }

    /// Clear the system clipboard.
    pub fn clipboard_clear(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::ClipboardClear,
        })
    }

    /// Read text from the primary selection (X11/Wayland).
    pub fn clipboard_read_primary(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::ClipboardReadPrimary,
        })
    }

    /// Write text to the primary selection (X11/Wayland).
    pub fn clipboard_write_primary(tag: &str, text: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::ClipboardWritePrimary(text.to_string()),
        })
    }

    /// Show a desktop notification.
    pub fn notification(tag: &str, title: &str, body: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::Notification {
                title: title.to_string(),
                body: body.to_string(),
                opts: Default::default(),
            },
        })
    }

    /// Show a desktop notification with options.
    pub fn notification_with(tag: &str, title: &str, body: &str, opts: NotificationOpts) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            request: EffectRequest::Notification {
                title: title.to_string(),
                body: body.to_string(),
                opts,
            },
        })
    }

    // -- Widget commands --

    /// Send a command to a native widget.
    pub fn widget_command(node_id: &str, op: &str, payload: Value) -> Self {
        Self::Renderer(RendererOp::WidgetCommand {
            node_id: node_id.to_string(),
            op: op.to_string(),
            payload,
        })
    }
}
