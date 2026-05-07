//! ZAP message-type discriminator.
//!
//! The header has a single 16-bit `flags` field for transport-layer
//! flags (compression, encryption, signing). Message routing happens
//! inside the data segment via the capnp root struct id, NOT a
//! separate type byte — Cap'n-Proto-style typed dispatch keeps the
//! header minimal and lets schemas evolve independently.
//!
//! These constants are the canonical 64-bit type ids exposed via the
//! capnp schema in `schema/zap.capnp`. Handlers match against the
//! root struct's type id rather than parsing the segment first.

/// ── Connection lifecycle ────────────────────────────────────────
pub const INIT: u64 = 0xa001;
pub const INIT_ACK: u64 = 0xa002;

/// ── RPC ─────────────────────────────────────────────────────────
pub const PUSH: u64 = 0xa010;
pub const RESOLVE: u64 = 0xa012;
pub const REJECT: u64 = 0xa013;

/// ── JWKS-via-ZAP push ───────────────────────────────────────────
///
/// Replaces JWKS HTTP polling. IAM publishes KEYS_SNAPSHOT on
/// `KEYS_SUBSCRIBE`, then KEYS_ROTATE / KEYS_REVOKE on every Cert
/// mutation. Consumers cache locally, no refetching.
pub const KEYS_SUBSCRIBE: u64 = 0xa050;
pub const KEYS_SNAPSHOT: u64 = 0xa051;
pub const KEYS_ROTATE: u64 = 0xa052;
pub const KEYS_REVOKE: u64 = 0xa053;

/// ── Control ─────────────────────────────────────────────────────
pub const PING: u64 = 0xa0f0;
pub const PONG: u64 = 0xa0f1;
pub const ERROR: u64 = 0xa0ff;
