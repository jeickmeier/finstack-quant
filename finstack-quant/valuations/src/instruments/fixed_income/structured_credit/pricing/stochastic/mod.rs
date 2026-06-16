//! Stochastic models for structured credit.
//!
//! This module provides stochastic prepayment and default models with:
//! - Factor-driven CPR/CDR models with correlation
//! - Scenario tree infrastructure for path-dependent valuation
//! - Industry-standard calibrations (RMBS, CLO, CMBS)
//! - Stochastic pricing engine with NPV and risk metrics
//! - Comprehensive risk metrics and correlation sensitivities
//!
//! # Module Organization
//!
//! - [`calibrations`]: Standard calibration constants for RMBS, CLO, CMBS
//! - [`prepayment`]: Stochastic prepayment models (factor-correlated, Richard-Roll)
//! - [`default`]: Stochastic default models (copula-based, intensity process)
//! - [`correlation`]: Correlation structures for structured credit
//! - [`tree`]: Scenario tree infrastructure for path-dependent analysis
//! - [`pricer`]: Stochastic pricing engine with tree and MC modes
//! - [`metrics`]: Risk metrics and correlation sensitivities

pub(crate) mod calibrations;
pub(crate) mod correlation;
pub(crate) mod default;
#[cfg(test)]
pub(crate) mod metrics;
pub(crate) mod prepayment;
pub(crate) mod pricer;
pub(crate) mod tree;

// Re-export main types (may be used by external bindings or tests)
#[allow(unused_imports)]
pub(crate) use calibrations::{
    clo_standard, cmbs_standard, rmbs_standard, CloCalibration, CmbsCalibration, RmbsCalibration,
};
pub use correlation::CorrelationStructure;
#[allow(unused_imports)]
pub(crate) use default::{CopulaBasedDefault, IntensityProcessDefault, StochasticDefault};
pub use default::{PoolGranularity, StochasticDefaultSpec};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use metrics::{
    CorrelationSensitivities, SensitivityConfig, StochasticMetrics, StochasticMetricsCalculator,
};
pub use prepayment::StochasticPrepaySpec;
#[allow(unused_imports)]
pub(crate) use prepayment::{FactorCorrelatedPrepay, RichardRollPrepay, StochasticPrepayment};
pub use pricer::{PricingMode, StochasticPricingResult, TranchePricingResult};
#[allow(unused_imports)] // May be used by external bindings
pub(crate) use pricer::{StochasticPricer, StochasticPricerConfig};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use tree::{
    BranchingSpec, ScenarioNode, ScenarioNodeId, ScenarioPath, ScenarioTree, ScenarioTreeConfig,
};
