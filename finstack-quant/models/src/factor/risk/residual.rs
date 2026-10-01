//! Residual-risk overlays for additive factor decompositions.

use super::math::{normal_pdf, normal_quantile};
use super::parametric::ParametricDecomposer;
use super::types::{PositionResidualContribution, RiskDecomposition};
use crate::factor::RiskMeasure;

/// Add one residual-risk overlay to a factor-only decomposition.
///
/// Factor and position-factor contributions are rescaled so the resulting
/// decomposition remains Euler-additive under the selected risk measure.
/// Pass all residual contributions together. An already-overlaid decomposition
/// is rejected without mutation rather than treating earlier residual risk as
/// systematic risk.
///
/// # Arguments
///
/// * `decomposition` - Factor-only decomposition to update in place; residual
///   risk must be zero and residual contribution rows must be empty.
/// * `residual_contributions` - Finite per-position annualized variance
///   allocations whose sum is non-negative. Individual allocations may be
///   negative for hedging positions sharing an issuer shock.
///
/// # Errors
///
/// Returns a validation error before mutation for an existing residual overlay,
/// non-finite allocations, a negative aggregate residual variance, or an invalid
/// risk measure or total risk.
pub fn apply_residual_contributions(
    decomposition: &mut RiskDecomposition,
    residual_contributions: Vec<PositionResidualContribution>,
) -> finstack_quant_core::Result<()> {
    if decomposition.residual_risk != 0.0
        || !decomposition.position_residual_contributions.is_empty()
    {
        return Err(finstack_quant_core::Error::Validation(
            "residual contributions must be applied together to a factor-only decomposition".into(),
        ));
    }
    decomposition.measure.validate()?;
    if !decomposition.total_risk.is_finite() {
        return Err(finstack_quant_core::Error::Validation(
            "total risk must be finite before applying residual contributions".into(),
        ));
    }
    if residual_contributions
        .iter()
        .any(|contribution| !contribution.residual_variance.is_finite())
    {
        return Err(finstack_quant_core::Error::Validation(
            "residual variance allocations must be finite".into(),
        ));
    }
    let residual_variance: f64 = residual_contributions
        .iter()
        .map(|contribution| contribution.residual_variance)
        .sum();
    if !residual_variance.is_finite() || residual_variance < 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "aggregate residual variance must be finite and non-negative".into(),
        ));
    }
    if residual_variance == 0.0 {
        decomposition
            .position_residual_contributions
            .extend(residual_contributions);
        return Ok(());
    }

    let systematic_variance =
        variance_from_measure(decomposition.measure, decomposition.total_risk)?;
    let combined_variance = systematic_variance + residual_variance;
    let (combined_total, combined_component_scale) =
        ParametricDecomposer::scale_for_measure(&decomposition.measure, combined_variance)?;
    let (_, systematic_component_scale) =
        ParametricDecomposer::scale_for_measure(&decomposition.measure, systematic_variance)?;
    let factor_rescale = if systematic_component_scale.abs() > 0.0 {
        combined_component_scale / systematic_component_scale
    } else {
        0.0
    };

    for contribution in &mut decomposition.factor_contributions {
        contribution.absolute_risk *= factor_rescale;
        contribution.marginal_risk *= factor_rescale;
        contribution.relative_risk = if combined_total.abs() > 0.0 {
            contribution.absolute_risk / combined_total
        } else {
            0.0
        };
    }
    for contribution in &mut decomposition.position_factor_contributions {
        contribution.risk_contribution *= factor_rescale;
    }

    decomposition.total_risk = combined_total;
    decomposition.residual_risk = residual_variance * combined_component_scale;
    decomposition
        .position_residual_contributions
        .extend(residual_contributions);
    Ok(())
}

fn variance_from_measure(
    measure: RiskMeasure,
    total_risk: f64,
) -> finstack_quant_core::Result<f64> {
    let valid_sign = match measure {
        RiskMeasure::Variance | RiskMeasure::Volatility => total_risk >= 0.0,
        RiskMeasure::VaR { .. } | RiskMeasure::ExpectedShortfall { .. } => total_risk <= 0.0,
    };
    if !valid_sign {
        return Err(finstack_quant_core::Error::Validation(
            "total risk has the wrong sign for its risk measure".into(),
        ));
    }
    let variance = match measure {
        RiskMeasure::Variance => total_risk.max(0.0),
        RiskMeasure::Volatility => total_risk * total_risk,
        RiskMeasure::VaR { confidence } => {
            let z = normal_quantile(confidence);
            if z > 0.0 {
                (total_risk / -z).powi(2)
            } else {
                0.0
            }
        }
        RiskMeasure::ExpectedShortfall { confidence } => {
            let z = normal_quantile(confidence);
            let es_multiplier = normal_pdf(z) / (1.0 - confidence);
            if es_multiplier > 0.0 {
                (total_risk / -es_multiplier).powi(2)
            } else {
                0.0
            }
        }
    };
    Ok(variance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factor::risk::ResidualContributionSource;
    use crate::factor::{FactorCovarianceMatrix, FactorId, SensitivityMatrix};

    fn base(measure: RiskMeasure) -> finstack_quant_core::Result<RiskDecomposition> {
        let factors = vec![FactorId::new("F")];
        let covariance = FactorCovarianceMatrix::new(factors.clone(), vec![1.0])?;
        let mut sensitivities = SensitivityMatrix::zeros(vec!["P".into()], factors);
        sensitivities.set_delta(0, 0, 10.0);
        ParametricDecomposer.decompose(&sensitivities, &covariance, &measure)
    }

    fn residual(variance: f64) -> PositionResidualContribution {
        PositionResidualContribution {
            position_id: "P".into(),
            residual_variance: variance,
            source: ResidualContributionSource::Other,
        }
    }

    #[test]
    fn repeated_residual_overlay_is_rejected_without_mutation() -> finstack_quant_core::Result<()> {
        for measure in [
            RiskMeasure::Variance,
            RiskMeasure::Volatility,
            RiskMeasure::VaR { confidence: 0.99 },
        ] {
            let mut decomposition = base(measure)?;
            apply_residual_contributions(&mut decomposition, vec![residual(25.0)])?;
            let first = decomposition.clone();
            assert!(
                apply_residual_contributions(&mut decomposition, vec![residual(25.0)]).is_err()
            );
            assert_eq!(decomposition, first);
            let contribution_sum: f64 = decomposition
                .factor_contributions
                .iter()
                .map(|row| row.absolute_risk)
                .sum::<f64>()
                + decomposition.residual_risk;
            assert!((contribution_sum - decomposition.total_risk).abs() < 1e-12);
        }
        Ok(())
    }

    #[test]
    fn residual_allocations_allow_hedges_but_reject_invalid_totals(
    ) -> finstack_quant_core::Result<()> {
        let mut decomposition = base(RiskMeasure::Variance)?;
        apply_residual_contributions(&mut decomposition, vec![residual(50.0), residual(-25.0)])?;
        assert_eq!(decomposition.total_risk, 125.0);
        assert_eq!(decomposition.residual_risk, 25.0);
        for invalid in [-1.0, f64::NAN, f64::INFINITY] {
            let mut decomposition = base(RiskMeasure::Variance)?;
            let before = decomposition.clone();
            assert!(
                apply_residual_contributions(&mut decomposition, vec![residual(invalid)]).is_err()
            );
            assert_eq!(decomposition, before);
        }
        Ok(())
    }
}
