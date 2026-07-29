//! ZAP — the Zero-copy Application Protocol, in Rust.
//!
//! A hand-rolled little-endian binary format used for service-to-service
//! communication across the Hanzo / Lux stacks. The on-wire bytes match
//! the Go reference (`github.com/zap-proto/go`) and the TypeScript
//! reference (`@zap-proto/zap`); a message produced by any one
//! implementation is readable by the other two, and `tests/crosswire.rs`
//! pins Go's actual output as the assertion.
//!
//! The crate is published as `zap-proto` and imported as `zap` — the
//! same split the Go module has (`github.com/zap-proto/go`, package
//! `zap`).
//!
//! ## The four layers
//!
//! | module | what it owns |
//! |---|---|
//! | [`wire`] | the 16-byte message header and self-delimiting message I/O |
//! | [`codec`] | the data segment — offset-driven readers and a builder |
//! | [`rpc`] | the call envelope (method / promise / capability) and promise pipelining |
//! | [`transport`] | connections: [`transport::Conn::connect`], [`transport::Server::listen`] |
//!
//! Each layer is complete on its own: a message needs no envelope, an
//! envelope needs no connection, and a connection adds only the
//! direction tag a socket requires.
//!
//! ## One round trip, end to end
//!
//! ```no_run
//! use zap::rpc::{build_response, parse_request, STATUS_OK};
//! use zap::transport::{Conn, Server};
//!
//! # async fn demo() -> Result<(), zap::WireError> {
//! let srv = Server::listen("/run/hanzo/app.sock", |_| {
//!     |env: &[u8]| {
//!         let call = parse_request(env)?;
//!         Ok(build_response(STATUS_OK, call.promise_id, &call.payload))
//!     }
//! })
//! .await?;
//!
//! let conn = Conn::connect(srv.addr()).await?;
//! let answer = conn.invoke(1, &[], b"ping").await?;
//! assert_eq!(answer.body, b"ping");
//! # Ok(()) }
//! ```
//!
//! ## Confidentiality
//!
//! ZAP frames are PLAINTEXT and carry no session crypto. A Unix socket
//! is confined to one host and its permissions — that is the supported
//! deployment. A remote hop must stay on HTTPS until the crypto work
//! lands; see `README.md` § Session crypto.

pub mod codec;
pub mod msg;
pub mod rpc;
pub mod schema;
pub mod transport;
pub mod wire;

pub use codec::{Builder, List, Message, Object};
pub use wire::{read_frame, write_frame};
pub use wire::{Flags, Header, WireError, HEADER_SIZE, MAGIC, MAX_SIZE, VERSION, VERSION2};

/// Generated bindings — `init`, `push`, `keys_snapshot`, etc. Each
/// produced by zapc (../zapc-rs) at build time from `schema/zap.zap`.
/// Re-exported so callers can `use zap::proto::keys_snapshot;` without
/// poking at the include path.
pub mod proto {
    pub use super::schema::*;
}
