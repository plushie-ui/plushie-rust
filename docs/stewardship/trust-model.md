# Trust model

What plushie protects, what it does not protect against today, and
how those choices shape the codebase.

## What is protected today

The renderer-to-host channel is closed and typed. The wire protocol
defines a fixed enumeration of event variants and structured
response types; a host SDK that follows the protocol decodes these
into typed values and dispatches them to user-defined handlers.
The renderer cannot invent operations, hijack control flow, or
drive the host outside this surface. There is no opaque-blob path,
no string-eval path, no generic "run this on the host" instruction
in the protocol.

The host is therefore structurally protected from a compromised or
malicious renderer today, and we make that claim deliberately. The
remote-rendering use case relies on it: a server can expose a
plushie host to remote or untrusted users without those users
being able to RCE the server through the protocol. The renderer
can push misleading UI; it cannot push instructions.

Caveats are bug-class rather than design-class:

- The wire parser must remain robust against malformed input.
- Effect and query responses must be correlated to outstanding
  requests, so a fabricated response cannot deliver to the wrong
  handler.
- An app that explicitly wires up code execution from event
  content (evaluating an event string as code, etc.) is making
  its own choice; the protocol cannot enforce app-side hygiene.

## What is not protected today

The host-to-renderer channel is broader by design. The host has
to be able to ask the renderer to do real things: load fonts and
images from paths, load SVGs, save screenshots, exercise platform
effects (clipboard, file dialogs, OS notifications), spawn child
processes in `--exec` mode. A malicious or compromised host can
drive the full operation set against the user's machine wherever
the renderer is running.

Bounding this surface is the focus of the capability/manifest
direction (see `roadmap/capability-manifest.md`). It is a stated
direction, not currently scheduled work.

Defensive work in the codebase that is not part of either side
above is for *resilience*: panic isolation between widgets,
malformed-input handling, parser timeouts, hard caps on parser
inputs. See `resilience.md` for the resilience axis treated on
its own terms.

## Channel posture

The wire protocol is byte-stream agnostic. Confidentiality and
integrity are delegated to the outer transport (SSH, mTLS, named
pipe, OS pipe). The wire is not its own crypto layer, by design.
Proposals to add per-message MACs, encrypted fields, or similar
to the wire protocol itself are misframed; that responsibility
belongs with the outer transport.

The session token at the wire boundary is for binding a host to
a particular renderer instance, not for confidentiality.

## Same-access caveat

Same-access channels are out of scope on both sides. If a user
runs the host binary locally with their own shell access, or
SSHes into the machine running it as themselves, that is their
filesystem and process access; plushie does not protect against
the user acting on themselves.

## What plushie does not aim to protect against

- DoS or resource exhaustion at any cost. The renderer-to-host
  channel has parser timeouts and frame caps, but a malicious
  renderer can still flood typed events at the protocol rate;
  the host SDK has to handle that gracefully.
- Resource caps tuned tightly enough that legitimate-but-edge
  use cases break under normal load.
- Browser-grade isolation of arbitrary remote hosts.
- App-level mistakes. An app that does dangerous things in
  response to event content is making its own choice.

## Direction under consideration

A capability-based system could let a vanilla renderer plus an
arbitrary host be a bounded security boundary on the host-to-renderer
direction as well, anchored in a manifest of declared capabilities.
The mechanism, the manifest format, and when (or whether) the
work is taken up are all open. See
`roadmap/capability-manifest.md` for the working sketch and
accumulated observations.

## Implications

The trust-model claims map onto practice across these axes:

- Renderer-to-host integrity is load-bearing. Work that
  loosens the closed protocol surface (the renderer driving
  the host outside its documented operation set, an unsafe
  wire parser, spoofable response correlation) undermines the
  host-protection claim and is a deliberate decision rather
  than a routine refactor.
- Memory corruption and RCE risk on either side stay in scope
  today, regardless of the broader capability-manifest
  direction.
- Host-to-renderer concerns (file path inputs, effect
  dispatch, spawn surface, resource caps that would bound a
  malicious host) defer to the capability-manifest roadmap,
  unless the specific issue is also memory-corruption or RCE
  shaped.
- Generic DoS or resource-exhaustion concerns are low
  priority. Configurable knobs are preferred over aggressive
  defaults; caps tight enough to break legitimate-but-edge
  use are worse than no cap.
- Wire-level confidentiality or integrity expectations belong
  with the outer transport, not with the wire protocol
  itself.
- Information disclosure (panic locations, version strings,
  debug payloads) framed against a host-compromise threat
  model is misframed under the current claims. Revisit if the
  renderer-protection direction is taken up.
