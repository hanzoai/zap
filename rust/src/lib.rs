//! hanzo-zap — Rust ZAP binary protocol.
//!
//! ZAP is the **Zero-copy Application Protocol** — a Cap'n-Proto-style
//! binary RPC format used for service-to-service communication across
//! the Hanzo / Lux / Liquidity stacks. The on-wire format matches the
//! Go reference (`github.com/luxfi/zap`) and the TypeScript reference
//! (`@zap-proto/zap`); messages produced by any one implementation are
//! readable by the other two.
//!
//! ## Wire format
//!
//! Each message is a 16-byte little-endian header followed by a capnp
//! data segment. See `wire::Header` for the field layout.
//!
//! ## Zero-copy
//!
//! Decoded messages reference the underlying byte buffer directly; no
//! parsing or allocation occurs on the read path. Use
//! `capnp::message::Reader` (re-exported as `capnp_runtime`) to walk
//! the data segment.
//!
//! ## v1.x → v2.x migration
//!
//! Versions 1.x of this crate were a JSON-payload-in-binary-frame
//! WebSocket experiment. Version 2.0 is the real ZAP — wire-incompatible
//! with v1.x. Callers should upgrade together with the Go and TS
//! implementations.

pub mod msg;
pub mod server;
pub mod wire;

pub use wire::{Flags, Header, WireError, MAGIC, HEADER_SIZE, VERSION, MAX_SIZE};
pub use wire::{read_frame, write_frame};

/// Re-export of the capnp runtime so consumers don't need a direct dep.
pub use capnp as capnp_runtime;
