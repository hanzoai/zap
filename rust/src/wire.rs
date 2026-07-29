//! ZAP binary wire format — the message header, and framed message I/O.
//!
//! Aligned with `github.com/zap-proto/go@v1.8.2` (the Go reference):
//!
//! ```text
//! ┌─ Header (16 bytes) ────────────────────────────────────────────┐
//! │  Magic       4  "ZAP\0"                                        │
//! │  Version     2  u16 LE  (1 or 2 — both accepted)               │
//! │  Flags       2  u16 LE  (compressed, encrypted, signed, …)     │
//! │  RootOffset  4  u32 LE  offset to root within data segment     │
//! │  Size        4  u32 LE  total message size including header    │
//! └────────────────────────────────────────────────────────────────┘
//! ┌─ Data segment (variable, 8-byte aligned) ──────────────────────┐
//! │  structs / lists / text / bytes at fixed offsets                │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! All multi-byte integers are little-endian. This module owns the
//! HEADER only; [`crate::codec`] walks the data segment.

use std::io::{self, Read, Write};

/// Magic bytes identifying a ZAP message: `ZAP\0`.
pub const MAGIC: [u8; 4] = [b'Z', b'A', b'P', 0];

/// Header size in bytes.
pub const HEADER_SIZE: usize = 16;

/// The wire version this runtime's [`crate::codec::Builder`] emits for a
/// plain message. Two schema generations share one data-segment encoding
/// and differ only in the meaning of the leading struct byte, so a reader
/// ACCEPTS both — a consumer that needs a specific generation gates on
/// [`Header::version`] after decoding.
pub const VERSION: u16 = 1;

/// The wire version the RPC call envelope and the transport framing use
/// (see [`crate::rpc`]). Byte-identical data segment; only header byte 4
/// differs. `github.com/zap-proto/go`'s `NewBuilderV2` emits this, so a
/// reader that rejects it cannot read a single Go RPC envelope.
pub const VERSION2: u16 = 2;

/// Maximum message size — 16 MiB. Matches Go reference.
pub const MAX_SIZE: usize = 16 * 1024 * 1024;

/// Header flags. Reserved bits are zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags(pub u16);

impl Flags {
    pub const NONE: Flags = Flags(0);
    pub const COMPRESSED: Flags = Flags(1 << 0);
    pub const ENCRYPTED: Flags = Flags(1 << 1);
    pub const SIGNED: Flags = Flags(1 << 2);

    pub fn contains(self, other: Flags) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// ZAP wire errors.
#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("zap: invalid magic bytes")]
    InvalidMagic,
    #[error("zap: unsupported version {0}")]
    InvalidVersion(u16),
    #[error("zap: message size {got} exceeds limit {max}")]
    SizeExceeded { got: usize, max: usize },
    #[error("zap: short read — header expects {want}, got {got}")]
    ShortHeader { want: usize, got: usize },
    #[error("zap: short read — body expects {want}, got {got}")]
    ShortBody { want: usize, got: usize },
    #[error("zap: connection closed")]
    Closed,
    #[error("zap: io: {0}")]
    Io(#[from] io::Error),
}

/// Decoded header — the framing metadata, not the payload.
#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub version: u16,
    pub flags: Flags,
    pub root_offset: u32,
    pub size: u32,
}

impl Header {
    /// Encode this header into a 16-byte buffer.
    pub fn encode(&self) -> [u8; HEADER_SIZE] {
        let mut out = [0u8; HEADER_SIZE];
        out[0..4].copy_from_slice(&MAGIC);
        out[4..6].copy_from_slice(&self.version.to_le_bytes());
        out[6..8].copy_from_slice(&self.flags.0.to_le_bytes());
        out[8..12].copy_from_slice(&self.root_offset.to_le_bytes());
        out[12..16].copy_from_slice(&self.size.to_le_bytes());
        out
    }

    /// Decode a 16-byte buffer into a header. Validates magic + version.
    pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
        if buf.len() < HEADER_SIZE {
            return Err(WireError::ShortHeader {
                want: HEADER_SIZE,
                got: buf.len(),
            });
        }
        if buf[0..4] != MAGIC {
            return Err(WireError::InvalidMagic);
        }
        let version = u16::from_le_bytes([buf[4], buf[5]]);
        if version != VERSION && version != VERSION2 {
            return Err(WireError::InvalidVersion(version));
        }
        let flags = Flags(u16::from_le_bytes([buf[6], buf[7]]));
        let root_offset = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let size = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
        if (size as usize) > MAX_SIZE {
            return Err(WireError::SizeExceeded {
                got: size as usize,
                max: MAX_SIZE,
            });
        }
        Ok(Header {
            version,
            flags,
            root_offset,
            size,
        })
    }
}

/// Read one complete ZAP message (header + data segment) from a reader.
///
/// A ZAP message is SELF-DELIMITING — the header's `size` says how long
/// it is — so this is all that is needed to pull messages off a stream,
/// a file, or a pipe. [`crate::transport`] adds a length+direction
/// prefix on top because a CONNECTION additionally has to say whether a
/// message is a call or an answer; the message itself never carries
/// that.
pub fn read_frame<R: Read>(r: &mut R) -> Result<(Header, Vec<u8>), WireError> {
    let mut hdr_buf = [0u8; HEADER_SIZE];
    r.read_exact(&mut hdr_buf)?;
    let hdr = Header::decode(&hdr_buf)?;

    let body_len = hdr.size as usize - HEADER_SIZE;
    let mut body = vec![0u8; body_len];
    r.read_exact(&mut body).map_err(|_| WireError::ShortBody {
        want: body_len,
        got: 0,
    })?;
    Ok((hdr, body))
}

/// Write a ZAP message (header + data segment) to a writer.
pub fn write_frame<W: Write>(
    w: &mut W,
    flags: Flags,
    root_offset: u32,
    body: &[u8],
) -> Result<(), WireError> {
    let total = HEADER_SIZE + body.len();
    if total > MAX_SIZE {
        return Err(WireError::SizeExceeded {
            got: total,
            max: MAX_SIZE,
        });
    }
    let hdr = Header {
        version: VERSION,
        flags,
        root_offset,
        size: total as u32,
    };
    w.write_all(&hdr.encode())?;
    w.write_all(body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let hdr = Header {
            version: VERSION,
            flags: Flags::SIGNED,
            root_offset: 16,
            size: 64,
        };
        let bytes = hdr.encode();
        let decoded = Header::decode(&bytes).unwrap();
        assert_eq!(decoded.version, VERSION);
        assert!(decoded.flags.contains(Flags::SIGNED));
        assert_eq!(decoded.root_offset, 16);
        assert_eq!(decoded.size, 64);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = [0u8; HEADER_SIZE];
        bytes[0..4].copy_from_slice(b"NOPE");
        assert!(matches!(
            Header::decode(&bytes),
            Err(WireError::InvalidMagic)
        ));
    }

    #[test]
    fn rejects_unsupported_version() {
        let hdr = Header {
            version: 99,
            flags: Flags::NONE,
            root_offset: 0,
            size: 16,
        };
        assert!(matches!(
            Header::decode(&hdr.encode()),
            Err(WireError::InvalidVersion(99))
        ));
    }

    /// The Go RPC envelope is a v2 header. A reader that takes v1 only
    /// cannot read a single call off the wire — this is the gap that
    /// made the Rust side unable to talk to `zap-proto/go` at all.
    #[test]
    fn accepts_both_wire_versions() {
        for v in [VERSION, VERSION2] {
            let hdr = Header {
                version: v,
                flags: Flags::NONE,
                root_offset: 16,
                size: 24,
            };
            assert_eq!(Header::decode(&hdr.encode()).unwrap().version, v);
        }
    }

    #[test]
    fn frame_io_roundtrip() {
        let body = b"opaque-capnp-segment-bytes";
        let mut buf = Vec::new();
        write_frame(&mut buf, Flags::NONE, 0, body).unwrap();
        let mut cur = std::io::Cursor::new(&buf);
        let (hdr, got) = read_frame(&mut cur).unwrap();
        assert_eq!(hdr.size as usize, HEADER_SIZE + body.len());
        assert_eq!(got, body);
    }

    /// End-to-end through the zapc-generated bindings. Builds a
    /// `KeysSubscribe` via the writer, wraps in a ZAP frame, reads
    /// back zero-copy via the reader. Proves the codegen output and
    /// the wire layer compose.
    #[test]
    fn proto_roundtrip() {
        use crate::schema::keys_subscribe;

        let body = keys_subscribe::Writer {
            application: "hanzo".to_string(),
            cursor: 42,
        }
        .to_bytes();

        let mut framed = Vec::new();
        write_frame(&mut framed, Flags::NONE, 0, &body).unwrap();

        let mut cur = std::io::Cursor::new(&framed);
        let (_hdr, body_back) = read_frame(&mut cur).unwrap();

        let r = keys_subscribe::Reader::from_slice(&body_back).unwrap();
        assert_eq!(r.application(), "hanzo");
        assert_eq!(r.cursor(), 42);
    }

    /// Variable-length payload (Text + Data) — exercises the relative
    /// offset encoding that lives at the field slot.
    #[test]
    fn proto_keys_snapshot_with_payload() {
        use crate::schema::keys_snapshot;

        let body = keys_snapshot::Writer {
            application: "hanzo".to_string(),
            seq: 7,
            jwks_json: br#"{"keys":[]}"#.to_vec(),
        }
        .to_bytes();

        let r = keys_snapshot::Reader::from_slice(&body).unwrap();
        assert_eq!(r.application(), "hanzo");
        assert_eq!(r.seq(), 7);
        assert_eq!(r.jwks_json(), br#"{"keys":[]}"# as &[u8]);
    }
}
