//! Product-independent factor and position risk decomposition kernels.
//!
//! The engines in this module operate on models-owned matrices and string
//! identifiers. Portfolio crates are responsible for converting richer
//! position identifiers and for constructing valuation-backed sensitivities.

mod budget;
mod kernel;
mod parametric;
mod position;
mod report;
mod residual;
mod stress;
mod types;

pub use budget::{
    DEFAULT_UTILIZATION_THRESHOLD, PositionBudgetEntry, RiskBudgetResult, evaluate_risk_budget,
    evaluate_risk_budget_from_decomposition,
};
pub use parametric::decompose_factors;
pub use position::engine::{
    DecompositionMethod, PositionEsContribution, PositionRiskDecomposition,
    PositionVarContribution, flatten_position_pnls, historical_var_decomposition,
    parametric_var_decomposition,
};
pub use report::{
    PositionRiskBudget, PositionRiskReport, PositionRiskRequest, PositionRiskSource,
    position_risk_report,
};
pub use residual::apply_residual_contributions;
pub use stress::{
    StressAttribution, StressPositionEntry, TailScenarioBreakdown, build_stress_attribution,
};
pub use types::{
    FactorContribution, PositionFactorContribution, PositionResidualContribution,
    ResidualContributionSource, RiskDecomposition,
};

#[cfg(test)]
pub(super) use kernel::historical_sample_pnls;
use kernel::{
    MinTail, PreparedHistoricalScenarios, TAIL_COUNT_SNAP_TOLERANCE, VARIANCE_TOLERANCE,
    matvec_square, signed_sigma_multiplier, tail_multipliers, tail_scenario_count,
    validated_variance,
};
