//! Built-in financial metrics.
//!
//! This module provides access to standard financial metrics that are
//! bundled with the crate. Metric JSON sources are embedded at compile time
//! from `data/metrics` and are exposed via a small helper that is used by
//! [`Registry::load_builtins()`](crate::registry::Registry::load_builtins).
//!
//! Metrics are organized into namespaces:
//! - `fin.*` - Standard financial metrics
//!   - `fin_basic.json` - Basic metrics (gross_profit, net_income, etc.)
//!   - `fin_margins.json` - Margin calculations
//!   - `fin_returns.json` - Return metrics (ROE, ROA, ROIC, etc.)
//!   - `fin_leverage.json` - Leverage ratios
//!
//! ## Reported-cost convention
//!
//! Bundled metrics read `cogs` and `opex` as reported costs, with their
//! depreciation and amortization included. `depreciation` and `amortization`
//! are the D&A included in those lines, added back once in EBITDA:
//!
//! - `fin.gross_profit = revenue - cogs`
//! - `fin.operating_income = fin.gross_profit - opex`
//! - `fin.ebit = fin.operating_income`
//! - `fin.ebitda = fin.ebit + depreciation + amortization`
//! - `fin.ebt = fin.ebit - interest_expense`
//! - `fin.net_income = fin.ebt - tax_expense`
//!
//! The margins and cost ratios are `ratio(numerator, revenue)`, which is
//! unavailable when revenue is missing, zero or negative.
//!
//! ## Usage
//!
//! Built-in metrics are loaded via [`Registry::load_builtins()`](crate::registry::Registry::load_builtins)
//! from compile-time embedded JSON sources, so packaged binaries and WASM
//! builds do not require a runtime `data/metrics` directory.

/// Discover and load all bundled metric registry JSON files.
///
/// Built-in metrics are embedded at compile time for all targets so packaged
/// binaries do not depend on a source-tree `data/metrics` directory at runtime.
pub(crate) fn builtin_metric_sources() -> &'static [&'static str] {
    &[
        include_str!("../../data/metrics/fin_basic.json"),
        include_str!("../../data/metrics/fin_leverage.json"),
        include_str!("../../data/metrics/fin_margins.json"),
        include_str!("../../data/metrics/fin_returns.json"),
    ]
}
