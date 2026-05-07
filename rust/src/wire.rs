//! ZAP binary wire format — header parser/builder.
//!
//! Aligned with `github.com/luxfi/zap@v0.2.0` (Go reference):
//!
//! ```text
//! ┌─ Header (16 bytes) ────────────────────────────────────────────┐
//! │  Magic       4  "ZAP\0"                                        │
//! │  Version     2  u16 LE  (currently 1)                          │
//! │  Flags       2  u16 LE  (compressed, encrypted, signed, …)     │
//! │  RootOffset  4  u32 LE  offset to root within data segment     │
//! │  Size        4  u32 LE  total message size including header    │
//! └────────────────────────────────────────────────────────────────┘
//! ┌─ Data segment (variable, 8-byte aligned) ──────────────────────┐
//! │  Cap'n-Proto-style structs / lists / text / bytes              │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! All multi-byte integers are little-endian. The data segment uses
//! the canonical capnp segment layout (see `capnp::serialize`); the
//! ZAP header is a thin extension that adds a magic+version self-check
//! before handing the segment to capnp for zero-copy reads.
//!
//! The protocol is wire-compatible with the Go and TypeScript ZAP
//! implementations. Rust callers should use `capnp::message::Reader`
//! to walk the data segment with zero allocation.

use std::io::{self, Read, Write};

/// Magic bytes identifying a ZAP message: `ZAP\0`.
pub const MAGIC: [u8; 4] = [b'Z', b'A', b'P', 0];

/// Header size in bytes.
pub const HEADER_SIZE: usize = 16;

/// Current protocol version.
pub const VERSION: u16 = 1;

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
        if version != VERSION {
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

/// Read a complete ZAP message (header + data segment) from a reader.
/// Returns the data-segment bytes ready to feed into `capnp::message::Reader`.
///
/// Zero-copy is achieved at the capnp layer; this function still
/// allocates a `Vec` for the body since `Read` doesn't promise an
/// underlying slice. Use the async variant on TCP for pooled buffers.
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

/// Write a ZAP frame to an async writer. Used by `server::handle_conn`.
pub async fn write_frame_async<W: tokio::io::AsyncWrite + Unpin>(
    w: &mut W,
    flags: Flags,
    root_offset: u32,
    body: &[u8],
) -> Result<(), WireError> {
    use tokio::io::AsyncWriteExt;
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
    w.write_all(&hdr.encode()).await?;
    w.write_all(body).await?;
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
        assert!(matches!(Header::decode(&bytes), Err(WireError::InvalidMagic)));
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
            application: "liquidity".to_string(),
            cursor: 42,
        }
        .to_bytes();

        let mut framed = Vec::new();
        write_frame(&mut framed, Flags::NONE, 0, &body).unwrap();

        let mut cur = std::io::Cursor::new(&framed);
        let (_hdr, body_back) = read_frame(&mut cur).unwrap();

        let r = keys_subscribe::Reader::from_slice(&body_back).unwrap();
        assert_eq!(r.application(), "liquidity");
        assert_eq!(r.cursor(), 42);
    }

    /// Variable-length payload (Text + Data) — exercises the relative
    /// offset encoding that lives at the field slot.
    #[test]
    fn proto_keys_snapshot_with_payload() {
        use crate::schema::keys_snapshot;

        let body = keys_snapshot::Writer {
            application: "liquidity".to_string(),
            seq: 7,
            jwks_json: br#"{"keys":[]}"#.to_vec(),
        }
        .to_bytes();

        let r = keys_snapshot::Reader::from_slice(&body).unwrap();
        assert_eq!(r.application(), "liquidity");
        assert_eq!(r.seq(), 7);
        assert_eq!(r.jwks_json(), br#"{"keys":[]}"# as &[u8]);
    }
}
