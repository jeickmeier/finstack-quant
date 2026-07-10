//! Build script for finstack-quant-core: generates calendar implementations from JSON.

#[path = "build/currency_build.rs"]
mod currency_build;
#[path = "build/generate_calendars.rs"]
mod generate_calendars;
#[path = "build/generate_sifma_settlements.rs"]
mod generate_sifma_settlements;

use std::io;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=data/iso_4217.csv");
    println!("cargo:rerun-if-changed=data/chinese_new_year.csv");
    println!("cargo:rerun-if-changed=data/dragon_boat.csv");
    println!("cargo:rerun-if-changed=data/mid_autumn.csv");
    println!("cargo:rerun-if-changed=data/sifma_settlements.csv");
    println!("cargo:rerun-if-changed=data/calendars");
    println!("cargo:rerun-if-changed=build/currency_build.rs");
    println!("cargo:rerun-if-changed=build/generate_calendars.rs");
    println!("cargo:rerun-if-changed=build/generate_sifma_settlements.rs");
    println!("cargo:rerun-if-changed=src/generated/currency_generated.rs");
    currency_build::generate()?;
    generate_calendars::generate()?;
    generate_sifma_settlements::generate()
}
