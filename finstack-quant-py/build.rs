//! Linker flags for the Python cdylib.
//!
//! At dev `opt-level = 0` the extension's `__eh_frame` is about 25 MB. macOS
//! `ld` cannot encode compact-unwind offsets once that section passes 16 MB.
//! Skipping compact unwind leaves DWARF unwind info in `__eh_frame`, which is
//! what rustc uses to unwind panics.

fn main() {
    let apple = std::env::var("CARGO_CFG_TARGET_VENDOR").ok().as_deref() == Some("apple");
    let dev = std::env::var("PROFILE").ok().as_deref() == Some("debug");
    if apple && dev {
        println!("cargo:rustc-link-arg-cdylib=-Wl,-no_compact_unwind");
    }
}
