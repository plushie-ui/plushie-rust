# Shared State

The testing chapter closed the single-user story. This chapter opens
the multi-user one: one Plushie app, many users, each with their own
renderer, all looking at the same model. Users connect over SSH, TCP,
or a Unix socket, see each other's changes in real time, and leave
without disturbing the others.

Plushie's MVU loop is single-model by design. Multi-user support is
not baked into the loop. Instead, it's a composition: one SDK session
per connected user, a shared authoritative store behind them, and
broadcasts that flow back through each session's own update cycle.

## The shared-state problem

A normal Plushie app is a single process driving a single renderer.
`App::update` returns a next model for exactly the one view that
renders it. Two users need two views, two event streams, and one
authoritative model that both see.

The wrong shape is a single MVU loop that tries to serve N renderers.
`update` would need a user identity on every event, `view` would need
to produce N trees keyed by user, and the diff pipeline would have to
route patches per user. That path reinvents the runner.

The right shape is N MVU loops, one per user, fed from a shared
store:

```
ssh user 1 ---> renderer 1 <---socket--- SDK session 1 (run_connect)
                                              |
                                         shared store
                                         (authoritative model,
                                          central update, broadcast)
                                              |
ssh user 2 ---> renderer 2 <---socket--- SDK session 2 (run_connect)
```

Each SDK session owns its own `App`, its own model copy, its own
subscriptions, its own view diffing. The shared store is an ordinary
Rust service (a channel-backed actor, a `Mutex<Model>`, a database,
whatever fits) that holds the authoritative state and fans changes
out to every session.

## Architecture

The distinguishing move is where `update` runs. In a single-user app,
`update` runs inside the MVU loop. For shared state, the canonical
`update` runs inside the shared store. The per-session `update`
handles only two things: widget events from this user (which it
forwards to the store) and `Broadcast` events from the store (which
it applies to the local model so the view can re-render).

Broadcasts ride the normal event bus via `Command::dispatch`. The
shared store pushes a new model into every session through a session
handle; the session converts the incoming model into an `Event`, the
runner calls `update`, `view` runs, and the diff pipeline sends only
the changed patches to that user's renderer. No full snapshots, no
special rendering path.

## Wire mode with a socket transport

Direct mode links the renderer into the app binary, so the renderer
and the app share a process and a screen. That is never what you want
for multi-user. Every session needs its own `plushie-renderer`
process, running wherever the user is sitting. Wire mode is the only
option here; enable it in `Cargo.toml`:

```toml
[dependencies]
plushie = { version = "0.7.1", default-features = false, features = ["wire"] }
```

The `wire` feature pulls in the socket transport
(`plushie::runner::socket`). It parses three address shapes:

- `:4567` binds TCP on `127.0.0.1:4567`.
- `0.0.0.0:4567` binds TCP on an arbitrary interface.
- `/tmp/plushie.sock` (or any absolute path) is a Unix domain socket.

Unix sockets are preferred on the same host: file-system permissions
become access control, there is no port to manage, and the kernel
cleans up cheap.

On the renderer side, start it in listen mode:

```bash
plushie-renderer --listen /tmp/plushie-alice.sock
```

The renderer prints the resolved address and a one-shot token to
stdout, then waits. The SDK side connects to that same address and
hands over the token.

See [direct vs wire](../reference/direct-vs-wire.md) for the feature
matrix and [wire protocol](../reference/wire-protocol.md) for the
framing details.

## The run_connect API

`plushie::run_connect::<A>(opts)` is wire mode without the spawn:
instead of launching `plushie-renderer` itself, the SDK opens a
socket to a renderer that is already listening. The signature is:

```rust
use plushie::{run_connect, ConnectOpts};

fn main() -> plushie::Result {
    run_connect::<MyApp>(ConnectOpts {
        socket: Some("/tmp/plushie-alice.sock".to_string()),
        token: Some(token_from_handshake()),
    })
}
```

`ConnectOpts` resolves fields in this order:

- `socket`: explicit string wins. Falls back to the `PLUSHIE_SOCKET`
  env var. If neither is set, `run_connect` returns
  `Error::InvalidSettings`.
- `token`: explicit string wins. Falls back to `PLUSHIE_TOKEN`, then
  to a one-line JSON message on stdin with a one-second timeout. The
  resolved token is hashed into the Settings `token_sha256` field;
  the renderer rejects the connection on mismatch.

`run_connect` runs a single session and returns when the socket
closes. There is no restart loop: a remote renderer the SDK did not
spawn is not the SDK's to respawn. When the session ends, let the
supervising code (a connection-manager task, systemd, or the
multi-session orchestrator) decide what happens next.

For apps that already own a tokio runtime, there is a mirror:

```rust
use plushie::runner::wire::run_connect_with_runtime;

run_connect_with_runtime::<MyApp>(opts, handle)
```

This avoids the SDK starting its own two-worker runtime when a
surrounding service already has one.

## Per-session state

`App::Model` is per-session. Store everything the user sees plus
everything they need to distinguish themselves from other users. A
minimal shape:

```rust
use plushie::prelude::*;

pub struct Session {
    user_id: String,
    shared: SharedHandle,
    model: Document,
    outbox: Option<SenderHandle>,
}
```

`user_id` is how the shared store tells sessions apart. `shared` is a
handle the session uses to forward events to the store. `model` is
the view of the authoritative document this session last applied.
Anything truly per-user (local selection, dark-mode preference, a
"dirty" flag while drafting) lives here and stays here.

`App::init` takes no arguments, so the per-session context (the
shared store handle and the current user's ID) lives in process
globals that the session orchestrator populates before
`run_connect` is called. The store handle is `OnceLock`-installed
at boot; the user ID comes from an env var the orchestrator sets
per spawn.

```rust
use std::sync::{Arc, OnceLock};

static STORE: OnceLock<Arc<Store>> = OnceLock::new();

fn store() -> Arc<Store> {
    STORE.get().expect("store installed before App::init runs").clone()
}

fn current_user_id() -> String {
    std::env::var("PLUSHIE_USER_ID").expect("orchestrator must set PLUSHIE_USER_ID")
}
```

`init` then reads from those globals, snapshots the document, and
starts a streaming command that bridges store broadcasts back into
this session's update loop:

```rust
impl App for Session {
    type Model = Self;

    fn init() -> (Self, Command) {
        let user_id = current_user_id();
        let shared = store();
        let model = shared.snapshot();
        let mut rx = shared.register(user_id.clone());

        let me = Self {
            user_id: user_id.clone(),
            shared: shared.clone(),
            model,
            outbox: None,
        };

        // Forward each Broadcast into the MVU loop as a StreamEvent
        // tagged "broadcast". The stream stays open for the lifetime
        // of the session; cancel via Command::cancel("broadcast") on
        // teardown if needed.
        let bridge = Command::stream("broadcast", move |emitter| async move {
            while let Some(bcast) = rx.recv().await {
                emitter.emit(serde_json::to_value(&bcast).unwrap_or_default());
            }
            Ok(serde_json::Value::Null)
        });

        (me, bridge)
    }
}
```

`shared.register` allocates a per-session `mpsc::Sender` inside
the store, returns the matching `Receiver`, and is the single
source of truth for who is connected. The store fans broadcasts
out to every registered sender; when this session ends the
receiver drops and the store cleans up on the next send.

## Broadcasting updates to all sessions

The shared store is an ordinary async actor. It holds the canonical
model, runs the real `update`, and fans a `Broadcast` out to every
registered session after a successful mutation:

```rust
use parking_lot::RwLock as SyncRwLock;
use tokio::sync::{mpsc, RwLock};
use std::sync::Arc;
use serde::{Deserialize, Serialize};

pub struct Store {
    inner: Arc<RwLock<Document>>,
    subscribers: Arc<SyncRwLock<Vec<(String, mpsc::Sender<Broadcast>)>>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Broadcast {
    pub originator: String,
    pub model: Document,
}

impl Store {
    /// Synchronous so a `submit` racing right after `register`
    /// returns is guaranteed to see the new subscriber.
    pub fn register(&self, user_id: String) -> mpsc::Receiver<Broadcast> {
        let (tx, rx) = mpsc::channel(32);
        self.subscribers.write().push((user_id, tx));
        rx
    }

    pub async fn submit(&self, originator: &str, msg: UserMsg) {
        let mut doc = self.inner.write().await;
        if let Err(e) = doc.apply(msg) {
            log::warn!("rejected msg from {originator}: {e}");
            return;
        }
        let snapshot = doc.clone();
        drop(doc);
        let bcast = Broadcast {
            originator: originator.to_string(),
            model: snapshot,
        };
        let mut subs = self.subscribers.write();
        subs.retain(|(_id, tx)| tx.try_send(bcast.clone()).is_ok());
    }
}
```

The store runs `apply` behind its own lock, so a crash in one user's
event cannot corrupt another user's view. Rejections are logged, not
propagated, and the broadcast is skipped. A more defensive store
wraps `apply` in `catch_unwind` and downgrades panics to logged
errors. `submit` drops senders whose receiver has gone away (the
session ended), so the registration list does not grow without
bound.

Inside the session, the `Command::stream` started in `init` is the
bridge. Each `emitter.emit(json)` call delivers a
`StreamEvent { tag: "broadcast", value }` through the normal
update pipeline, so the runner's loop wakes up the same way it
does for any subscription:

```rust
fn update(model: &Session, event: Event) -> (Session, Command) {
    let mut next = model.clone();
    if let Some(stream) = event.as_stream() {
        if stream.tag == "broadcast" {
            let bcast: Broadcast = match serde_json::from_value(stream.value.clone()) {
                Ok(b) => b,
                Err(e) => {
                    log::warn!("malformed broadcast: {e}");
                    return (next, Command::none());
                }
            };
            if bcast.originator == model.user_id {
                // Echo of this user's own submit; reconcile only the
                // fields the optimistic update did not already set.
                next.model.status = bcast.model.status;
            } else {
                let local_prefs = std::mem::take(&mut next.model.prefs);
                next.model = bcast.model;
                next.model.prefs = local_prefs;
            }
            return (next, Command::none());
        }
    }

    if let Some(Click(id)) = event.widget_match() {
        if id == "save" {
            let shared = model.shared.clone();
            let user = model.user_id.clone();
            return (next, Command::task("submit", move || async move {
                shared.submit(&user, UserMsg::Save).await;
                Ok(serde_json::Value::Null)
            }));
        }
    }

    (next, Command::none())
}
```

The runner handles the rest: after `update` returns, `view` rebuilds
the tree, the diff engine produces the minimal patch set, and the
socket transport ships it to this user's renderer. Only the user
whose view actually changed pays the diff cost. An `originator`
check skips the "echo to self" reconciliation when the user already
saw their own change optimistically.

## Over SSH

SSH is the path of least friction for a terminal-shaped collaboration
tool: authentication is already solved, transport is already
encrypted, and every user already knows how to use it. The trick is
that the renderer is native code, not a shell command, so the user
runs it themselves over the SSH channel:

```bash
ssh user@host plushie-renderer --stdio
```

`--stdio` is the default; `--stdio` is called out here for clarity.
The user sees a Plushie window on their local machine; bytes flow
between that renderer and `sshd` on the host. On the host side, the
SSH session runs a process that connects to the renderer via the
SDK. The simplest shape uses an OpenSSH `ForceCommand` or
`authorized_keys` `command=` entry:

```
command="/usr/local/bin/plushie-session --user=alice"
```

`plushie-session` is the orchestrator binary. It reads handshake
bytes from stdin, connects stdin/stdout through to the Plushie
renderer running on the client, and runs `plushie::run_connect` with
the shared store handle.

For shops with existing SSH infrastructure, forwarding a Unix socket
is simpler than a ForceCommand wrapper:

```bash
ssh -R /run/user/1000/plushie.sock:/var/run/plushie/alice.sock \
    user@host plushie-renderer --listen /run/user/1000/plushie.sock
```

The renderer listens on the forwarded socket; the server-side SDK
connects to `/var/run/plushie/alice.sock` via `run_connect`. No
custom wrapper is needed.

For TCP deployments, wrap the socket in TLS at the transport layer
(stunnel, a reverse proxy, or a tokio TLS handler in front of
`run_connect`). The wire protocol itself does not negotiate TLS.

## Deployment considerations

> [!WARNING]
> A socket exposed on TCP without a token accepts any connection
> that can reach the port. Always set a token in `ConnectOpts` and
> verify it on the renderer side with `--listen`; the renderer
> generates a token automatically when bound through `--listen` and
> prints it on connection. Do not disable token checking in
> production.

- **Authentication.** Terminate auth at a layer that predates
  `run_connect`. SSH keys, a reverse-proxy mTLS policy, or an
  upstream OAuth gate are all workable. The token in `ConnectOpts`
  is a handshake check, not an identity; it proves the connection
  came from the peer that saw the token, not that any particular
  user is on the other end.
- **Rate limits.** The shared store is the chokepoint for mutation
  traffic. Cap inbound `submit` calls per user with a token bucket
  (`governor` crate, `tower::limit`, or a hand-rolled
  `Semaphore`). Rejected submits drop quietly; the broadcast path
  is not touched, so other users see nothing.
- **Crash recovery.** The shared store is in-process state. If the
  orchestrator crashes, every user loses their renderer connection
  and reconnects. Persist the authoritative model to disk (SQLite,
  a log-structured file, Postgres) and reload it on startup.
  Per-session state is expected to be lost on reconnect; users see
  the shared model when they come back.
- **Slow consumers.** A stalled renderer cannot drain its socket,
  which eventually blocks the SDK's writer task. Wrap the
  per-session broadcast channel in a bounded `mpsc::Sender` and
  drop sessions that exceed a backlog threshold. The store's
  `register`/`deregister` pair should be the single source of
  truth for who is connected.
- **Observability.** Tag every log line with `user_id`. A single
  user's broken input is easy to track down when its submissions
  are scoped; a store that accepts submissions from every session
  without discrimination is nearly impossible to debug.

## What's next

The final chapter, [WASM deployment](17-wasm-deployment.md), covers
the other remote-rendering path: compiling the renderer to
`wasm32-unknown-unknown` and driving it from a host SDK in the
browser. Shared state over WebSocket looks much like shared state
over SSH, with the transport swapped and the renderer in a browser
tab instead of a native window.
