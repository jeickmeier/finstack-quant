//! WASM bindings for the `finstack-quant-valuations` crate.
//!
//! Split by domain:
//! - [`pricing`] — instrument JSON validation, pricing, metric introspection.
//! - [`credit_derivatives`] — CDS-family example payload factories.
//! - [`exotic_rates`] — deterministic TARN / snowball / range-accrual helpers.
//! - [`fixed_income`] — typed `Bond` / `TermLoan` instrument classes.

pub mod composite;
pub mod credit_derivatives;
pub mod exotic_rates;
pub mod fixed_income;
pub mod fx;
pub mod pricing;
pub mod structured_credit;
