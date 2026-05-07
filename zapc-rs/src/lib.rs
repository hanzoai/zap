//! zapc — ZAP schema compiler.
//!
//! Parses `.zap` schema files into an in-memory IR (`Schema`) and
//! emits Rust readers/writers via `codegen::emit_rust`. Hand-rolled
//! parser — no dependency on upstream Cap'n Proto tooling.
//!
//! # Schema syntax
//!
//! ```text
//! @0xc0a8b5e0e5f0a000;     // file id (random 64-bit, hex)
//!
//! struct KeysSnapshot {
//!     application @0 :Text;
//!     seq         @1 :UInt64;
//!     jwksJson    @2 :Data;
//! }
//!
//! struct Ping { nonce @0 :UInt64; }
//! ```
//!
//! Supported types: `Bool`, `UInt8/16/32/64`, `Int8/16/32/64`,
//! `Float32/64`, `Text` (UTF-8 string), `Data` (byte slice).
//!
//! Each field has a stable index. The on-wire layout is the canonical
//! ZAP packing (matches Go luxfi/zap):
//!
//!   - Fixed-size primitives are packed at the field offset (computed
//!     from field index × max(field-size, alignment)).
//!   - `Text` and `Data` store an i32 LE relative offset + u32 LE
//!     length at the field offset; payload bytes follow the struct.
//!   - Structs are 8-byte aligned.

pub mod codegen;
pub mod parser;

pub use codegen::emit_rust;
pub use parser::{Field, FieldType, Schema, Struct};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("zapc: parse error at {0}: {1}")]
    Parse(String, String),
    #[error("zapc: io: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Convenience for build.rs callers — parse + emit in one shot.
///
/// `out_path` is typically `concat!(env!("OUT_DIR"), "/zap_schema.rs")`.
pub fn compile_to_rust(schema_path: &std::path::Path, out_path: &std::path::Path) -> Result<()> {
    let src = std::fs::read_to_string(schema_path)?;
    let schema = parser::parse(&src)?;
    let rs = codegen::emit_rust(&schema);
    std::fs::write(out_path, rs)?;
    Ok(())
}
