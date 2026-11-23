//! Metrics for structured credit instruments.
//!

//! This module organizes metrics by category:
//! - pricing: Valuation-focused metrics (prices, accrued, WAL)
//! - risk: Risk and sensitivity metrics (duration, spreads, YTM)
//! - pool: Collateral pool characteristics (WAM, CPR, CDR, WARF, WAS)
//! - deal_specific: Deal-type specific metrics (ABS, CLO, CMBS, RMBS)

pub(crate) mod deal_specific;
mod finite;
pub(crate) mod pool;
pub(crate) mod pricing;
pub(crate) mod quote;
mod register;
pub(crate) mod risk;
pub(crate) mod scenario;
pub(crate) mod summary;
#[cfg(test)]
mod tests;

pub(crate) use pricing::calculate_tranche_wal;
pub(crate) use register::{
    METRIC_TIME_BASIS, discounted_flows, metric_deal, register_structured_credit_metrics,
    reprice_in_scope,
};
pub(crate) use risk::{OasResult, calculate_tranche_cs01, calculate_tranche_z_spread};
pub(crate) use scenario::{ScenarioCell, ScenarioTable};
pub(crate) use summary::TrancheMetrics;
