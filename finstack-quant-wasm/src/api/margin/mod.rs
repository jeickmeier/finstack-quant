//! WASM bindings for the `finstack-quant-margin` crate.
//!
//! Data types (CSA specifications, schedules, profiles, results) cross the
//! boundary as plain JSON values typed by the generated TypeScript; engines,
//! calculators and the sensitivity builders are classes. The submodules hold
//! one family each.

mod calculators;
mod frtb;
mod im;
mod metrics;
mod sa_ccr;
mod types;
mod xva;

mod bilateral;
mod parse;
#[cfg(all(test, target_arch = "wasm32"))]
mod tests;

pub use bilateral::*;
