//! Windowed automation runner: spawns a real renderer subprocess and
//! drives it over the wire protocol.
//!
//! The public entry point [`run_windowed`] is a stub in this commit;
//! a follow-up commit lands the full renderer-spawn + MVU-mirror
//! implementation. The module exists now so
//! [`crate::automation::runner::run_with_backend`] can dispatch to it
//! without a `cfg` dance at every call site.
//!
//! The module itself is gated on the `wire` feature at its declaration
//! site in [`crate::automation`].

use crate::App;
use crate::automation::file::PlushieFile;
use crate::{Error, Result as PlushieResult};

/// Run a `.plushie` script against a real windowed renderer.
///
/// The full implementation lands in a follow-up commit; today this
/// returns [`Error::Startup`] with a message that the windowed path
/// is being wired up. Callers that parse a script with
/// `backend: windowed` will still see the parse success before
/// hitting this error.
///
/// # Errors
///
/// Today always returns [`Error::Startup`]; will become a real
/// renderer-spawn path once the follow-up commit lands.
pub fn run_windowed<A: App>(_file: &PlushieFile) -> PlushieResult {
    Err(Error::Startup(
        "windowed automation runner not yet implemented; \
         follow-up commit wires the renderer-spawn path"
            .to_string(),
    ))
}
