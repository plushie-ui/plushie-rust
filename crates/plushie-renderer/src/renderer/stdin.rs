//! Stdin I/O: background reader thread and the iced subscription that
//! bridges stdin events into the update loop.

use std::io::{BufReader, Read};
use std::thread;

use iced::futures::SinkExt;
use iced::stream;
use parking_lot::Mutex;

use plushie_widget_sdk::protocol::IncomingMessage;
use plushie_widget_sdk::runtime::Codec;
use plushie_widget_sdk::runtime::StdinEvent;

/// The generic reader type used throughout the transport layer.
pub(crate) type TransportReader = BufReader<Box<dyn Read + Send>>;

/// One-shot slot for the stdin receiver. The subscription takes it once on
/// first call. Uses a Mutex because `Subscription::run` requires `fn() -> Stream`
/// (a function pointer, not a closure), so we can't capture local state.
pub(crate) static STDIN_RX: Mutex<Option<tokio::sync::mpsc::Receiver<StdinEvent>>> =
    Mutex::new(None);

/// Async stream that yields StdinEvents. Bridges the background stdin reader
/// thread into iced's subscription system. Only wakes iced when data arrives
/// (zero CPU when idle).
pub(crate) fn stdin_subscription() -> impl iced::futures::Stream<Item = StdinEvent> {
    stream::channel(32, async |mut sender| {
        // parking_lot::Mutex doesn't poison; a panic in a previous
        // holder leaves the slot intact. The double-call guard on
        // `take()` is what catches the second-subscription bug.
        let mut rx = STDIN_RX
            .lock()
            .take()
            .expect("stdin_subscription: no receiver (called more than once?)");

        while let Some(event) = rx.recv().await {
            if sender.send(event).await.is_err() {
                break;
            }
        }
    })
}

pub(crate) fn spawn_stdin_reader(
    codec: Codec,
    sender: tokio::sync::mpsc::Sender<StdinEvent>,
    mut reader: TransportReader,
) {
    thread::spawn(move || {
        loop {
            match codec.read_message(&mut reader) {
                Ok(None) => {
                    let _ = sender.blocking_send(StdinEvent::Closed);
                    break;
                }
                Ok(Some(bytes)) => match codec.decode::<IncomingMessage>(&bytes) {
                    Ok(msg) => {
                        if sender.blocking_send(StdinEvent::Message(msg)).is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        let warning = format!("parse error: {e}");
                        if sender.blocking_send(StdinEvent::Warning(warning)).is_err() {
                            return;
                        }
                    }
                },
                Err(e) => {
                    let _ = sender.blocking_send(StdinEvent::Warning(format!("read error: {e}")));
                    let _ = sender.blocking_send(StdinEvent::Closed);
                    break;
                }
            }
        }
    });
}
