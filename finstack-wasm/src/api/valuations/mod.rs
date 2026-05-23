//! WASM bindings for the `finstack-valuations` crate.
//!
//! Split by domain:
//! - [`pricing`] — instrument JSON validation, pricing, metric introspection.
//! - [`analytic`] — closed-form option primitives (Black-Scholes, Black-76, IV).
//! - [`attribution`] — P&L attribution across multiple methodologies.
//! - [`credit_factor_model`] — credit factor hierarchy: calibration, level
//!   decomposition, period decomposition, covariance forecast.
//! - [`calibration`] — plan-driven calibration engine.
//! - [`correlation`] — mirrors `finstack_valuations::correlation`.
//! - [`credit`] — structural-credit model factories (Merton, CreditGrades,
//!   dynamic recovery, endogenous hazard, toggle exercise).
//! - [`credit_derivatives`] — CDS-family example payload factories.
//! - [`fourier`] — COS-method Fourier pricers (Black-Scholes, VG, Merton).
//! - [`exotic_rates`] — deterministic TARN / snowball / range-accrual helpers.
//! - [`sabr`] — SABR parameters, model, smile, and calibrator.

pub mod analytic;
pub mod attribution;
pub mod calibration;
pub mod correlation;
pub mod credit;
pub mod credit_derivatives;
pub mod credit_factor_model;
pub mod exotic_rates;
pub mod fourier;
pub mod fx;
pub mod market_handle;
pub mod pricing;
pub mod sabr;
