//! Parametric factor risk decomposition using covariance-based Euler allocation.

use super::simulation::cholesky;
use super::traits::RiskDecomposer;
use super::types::{FactorContribution, PositionFactorContribution, RiskDecomposition};
use crate::types::PositionId;
use finstack_core::factor_model::{FactorCovarianceMatrix, RiskMeasure};

use crate::sensitivity::SensitivityMatrix;

/// Covariance-based decomposer for linear factor risk measures.
///
/// `ParametricDecomposer` assumes the incoming [`SensitivityMatrix`] rows are already
/// position-weighted by the upstream sensitivity engine. Portfolio exposures are therefore
/// just the column sums of the matrix, and Euler allocations are computed directly from those
/// weighted exposures.
///
/// # Sign convention
///
/// `Variance` and `Volatility` are returned as non-negative numbers. `VaR` and
/// `ExpectedShortfall` follow the P&L sign convention: **losses are reported as
/// negative numbers**, so `total_risk` and factor contributions for VaR / ES are
/// non-positive for a long-risk portfolio. `relative_risk` is preserved as a
/// non-negative share because numerator and denominator carry the same sign.
///
/// # References
///
/// - `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
/// - `docs/REFERENCES.md#tasche-2008-capital-allocation`
#[derive(Debug, Clone, Copy, Default)]
pub struct ParametricDecomposer;

impl ParametricDecomposer {
    const VARIANCE_TOLERANCE: f64 = 1e-12;

    fn validate_factor_axes(
        sensitivities: &SensitivityMatrix,
        covariance: &FactorCovarianceMatrix,
    ) -> finstack_core::Result<()> {
        if sensitivities.n_factors() != covariance.n_factors() {
            return Err(finstack_core::Error::Validation(
                "SensitivityMatrix and FactorCovarianceMatrix factor dimensions do not match"
                    .to_string(),
            ));
        }

        if sensitivities.factor_ids() != covariance.factor_ids() {
            return Err(finstack_core::Error::Validation(
                "SensitivityMatrix and FactorCovarianceMatrix factor order does not match"
                    .to_string(),
            ));
        }

        let n = covariance.n_factors();
        let data = covariance.as_slice();

        if data.iter().any(|entry| !entry.is_finite()) {
            return Err(finstack_core::Error::Validation(
                "Covariance matrix entries must be finite".to_string(),
            ));
        }

        for i in 0..n {
            for j in (i + 1)..n {
                if (data[i * n + j] - data[j * n + i]).abs() > Self::VARIANCE_TOLERANCE {
                    return Err(finstack_core::Error::Validation(format!(
                        "Covariance matrix is not symmetric at ({i}, {j})"
                    )));
                }
            }
        }

        // Verify positive semi-definiteness via a rank-tolerant Cholesky.
        // A non-PSD covariance matrix can produce meaningless (negative)
        // factor contributions that look like diversification benefits.
        // We reuse the simulation-module factorization which accepts
        // rank-deficient (PSD-but-not-PD) matrices — these arise naturally
        // when users regularize a covariance matrix with shrinkage or when
        // two factors are perfectly collinear at a given as-of.
        //
        // The error message includes the smallest diagonal entry (a cheap
        // proxy for "where conditioning likely failed") and the matrix size
        // so risk teams can diagnose whether they need shrinkage / ridge
        // regularization without re-running an external tool.
        if n > 0 {
            let min_diag = (0..n)
                .map(|i| data[i * n + i])
                .fold(f64::INFINITY, f64::min);
            let max_diag = (0..n)
                .map(|i| data[i * n + i])
                .fold(f64::NEG_INFINITY, f64::max);

            // Cheap ill-conditioning proxy: a covariance whose variances span
            // many orders of magnitude can produce numerically unstable Euler
            // allocations even when Cholesky succeeds (a near-zero pivot still
            // factorizes). Warn rather than reject — the matrix may be
            // legitimately PSD — and never auto-regularize, which would
            // silently alter the risk estimate.
            const CONDITION_WARN_RATIO: f64 = 1e12;
            if min_diag > 0.0 && max_diag / min_diag > CONDITION_WARN_RATIO {
                tracing::warn!(
                    n,
                    min_diag,
                    max_diag,
                    "factor covariance diagonal spans >1e12; Euler risk \
                     allocations may be numerically unstable — consider \
                     Ledoit-Wolf shrinkage or ridge regularization"
                );
            }

            cholesky(data, n).map_err(|e| {
                finstack_core::Error::Validation(format!(
                    "Covariance matrix is not positive semi-definite \
                     (n = {n}, min diagonal = {min_diag:.6e}, max diagonal = {max_diag:.6e}): \
                     {e}. Consider Ledoit-Wolf shrinkage or a ridge regularization \
                     of the covariance estimate."
                ))
            })?;
        }

        Ok(())
    }

    fn portfolio_exposures(sensitivities: &SensitivityMatrix) -> Vec<f64> {
        let mut exposures = vec![0.0; sensitivities.n_factors()];

        for row in sensitivities
            .as_slice()
            .chunks_exact(sensitivities.n_factors())
        {
            for (exposure, delta) in exposures.iter_mut().zip(row.iter()) {
                *exposure += *delta;
            }
        }

        exposures
    }

    fn covariance_times_exposures(
        covariance: &FactorCovarianceMatrix,
        exposures: &[f64],
    ) -> Vec<f64> {
        let mut cov_times_exposure = vec![0.0; covariance.n_factors()];

        for (result, row) in cov_times_exposure
            .iter_mut()
            .zip(covariance.as_slice().chunks_exact(covariance.n_factors()))
        {
            *result = row
                .iter()
                .zip(exposures.iter())
                .map(|(entry, exposure)| entry * exposure)
                .sum();
        }

        cov_times_exposure
    }

    fn scale_for_measure(
        measure: &RiskMeasure,
        variance: f64,
    ) -> finstack_core::Result<(f64, f64)> {
        measure.validate()?;
        let variance = Self::validated_variance(variance)?;
        let sigma = variance.sqrt();

        let scaled = match measure {
            RiskMeasure::Variance => (variance, 1.0),
            RiskMeasure::Volatility => {
                if sigma > 0.0 {
                    (sigma, sigma.recip())
                } else {
                    (0.0, 0.0)
                }
            }
            RiskMeasure::VaR { confidence } => {
                // Loss convention: VaR is reported as a negative number.
                let z_score = super::math::normal_quantile(*confidence);
                if sigma > 0.0 {
                    (-sigma * z_score, -z_score * sigma.recip())
                } else {
                    (0.0, 0.0)
                }
            }
            RiskMeasure::ExpectedShortfall { confidence } => {
                // Loss convention: ES is reported as a negative number.
                let z_score = super::math::normal_quantile(*confidence);
                let es_multiplier = super::math::normal_pdf(z_score) / (1.0 - confidence);
                if sigma > 0.0 {
                    (-sigma * es_multiplier, -es_multiplier * sigma.recip())
                } else {
                    (0.0, 0.0)
                }
            }
        };

        Ok(scaled)
    }

    fn validated_variance(variance: f64) -> finstack_core::Result<f64> {
        if variance < -Self::VARIANCE_TOLERANCE {
            Err(finstack_core::Error::Validation(format!(
                "Portfolio variance must be non-negative, got {variance}"
            )))
        } else {
            Ok(variance.max(0.0))
        }
    }
}

impl RiskDecomposer for ParametricDecomposer {
    fn decompose(
        &self,
        sensitivities: &SensitivityMatrix,
        covariance: &FactorCovarianceMatrix,
        measure: &RiskMeasure,
    ) -> finstack_core::Result<RiskDecomposition> {
        Self::validate_factor_axes(sensitivities, covariance)?;
        measure.validate()?;

        if sensitivities.n_factors() == 0 {
            return Ok(RiskDecomposition {
                total_risk: 0.0,
                measure: *measure,
                factor_contributions: Vec::new(),
                residual_risk: 0.0,
                position_factor_contributions: Vec::new(),
                position_residual_contributions: Vec::new(),
            });
        }

        let exposures = Self::portfolio_exposures(sensitivities);
        let cov_times_exposure = Self::covariance_times_exposures(covariance, &exposures);
        let component_variance: Vec<f64> = exposures
            .iter()
            .zip(cov_times_exposure.iter())
            .map(|(exposure, covariance_exposure)| exposure * covariance_exposure)
            .collect();
        let variance: f64 = component_variance.iter().sum();

        let (total_risk, scale) = Self::scale_for_measure(measure, variance)?;
        let factor_ids = covariance.factor_ids();

        let factor_contributions = factor_ids
            .iter()
            .zip(component_variance.iter().zip(cov_times_exposure.iter()))
            .map(
                |(factor_id, (factor_component_variance, marginal_component_variance))| {
                    let absolute_risk = factor_component_variance * scale;
                    let relative_risk = if total_risk.abs() > 0.0 {
                        absolute_risk / total_risk
                    } else {
                        0.0
                    };
                    let marginal_risk = marginal_component_variance * scale;

                    FactorContribution {
                        factor_id: factor_id.clone(),
                        absolute_risk,
                        relative_risk,
                        marginal_risk,
                    }
                },
            )
            .collect();

        let position_factor_contributions = sensitivities
            .position_ids()
            .iter()
            .zip(
                sensitivities
                    .as_slice()
                    .chunks_exact(sensitivities.n_factors()),
            )
            .flat_map(|(position_id, row)| {
                factor_ids
                    .iter()
                    .zip(row.iter().zip(cov_times_exposure.iter()))
                    .map(move |(factor_id, (delta, covariance_exposure))| {
                        PositionFactorContribution {
                            position_id: PositionId::from(position_id.clone()),
                            factor_id: factor_id.clone(),
                            risk_contribution: delta * covariance_exposure * scale,
                        }
                    })
            })
            .collect();

        Ok(RiskDecomposition {
            total_risk,
            measure: *measure,
            factor_contributions,
            residual_risk: 0.0,
            position_factor_contributions,
            position_residual_contributions: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ParametricDecomposer;
    use crate::factor_model::RiskDecomposer;
    use crate::sensitivity::SensitivityMatrix;
    use crate::types::PositionId;
    use finstack_core::factor_model::{FactorCovarianceMatrix, FactorId, RiskMeasure};

    type TestResult = finstack_core::Result<()>;

    fn test_setup() -> finstack_core::Result<(SensitivityMatrix, FactorCovarianceMatrix)> {
        let mut sensitivities = SensitivityMatrix::zeros(
            vec!["pos-A".into(), "pos-B".into()],
            vec![FactorId::new("Rates"), FactorId::new("Credit")],
        );
        sensitivities.set_delta(0, 0, 100.0);
        sensitivities.set_delta(0, 1, 0.0);
        sensitivities.set_delta(1, 0, 0.0);
        sensitivities.set_delta(1, 1, 50.0);

        let covariance = FactorCovarianceMatrix::new(
            vec![FactorId::new("Rates"), FactorId::new("Credit")],
            vec![0.04, 0.03, 0.03, 0.09],
        )?;

        Ok((sensitivities, covariance))
    }

    #[test]
    fn test_parametric_variance_decomposition_uses_weighted_sensitivities_directly() -> TestResult {
        let (sensitivities, covariance) = test_setup()?;
        let decomposer = ParametricDecomposer;
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Variance)?;

        assert!((result.total_risk - 925.0).abs() < 1e-10);

        let Some(rates) = result
            .factor_contributions
            .iter()
            .find(|contribution| contribution.factor_id == FactorId::new("Rates"))
        else {
            return Err(finstack_core::Error::Validation(
                "rates contribution must exist".to_string(),
            ));
        };
        let Some(credit) = result
            .factor_contributions
            .iter()
            .find(|contribution| contribution.factor_id == FactorId::new("Credit"))
        else {
            return Err(finstack_core::Error::Validation(
                "credit contribution must exist".to_string(),
            ));
        };

        assert!((rates.absolute_risk - 550.0).abs() < 1e-10);
        assert!((credit.absolute_risk - 375.0).abs() < 1e-10);
        assert!((rates.relative_risk - (550.0 / 925.0)).abs() < 1e-10);
        assert!((credit.relative_risk - (375.0 / 925.0)).abs() < 1e-10);
        assert!((rates.marginal_risk - 5.5).abs() < 1e-10);
        assert!((credit.marginal_risk - 7.5).abs() < 1e-10);
        assert!((result.residual_risk).abs() < 1e-12);

        let sum_relative: f64 = result
            .factor_contributions
            .iter()
            .map(|contribution| contribution.relative_risk)
            .sum();
        assert!((sum_relative - 1.0).abs() < 1e-12);

        let expected_rows = vec![
            (PositionId::new("pos-A"), FactorId::new("Rates"), 550.0),
            (PositionId::new("pos-A"), FactorId::new("Credit"), 0.0),
            (PositionId::new("pos-B"), FactorId::new("Rates"), 0.0),
            (PositionId::new("pos-B"), FactorId::new("Credit"), 375.0),
        ];

        assert_eq!(
            result.position_factor_contributions.len(),
            expected_rows.len()
        );

        for (position_id, factor_id, expected_contribution) in expected_rows {
            let Some(row) = result
                .position_factor_contributions
                .iter()
                .find(|contribution| {
                    contribution.position_id == position_id && contribution.factor_id == factor_id
                })
            else {
                return Err(finstack_core::Error::Validation(
                    "position-factor contribution must exist".to_string(),
                ));
            };
            assert!((row.risk_contribution - expected_contribution).abs() < 1e-10);
        }

        Ok(())
    }

    #[test]
    fn test_parametric_volatility_decomposition_scales_component_variance() -> TestResult {
        let (sensitivities, covariance) = test_setup()?;
        let decomposer = ParametricDecomposer;
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Volatility)?;

        let sigma = 925.0_f64.sqrt();
        assert!((result.total_risk - sigma).abs() < 1e-10);

        let Some(rates) = result
            .factor_contributions
            .iter()
            .find(|contribution| contribution.factor_id == FactorId::new("Rates"))
        else {
            return Err(finstack_core::Error::Validation(
                "rates contribution must exist".to_string(),
            ));
        };
        let Some(credit) = result
            .factor_contributions
            .iter()
            .find(|contribution| contribution.factor_id == FactorId::new("Credit"))
        else {
            return Err(finstack_core::Error::Validation(
                "credit contribution must exist".to_string(),
            ));
        };

        assert!((rates.absolute_risk - (550.0 / sigma)).abs() < 1e-10);
        assert!((credit.absolute_risk - (375.0 / sigma)).abs() < 1e-10);
        assert!((rates.marginal_risk - (5.5 / sigma)).abs() < 1e-10);
        assert!((credit.marginal_risk - (7.5 / sigma)).abs() < 1e-10);

        Ok(())
    }

    #[test]
    fn test_parametric_var_decomposition_uses_validated_confidence_scaling() -> TestResult {
        let (sensitivities, covariance) = test_setup()?;
        let decomposer = ParametricDecomposer;
        let result = decomposer.decompose(
            &sensitivities,
            &covariance,
            &RiskMeasure::VaR { confidence: 0.99 },
        )?;

        let sigma = 925.0_f64.sqrt();
        let z_99 = 2.326_347_874_040_840_8;
        // VaR is reported as a negative loss.
        let expected_var = -sigma * z_99;
        assert!((result.total_risk - expected_var).abs() < 1e-6);
        assert!(result.total_risk < 0.0, "VaR must be negative");

        let Some(rates) = result
            .factor_contributions
            .iter()
            .find(|contribution| contribution.factor_id == FactorId::new("Rates"))
        else {
            return Err(finstack_core::Error::Validation(
                "rates contribution must exist".to_string(),
            ));
        };
        assert!((rates.absolute_risk - (-(550.0 / sigma) * z_99)).abs() < 1e-6);
        assert!(rates.absolute_risk < 0.0);
        // Relative share is preserved as a non-negative fraction because total
        // and component share the same sign.
        assert!((rates.relative_risk - (550.0 / 925.0)).abs() < 1e-10);

        Ok(())
    }

    #[test]
    fn test_parametric_expected_shortfall_scales_component_variance() -> TestResult {
        let (sensitivities, covariance) = test_setup()?;
        let decomposer = ParametricDecomposer;
        let result = decomposer.decompose(
            &sensitivities,
            &covariance,
            &RiskMeasure::ExpectedShortfall { confidence: 0.99 },
        )?;

        let sigma = 925.0_f64.sqrt();
        let z_99 = 2.326_347_874_040_840_8;
        let pdf = (-0.5_f64 * z_99 * z_99).exp() / (2.0_f64 * std::f64::consts::PI).sqrt();
        let es_multiplier = pdf / 0.01;

        // ES is reported as a negative loss.
        assert!((result.total_risk - (-sigma * es_multiplier)).abs() < 1e-6);
        assert!(result.total_risk < 0.0, "ES must be negative");

        let Some(rates) = result
            .factor_contributions
            .iter()
            .find(|contribution| contribution.factor_id == FactorId::new("Rates"))
        else {
            return Err(finstack_core::Error::Validation(
                "rates contribution must exist".to_string(),
            ));
        };
        assert!((rates.absolute_risk - (-(550.0 / sigma) * es_multiplier)).abs() < 1e-6);
        assert!(rates.absolute_risk < 0.0);

        Ok(())
    }

    #[test]
    fn test_parametric_zero_risk_portfolio_has_no_nan_relative_risk() -> TestResult {
        let sensitivities = SensitivityMatrix::zeros(
            vec!["cash".into()],
            vec![FactorId::new("Rates"), FactorId::new("Credit")],
        );
        let covariance = FactorCovarianceMatrix::new(
            vec![FactorId::new("Rates"), FactorId::new("Credit")],
            vec![0.04, 0.0, 0.0, 0.09],
        )?;
        let decomposer = ParametricDecomposer;
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Variance)?;

        assert!((result.total_risk).abs() < 1e-12);
        for contribution in &result.factor_contributions {
            assert!((contribution.absolute_risk).abs() < 1e-12);
            assert!((contribution.relative_risk).abs() < 1e-12);
            assert!(!contribution.relative_risk.is_nan());
        }

        Ok(())
    }

    #[test]
    fn test_parametric_rejects_factor_axis_order_mismatch() -> TestResult {
        let (sensitivities, _) = test_setup()?;
        let covariance = FactorCovarianceMatrix::new(
            vec![FactorId::new("Credit"), FactorId::new("Rates")],
            vec![0.09, 0.03, 0.03, 0.04],
        )?;
        let decomposer = ParametricDecomposer;
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Variance);

        assert!(result.is_err());
        let Err(error) = result else {
            return Err(finstack_core::Error::Validation(
                "factor order mismatch should be rejected".to_string(),
            ));
        };
        assert!(format!("{error}").contains("factor"));

        Ok(())
    }

    #[test]
    fn test_parametric_zero_factor_axes_return_empty_zero_risk_decomposition() -> TestResult {
        let sensitivities = SensitivityMatrix::zeros(vec!["cash".into()], vec![]);
        let covariance = FactorCovarianceMatrix::new(vec![], vec![])?;
        let decomposer = ParametricDecomposer;
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Variance)?;

        assert!((result.total_risk).abs() < 1e-12);
        assert!(result.factor_contributions.is_empty());
        assert!(result.position_factor_contributions.is_empty());
        assert!((result.residual_risk).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn test_parametric_rejects_negative_portfolio_variance_from_unchecked_covariance() {
        let mut sensitivities =
            SensitivityMatrix::zeros(vec!["pos-A".into()], vec![FactorId::new("Rates")]);
        sensitivities.set_delta(0, 0, 10.0);

        let covariance =
            FactorCovarianceMatrix::new_unchecked(vec![FactorId::new("Rates")], vec![-1.0]);
        let decomposer = ParametricDecomposer;
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Variance);

        assert!(result.is_err());
    }

    #[test]
    fn test_parametric_rejects_non_finite_unchecked_covariance() {
        let mut sensitivities =
            SensitivityMatrix::zeros(vec!["pos-A".into()], vec![FactorId::new("Rates")]);
        sensitivities.set_delta(0, 0, 10.0);

        let covariance =
            FactorCovarianceMatrix::new_unchecked(vec![FactorId::new("Rates")], vec![f64::NAN]);
        let decomposer = ParametricDecomposer;
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Variance);

        assert!(result.is_err());
    }

    #[test]
    fn test_parametric_accepts_rank_deficient_psd_covariance() -> TestResult {
        // Two perfectly collinear factors: covariance is PSD but not PD
        // (rank 1). The parametric decomposer should accept this because
        // the rank-tolerant Cholesky used for validation handles the
        // zero-eigenvalue direction cleanly.
        let mut sensitivities = SensitivityMatrix::zeros(
            vec!["pos-A".into()],
            vec![FactorId::new("Rates"), FactorId::new("Duplicate")],
        );
        sensitivities.set_delta(0, 0, 10.0);
        sensitivities.set_delta(0, 1, 10.0);

        // Rates and Duplicate are perfectly correlated with the same variance.
        let covariance = FactorCovarianceMatrix::new(
            vec![FactorId::new("Rates"), FactorId::new("Duplicate")],
            vec![0.04, 0.04, 0.04, 0.04],
        )?;
        let decomposer = ParametricDecomposer;
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Variance)?;

        // Portfolio variance = (10+10)^2 * 0.04 = 16.0.
        assert!((result.total_risk - 16.0).abs() < 1e-10);

        Ok(())
    }

    #[test]
    fn test_parametric_rejects_asymmetric_unchecked_covariance() {
        let mut sensitivities = SensitivityMatrix::zeros(
            vec!["pos-A".into()],
            vec![FactorId::new("Rates"), FactorId::new("Credit")],
        );
        sensitivities.set_delta(0, 0, 10.0);
        sensitivities.set_delta(0, 1, 5.0);

        let covariance = FactorCovarianceMatrix::new_unchecked(
            vec![FactorId::new("Rates"), FactorId::new("Credit")],
            vec![1.0, 0.25, 0.10, 1.0],
        );
        let decomposer = ParametricDecomposer;
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Variance);

        assert!(result.is_err());
    }
}
