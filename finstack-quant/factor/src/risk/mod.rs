//! Product-independent factor and position risk decomposition kernels.
//!
//! The engines in this module operate on models-owned matrices and string
//! identifiers. Portfolio crates are responsible for converting richer
//! position identifiers and for constructing valuation-backed sensitivities.

mod budget;
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
pub use position::{
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

use tracing::warn;

use crate::RiskMeasure;

/// Absolute tolerance below which a computed variance is numerical noise:
/// variances in `[-VARIANCE_TOLERANCE, 0)` clamp to zero, lower ones are
/// rejected.
const VARIANCE_TOLERANCE: f64 = 1e-12;

/// Snap tolerance used when deciding whether a floating-point tail-size
/// product represents an integer.
const TAIL_COUNT_SNAP_TOLERANCE: f64 = 1e-9;

/// Normal-tail multipliers at `confidence`: the quantile `z = Φ⁻¹(c)` and the
/// expected-shortfall multiplier `φ(z) / (1 − c)`.
fn tail_multipliers(confidence: f64) -> finstack_quant_core::Result<(f64, f64)> {
    let z = finstack_quant_core::math::special_functions::standard_normal_inv_cdf(confidence)?;
    Ok((
        z,
        finstack_quant_core::math::special_functions::norm_pdf(z)? / (1.0 - confidence),
    ))
}

/// Return the number of scenarios in the loss tail.
fn tail_scenario_count(confidence: f64, n_scenarios: usize) -> usize {
    let raw = (1.0 - confidence) * n_scenarios as f64;
    let snapped = if (raw - raw.round()).abs() < TAIL_COUNT_SNAP_TOLERANCE {
        raw.round()
    } else {
        raw.ceil()
    };
    snapped as usize
}

/// Matrix-vector product `M * v` for row-major square `n x n` matrix `M`.
fn matvec_square(matrix: &[f64], vector: &[f64]) -> Vec<f64> {
    let n = vector.len();
    if n == 0 {
        return Vec::new();
    }
    matrix
        .chunks_exact(n)
        .map(|row| {
            row.iter()
                .zip(vector)
                .map(|(entry, weight)| entry * weight)
                .sum()
        })
        .collect()
}

/// Reject non-finite or materially negative variance; clamp rounding noise `[-tol, 0)` to `0.0`.
fn validated_variance(variance: f64) -> finstack_quant_core::Result<f64> {
    if !variance.is_finite() {
        Err(finstack_quant_core::Error::Validation(format!(
            "Portfolio variance is not finite ({variance}); check sensitivities and covariance inputs"
        )))
    } else if variance < -VARIANCE_TOLERANCE {
        Err(finstack_quant_core::Error::Validation(format!(
            "Portfolio variance must be non-negative, got {variance}"
        )))
    } else {
        Ok(variance.max(0.0))
    }
}

/// Signed `σ` multiplier under the P&L-loss sign convention (`None` for `Variance`).
fn signed_sigma_multiplier(measure: RiskMeasure) -> finstack_quant_core::Result<Option<f64>> {
    match measure {
        RiskMeasure::Variance => Ok(None),
        RiskMeasure::Volatility => Ok(Some(1.0)),
        RiskMeasure::VaR { confidence } => Ok(Some(
            -finstack_quant_core::math::special_functions::standard_normal_inv_cdf(confidence)?,
        )),
        RiskMeasure::ExpectedShortfall { confidence } => {
            let (_, es_multiplier) = tail_multipliers(confidence)?;
            Ok(Some(-es_multiplier))
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum MinTail {
    AtLeastOne,
    AtLeastTwo,
}

struct PreparedHistoricalScenarios<'a> {
    position_ids: &'a [String],
    position_pnls: &'a [f64],
    n_scenarios: usize,
    confidence: f64,
    portfolio_pnls: Vec<(usize, f64)>,
}

impl<'a> PreparedHistoricalScenarios<'a> {
    fn prepare(
        position_ids: &'a [String],
        position_pnls: &'a [f64],
        n_scenarios: usize,
        confidence: Option<f64>,
        min_tail: MinTail,
    ) -> finstack_quant_core::Result<Self> {
        let confidence = confidence.unwrap_or(0.95);
        let n_positions = position_ids.len();
        let expected_len = n_scenarios.checked_mul(n_positions).ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "position_pnls size overflows usize for n_scenarios ({n_scenarios}) * \
                 n_positions ({n_positions})"
            ))
        })?;
        if position_pnls.len() != expected_len {
            return Err(finstack_quant_core::Error::Validation(format!(
                "position_pnls length ({}) must equal n_scenarios ({n_scenarios}) * \
                 n_positions ({n_positions}) = {expected_len}",
                position_pnls.len()
            )));
        }

        crate::config::validate_confidence(confidence)?;

        if n_positions == 0 || n_scenarios == 0 {
            return Ok(Self {
                position_ids,
                position_pnls,
                n_scenarios,
                confidence,
                portfolio_pnls: Vec::new(),
            });
        }

        match min_tail {
            MinTail::AtLeastOne => {
                if ((1.0 - confidence) * (n_scenarios as f64)) < 1.0 - TAIL_COUNT_SNAP_TOLERANCE {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "confidence {confidence} with {n_scenarios} scenarios yields zero tail \
                         scenarios; lower confidence or provide more scenarios"
                    )));
                }
            }
            MinTail::AtLeastTwo => {
                let n_tail = tail_scenario_count(confidence, n_scenarios);
                if n_tail < 2 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "historical decomposition requires at least two tail scenarios for \
                         confidence {confidence} (got {n_tail} from {n_scenarios} scenarios); \
                         increase n_scenarios or lower the confidence level"
                    )));
                }
                if n_tail < 30 {
                    warn!(
                        n_tail,
                        n_scenarios,
                        confidence,
                        "Tail sample size is small; historical VaR/ES decomposition may lack \
                         statistical reliability"
                    );
                }
            }
        }

        // Pre-flight: any non-finite P&L corrupts the sort below
        // (`partial_cmp(NaN, _) = None`) and silently degrades the tail ordering.
        if let Some(bad_idx) = position_pnls.iter().position(|p| !p.is_finite()) {
            let scenario = bad_idx / n_positions;
            let position = bad_idx % n_positions;
            return Err(finstack_quant_core::Error::Validation(format!(
                "position_pnls contains non-finite value at scenario {scenario}, \
                 position {position} (value = {}); upstream P&L generator must \
                 produce finite values",
                position_pnls[bad_idx]
            )));
        }

        let mut portfolio_pnls: Vec<(usize, f64)> = (0..n_scenarios)
            .map(|s| {
                let row_start = s * n_positions;
                let pnl: f64 = position_pnls[row_start..row_start + n_positions]
                    .iter()
                    .sum();
                (s, pnl)
            })
            .collect();
        portfolio_pnls.sort_by(|a, b| a.1.total_cmp(&b.1));

        Ok(Self {
            position_ids,
            position_pnls,
            n_scenarios,
            confidence,
            portfolio_pnls,
        })
    }
}

#[cfg(test)]
pub(super) fn historical_sample_pnls() -> (Vec<String>, Vec<f64>, usize) {
    let ids = vec!["A".into(), "B".into(), "C".into()];
    let n_scenarios = 200;
    let pnls: Vec<f64> = (0..n_scenarios * 3)
        .map(|k| {
            let x = (k as f64 * 0.731).sin() * 100.0;
            x - (k % 7) as f64 * 3.5
        })
        .collect();
    (ids, pnls, n_scenarios)
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
