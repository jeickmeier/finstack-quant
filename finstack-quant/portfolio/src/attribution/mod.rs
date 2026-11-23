//! Portfolio-level P&L attribution.
//!
//! Aggregates instrument-level attribution across all positions in a portfolio,
//! with currency conversion to portfolio base currency.

mod engine;
#[cfg(test)]
mod tests;

pub use engine::{PortfolioAttribution, ReconciliationReport, attribute_portfolio_pnl};
pub(crate) use engine::{attribution_endpoint_profile, reduce_prepared};
