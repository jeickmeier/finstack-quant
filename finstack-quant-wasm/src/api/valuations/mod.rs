//! WASM bindings for the `finstack-quant-valuations` crate.
//!
//! Split by domain:
//! - [`pricing`] — instrument JSON validation, pricing, metric introspection.
//! - [`composite`] — composite-instrument initialization, rebalancing,
//!   decomposition and history.
//! - [`exotic_rates`] — deterministic TARN / snowball / range-accrual helpers.
//! - [`fixed_income`] — typed `Bond` / `TermLoan` / `RevolvingCredit` /
//!   `AssetBackedFacility` instrument classes.
//! - [`fx`] — typed FX instrument classes.
//! - [`market`] — the market convention registry.
//! - [`results`] — `ValuationResult` methods as free functions.
//! - [`risk`] — portfolio historical VaR / expected shortfall.
//! - [`schema`] — compiled-in JSON Schemas of the valuations wire format.
//! - [`structured_credit`] — standalone structured-credit tranche analytics
//!   (discount margin, OAS, break-even CDR, scenario table).
//! - [`typed`] — member parity for the typed instrument classes, their
//!   builders, and the instrument data-type constructors.

pub mod composite;
pub mod exotic_rates;
pub mod fixed_income;
pub mod fx;
pub mod market;
pub mod pricing;
pub mod results;
pub mod risk;
pub mod schema;
pub mod structured_credit;
pub mod typed;
