# plushie-core

Internal core types crate shared between the Rust SDK, renderer,
and widget SDK. No iced dependency, making it suitable for wire-mode
apps, FFI bindings, and tooling.

## Quick reference

```
cargo test -p plushie-core              # run all tests
cargo clippy -p plushie-core            # lint
```

## Project layout

```
plushie-core/
  src/
    lib.rs              Re-exports, module declarations
    event_type.rs       EventType enum, from_family/as_family (single source of truth)
    key.rs              Key, KeyPress, MouseButton, PointerKind, EffectKind, InteractAction
    pointer.rs          Typed pointer event data (PointerPress, KeyData, ScrollPosition, etc.)
    outgoing_message.rs OutgoingMessage enum for typed wire protocol (SDK -> renderer), 20 variants
    selector.rs         Selector enum (Id, Text, Role, Label, Focused) with tree search
    scoped_id.rs        ScopedId for window#scope/widget paths
    settings.rs         Settings, WindowConfig, ExitReason (uses unified Theme)
    ops.rs              RendererOp, WindowOp, SystemOp, ImageOp command types
    spec.rs             EventSpec, CommandSpec for widget capability declarations
    widget_builder.rs   WidgetBuilder trait for SDK view construction
    tree_walk.rs        Composable tree walker: TreeTransform trait and walk() driver
    diagnostic.rs       Typed diagnostic variants from normalization, validation, and runtime
    diagnostics.rs      Diagnostic emission hook (inline sites call this across crates)
    codec_safety.rs     Wire-codec safety checks shared across the Rust SDK and widget SDK
    types/
      mod.rs            PlushieType trait, re-exports, primitive impls
      a11y.rs           A11y struct (Option<bool> fields, merge()), Role, Live, Orientation
      angle.rs          Angle type (dual-storage degrees/radians, degrees on wire)
      animatable.rs     Animatable<T> with Transition/Spring/Sequence support
      color.rs          Color (hex validation, short hex expansion, lowercase normalization)
      theme.rs          Theme enum (System/Named/Custom), CustomTheme with 52 shade builders
      padding.rs        Padding (per-side constructors, axis constructors)
      canvas/           Canvas shape types (PathCommand with Angle, Transform with Angle)
      ...               Other types (alignment, background, border, font, gradient, input,
                        interaction, layout, length, line_height, shadow, style, text,
                        value_range, etc.)
    animation/
      mod.rs            Transition, Spring, Sequence, Easing (renderer descriptors)
      easing.rs         31 easing curves with wire encode/decode
    protocol/
      mod.rs            Props, PropValue, PropMap
      types.rs          TreeNode, KeyModifiers
      outgoing.rs       OutgoingEvent (renderer -> SDK events)
      incoming.rs       Incoming wire messages from the host process (renderer-side decode)
```

## Key types

| Type | Module | Purpose |
|------|--------|---------|
| `EventType` | event_type | Canonical family-to-type mapping for all widget events |
| `OutgoingMessage` | outgoing_message | Typed wire messages (SDK -> renderer), 20 variants |
| `Key` | key | Typed key enum (~50 named variants + Char + Named fallback) |
| `KeyPress` | key | Key + modifiers combo, From<&str> with normalization |
| `MouseButton` | key | Left/Right/Middle/Back/Forward |
| `PointerKind` | key | Mouse/Touch/Pen device type |
| `Selector` | selector | Widget targeting (Id, Text, Role, Label, Focused) |
| `Angle` | types/angle | Dual-storage (degrees/radians), degrees on wire |
| `A11y` | types/a11y | Accessibility props with Option<bool> fields, merge() |
| `Theme` | types/theme | System/Named/Custom with 52 shade builder methods |
| `Color` | types/color | Hex validation, short hex expansion, 148 named colors |

## Design principles

- **No iced dependency.** Types are pure data, shared across all layers.
- **Wire protocol is the source of truth.** PlushieType trait handles
  encode/decode. EventType::from_family is the canonical mapping.
- **Degrees on the wire.** The Angle type stores the original unit
  internally but the wire protocol always uses degrees.
- **Option<bool> for A11y.** Three-state semantics (None = not
  specified, Some(false) = explicitly false, Some(true) = explicitly
  true) enabling proper merge behavior.
