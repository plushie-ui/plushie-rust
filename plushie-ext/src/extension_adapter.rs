//! Adapter that wraps a [`WidgetExtension`] as a [`PlushieWidget`].
//!
//! Enables registering existing WidgetExtension implementations in the
//! WidgetRegistry without modifying their source code. The adapter owns
//! an [`ExtensionCaches`] instance and bridges the API differences
//! between the two traits.
//!
//! # Example
//!
//! ```ignore
//! use plushie_ext::prelude::*;
//! use plushie_ext::extension_adapter::ExtensionAdapter;
//!
//! // Wrap an existing WidgetExtension for the registry
//! let adapter = ExtensionAdapter::new(MyExtension::new());
//! builder.widget(adapter);
//! ```

use iced::{Element, Theme};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::extensions::{ExtensionCaches, InitCtx, WidgetEnv, WidgetExtension};
use crate::message::Message;
use crate::protocol::{OutgoingEvent, TreeNode};
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widgets::a11y::A11yOverrides;

/// Wraps a [`WidgetExtension`] implementation as a [`PlushieWidget`].
///
/// Each adapter owns its own [`ExtensionCaches`] for the extension to
/// use during prepare/render/handle_event/handle_command/cleanup.
pub struct ExtensionAdapter<R: PlushieRenderer> {
    extension: Box<dyn WidgetExtension<R>>,
    caches: ExtensionCaches,
}

impl<R: PlushieRenderer> ExtensionAdapter<R> {
    /// Wrap a concrete extension type for registration in the WidgetRegistry.
    pub fn new(extension: impl WidgetExtension<R> + 'static) -> Self {
        Self {
            extension: Box::new(extension),
            caches: ExtensionCaches::new(),
        }
    }

    /// Wrap a pre-boxed extension for registration in the WidgetRegistry.
    pub fn from_boxed(extension: Box<dyn WidgetExtension<R>>) -> Self {
        Self {
            extension,
            caches: ExtensionCaches::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for ExtensionAdapter<R> {
    fn type_names(&self) -> &[&str] {
        self.extension.type_names()
    }

    fn namespace(&self) -> &str {
        self.extension.config_key()
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let env = WidgetEnv {
            caches: &self.caches,
            ctx: *ctx,
        };
        self.extension.render(node, &env)
    }

    fn prepare(&mut self, node: &TreeNode, _window_id: &str, theme: &Theme) {
        self.extension.prepare(node, &mut self.caches, theme);
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<OutgoingEvent>> {
        if let Message::Event {
            id, family, data, ..
        } = msg
        {
            let result = self
                .extension
                .handle_event(id, family, data, &mut self.caches);
            match result {
                crate::extensions::EventResult::PassThrough => None,
                crate::extensions::EventResult::Consumed(events) => Some(events),
                crate::extensions::EventResult::Observed(mut events) => {
                    let data_opt = if data.is_null() {
                        None
                    } else {
                        Some(data.clone())
                    };
                    let original = OutgoingEvent::generic(family.clone(), id.clone(), data_opt);
                    events.insert(0, original);
                    Some(events)
                }
            }
        } else {
            None
        }
    }

    fn handle_widget_op(
        &mut self,
        node_id: &str,
        op: &str,
        payload: &Value,
    ) -> Option<Vec<OutgoingEvent>> {
        let events = self
            .extension
            .handle_command(node_id, op, payload, &mut self.caches);
        if events.is_empty() {
            None
        } else {
            Some(events)
        }
    }

    fn infer_a11y(&self, _node: &TreeNode) -> Option<A11yOverrides> {
        None
    }

    fn cleanup(&mut self, node_id: &str, _window_id: &str) {
        self.extension.cleanup(node_id, &mut self.caches);
    }

    fn init(&mut self, ctx: &InitCtx<'_>) {
        self.extension.init(ctx);
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ExtensionAdapter {
            extension: self.extension.new_instance(),
            caches: ExtensionCaches::new(),
        })
    }
}
