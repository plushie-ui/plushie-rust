//! Socket transport for wire mode.
//!
//! `SocketAdapter` connects the SDK to a pre-existing Unix socket or
//! TCP port where a renderer is listening, bypassing the normal
//! `Command::spawn()` path in `Bridge`. Ported from Elixir's
//! `Plushie.SocketAdapter` which bridges a gen_tcp socket into the
//! iostream-protocol surface `Bridge` consumes.
//!
//! Wire-framing is identical across transports: the only difference
//! between stdin/stdout mode and socket mode is how the raw bytes get
//! moved. MessagePack uses a 4-byte big-endian length prefix per
//! message; JSONL uses newline-delimited records.
//!
//! This module is the scaffold laid during the hat 16 foundation pass.
//! The `run_connect` entry point resolves options and opens the
//! socket; full Bridge integration (replacing the subprocess stdin /
//! stdout pair with the socket reader/writer) arrives in a follow-on
//! commit that refactors `Bridge` behind a transport trait.

use std::io;
use std::net::TcpStream;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

/// Resolved address type for a socket connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketAddr {
    /// Unix domain socket path.
    #[cfg(unix)]
    Unix(PathBuf),
    /// TCP `host:port`.
    Tcp(String, u16),
}

/// Parse a user-supplied socket string into a resolved address.
///
/// Mirrors Elixir's `parse_addr/1` in `Plushie.SocketAdapter`:
///
/// - `":<port>"` -> TCP on 127.0.0.1.
/// - `"<host>:<port>"` -> TCP on `<host>`.
/// - `"/..."` -> absolute Unix socket path.
/// - anything else -> interpreted as a Unix path (relative or
///   platform-specific).
///
/// # Errors
///
/// Returns an error string when the port component is not a valid
/// `u16`.
pub fn parse_addr(s: &str) -> Result<SocketAddr, String> {
    if let Some(rest) = s.strip_prefix(':') {
        let port: u16 = rest
            .parse()
            .map_err(|_| format!("invalid TCP port in `{s}`"))?;
        return Ok(SocketAddr::Tcp("127.0.0.1".to_string(), port));
    }
    #[cfg(unix)]
    if s.starts_with('/') {
        return Ok(SocketAddr::Unix(PathBuf::from(s)));
    }
    if let Some((host, port_str)) = s.split_once(':')
        && !host.is_empty()
        && let Ok(port) = port_str.parse::<u16>()
    {
        return Ok(SocketAddr::Tcp(host.to_string(), port));
    }
    #[cfg(unix)]
    {
        Ok(SocketAddr::Unix(PathBuf::from(s)))
    }
    #[cfg(not(unix))]
    {
        Err(format!(
            "ambiguous socket address `{s}` on non-unix platforms"
        ))
    }
}

/// Concrete underlying stream, owned by [`SocketAdapter`].
pub enum SocketStream {
    /// Unix socket stream.
    #[cfg(unix)]
    Unix(UnixStream),
    /// TCP stream.
    Tcp(TcpStream),
}

impl SocketStream {
    /// Connect to the given resolved address.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] if the connect fails.
    pub fn connect(addr: &SocketAddr) -> io::Result<Self> {
        match addr {
            #[cfg(unix)]
            SocketAddr::Unix(path) => Ok(SocketStream::Unix(UnixStream::connect(path)?)),
            SocketAddr::Tcp(host, port) => Ok(SocketStream::Tcp(TcpStream::connect((
                host.as_str(),
                *port,
            ))?)),
        }
    }
}

impl io::Read for SocketStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            SocketStream::Unix(s) => s.read(buf),
            SocketStream::Tcp(s) => s.read(buf),
        }
    }
}

impl io::Write for SocketStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            SocketStream::Unix(s) => s.write(buf),
            SocketStream::Tcp(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            SocketStream::Unix(s) => s.flush(),
            SocketStream::Tcp(s) => s.flush(),
        }
    }
}

/// Pre-existing-renderer connection adapter.
///
/// Owns the socket stream and the resolved address. A future commit
/// will wire this into the `Bridge` transport layer so
/// [`crate::run_connect`] can drive the normal wire-mode event loop
/// over the socket instead of a subprocess.
pub struct SocketAdapter {
    /// Resolved socket address.
    pub addr: SocketAddr,
    /// Open stream to the renderer.
    pub stream: SocketStream,
}

impl SocketAdapter {
    /// Connect to the renderer listening at `addr_str`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::InvalidSettings`] on parse failure and
    /// [`crate::Error::Io`] on connect failure.
    pub fn connect(addr_str: &str) -> std::result::Result<Self, crate::Error> {
        let addr = parse_addr(addr_str).map_err(crate::Error::InvalidSettings)?;
        let stream = SocketStream::connect(&addr)?;
        Ok(Self { addr, stream })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tcp_port_only() {
        let addr = parse_addr(":4567").unwrap();
        assert_eq!(addr, SocketAddr::Tcp("127.0.0.1".to_string(), 4567));
    }

    #[test]
    fn parses_host_port() {
        let addr = parse_addr("example.com:8080").unwrap();
        assert_eq!(addr, SocketAddr::Tcp("example.com".to_string(), 8080));
    }

    #[cfg(unix)]
    #[test]
    fn parses_unix_absolute_path() {
        let addr = parse_addr("/tmp/plushie.sock").unwrap();
        assert_eq!(addr, SocketAddr::Unix(PathBuf::from("/tmp/plushie.sock")));
    }

    #[cfg(unix)]
    #[test]
    fn parses_bare_name_as_unix_path() {
        let addr = parse_addr("plushie.sock").unwrap();
        assert_eq!(addr, SocketAddr::Unix(PathBuf::from("plushie.sock")));
    }

    #[test]
    fn rejects_bad_port() {
        assert!(parse_addr(":not_a_port").is_err());
    }
}
