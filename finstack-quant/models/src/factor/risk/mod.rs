//! Product-independent factor and position risk decomposition kernels.
//!
//! The engines in this module operate on models-owned matrices and string
//! identifiers. Portfolio crates are responsible for converting richer
//! position identifiers and for constructing valuation-backed sensitivities.

mod budget;
mod math;
mod parametric;
mod position;
mod residual;
mod types;
mod views;

pub use budget::{
    evaluate_risk_budget_arrays, PositionBudgetEntry, RiskBudgetResult,
    DEFAULT_UTILIZATION_THRESHOLD,
};
pub use parametric::ParametricDecomposer;
pub use position::{
    build_stress_attribution, DecompositionConfig, DecompositionMethod,
    HistoricalPositionDecomposer, ParametricPositionDecomposer, PositionEsContribution,
    PositionRiskDecomposition, PositionVarContribution, StressAttribution, StressPositionEntry,
    TailScenarioBreakdown,
};
pub use residual::apply_residual_contributions;
pub use types::{
    FactorContribution, PositionFactorContribution, PositionResidualContribution,
    ResidualContributionSource, RiskDecomposition,
};
pub use views::{
    flatten_position_pnls, flatten_square_matrix, parametric_es_decomposition_view,
    ParametricEsDecompositionView, PositionEsContributionView,
};

/// Snap tolerance used when deciding whether a floating-point tail-size
/// product represents an integer.
pub(crate) const TAIL_COUNT_SNAP_TOLERANCE: f64 = 1e-9;

/// Return the number of scenarios in the loss tail.
pub(crate) fn tail_scenario_count(confidence: f64, n_scenarios: usize) -> usize {
    let raw = (1.0 - confidence) * n_scenarios as f64;
    let snapped = if (raw - raw.round()).abs() < TAIL_COUNT_SNAP_TOLERANCE {
        raw.round()
    } else {
        raw.ceil()
    };
    snapped as usize
}

#[cfg(test)]
mod tests {
    #[test]
    fn tail_scenario_count_uses_exact_rational_ceil() {
        assert_eq!(super::tail_scenario_count(0.99, 1000), 10);
        assert_eq!(super::tail_scenario_count(0.99, 200), 2);
        assert_eq!(super::tail_scenario_count(0.995, 200), 1);
        assert_eq!(super::tail_scenario_count(0.90, 1000), 100);
        assert_eq!(super::tail_scenario_count(0.99, 250), 3);
    }
}
