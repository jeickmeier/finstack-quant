//! WASM bindings for factor, credit-factor, and factor-risk models.
//!
//! Mirrors the Rust `finstack_quant_models::factor` layout:
//! - [`credit`] — credit hierarchy artifacts, calibration, covariance
//!   forecasts and the factor-covariance helpers (`models.factor.credit`).
//! - [`risk`] — product-independent position risk decomposition, risk
//!   budgeting and stress attribution (`models.factor.risk`).

pub mod credit;
pub mod risk;
