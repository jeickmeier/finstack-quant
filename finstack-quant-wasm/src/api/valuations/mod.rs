//! WASM bindings for the `finstack-quant-valuations` crate.
//!
//! Split by domain:
//! - [`pricing`] — instrument JSON validation, pricing, metric introspection.
//! - [`composite`] — composite-instrument initialization, rebalancing,
//!   decomposition and history.
//! - [`market`] — the market convention registry.
//! - [`results`] — `ValuationResult` methods as free functions.
//! - [`risk`] — portfolio historical VaR / expected shortfall.
//! - [`schema`] — compiled-in JSON Schemas of the valuations wire format.
//! - [`typed`] — the typed instrument classes (fixed income, FX, rates,
//!   credit, equity, structured credit), their builders, the standalone
//!   structured-credit tranche analytics, and the instrument data-type
//!   constructors.

pub mod composite;
pub mod market;
pub mod pricing;
pub mod results;
pub mod risk;
pub mod schema;
pub mod typed;
