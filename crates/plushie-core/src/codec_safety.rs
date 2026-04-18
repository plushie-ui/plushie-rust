//! Wire-codec safety checks shared across the Rust SDK and the
//! renderer-side widget SDK.
//!
//! Both the widget-sdk codec (which reads inbound host messages) and
//! the Rust SDK's wire bridge (which reads renderer-generated
//! messages in wire mode) must defend against forged or pathological
//! msgpack payloads. The same defensive helpers live here so both
//! sides agree on the limits and both benefit from any hardening.
//!
//! # What's covered
//!
//! - [`MAX_RMPV_DEPTH`]: maximum nesting depth any rmpv-driven decode
//!   is willing to walk.
//! - [`check_msgpack_depth`]: iterative pre-scan of raw msgpack bytes
//!   that rejects overly nested payloads and containers whose declared
//!   element count exceeds the remaining bytes.
//!
//! The scan is intentionally iterative. `rmpv::read_value` is recursive
//! itself, so a depth-bomb payload would blow rmpv's stack before any
//! user-space depth check could observe it. The pre-scan uses an
//! explicit `Vec` stack, keeping scan depth bounded by heap rather
//! than call stack.

/// Maximum nesting depth for rmpv-driven msgpack decodes. Prevents
/// stack overflow from deeply nested (or maliciously crafted)
/// payloads. Matches serde_json's built-in 128-depth recursion limit
/// so the JSON and msgpack paths accept the same shape of input.
pub const MAX_RMPV_DEPTH: usize = 128;

/// Iteratively scan raw msgpack bytes and reject payloads that would
/// cause problems for `rmpv::read_value`:
///
/// - **Nesting depth** exceeding `max_depth` (prevents stack overflow
///   from rmpv's recursive parser).
/// - **Declared element counts** exceeding the remaining bytes
///   (prevents rmpv from pre-allocating `Vec::with_capacity(billions)`
///   when the declared count is larger than the payload can possibly
///   contain).
///
/// The scan walks format bytes iteratively on purpose. Every msgpack
/// format marker is enumerated so any new marker rmpv starts decoding
/// stays matched by an equivalent pre-scan case; a missing marker
/// would mean the scan walks into an unexpected region and either
/// under-counts children or mis-reports size.
///
/// Total element count is bounded transitively by
/// `max_depth * children_at_this_level`, so the per-level count
/// check is enough to defeat forged-count-plus-forged-depth attacks
/// in combination with the depth cap.
///
/// This pre-scan defends against deeply nested or forged-count
/// msgpack. A scan that runs past the end of an incomplete length
/// marker (e.g. a `bin8` byte with no size byte following) breaks out
/// of the loop and returns Ok; rmpv's downstream parser will surface
/// the malformed-stream error in that case.
///
/// # Errors
///
/// Returns a human-readable reason string when the payload nests
/// deeper than `max_depth` or declares a container size larger than
/// the remaining bytes can hold.
pub fn check_msgpack_depth(bytes: &[u8], max_depth: usize) -> Result<(), String> {
    let len = bytes.len();
    let mut pos: usize = 0;
    let mut depth: usize = 0;
    // Stack tracks how many child elements remain at each nesting level.
    let mut remaining: Vec<usize> = Vec::new();

    while pos < len {
        let b = bytes[pos];
        pos += 1;

        // Classify the format marker: (data_bytes_to_skip, child_element_count).
        // For containers (array/map), child_count > 0 and we push a new depth level.
        // For scalars, child_count == 0 and we consume one element from the parent.
        let (skip, children) = match b {
            // positive fixint
            0x00..=0x7f => (0, 0),
            // fixmap: N key-value pairs = 2N child elements
            0x80..=0x8f => (0, ((b & 0x0f) as usize) * 2),
            // fixarray
            0x90..=0x9f => (0, (b & 0x0f) as usize),
            // fixstr
            0xa0..=0xbf => ((b & 0x1f) as usize, 0),
            // nil, (unused), false, true
            0xc0..=0xc3 => (0, 0),
            // bin8
            0xc4 => {
                if pos >= len {
                    break;
                }
                (1 + bytes[pos] as usize, 0)
            }
            // bin16
            0xc5 => {
                if pos + 1 >= len {
                    break;
                }
                let n = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
                (2 + n, 0)
            }
            // bin32
            0xc6 => {
                if pos + 3 >= len {
                    break;
                }
                let n = u32::from_be_bytes([
                    bytes[pos],
                    bytes[pos + 1],
                    bytes[pos + 2],
                    bytes[pos + 3],
                ]) as usize;
                (4 + n, 0)
            }
            // ext8
            0xc7 => {
                if pos >= len {
                    break;
                }
                (2 + bytes[pos] as usize, 0)
            }
            // ext16
            0xc8 => {
                if pos + 1 >= len {
                    break;
                }
                let n = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
                (3 + n, 0)
            }
            // ext32
            0xc9 => {
                if pos + 3 >= len {
                    break;
                }
                let n = u32::from_be_bytes([
                    bytes[pos],
                    bytes[pos + 1],
                    bytes[pos + 2],
                    bytes[pos + 3],
                ]) as usize;
                (5 + n, 0)
            }
            // float32
            0xca => (4, 0),
            // float64
            0xcb => (8, 0),
            // uint8, int8
            0xcc | 0xd0 => (1, 0),
            // uint16, int16
            0xcd | 0xd1 => (2, 0),
            // uint32, int32
            0xce | 0xd2 => (4, 0),
            // uint64, int64
            0xcf | 0xd3 => (8, 0),
            // fixext 1, 2, 4, 8, 16 (type byte + data)
            0xd4 => (2, 0),
            0xd5 => (3, 0),
            0xd6 => (5, 0),
            0xd7 => (9, 0),
            0xd8 => (17, 0),
            // str8
            0xd9 => {
                if pos >= len {
                    break;
                }
                (1 + bytes[pos] as usize, 0)
            }
            // str16
            0xda => {
                if pos + 1 >= len {
                    break;
                }
                let n = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
                (2 + n, 0)
            }
            // str32
            0xdb => {
                if pos + 3 >= len {
                    break;
                }
                let n = u32::from_be_bytes([
                    bytes[pos],
                    bytes[pos + 1],
                    bytes[pos + 2],
                    bytes[pos + 3],
                ]) as usize;
                (4 + n, 0)
            }
            // array16
            0xdc => {
                if pos + 1 >= len {
                    break;
                }
                let n = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
                pos += 2;
                (0, n)
            }
            // array32
            0xdd => {
                if pos + 3 >= len {
                    break;
                }
                let n = u32::from_be_bytes([
                    bytes[pos],
                    bytes[pos + 1],
                    bytes[pos + 2],
                    bytes[pos + 3],
                ]) as usize;
                pos += 4;
                (0, n)
            }
            // map16
            0xde => {
                if pos + 1 >= len {
                    break;
                }
                let n = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
                pos += 2;
                (0, n * 2)
            }
            // map32
            0xdf => {
                if pos + 3 >= len {
                    break;
                }
                let n = u32::from_be_bytes([
                    bytes[pos],
                    bytes[pos + 1],
                    bytes[pos + 2],
                    bytes[pos + 3],
                ]) as usize;
                pos += 4;
                (0, n * 2)
            }
            // negative fixint
            0xe0..=0xff => (0, 0),
        };

        pos += skip;

        if children > 0 {
            // Each child element needs at least 1 byte. Reject declared
            // counts that exceed the remaining data to prevent rmpv from
            // pre-allocating huge Vecs based on a forged count field.
            let remaining_bytes = len.saturating_sub(pos);
            if children > remaining_bytes {
                return Err(format!(
                    "msgpack container declares {children} elements but only {remaining_bytes} bytes remain"
                ));
            }

            depth += 1;
            if depth > max_depth {
                return Err(format!("msgpack nesting depth exceeds limit ({max_depth})"));
            }
            remaining.push(children);
        } else {
            // Leaf value consumed: pop completed containers.
            while let Some(count) = remaining.last_mut() {
                *count -= 1;
                if *count == 0 {
                    remaining.pop();
                    depth -= 1;
                } else {
                    break;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_flat_structures() {
        // A payload with no containers trivially passes.
        assert!(check_msgpack_depth(&[0x01, 0x02, 0x03], 128).is_ok());
    }

    #[test]
    fn accepts_empty_input() {
        assert!(check_msgpack_depth(&[], 128).is_ok());
    }

    #[test]
    fn rejects_truncated_container() {
        // fixmap(1) declares 2 children, provides 0 bytes.
        assert!(check_msgpack_depth(&[0x81], 128).is_err());
        // fixarray(1) declares 1 child, provides 0 bytes.
        assert!(check_msgpack_depth(&[0x91], 128).is_err());
    }

    #[test]
    fn rejects_forged_element_count() {
        // map32 marker + 4 billion declared entries + a few bytes.
        let mut bytes = vec![0xdf];
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
        bytes.extend_from_slice(&[0xa1, b'k', 0x01]);
        let err = check_msgpack_depth(&bytes, 128).unwrap_err();
        assert!(err.contains("elements"));
    }

    #[test]
    fn rejects_deeply_nested() {
        // Build {a: {a: {a: ... {a: 1} ...}}}, 200 levels deep.
        let mut bytes = vec![0x01]; // innermost value
        for _ in 0..200 {
            // fixmap(1), fixstr(1) "a", then existing payload becomes the value.
            let mut wrapper = vec![0x81, 0xa1, b'a'];
            wrapper.extend_from_slice(&bytes);
            bytes = wrapper;
        }
        assert!(check_msgpack_depth(&bytes, 128).is_err());
        assert!(check_msgpack_depth(&bytes, 300).is_ok());
    }

    #[test]
    fn max_rmpv_depth_matches_serde_json() {
        // serde_json's built-in RECURSION_LIMIT is 128; MAX_RMPV_DEPTH
        // mirrors that so inputs accepted on the JSON path are also
        // accepted on the msgpack path and vice versa.
        assert_eq!(MAX_RMPV_DEPTH, 128);
    }
}
