//! WASM bindings for the `finstack-quant-valuations` crate.
//!
//! Split by domain:
//! - [`pricing`] — instrument JSON validation, pricing, metric introspection.
//! - [`analytic`] — closed-form option primitives (Black-Scholes, Black-76, IV).
//! - [`calibration`] — plan-driven calibration engine.
//! - [`correlation`] — mirrors `finstack_quant_valuations::correlation`.
//! - [`credit`] — structural-credit model factories (Merton, CreditGrades,
//!   dynamic recovery, endogenous hazard, toggle exercise).
//! - [`credit_derivatives`] — CDS-family example payload factories.
//! - [`fourier`] — COS-method Fourier pricers (Black-Scholes, VG, Merton).
//! - [`exotic_rates`] — deterministic TARN / snowball / range-accrual helpers.
//! - [`sabr`] — SABR parameters, model, smile, and calibrator.

pub mod analytic;
pub mod calibration;
pub mod correlation;
pub mod credit;
pub mod credit_derivatives;
pub mod exotic_rates;
pub mod fourier;
pub mod fx;
pub mod market_handle;
pub mod pricing;
pub mod sabr;
pub mod structured_credit;
