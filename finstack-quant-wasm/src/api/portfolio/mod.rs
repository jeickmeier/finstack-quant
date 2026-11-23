//! WASM bindings for the `finstack-quant-portfolio` crate.
//!
//! Exposes portfolio spec parsing, validation, and result extraction for
//! JavaScript/TypeScript consumption.
//!
//! # Return conventions
//!
//! Two return shapes are used, and the name says which:
//!
//! - **Computation results** — valuations, aggregations, attributions,
//!   optimizations, decompositions and P&L — return **plain structured
//!   JavaScript objects** built with
//!   [`crate::utils::to_js_value`]. Read properties directly; call
//!   `JSON.stringify` when a canonical JSON string is needed (for example to
//!   chain into an entry point that still takes a `*Json` argument).
//! - **Wire / spec / validator surfaces** — anything whose purpose is to echo a
//!   canonical document for re-ingest — return a **JSON string** and their
//!   names end in `Json` (`Portfolio.toJson`, `parsePortfolioSpecJson`).
//!
//! The materialization entry points (`Portfolio.fromMaterialization`,
//! `Portfolio.validateMaterialization`) return plain structured objects
//! (`{ portfolio, report }`, and `MaterializationReport | ValidationReport`
//! respectively); validation diagnostics are returned, not thrown.
//!
//! A Rust method on a result type is a free function that takes the plain
//! result object first (`portfolioAttributionExplainText`,
//! `portfolioValuationGetPositionValue`, `portfolioMetricsGetTotal`, ...); see
//! the `results` module.
//!
//! For repeated calls against the same portfolio (scenario sweeps,
//! interactive dashboards), reuse the `Portfolio` and `MarketContext` handles
//! accepted by the calculation entry points.

pub mod attribution;
pub mod brinson;
pub mod builder;
pub mod cashflows;
pub mod excess_return;
pub mod exposure;
pub mod factor_brinson;
pub mod factor_model;
pub mod fi_attribution;
pub mod grid_attribution;
mod handle;
pub mod margin;
pub mod materialization;
pub mod metrics;
pub mod optimization;
pub mod performance;
pub mod position;
pub mod primitive;
pub mod replay;
pub mod results;
pub mod scenarios;
pub mod sensitivity;
pub mod valuation;

pub use exposure::{instrument_exposure_profile, portfolio_exposure_profile};
pub use handle::*;
