//! Build script — compiles the canonical Cap'n Proto schema into
//! Rust bindings that the runtime walks zero-copy.
//!
//! Generated module lives at `OUT_DIR/zap_capnp.rs` and is included
//! by `src/schema.rs`. Re-run this whenever `schema/zap.capnp`
//! changes; cargo handles the dep tracking via `rerun-if-changed`.

fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/zap.capnp")
        .run()
        .expect("compiling schema/zap.capnp failed — is `capnp` on PATH?");

    println!("cargo:rerun-if-changed=schema/zap.capnp");
}
