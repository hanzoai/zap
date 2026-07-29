//! zapc CLI — `zapc <input.zap> [-o output.rs]`. Used directly or
//! via `zapc::compile_to_rust` from a build.rs.

use anyhow::{Context, Result};
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut iter = args.into_iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "-o" => output = Some(iter.next().context("`-o` requires an output path")?.into()),
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            _ => input = Some(a.into()),
        }
    }
    let input = input.context("missing schema path; usage: zapc <schema.zap> [-o out.rs]")?;
    let output = output.unwrap_or_else(|| input.with_extension("rs"));

    zapc::compile_to_rust(&input, &output)?;
    eprintln!("zapc: wrote {}", output.display());
    Ok(())
}

fn print_help() {
    eprintln!(
        "zapc — ZAP schema compiler\n\
         \n\
         USAGE\n\
             zapc <SCHEMA.zap> [-o OUTPUT.rs]\n\
         \n\
         If -o is omitted, the output filename is the schema with the\n\
         extension swapped to .rs."
    );
}
