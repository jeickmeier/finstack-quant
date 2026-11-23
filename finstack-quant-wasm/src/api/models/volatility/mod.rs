//! Product-independent volatility model and evaluator bindings for WASM.
//!
//! Exposes `SabrParameters`, `SabrSmile`, and `SabrCalibrator` to
//! JS/TS alongside evaluators for the core data-only volatility artifacts.
//!
//! Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.

mod implied_svi;
mod sabr;
mod surface;
#[cfg(test)]
mod tests;

pub use implied_svi::*;
pub use sabr::*;
pub use surface::*;
