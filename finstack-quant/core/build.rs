//! Build script for finstack-quant-core: generates calendar implementations from JSON.

#[path = "build/currency_build.rs"]
mod currency_build;
#[path = "build/generate_calendars.rs"]
mod generate_calendars;

use std::io;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=data/iso_4217.csv");
    println!("cargo:rerun-if-changed=data/chinese_new_year.csv");
    println!("cargo:rerun-if-changed=data/calendars");
    println!("cargo:rerun-if-changed=build/currency_build.rs");
    println!("cargo:rerun-if-changed=build/generate_calendars.rs");
    println!("cargo:rerun-if-changed=src/generated/currency_generated.rs");
    currency_build::generate()?;
    generate_calendars::generate()
}
