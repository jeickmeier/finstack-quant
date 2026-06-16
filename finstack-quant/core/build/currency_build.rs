//! Copy the pre-generated currency enum into `OUT_DIR`.

use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

const SOURCE_FILE: &str = "src/generated/currency_generated.rs";

pub(crate) fn generate() -> io::Result<()> {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    let source = manifest_dir.join(SOURCE_FILE);
    let dest = out_dir.join("currency_generated.rs");
    fs::copy(&source, &dest)?;
    Ok(())
}
