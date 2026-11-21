//! WASM bindings for factor, credit-factor, and factor-risk models.
//!
//! Mirrors the Rust `finstack_quant_factor` layout:
//! - [`config`] — crate-root factor covariance and configuration helpers (`models.factor`).
//! - [`credit`] — credit hierarchy artifacts, calibration, and covariance
//!   forecasts (`models.factor.credit`).
//! - [`risk`] — product-independent position risk decomposition, risk
//!   budgeting and stress attribution (`models.factor.risk`).

pub mod config;
pub mod credit;
pub mod risk;
