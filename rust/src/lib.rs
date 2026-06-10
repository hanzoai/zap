//! hanzo-zap — Rust ZAP binary protocol.
//!
//! ZAP is the **Zero-copy Application Protocol** — a hand-rolled
//! little-endian binary format used for service-to-service
//! communication across the Hanzo / Lux stacks. The
//! on-wire bytes match the Go reference (`github.com/luxfi/zap`) and
//! the TypeScript reference (`@zap-proto/zap`); messages produced by
//! any one implementation are readable by the other two.
//!
//! ## Wire format
//!
//! Each message is a 16-byte little-endian header followed by a
//! data segment. See `wire::Header` for the field layout. Inside the
//! data segment, structs use offset-driven slot regions: fixed-size
//! fields packed at field offsets, variable-length fields (`Text`,
//! `Data`) reference an i32 LE relative offset + u32 LE length pair.
//!
//! ## Schema
//!
//! Schemas live in `.zap` files compiled by our own `zapc`
//! (`../zapc-rs`) into Rust readers/writers. There is no upstream
//! Cap'n Proto dependency.
//!
//! ## v1 history
//!
//! Earlier 1.0.0 of this crate was a JSON-payload-in-binary-frame
//! WebSocket prototype that never shipped. 1.0.1 is the canonical
//! 1.x impl.

pub mod msg;
pub mod schema;
pub mod server;
pub mod wire;

pub use wire::{Flags, Header, WireError, MAGIC, HEADER_SIZE, VERSION, MAX_SIZE};
pub use wire::{read_frame, write_frame};

/// Generated bindings — `init`, `push`, `keys_snapshot`, etc. Each
/// produced by zapc (../zapc-rs) at build time from `schema/zap.zap`.
/// Re-exported so callers can `use hanzo_zap::proto::keys_snapshot;`
/// without poking at the include path.
pub mod proto {
    pub use super::schema::*;
}
