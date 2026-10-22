//! Build script for the Python cdylib.
//!
//! - Compiles the `.pyi` stub docstrings into `$OUT_DIR/pydoc` (see
//!   `build/pydoc.rs`); the `pydoc!` macro reads them into `__doc__`.
//! - Linker flags: at dev `opt-level = 0` the extension's `__eh_frame` is about
//!   25 MB. macOS `ld` cannot encode compact-unwind offsets once that section
//!   passes 16 MB. Skipping compact unwind leaves DWARF unwind info in
//!   `__eh_frame`, which is what rustc uses to unwind panics.

#[path = "build/pydoc.rs"]
mod pydoc;

use std::path::PathBuf;

fn main() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    // Each stub is watched individually (the package root also holds the built
    // extension, which must not trigger a rebuild). A new stub file is picked up
    // the next time the Rust bindings that reference it change.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build/pydoc.rs");
    println!("cargo:rerun-if-changed=src/bindings");
    pydoc::generate(&manifest_dir.join("finstack_quant"), &out_dir);

    let apple = std::env::var("CARGO_CFG_TARGET_VENDOR").ok().as_deref() == Some("apple");
    let dev = std::env::var("PROFILE").ok().as_deref() == Some("debug");
    if apple && dev {
        println!("cargo:rustc-link-arg-cdylib=-Wl,-no_compact_unwind");
    }
}
