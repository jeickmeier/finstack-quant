//! WASM bindings for the `finstack-factor-model` crate.
//!
//! Re-exports credit factor model types from the valuations domain so the
//! Rust module tree mirrors the JS namespace tree (`factor_model` is a
//! top-level namespace in `finstack-wasm/index.js`).

pub use super::valuations::credit_factor_model::{
    decompose_levels, decompose_period, WasmCreditCalibrator, WasmCreditFactorModel,
    WasmFactorCovarianceForecast, WasmLevelsAtDate, WasmPeriodDecomposition,
};
