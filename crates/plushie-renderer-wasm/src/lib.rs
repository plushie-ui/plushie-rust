//! WASM entry point for the plushie renderer.
//!
//! Provides a `wasm-bindgen` API for running plushie in the browser.
//! Uses `iced::daemon` with a canvas-based backend and communicates
//! with the host via JavaScript callbacks.
//!
//! # Usage from JavaScript
//!
//! ```js
//! import init, { PlushieApp } from './plushie_renderer_wasm.js';
//!
//! await init();
//! const app = new PlushieApp(settingsJson, (event) => {
//!     console.log('event:', event);
//! });
//! app.send_message(snapshotJson);
//! ```
//!
//! # Usage from Rust (custom WASM builds with widgets)
//!
//! ```ignore
//! let mut builder = plushie_widget_sdk::app::PlushieAppBuilder::new();
//! builder.register(Box::new(MyWidget));
//! let app = PlushieApp::with_widgets(settings, on_event, builder)?;
//! app.send_message(snapshot_json)?;
//! ```
//!
//! # Limitations
//!
//! - Platform effects (file dialogs, clipboard, notifications) are
//!   stubbed as unsupported. Web API implementations can be added in
//!   a future iteration.

mod effects;
mod output;

use std::sync::Mutex;

use wasm_bindgen::prelude::*;

use plushie_widget_sdk::protocol::IncomingMessage;
use plushie_widget_sdk::runtime::Codec;
use plushie_widget_sdk::runtime::{Message, StdinEvent};

use plushie_renderer_lib::App;
use plushie_renderer_lib::emitters::emit_hello;

use effects::WebEffectHandler;
use output::WebOutputWriter;

/// Global message receiver slot. Initialized by the [`PlushieApp`]
/// constructor, consumed once by the message subscription.
static MSG_RX: Mutex<Option<futures::channel::mpsc::UnboundedReceiver<String>>> = Mutex::new(None);

/// WASM plushie renderer handle.
///
/// Created via the constructor, which initializes the renderer and
/// starts the iced daemon in the background. The host sends messages
/// (Snapshots, Patches, etc.) via [`send_message`](PlushieApp::send_message)
/// and receives events via the `on_event` callback.
#[wasm_bindgen]
pub struct PlushieApp {
    sender: futures::channel::mpsc::UnboundedSender<String>,
}

#[wasm_bindgen]
impl PlushieApp {
    /// Create a new plushie renderer with no custom widgets.
    ///
    /// Parses settings, validates the protocol version, initializes the
    /// output writer, and starts the iced daemon in the background.
    /// Returns a handle for sending messages.
    ///
    /// `on_event` is a JavaScript callback that receives serialized
    /// event strings whenever the renderer emits an outgoing event.
    #[wasm_bindgen(constructor)]
    pub fn new(settings_json: &str, on_event: js_sys::Function) -> Result<PlushieApp, JsValue> {
        Self::with_widgets(
            settings_json,
            on_event,
            plushie_widget_sdk::app::PlushieAppBuilder::new(),
        )
    }

    /// Send a JSON-encoded protocol message to the renderer.
    ///
    /// The message is parsed as an [`IncomingMessage`] and processed
    /// by the iced daemon on the next event loop tick. This is the
    /// WASM equivalent of writing to stdin on native.
    ///
    /// Accepts any valid protocol message: Snapshot, Patch, Settings,
    /// Subscribe, Unsubscribe, WidgetOp, WindowOp, Effect,
    /// WidgetCommand, etc.
    pub fn send_message(&self, json: &str) -> Result<(), JsValue> {
        self.sender
            .unbounded_send(json.to_string())
            .map_err(|e| JsValue::from_str(&format!("send failed: {e}")))
    }
}

impl PlushieApp {
    /// Create a renderer with pre-registered custom widgets.
    ///
    /// Rust callers building custom WASM modules use this to register
    /// widgets at compile time. Widgets are Rust code compiled
    /// into the WASM binary, they cannot be added at runtime from JS.
    ///
    /// ```ignore
    /// let mut builder = PlushieAppBuilder::new();
    /// builder.register(Box::new(MyWidget));
    /// let app = PlushieApp::with_widgets(settings, on_event, builder)?;
    /// ```
    pub fn with_widgets(
        settings_json: &str,
        on_event: js_sys::Function,
        builder: plushie_widget_sdk::app::PlushieAppBuilder,
    ) -> Result<PlushieApp, JsValue> {
        console_log::init_with_level(log::Level::Warn).ok();

        // Order matters: parse settings and validate the protocol
        // version before wiring up the event sink. Error paths here
        // return Err(JsValue) directly to the caller; they must not
        // route through a half-initialised sink.
        let settings: serde_json::Value = serde_json::from_str(settings_json)
            .map_err(|e| JsValue::from_str(&format!("invalid settings JSON: {e}")))?;

        let expected = u64::from(plushie_widget_sdk::protocol::PROTOCOL_VERSION);
        if let Some(version) = settings.get("protocol_version").and_then(|v| v.as_u64())
            && version != expected
        {
            return Err(JsValue::from_str(&format!(
                "protocol version mismatch: expected {expected}, got {version}"
            )));
        }

        // Settings validated. Safe to initialise the output sink now.
        let writer = WebOutputWriter::new(on_event);
        let codec = Codec::Json;
        let sink = plushie_renderer_lib::WriterSink::new(Box::new(writer), codec);
        plushie_renderer_lib::emitters::init_sink(Box::new(sink));

        plushie_renderer_lib::settings::apply_validate_props(&settings);
        let iced_settings = plushie_renderer_lib::settings::parse_iced_settings(&settings);
        let font_bytes = plushie_renderer_lib::settings::parse_inline_fonts(&settings);

        // Load inline fonts directly into the global font system so they're
        // available before the first render. On WASM there are no system fonts,
        // so without this all text renders blank. Also set the sans-serif
        // family mapping, since the default Family::SansSerif won't resolve to
        // anything unless this mapping exists.
        if !font_bytes.is_empty() {
            let font_system = iced::advanced::graphics::text::font_system();
            let mut fs = font_system.write().expect("font_system lock");
            for bytes in &font_bytes {
                fs.load_font(std::borrow::Cow::Owned(bytes.clone()));
            }
            // Find the first non-icon font and set it as sans-serif fallback.
            let family_name = {
                let raw = fs.raw();
                let db = raw.db();
                db.faces()
                    .find(|f| !f.families.iter().any(|(n, _)| n == "Iced-Icons"))
                    .and_then(|f| f.families.first().map(|(n, _)| n.clone()))
            };
            if let Some(name) = family_name {
                log::info!("setting sans-serif family to: {}", name);
                fs.raw().db_mut().set_sans_serif_family(name);
            }
        }

        // Include custom type names in the hello message.
        let ext_keys: Vec<String> = builder
            .custom_type_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let ext_key_refs: Vec<&str> = ext_keys.iter().map(|s| s.as_str()).collect();

        emit_hello("web", "wgpu", &ext_key_refs, &["iced"], "wasm")
            .map_err(|e| JsValue::from_str(&format!("failed to emit hello: {e}")))?;

        // Create the message channel for JS -> renderer communication.
        let (sender, receiver) = futures::channel::mpsc::unbounded::<String>();
        *MSG_RX.lock().expect("MSG_RX lock") = Some(receiver);

        // Pack init data into a Mutex so the Fn closure can move it out once.
        type InitData = (
            serde_json::Value,
            plushie_widget_sdk::app::PlushieAppBuilder,
            Vec<Vec<u8>>,
        );
        let app_slot: Mutex<Option<InitData>> = Mutex::new(Some((settings, builder, font_bytes)));

        // Spawn the iced daemon in the background. On WASM, spawn_local
        // schedules the future on the browser's microtask queue, driven
        // by requestAnimationFrame.
        wasm_bindgen_futures::spawn_local(async move {
            let result = iced::daemon(
                move || {
                    let (settings, builder, fonts) = app_slot
                        .lock()
                        .expect("app_slot lock poisoned")
                        .take()
                        .expect("daemon init closure called more than once");

                    let builder =
                        builder.widget_set(&plushie_widget_sdk::runtime::iced_widget_set());
                    let registry = builder.build();
                    let effect_handler = Box::new(WebEffectHandler);
                    let sink = plushie_renderer_lib::emitters::sink_arc();
                    let mut app = App::new(registry, effect_handler, sink);

                    app.scale_factor = plushie_renderer_lib::validate_scale_factor(
                        settings
                            .get("scale_factor")
                            .and_then(|v| v.as_f64())
                            .map(plushie_widget_sdk::prop_helpers::f64_to_f32)
                            .unwrap_or(1.0),
                    );

                    let effects = app.core.apply(IncomingMessage::Settings { settings });
                    for effect in effects {
                        use plushie_widget_sdk::runtime::{CoreEffect, StateChange};
                        if let CoreEffect::StateChange(StateChange::WidgetConfig(config)) = effect {
                            let ctx = plushie_widget_sdk::registry::InitCtx {
                                config: &config,
                                theme: &app.theme,
                                default_text_size: app.core.default_text_size,
                                default_font: app.core.default_font,
                            };
                            app.registry.init_all(&ctx);
                        }
                    }

                    let font_tasks: Vec<iced::Task<Message>> = fonts
                        .into_iter()
                        .map(|bytes| {
                            iced::font::load(bytes).map(|result| {
                                if let Err(e) = result {
                                    log::error!("font load error: {e:?}");
                                }
                                Message::NoOp
                            })
                        })
                        .collect();

                    let task = if font_tasks.is_empty() {
                        iced::Task::none()
                    } else {
                        iced::Task::batch(font_tasks)
                    };

                    (app, task)
                },
                App::update,
                App::view_window,
            )
            .title(App::title_for_window)
            .subscription(|app: &App| {
                iced::Subscription::batch([
                    app.renderer_subscriptions(),
                    iced::Subscription::run(message_subscription).map(Message::Stdin),
                ])
            })
            .theme(App::theme_for_window)
            .scale_factor(App::scale_factor_for_window)
            .settings(iced_settings)
            .run();

            if let Err(e) = result {
                log::error!("iced daemon error: {e}");
            }
        });

        Ok(PlushieApp { sender })
    }
}

/// Subscription that reads JSON messages from the JS channel and feeds
/// them to the iced event loop as [`StdinEvent`]s. Mirrors the native
/// stdin subscription pattern.
fn message_subscription() -> impl iced::futures::Stream<Item = StdinEvent> {
    iced::stream::channel(32, async |mut sender| {
        use iced::futures::{SinkExt, StreamExt};

        let mut rx = MSG_RX
            .lock()
            .expect("MSG_RX lock poisoned")
            .take()
            .expect("message_subscription: no receiver (called more than once?)");

        while let Some(json) = rx.next().await {
            let event = match serde_json::from_str::<IncomingMessage>(&json) {
                Ok(msg) => StdinEvent::Message(msg),
                Err(e) => StdinEvent::Warning(format!("parse error: {e}")),
            };
            if sender.send(event).await.is_err() {
                break;
            }
        }

        // Channel closed (PlushieApp dropped); signal the daemon.
        let _ = sender.send(StdinEvent::Closed).await;
    })
}
