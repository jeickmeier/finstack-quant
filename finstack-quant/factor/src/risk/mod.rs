//! Product-independent factor and position risk decomposition kernels.
//!
//! The engines in this module operate on models-owned matrices and string
//! identifiers. Portfolio crates are responsible for converting richer
//! position identifiers and for constructing valuation-backed sensitivities.

mod budget;
mod parametric;
mod position;
mod residual;
mod types;

pub use budget::{
    evaluate_risk_budget, PositionBudgetEntry, RiskBudgetResult, DEFAULT_UTILIZATION_THRESHOLD,
};
pub use parametric::decompose_factors;
pub use position::{
    build_stress_attribution, flatten_position_pnls, historical_var_decomposition,
    parametric_var_decomposition, DecompositionMethod, PositionEsContribution,
    PositionRiskDecomposition, PositionVarContribution, StressAttribution, StressPositionEntry,
    TailScenarioBreakdown,
};
pub use residual::apply_residual_contributions;
pub use types::{
    FactorContribution, PositionFactorContribution, PositionResidualContribution,
    ResidualContributionSource, RiskDecomposition,
};

/// Absolute tolerance below which a computed variance is numerical noise:
/// variances in `[-VARIANCE_TOLERANCE, 0)` clamp to zero, lower ones are
/// rejected.
pub(crate) const VARIANCE_TOLERANCE: f64 = 1e-12;

/// Normal-tail multipliers at `confidence`: the quantile `z = Φ⁻¹(c)` and the
/// expected-shortfall multiplier `φ(z) / (1 − c)`.
pub(crate) fn tail_multipliers(confidence: f64) -> (f64, f64) {
    let z = finstack_quant_core::math::standard_normal_inv_cdf(confidence);
    (
        z,
        finstack_quant_core::math::norm_pdf(z) / (1.0 - confidence),
    )
}

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
