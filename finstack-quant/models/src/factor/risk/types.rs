//! Portfolio-level factor assignment, decomposition, and what-if analysis.
//!
use crate::factor::{FactorId, RiskMeasure};
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
#[serde(deny_unknown_fields)]
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
    /// Per-position residual (idiosyncratic) variance contributions.
    ///
    /// Populated only by credit-aware position decomposers that have access to
    /// per-issuer idiosyncratic vol estimates. Empty when the decomposer has no
    /// position-level residual allocation.
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
        issuer_id: String,
    },
    /// Residual variance from any other source (e.g. unattributed simulation
    /// noise or a decomposer that does not track the issuer).
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
    pub position_id: String,
    /// Annualized variance allocated to this position's idiosyncratic risk.
    /// A hedge sharing another position's issuer shock may receive a negative
    /// allocation; the total residual variance must remain non-negative.
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
    pub position_id: String,
    /// Identifier of the contributing factor.
    pub factor_id: FactorId,
    /// Risk attributed to this position-factor pair.
    pub risk_contribution: f64,
}

#[cfg(test)]
mod tests {
    use super::{FactorContribution, RiskDecomposition};
    use crate::factor::{FactorId, RiskMeasure};

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
    fn risk_decomposition_requires_residual_contributions() {
        let incomplete_json = r#"{
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
        let error = serde_json::from_str::<RiskDecomposition>(incomplete_json)
            .expect_err("position_residual_contributions is required");
        assert!(error
            .to_string()
            .contains("position_residual_contributions"));
    }

    #[test]
    fn risk_decomposition_serializes_empty_residual_contributions() {
        let decomp = RiskDecomposition {
            total_risk: 0.0,
            measure: RiskMeasure::Variance,
            factor_contributions: vec![],
            residual_risk: 0.0,
            position_factor_contributions: vec![],
            position_residual_contributions: vec![],
        };
        let json = serde_json::to_value(&decomp).expect("serialize");
        assert_eq!(
            json["position_residual_contributions"],
            serde_json::json!([])
        );
    }
}
