//! Build script — compiles `schema/zap.zap` into Rust readers/writers
//! via our own `zapc` compiler (no upstream Cap'n Proto dependency).
//!
//! Generated module lives at `OUT_DIR/zap_schema.rs` and is included
//! by `src/schema.rs`.

use std::path::PathBuf;

fn main() {
    let schema = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema/zap.zap");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("zap_schema.rs");
    zapc::compile_to_rust(&schema, &out).expect("zapc compile failed");
    println!("cargo:rerun-if-changed=schema/zap.zap");
    println!("cargo:rerun-if-changed=../zapc-rs/src");
}
