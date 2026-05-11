# Sensor Resize Parity

## Status

Investigated on 2026-05-11. This is a cross-SDK shape issue.
`docs/stewardship/triage.md:30` routes SDK API shape and wire-form
drift to the parity workflow, and `docs/stewardship/triage.md:40`
calls out cross-SDK shape as the first routing check.

Rust and the sibling host SDKs have been moved to the canonical shape.
Cross-SDK change cost was not a reason to keep the wrong shape; it was
the work required to keep the SDK family coherent.

## Source-backed findings

The stable event shape is already documented as a built-in widget
resize event. Rust reference docs list `WidgetMatch::Resize(id,
ResizeDimensions)` with `width` and `height` for `responsive` and
`sensor` callbacks (`docs/reference/events.md:141`). They also state
that `Move` and `Resize` are coalescable and last-value wins before the
runtime queue drains (`docs/reference/events.md:276`).

The Rust sensor builder previously exposed a string event tag:
`SensorBuilder::on_resize(tag: &str)` stored the string under
`on_resize`. The renderer-side sensor widget also treated `on_resize`
as a string and emitted that string directly as the event family. It
also derived `{tag}:show` and `{tag}:hide` event families from the same
string.

That string shape conflicted with the Rust validator, which already
declared sensor `on_resize` as `Bool`. Rust now uses
`SensorBuilder::on_resize(enabled: bool)`, reads a boolean renderer
prop, and emits family `"resize"` when enabled.

The Rust typed event path already assumes the canonical resize family.
`EventType::Resize` maps to the wire family `"resize"`
(`crates/plushie-core/src/event_type.rs:207`). The outgoing constructor
for resize events emits `value.width`, `value.height`, uses family
`"resize"`, and marks the event replace-coalescable
(`crates/plushie-core/src/protocol/outgoing.rs:947`). The Rust app
event parser converts resize values into `ResizeDimensions`
(`crates/plushie/src/event.rs:189`) and `WidgetMatch::Resize`
(`crates/plushie/src/event.rs:487`).

`responsive` already follows the canonical built-in event shape. Its
renderer widget emits family `"resize"` with `width` and `height`
(`crates/plushie-widget-sdk/src/widget/responsive_widget.rs:68`).

At investigation time, sibling SDKs were split on the `on_resize` prop
shape but agreed on the decoded event payload:

- Elixir declared `on_resize` as a string field
  (`../plushie-elixir/lib/plushie/widget/sensor.ex:13`), yet some
  runtime tests build sensors with `on_resize: true`
  (`../plushie-elixir/test/plushie/runtime_test.exs:211`). Its decoder
  recognizes only family `"resize"` as the typed resize event and
  returns `%{width: ..., height: ...}`
  (`../plushie-elixir/lib/plushie/protocol/decode.ex:972`).
- Gleam stored `on_resize` as `Option(String)` and wrote it as a
  string prop (`../plushie-gleam/src/plushie/widget/sensor.gleam:16`,
  `../plushie-gleam/src/plushie/widget/sensor.gleam:99`). Its decoder
  maps family `"resize"` to `event.Resize` with width and height
  (`../plushie-gleam/src/plushie/protocol/decode.gleam:1374`).
- Python had a generic sensor builder and documented `on_resize` as a
  bool (`../plushie-python/src/plushie/ui.py:207`,
  `../plushie-python/docs/reference/built-in-widgets.md:441`). Its
  decoder maps family `"resize"` to a `Resize` event with width and
  height (`../plushie-python/src/plushie/protocol.py:1346`).
- Ruby documented `on_resize` as a string prop and had builder tests for
  `set_on_resize("resized")`
  (`../plushie-ruby/lib/plushie/widget/sensor.rb:15`,
  `../plushie-ruby/test/plushie/test_widget_builders_complete.rb:355`).
  Its decoder maps family `"resize"` to a widget event value with width
  and height (`../plushie-ruby/lib/plushie/protocol/decode.rb:287`).
- TypeScript treated `onResize` as either a callback or boolean and
  serializes both forms to `on_resize: true`
  (`../plushie-typescript/src/ui/widgets/sensor.ts:26`,
  `../plushie-typescript/src/ui/widgets/sensor.ts:44`). Its event type
  defines `ResizeData` as `width` and `height`
  (`../plushie-typescript/src/types.ts:189`).

## Canonical shape recommendation

Make sensor resize a normal built-in resize event, not a custom
tagged family.

Canonical wire prop:

```json
{"on_resize": true}
```

Absent or false means the sensor does not subscribe to resize events.
The prop should not carry a custom event family.

Canonical emitted event:

```json
{
  "type": "event",
  "family": "resize",
  "id": "sensor-id",
  "value": {"width": 800.0, "height": 600.0}
}
```

`window_id` and scoped IDs follow the existing event identity rules.
The event is replace-coalescable. Host SDKs should surface typed
resize data in their local idiom, but the wire family stays `"resize"`.
Callback-capable SDKs may keep local handler metadata, as TypeScript
does, but that metadata must not alter the wire family.

Visibility lifecycle should be separated from resize parity. If
`show` and `hide` are retained, they should be explicit built-in or
custom widget events with their own documented shape. They should not
be generated as a side effect of a resize tag.

## Migration impact

Rust has the authority-side change. Existing Rust code using
`.on_resize("resize")` migrates to `.on_resize(true)`. Code using any
other string previously received a custom family rather than
`WidgetMatch::Resize`; it should migrate to matching the sensor ID or
local app state.

Elixir, Gleam, and Ruby have moved their sensor builder surfaces from
string event tags to boolean enablement. Python already exposed the
boolean shape and now has regression coverage for `on_resize=True`.
TypeScript already serialized callbacks and boolean options to
`on_resize: true`; its handler type now carries `ResizeData`.

Decoding remains unchanged across the host SDKs because the typed event
path already expected family `"resize"` with `width` and `height`.

## Parity rollout

This shape is now the accepted SDK-family contract:

1. The canonical sensor node serializes `on_resize: true`.
2. The renderer emits family `"resize"` with `value.width` and
   `value.height`.
3. Host SDKs surface typed resize data in their local idiom.
4. Callback-capable SDKs may keep local handler metadata, but that
   metadata must not alter the wire family.

The parity suite should include this case when that sibling repository
is available in the workspace.
