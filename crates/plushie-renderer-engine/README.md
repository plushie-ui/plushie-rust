# plushie-renderer-engine

Renderer-internal state engine and wire codec for Plushie. Holds
`Core`, the pure UI tree state machine; `Tree`, the retained
node store; and `Codec`, the JSON + MessagePack wire codec.

This crate is internal to the renderer pipeline and is not published.
Widget authors depend on `plushie-widget-sdk` instead.
