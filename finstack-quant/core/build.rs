//! Build script for finstack-quant-core: generates calendar implementations from JSON.

#[path = "build/generate_calendars.rs"]
mod generate_calendars;

use std::io;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=data/calendars");
    println!("cargo:rerun-if-changed=build/generate_calendars.rs");
    generate_calendars::generate()
}
