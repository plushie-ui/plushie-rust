//! Common re-exports for app developers.
//!
//! `use plushie::prelude::*` imports everything needed to write
//! a plushie app: the `App` trait, event/command types, UI builders,
//! and common property types.

// Core trait
/// Core Elm-architecture trait implemented by every plushie app.
pub use crate::App;

// Automation
/// Typed TreeNode wrapper with text, role, and a11y accessors.
pub use crate::automation::Element;
/// Typed widget-targeting selector (id, text, role, label, focused).
pub use crate::automation::Selector;
/// Platform effect kind (file dialog, clipboard, notification).
pub use plushie_core::key::EffectKind;
/// Typed logical key (Enter, Escape, Char, Named, etc.).
pub use plushie_core::key::Key;
/// Key + modifiers combo accepted by [`TestSession::press`](crate::test::TestSession::press).
pub use plushie_core::key::KeyPress;
/// Mouse button enum used by pointer events.
pub use plushie_core::key::MouseButton;
/// Pointer device kind (Mouse, Touch, Pen).
pub use plushie_core::key::PointerKind;

// Derive macros for widget authoring
/// Derive macro generating widget command variants.
pub use crate::WidgetCommand;
/// Derive macro generating widget event variants.
pub use crate::WidgetEvent;
/// Derive macro generating typed widget prop structs.
pub use crate::WidgetProps;

// Widget registrar (for App::view)
/// Registrar passed to [`App::view`] so composite widgets can register expanders.
pub use crate::widget::WidgetRegistrar;

// View
/// Return type for [`App::view`]: a retained UI tree node.
pub use crate::View;

// Events
/// Top-level event enum delivered to `update`.
pub use crate::event::Event;
/// Canonical event-family-to-type mapping.
pub use crate::event::EventType;
/// Typed match type returned by [`Event::widget_match`].
pub use crate::event::WidgetMatch;
/// Typed widget-event match arms (Click, Input, Toggle, ...).
pub use crate::event::WidgetMatch::*;

// Scoped IDs
/// Window-qualified scoped identifier (`main#scope/widget`).
pub use plushie_core::ScopedId;

// Commands and operation types
/// Side-effect command returned from `update`.
pub use crate::command::Command;
/// File-dialog configuration.
pub use crate::command::FileDialogOpts;
/// OS-notification configuration.
pub use crate::command::NotificationOpts;
/// Notification urgency level.
pub use crate::command::NotificationUrgency;
/// Window display mode (windowed / fullscreen).
pub use crate::command::WindowMode;

// Subscriptions
/// Declarative event-source subscription returned from `subscribe`.
pub use crate::subscription::Subscription;

// UI builder functions
/// UI builder functions (`window`, `column`, `text`, `button`, ...).
pub use crate::ui::*;

// Property types
/// Trait for widget prop structs extractable from a `TreeNode`.
pub use crate::derive_support::FromNode;
/// Trait for plushie-typed values (wire encode/decode).
pub use crate::derive_support::PlushieType;
/// Encoder trait for widget command enums.
pub use crate::derive_support::WidgetCommandEncode;
/// Encoder trait for widget event enums.
pub use crate::derive_support::WidgetEventEncode;
/// Accessibility metadata.
pub use crate::types::A11y;
/// Alignment (start/center/end).
pub use crate::types::Align;
/// Scrollable anchor (start / end).
pub use crate::types::Anchor;
/// Angle with dual-storage (degrees-on-wire).
pub use crate::types::Angle;
/// Animated value wrapper (plain, transition, spring, sequence).
pub use crate::types::Animatable;
/// Arrow-key navigation mode.
pub use crate::types::ArrowMode;
/// Widget background descriptor.
pub use crate::types::Background;
/// Border descriptor.
pub use crate::types::Border;
/// RGBA color with hex validation.
pub use crate::types::Color;
/// Content-fit policy for images/SVG.
pub use crate::types::ContentFit;
/// Cursor interaction style.
pub use crate::types::CursorStyle;
/// Custom theme builder.
pub use crate::types::CustomTheme;
/// Primary-axis direction (Horizontal / Vertical / Both).
pub use crate::types::Direction;
/// Axis constraint for canvas drag interactions.
pub use crate::types::DragAxis;
/// Text ellipsis placement.
pub use crate::types::Ellipsis;
/// QR-code error-correction level.
pub use crate::types::ErrorCorrection;
/// Canvas fill rule.
pub use crate::types::FillRule;
/// Image filter method (nearest / linear).
pub use crate::types::FilterMethod;
/// Font specifier (family, weight, style, stretch).
pub use crate::types::Font;
/// Font stretch variant.
pub use crate::types::FontStretch;
/// Gradient descriptor.
pub use crate::types::Gradient;
/// Horizontal alignment variant.
pub use crate::types::HorizontalAlignment;
/// Text-input hint purpose (password, email, URL, ...).
pub use crate::types::InputPurpose;
/// Keyboard modifier state (Shift, Ctrl, Alt, Super).
pub use crate::types::KeyModifiers;
/// Layout length (fill, shrink, fixed, fill-portion).
pub use crate::types::Length;
/// `Length` variant shortcuts for builder call sites.
pub use crate::types::Length::*;
/// Canvas stroke line cap.
pub use crate::types::LineCap;
/// Text line height (relative or absolute).
pub use crate::types::LineHeight;
/// Canvas stroke line join.
pub use crate::types::LineJoin;
/// Inner padding.
pub use crate::types::Padding;
/// Position point.
pub use crate::types::Position;
/// Corner radius (uniform or per-corner).
pub use crate::types::Radius;
/// Drop-shadow descriptor.
pub use crate::types::Shadow;
/// Text shaping strategy.
pub use crate::types::Shaping;
/// Sort order for tables and lists.
pub use crate::types::SortOrder;
/// Named or custom widget style.
pub use crate::types::Style;
/// Map of widget-type to style presets.
pub use crate::types::StyleMap;
/// Text-specific horizontal alignment.
pub use crate::types::TextAlignment;
/// Text layout direction.
pub use crate::types::TextDirection;
/// Text-editor cursor movement direction.
pub use crate::types::TextMotion;
/// Theme variant (system / named / custom).
pub use crate::types::Theme;
/// Passthrough prop type that keeps untyped JSON.
pub use crate::types::UntypedProps;
/// Window stacking level.
pub use crate::types::WindowLevel;
/// Text-wrap policy.
pub use crate::types::Wrapping;

// A11y sub-types for typed accessibility builders
/// Accessibility popup hint kind.
pub use plushie_core::types::HasPopup;
/// Live-region politeness (Polite / Assertive).
pub use plushie_core::types::Live;
/// Accessibility orientation.
pub use plushie_core::types::Orientation;
/// Accessibility role.
pub use plushie_core::types::Role;

// Animation
/// A single step within a [`Sequence`].
pub use crate::animation::AnimationStep;
/// Easing curve enum.
pub use crate::animation::Easing;
/// Animation repeat policy.
pub use crate::animation::Repeat;
/// Multi-step animation sequence.
pub use crate::animation::Sequence;
/// Spring animation descriptor.
pub use crate::animation::Spring;
/// Spring physics parameters.
pub use crate::animation::SpringConfig;
/// Time-based transition descriptor.
pub use crate::animation::Transition;
/// SDK-side numeric interpolator.
pub use crate::animation::Tween;
