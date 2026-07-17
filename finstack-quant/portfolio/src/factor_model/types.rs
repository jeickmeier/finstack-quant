//! Portfolio-level factor assignment, decomposition, and what-if analysis.
//!
use crate::types::PositionId;
use finstack_quant_core::types::IssuerId;
use finstack_quant_factor_model::{FactorId, RiskMeasure};
use serde::{Deserialize, Serialize};

/// Portfolio-level decomposition of total risk across common factors and residuals.
///
/// # Sign convention
///
/// The sign of [`RiskDecomposition::total_risk`], [`FactorContribution::absolute_risk`],
/// [`FactorContribution::marginal_risk`], and [`RiskDecomposition::residual_risk`]
/// depends on the selected [`RiskMeasure`]:
///
/// * [`RiskMeasure::Variance`] and [`RiskMeasure::Volatility`] — non-negative.
/// * [`RiskMeasure::VaR`] and [`RiskMeasure::ExpectedShortfall`] — **non-positive**
///   (losses reported as negative numbers; see [`RiskMeasure`] for details).
///
/// [`FactorContribution::relative_risk`] is always a dimensionless share and
/// stays non-negative for a long-risk portfolio because numerator and denominator
/// carry the same sign.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskDecomposition {
    /// Total portfolio risk under the selected `measure`. Sign follows the
    /// measure's convention (see struct-level docs).
    pub total_risk: f64,
    /// Risk measure used to aggregate and report the decomposition.
    pub measure: RiskMeasure,
    /// Aggregate factor-level contributions to portfolio risk.
    pub factor_contributions: Vec<FactorContribution>,
    /// Unattributed or idiosyncratic risk left after factor aggregation.
    /// Same sign convention as `total_risk`.
    pub residual_risk: f64,
    /// Per-position, per-factor contributions that roll up into the portfolio view.
    pub position_factor_contributions: Vec<PositionFactorContribution>,
    /// Optional per-position residual (idiosyncratic) variance contributions.
    ///
    /// Populated only by credit-aware position decomposers that have access to
    /// per-issuer idiosyncratic vol estimates. Empty by default for backward
    /// compatibility with pre-PR-6 callers and serialized artifacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub position_residual_contributions: Vec<PositionResidualContribution>,
}

/// Source of a per-position residual variance contribution.
///
/// Distinguishes residuals derived from a credit factor model (where the
/// idiosyncratic adder is calibrated per issuer) from generic / unattributed
/// residual sources used by other decomposers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResidualContributionSource {
    /// Residual variance sourced from a `CreditFactorModel`'s idiosyncratic
    /// vol forecast for the named issuer.
    FromCreditModel {
        /// Issuer whose idiosyncratic vol drives this residual contribution.
        issuer_id: IssuerId,
    },
    /// Residual variance from any other source (e.g. unattributed simulation
    /// noise or a legacy decomposer that does not track the issuer).
    Other,
}

/// Annualized residual variance contributed by a single position.
///
/// `residual_variance` is reported in variance units (not vol) so that
/// per-position residuals add linearly into a portfolio-level total. Callers
/// that want per-position residual *vol* should take the square root of this
/// field after summing the relevant subset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionResidualContribution {
    /// Portfolio position identifier.
    pub position_id: PositionId,
    /// Annualized variance contributed by this position's idiosyncratic risk.
    /// Always non-negative.
    pub residual_variance: f64,
    /// Where the residual variance came from.
    pub source: ResidualContributionSource,
}

/// Contribution of a single factor to portfolio risk.
///
/// See [`RiskDecomposition`] for the sign convention applied to each field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactorContribution {
    /// Identifier of the factor being reported.
    pub factor_id: FactorId,
    /// Absolute contribution of the factor to the chosen risk measure.
    /// Sign follows the measure's convention.
    pub absolute_risk: f64,
    /// Contribution expressed as a share of total portfolio risk. Dimensionless,
    /// non-negative for standard long-risk portfolios (signs of numerator and
    /// denominator cancel).
    pub relative_risk: f64,
    /// Marginal sensitivity of portfolio risk to the factor. Same sign
    /// convention as `absolute_risk`.
    pub marginal_risk: f64,
}

/// Contribution of a single position to a specific factor bucket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionFactorContribution {
    /// Portfolio position identifier.
    pub position_id: PositionId,
    /// Identifier of the contributing factor.
    pub factor_id: FactorId,
    /// Risk attributed to this position-factor pair.
    pub risk_contribution: f64,
}

#[cfg(test)]
mod tests {
    use super::{FactorContribution, RiskDecomposition};
    use finstack_quant_factor_model::{FactorId, RiskMeasure};

    #[test]
    fn test_risk_decomposition_total_matches_sum() {
        let decomp = RiskDecomposition {
            total_risk: 100.0,
            measure: RiskMeasure::Variance,
            factor_contributions: vec![
                FactorContribution {
                    factor_id: FactorId::new("Rates"),
                    absolute_risk: 60.0,
                    relative_risk: 0.6,
                    marginal_risk: 0.3,
                },
                FactorContribution {
                    factor_id: FactorId::new("Credit"),
                    absolute_risk: 40.0,
                    relative_risk: 0.4,
                    marginal_risk: 0.2,
                },
            ],
            residual_risk: 0.0,
            position_factor_contributions: vec![],
            position_residual_contributions: vec![],
        };

        let sum: f64 = decomp
            .factor_contributions
            .iter()
            .map(|c| c.absolute_risk)
            .sum();
        assert!((sum + decomp.residual_risk - decomp.total_risk).abs() < 1e-10);
    }

    #[test]
    fn test_relative_risk_sums_to_one_when_residual_risk_is_zero() {
        let decomp = RiskDecomposition {
            total_risk: 100.0,
            measure: RiskMeasure::Variance,
            factor_contributions: vec![
                FactorContribution {
                    factor_id: FactorId::new("Rates"),
                    absolute_risk: 60.0,
                    relative_risk: 0.6,
                    marginal_risk: 0.3,
                },
                FactorContribution {
                    factor_id: FactorId::new("Credit"),
                    absolute_risk: 40.0,
                    relative_risk: 0.4,
                    marginal_risk: 0.2,
                },
            ],
            residual_risk: 0.0,
            position_factor_contributions: vec![],
            position_residual_contributions: vec![],
        };

        let sum_rel: f64 = decomp
            .factor_contributions
            .iter()
            .map(|c| c.relative_risk)
            .sum();
        assert!((sum_rel - 1.0).abs() < 1e-10);
    }

    #[test]
    fn risk_decomposition_deserializes_without_residual_contributions() {
        // Pre-PR-6 serialized JSON: no `position_residual_contributions` field.
        // Backward-compat invariant: this must deserialize cleanly with the
        // new field defaulting to an empty vector.
        let legacy_json = r#"{
            "total_risk": 100.0,
            "measure": "variance",
            "factor_contributions": [
                {
                    "factor_id": "Rates",
                    "absolute_risk": 60.0,
                    "relative_risk": 0.6,
                    "marginal_risk": 0.3
                }
            ],
            "residual_risk": 0.0,
            "position_factor_contributions": []
        }"#;

        let decomp: RiskDecomposition =
            serde_json::from_str(legacy_json).expect("legacy JSON must deserialize");

        assert!((decomp.total_risk - 100.0).abs() < 1e-12);
        assert_eq!(decomp.measure, RiskMeasure::Variance);
        assert_eq!(decomp.factor_contributions.len(), 1);
        assert!(decomp.position_residual_contributions.is_empty());
    }

    #[test]
    fn risk_decomposition_omits_empty_residual_contributions_in_serialized_form() {
        // The new field is `skip_serializing_if = "Vec::is_empty"`, so callers
        // that never populate it produce wire output identical to pre-PR-6.
        let decomp = RiskDecomposition {
            total_risk: 0.0,
            measure: RiskMeasure::Variance,
            factor_contributions: vec![],
            residual_risk: 0.0,
            position_factor_contributions: vec![],
            position_residual_contributions: vec![],
        };
        let json = serde_json::to_string(&decomp).expect("serialize");
        assert!(
            !json.contains("position_residual_contributions"),
            "empty residual contributions must be omitted from wire form, got {json}"
        );
    }
}
