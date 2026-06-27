//! Monte Carlo factor risk decomposition using already-weighted sensitivities.

use super::traits::RiskDecomposer;
use super::types::{FactorContribution, RiskDecomposition};
use crate::sensitivity::SensitivityMatrix;
use finstack_quant_factor_model::{FactorCovarianceMatrix, RiskMeasure};

const MATRIX_TOLERANCE: f64 = 1e-10;
const ZERO_TOLERANCE: f64 = 1e-15;

/// Cholesky decomposition returning a lower-triangular matrix `L` such that `L * L' = A`.
pub(crate) fn cholesky(data: &[f64], n: usize) -> finstack_quant_core::Result<Vec<f64>> {
    if data.len() != n * n {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Covariance storage length {} does not match matrix dimension {n}x{n}",
            data.len()
        )));
    }

    if data.iter().any(|entry| !entry.is_finite()) {
        return Err(finstack_quant_core::Error::Validation(
            "Covariance matrix entries must be finite".to_string(),
        ));
    }

    // Verify symmetry so callers need not pre-validate.
    for i in 0..n {
        for j in (i + 1)..n {
            if (data[i * n + j] - data[j * n + i]).abs() > MATRIX_TOLERANCE {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Covariance matrix must be symmetric at ({i}, {j})"
                )));
            }
        }
    }

    let mut lower = vec![0.0; n * n];

    for i in 0..n {
        for j in 0..=i {
            let mut sum = 0.0;
            for k in 0..j {
                sum += lower[i * n + k] * lower[j * n + k];
            }

            if i == j {
                let diagonal = data[i * n + i] - sum;
                if diagonal < -MATRIX_TOLERANCE {
                    return Err(finstack_quant_core::Error::Validation(
                        "Covariance matrix is not positive semi-definite".to_string(),
                    ));
                }
                lower[i * n + j] = diagonal.max(0.0).sqrt();
            } else {
                let denominator = lower[j * n + j];
                let value = data[i * n + j] - sum;
                if denominator.abs() <= MATRIX_TOLERANCE {
                    if value.abs() > MATRIX_TOLERANCE {
                        return Err(finstack_quant_core::Error::Validation(
                            "Covariance matrix is not positive semi-definite".to_string(),
                        ));
                    }
                    lower[i * n + j] = 0.0;
                } else {
                    lower[i * n + j] = value / denominator;
                }
            }
        }
    }

    Ok(lower)
}

/// Monte Carlo scenario output using scenario-major flat buffers.
///
/// The factor-pnls and factor-shocks buffers each have length
/// `n_scenarios * n_factors` and are indexed by `s * n_factors + i`. This
/// layout is cache-friendly for the per-scenario Cholesky transform (each
/// scenario writes a contiguous `[n_factors]` slice) and makes
/// `par_chunks_mut(n_factors)` a natural way to distribute work across
/// threads without any shared mutable state.
#[derive(Debug, Clone)]
struct ScenarioSet {
    portfolio_pnls: Vec<f64>,
    factor_pnls: Vec<f64>,
    factor_shocks: Vec<f64>,
    n_factors: usize,
}

#[derive(Debug, Clone, Copy)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_unit_f64(&mut self) -> f64 {
        let mantissa = self.next_u64() >> 11;
        let uniform = mantissa as f64 / ((1_u64 << 53) as f64);
        uniform.max(f64::MIN_POSITIVE)
    }
}

/// Monte Carlo decomposer for factor risk measures.
///
/// The input [`SensitivityMatrix`] is assumed to already be weighted by the upstream
/// sensitivity engine, so this decomposer aggregates exposures as simple column sums and
/// intentionally does not re-apply any position weights.
///
/// `position_factor_contributions` are left empty because a stable, scenario-based
/// per-position allocation is deferred for a later cluster.
///
/// # Sign convention
///
/// Variance and volatility are returned as non-negative numbers. VaR and ES
/// follow the P&L sign convention: **losses are reported as negative numbers**,
/// so `total_risk` and factor contributions for VaR / ES are non-positive for a
/// long-risk portfolio. Component allocations carry the same sign as
/// `total_risk`; `relative_risk` is preserved as a non-negative share because
/// numerator and denominator share the same sign.
///
/// # VaR Decomposition
///
/// VaR is decomposed using ES-prorated Euler allocation (Tasche 2008): component
/// ES values are computed first, then scaled by `VaR / ES`.  This is an
/// approximation — factor contributions may not sum exactly to total VaR under
/// distributional asymmetry.  ES decomposition is exact by construction.
///
/// # References
///
/// - `docs/REFERENCES.md#glasserman-2004-monte-carlo`
/// - `docs/REFERENCES.md#golub-van-loan-matrix-computations`
/// - `docs/REFERENCES.md#jpmorgan1996RiskMetrics`
/// - `docs/REFERENCES.md#artzner1999CoherentRisk`
#[derive(Debug, Clone, Copy)]
pub struct SimulationDecomposer {
    n_scenarios: usize,
    seed: u64,
}

impl SimulationDecomposer {
    /// Create a simulation decomposer with a fixed number of scenarios and deterministic seed.
    ///
    /// # Arguments
    ///
    /// * `n_scenarios` - Number of Monte Carlo scenarios to generate.
    /// * `seed` - Deterministic seed for reproducible scenario generation.
    ///
    /// # Returns
    ///
    /// A simulation decomposer configured for deterministic tail-risk analysis.
    #[must_use]
    pub fn new(n_scenarios: usize, seed: u64) -> Self {
        Self { n_scenarios, seed }
    }

    fn validate_inputs(
        &self,
        sensitivities: &SensitivityMatrix,
        covariance: &FactorCovarianceMatrix,
        measure: &RiskMeasure,
    ) -> finstack_quant_core::Result<()> {
        measure.validate()?;

        if self.n_scenarios < 2 {
            return Err(finstack_quant_core::Error::Validation(
                "SimulationDecomposer requires at least two scenarios".to_string(),
            ));
        }

        if let RiskMeasure::VaR { confidence } | RiskMeasure::ExpectedShortfall { confidence } =
            measure
        {
            let tail_count = ((1.0 - confidence) * self.n_scenarios as f64).ceil() as usize;
            if tail_count < 2 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "SimulationDecomposer requires at least two tail scenarios for confidence {confidence}; increase n_scenarios"
                )));
            }
            if tail_count < 30 {
                tracing::warn!(
                    tail_count,
                    n_scenarios = self.n_scenarios,
                    confidence,
                    "Tail sample size is small; ES/VaR decomposition may lack statistical reliability"
                );
            }
        }

        if sensitivities.n_factors() != covariance.n_factors() {
            return Err(finstack_quant_core::Error::Validation(
                "SensitivityMatrix and FactorCovarianceMatrix factor dimensions do not match"
                    .to_string(),
            ));
        }

        if sensitivities.factor_ids() != covariance.factor_ids() {
            return Err(finstack_quant_core::Error::Validation(
                "SensitivityMatrix and FactorCovarianceMatrix factor order does not match"
                    .to_string(),
            ));
        }

        super::parametric::validate_finite_sensitivities(sensitivities)?;

        Self::validate_covariance_storage(covariance.as_slice(), covariance.n_factors())
    }

    fn validate_covariance_storage(data: &[f64], n: usize) -> finstack_quant_core::Result<()> {
        if data.len() != n * n {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Covariance storage length {} does not match matrix dimension {n}x{n}",
                data.len()
            )));
        }

        if data.iter().any(|entry| !entry.is_finite()) {
            return Err(finstack_quant_core::Error::Validation(
                "Covariance matrix entries must be finite".to_string(),
            ));
        }

        for i in 0..n {
            for j in (i + 1)..n {
                let lhs = data[i * n + j];
                let rhs = data[j * n + i];
                if (lhs - rhs).abs() > MATRIX_TOLERANCE {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Covariance matrix must be symmetric at ({i}, {j})"
                    )));
                }
            }
        }

        let _ = cholesky(data, n)?;
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

    fn generate_standard_normals(&self, count: usize) -> Vec<f64> {
        let mut rng = SplitMix64::new(self.seed);
        let mut normals = Vec::with_capacity(count);

        while normals.len() < count {
            let u1 = rng.next_unit_f64();
            let u2 = rng.next_unit_f64();
            let radius = (-2.0 * u1.ln()).sqrt();
            let angle = 2.0 * std::f64::consts::PI * u2;

            normals.push(radius * angle.cos());
            if normals.len() < count {
                normals.push(radius * angle.sin());
            }
        }

        normals
    }

    fn generate_scenarios(
        &self,
        lower: &[f64],
        exposures: &[f64],
        n_factors: usize,
    ) -> ScenarioSet {
        // The standard-normal source is drawn serially so the RNG stream is
        // exactly the same every run. The Cholesky / P&L transform below is
        // a pure function of this `normals` buffer plus `lower` and
        // `exposures`, so distributing it across Rayon threads produces
        // bit-identical results regardless of scheduling.
        let n = self.n_scenarios;
        let normals = self.generate_standard_normals(n * n_factors);

        let mut portfolio_pnls = vec![0.0; n];
        let mut factor_pnls = vec![0.0; n * n_factors];
        let mut factor_shocks = vec![0.0; n * n_factors];

        use rayon::prelude::*;
        portfolio_pnls
            .par_iter_mut()
            .zip(factor_pnls.par_chunks_mut(n_factors))
            .zip(factor_shocks.par_chunks_mut(n_factors))
            .enumerate()
            .for_each(|(s, ((p_pnl, f_pnls), f_shocks))| {
                let z = &normals[s * n_factors..(s + 1) * n_factors];
                let mut total = 0.0;
                for i in 0..n_factors {
                    // Cholesky-triangular multiply:
                    //   shock_i = sum_{j<=i} L[i,j] * z[j]
                    let row_start = i * n_factors;
                    let mut shock = 0.0;
                    for j in 0..=i {
                        shock += lower[row_start + j] * z[j];
                    }
                    f_shocks[i] = shock;
                    let pnl_i = exposures[i] * shock;
                    f_pnls[i] = pnl_i;
                    total += pnl_i;
                }
                *p_pnl = total;
            });

        ScenarioSet {
            portfolio_pnls,
            factor_pnls,
            factor_shocks,
            n_factors,
        }
    }

    fn sample_mean(values: &[f64]) -> f64 {
        values.iter().sum::<f64>() / values.len() as f64
    }

    fn sample_covariance(lhs: &[f64], rhs: &[f64]) -> f64 {
        let lhs_mean = Self::sample_mean(lhs);
        let rhs_mean = Self::sample_mean(rhs);
        let centered_sum: f64 = lhs
            .iter()
            .zip(rhs.iter())
            .map(|(lhs_value, rhs_value)| (lhs_value - lhs_mean) * (rhs_value - rhs_mean))
            .sum();
        centered_sum / (lhs.len() - 1) as f64
    }

    /// Sample covariance of `lhs` against *every* column of a scenario-major
    /// `buffer` (a `(n_scenarios x n_factors)` matrix stored row-major as
    /// `buffer[s * n_factors + factor]`), returned as a `Vec<f64>` of length
    /// `n_factors`.
    ///
    /// This computes all factor covariances in two passes that read `buffer`
    /// *row-contiguously* (one accumulating per-column means, one accumulating
    /// centered cross-products) instead of one column-strided pass per factor.
    /// Strided access jumps `n_factors` elements per step and misses cache once
    /// `n_factors` exceeds a cache line; the contiguous layout keeps the hot
    /// `factor_pnls` / `factor_shocks` buffers streaming.
    ///
    /// Results are bit-identical to computing each column separately: for any
    /// fixed factor the accumulation order is still ascending scenario index,
    /// and the same `sum / n` (not `sum * (1/n)`) divisions are used.
    fn sample_covariances_columnwise(lhs: &[f64], buffer: &[f64], n_factors: usize) -> Vec<f64> {
        if n_factors == 0 {
            return Vec::new();
        }
        let n = lhs.len();
        let lhs_mean = Self::sample_mean(lhs);

        // Pass 1: per-column means, accumulated over contiguous rows.
        let mut col_means = vec![0.0; n_factors];
        for s in 0..n {
            let row = &buffer[s * n_factors..(s + 1) * n_factors];
            for (mean_acc, &value) in col_means.iter_mut().zip(row) {
                *mean_acc += value;
            }
        }
        for mean_acc in col_means.iter_mut() {
            *mean_acc /= n as f64;
        }

        // Pass 2: per-column centered cross-products against `lhs`.
        let mut cov = vec![0.0; n_factors];
        for s in 0..n {
            let lhs_centered = lhs[s] - lhs_mean;
            let row = &buffer[s * n_factors..(s + 1) * n_factors];
            for ((cov_acc, &value), &mean) in cov.iter_mut().zip(row).zip(col_means.iter()) {
                *cov_acc += lhs_centered * (value - mean);
            }
        }
        for cov_acc in cov.iter_mut() {
            *cov_acc /= (n - 1) as f64;
        }
        cov
    }

    /// Sum every column of a scenario-major `buffer` over the selected `rows`,
    /// reading each selected row contiguously. Returns a `Vec<f64>` of length
    /// `n_factors`.
    ///
    /// Equivalent to summing `buffer[row * n_factors + factor]` over `rows` for
    /// each factor; the per-column accumulation order (the order of `rows`) is
    /// preserved, so results are identical to the per-column computation while
    /// avoiding the column-strided gather.
    fn column_sums_over_rows(buffer: &[f64], rows: &[usize], n_factors: usize) -> Vec<f64> {
        let mut sums = vec![0.0; n_factors];
        for &row_idx in rows {
            let row = &buffer[row_idx * n_factors..(row_idx + 1) * n_factors];
            for (sum_acc, &value) in sums.iter_mut().zip(row) {
                *sum_acc += value;
            }
        }
        sums
    }

    fn build_factor_contributions(
        covariance: &FactorCovarianceMatrix,
        total_risk: f64,
        absolute: &[f64],
        marginal: &[f64],
    ) -> Vec<FactorContribution> {
        covariance
            .factor_ids()
            .iter()
            .cloned()
            .zip(absolute.iter().copied().zip(marginal.iter().copied()))
            .map(
                |(factor_id, (absolute_risk, marginal_risk))| FactorContribution {
                    factor_id,
                    absolute_risk,
                    relative_risk: if total_risk.abs() > ZERO_TOLERANCE {
                        absolute_risk / total_risk
                    } else {
                        0.0
                    },
                    marginal_risk,
                },
            )
            .collect()
    }

    fn variance_like_decomposition(
        covariance: &FactorCovarianceMatrix,
        scenarios: &ScenarioSet,
        measure: &RiskMeasure,
    ) -> RiskDecomposition {
        let variance =
            Self::sample_covariance(&scenarios.portfolio_pnls, &scenarios.portfolio_pnls).max(0.0);
        let sigma = variance.sqrt();

        let n_factors = scenarios.n_factors;
        let component_variances = Self::sample_covariances_columnwise(
            &scenarios.portfolio_pnls,
            &scenarios.factor_pnls,
            n_factors,
        );
        let marginal_component_variances = Self::sample_covariances_columnwise(
            &scenarios.portfolio_pnls,
            &scenarios.factor_shocks,
            n_factors,
        );

        let (total_risk, scale) = match measure {
            RiskMeasure::Variance => (variance, 1.0),
            RiskMeasure::Volatility => {
                if sigma > ZERO_TOLERANCE {
                    (sigma, sigma.recip())
                } else {
                    (0.0, 0.0)
                }
            }
            RiskMeasure::VaR { .. } | RiskMeasure::ExpectedShortfall { .. } => unreachable!(),
        };

        let absolute: Vec<f64> = component_variances
            .iter()
            .map(|value| value * scale)
            .collect();
        let marginal_multiplier = if matches!(measure, RiskMeasure::Variance) {
            2.0
        } else {
            1.0
        };
        let marginal: Vec<f64> = marginal_component_variances
            .iter()
            .map(|value| value * scale * marginal_multiplier)
            .collect();

        RiskDecomposition {
            total_risk,
            measure: *measure,
            factor_contributions: Self::build_factor_contributions(
                covariance, total_risk, &absolute, &marginal,
            ),
            residual_risk: 0.0,
            position_factor_contributions: Vec::new(),
            position_residual_contributions: Vec::new(),
        }
    }

    fn tail_risk_decomposition(
        &self,
        covariance: &FactorCovarianceMatrix,
        scenarios: &ScenarioSet,
        measure: &RiskMeasure,
        confidence: f64,
    ) -> RiskDecomposition {
        let mut indices: Vec<usize> = (0..self.n_scenarios).collect();
        indices.sort_by(|lhs, rhs| {
            scenarios.portfolio_pnls[*lhs].total_cmp(&scenarios.portfolio_pnls[*rhs])
        });

        let tail_count = ((1.0 - confidence) * self.n_scenarios as f64).ceil() as usize;
        let tail_count = tail_count.max(1);
        let tail_indices = &indices[..tail_count];
        let var_index = tail_indices[tail_count - 1];
        // Loss convention: VaR is the signed P&L at the alpha quantile (negative
        // for tail losses). Clamp to zero only if the quantile P&L is actually
        // a gain, which can happen for extremely low confidence levels.
        let var = scenarios.portfolio_pnls[var_index].min(0.0);
        // ES is the average tail P&L (negative for tail losses).
        let es = tail_indices
            .iter()
            .map(|index| scenarios.portfolio_pnls[*index])
            .sum::<f64>()
            / tail_count as f64;

        let n_factors = scenarios.n_factors;
        let mut component_es =
            Self::column_sums_over_rows(&scenarios.factor_pnls, tail_indices, n_factors);
        for value in component_es.iter_mut() {
            *value /= tail_count as f64;
        }
        let mut marginal_es =
            Self::column_sums_over_rows(&scenarios.factor_shocks, tail_indices, n_factors);
        for value in marginal_es.iter_mut() {
            *value /= tail_count as f64;
        }

        let (total_risk, absolute, marginal) = match measure {
            RiskMeasure::ExpectedShortfall { .. } => (es.min(0.0), component_es, marginal_es),
            RiskMeasure::VaR { .. } => {
                // Prorate negative ES contributions to negative VaR total.
                let ratio = if es.abs() > ZERO_TOLERANCE {
                    var / es
                } else {
                    0.0
                };
                (
                    var,
                    component_es.iter().map(|value| value * ratio).collect(),
                    marginal_es.iter().map(|value| value * ratio).collect(),
                )
            }
            RiskMeasure::Variance | RiskMeasure::Volatility => unreachable!(),
        };

        RiskDecomposition {
            total_risk,
            measure: *measure,
            factor_contributions: Self::build_factor_contributions(
                covariance, total_risk, &absolute, &marginal,
            ),
            residual_risk: 0.0,
            position_factor_contributions: Vec::new(),
            position_residual_contributions: Vec::new(),
        }
    }
}

impl RiskDecomposer for SimulationDecomposer {
    fn decompose(
        &self,
        sensitivities: &SensitivityMatrix,
        covariance: &FactorCovarianceMatrix,
        measure: &RiskMeasure,
    ) -> finstack_quant_core::Result<RiskDecomposition> {
        self.validate_inputs(sensitivities, covariance, measure)?;

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

        let lower = cholesky(covariance.as_slice(), covariance.n_factors())?;
        let exposures = Self::portfolio_exposures(sensitivities);
        let scenarios = self.generate_scenarios(&lower, &exposures, covariance.n_factors());

        let decomposition = match measure {
            RiskMeasure::Variance | RiskMeasure::Volatility => {
                Self::variance_like_decomposition(covariance, &scenarios, measure)
            }
            RiskMeasure::VaR { confidence } | RiskMeasure::ExpectedShortfall { confidence } => {
                self.tail_risk_decomposition(covariance, &scenarios, measure, *confidence)
            }
        };

        Ok(decomposition)
    }
}

#[cfg(test)]
mod tests {
    use super::{cholesky, SimulationDecomposer};
    use crate::factor_model::RiskDecomposer;
    use crate::sensitivity::SensitivityMatrix;
    use finstack_quant_factor_model::{FactorCovarianceMatrix, FactorId, RiskMeasure};

    type TestResult = finstack_quant_core::Result<()>;

    #[test]
    fn test_cholesky_2x2_example() -> TestResult {
        let lower = cholesky(&[4.0, 2.0, 2.0, 5.0], 2)?;

        assert!((lower[0] - 2.0).abs() < 1e-12);
        assert!(lower[1].abs() < 1e-12);
        assert!((lower[2] - 1.0).abs() < 1e-12);
        assert!((lower[3] - 2.0).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn test_simulation_expected_shortfall_converges_for_one_factor() -> TestResult {
        let mut sensitivities =
            SensitivityMatrix::zeros(vec!["pos-A".into()], vec![FactorId::new("Rates")]);
        sensitivities.set_delta(0, 0, 100.0);

        let covariance = FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04])?;
        let decomposer = SimulationDecomposer::new(100_000, 42);
        let result = decomposer.decompose(
            &sensitivities,
            &covariance,
            &RiskMeasure::ExpectedShortfall { confidence: 0.99 },
        )?;

        // ES follows the loss sign convention (negative for a long-risk book).
        assert!((result.total_risk - (-53.304_289)).abs() < 2.0);
        assert!(result.total_risk < 0.0, "ES must be negative");
        assert_eq!(result.position_factor_contributions, Vec::new());

        Ok(())
    }

    #[test]
    fn test_simulation_var_converges_for_one_factor() -> TestResult {
        let mut sensitivities =
            SensitivityMatrix::zeros(vec!["pos-A".into()], vec![FactorId::new("Rates")]);
        sensitivities.set_delta(0, 0, 100.0);

        let covariance = FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04])?;
        let decomposer = SimulationDecomposer::new(100_000, 4_242);
        let result = decomposer.decompose(
            &sensitivities,
            &covariance,
            &RiskMeasure::VaR { confidence: 0.99 },
        )?;

        // VaR follows the loss sign convention (negative for a long-risk book).
        assert!((result.total_risk - (-46.526_958)).abs() < 2.0);
        assert!(result.total_risk < 0.0, "VaR must be negative");
        assert_eq!(result.position_factor_contributions, Vec::new());

        Ok(())
    }

    #[test]
    fn test_simulation_nan_sensitivity_errors_instead_of_zero_risk() -> TestResult {
        let mut sensitivities =
            SensitivityMatrix::zeros(vec!["pos-A".into()], vec![FactorId::new("Rates")]);
        sensitivities.set_delta(0, 0, f64::NAN);

        let covariance = FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04])?;
        let decomposer = SimulationDecomposer::new(1_000, 7);
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Volatility);

        let Err(error) = result else {
            return Err(finstack_quant_core::Error::Validation(
                "NaN sensitivity must produce an error, not zero risk".to_string(),
            ));
        };
        let message = error.to_string();
        assert!(message.contains("pos-A"), "error names position: {message}");
        assert!(message.contains("Rates"), "error names factor: {message}");
        Ok(())
    }

    #[test]
    fn test_simulation_variance_uses_already_weighted_exposures() -> TestResult {
        let mut sensitivities = SensitivityMatrix::zeros(
            vec!["pos-A".into(), "pos-B".into()],
            vec![FactorId::new("Rates")],
        );
        sensitivities.set_delta(0, 0, 60.0);
        sensitivities.set_delta(1, 0, 40.0);

        let covariance = FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04])?;
        let decomposer = SimulationDecomposer::new(100_000, 99);
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Variance)?;

        assert!((result.total_risk - 400.0).abs() < 20.0);
        assert_eq!(result.factor_contributions.len(), 1);
        assert_eq!(result.position_factor_contributions, Vec::new());

        Ok(())
    }

    #[test]
    fn test_simulation_volatility_scales_sample_variance() -> TestResult {
        let mut sensitivities =
            SensitivityMatrix::zeros(vec!["pos-A".into()], vec![FactorId::new("Rates")]);
        sensitivities.set_delta(0, 0, 100.0);

        let covariance = FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04])?;
        let decomposer = SimulationDecomposer::new(100_000, 11);
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Volatility)?;

        assert!((result.total_risk - 20.0).abs() < 0.75);
        assert!((result.factor_contributions[0].absolute_risk - result.total_risk).abs() < 0.75);

        Ok(())
    }

    #[test]
    fn test_simulation_rejects_factor_order_mismatch() -> TestResult {
        let mut sensitivities = SensitivityMatrix::zeros(
            vec!["pos-A".into()],
            vec![FactorId::new("Rates"), FactorId::new("Credit")],
        );
        sensitivities.set_delta(0, 0, 10.0);
        sensitivities.set_delta(0, 1, 5.0);

        let covariance = FactorCovarianceMatrix::new(
            vec![FactorId::new("Credit"), FactorId::new("Rates")],
            vec![1.0, 0.25, 0.25, 1.0],
        )?;

        let decomposer = SimulationDecomposer::new(1_024, 7);
        let result = decomposer.decompose(&sensitivities, &covariance, &RiskMeasure::Variance);
        assert!(result.is_err());
        let Err(error) = result else {
            return Err(finstack_quant_core::Error::Validation(
                "factor order mismatch should be rejected".to_string(),
            ));
        };
        assert!(format!("{error}").contains("factor"));

        Ok(())
    }

    #[test]
    fn test_simulation_rejects_tail_risk_with_insufficient_tail_scenarios() -> TestResult {
        let mut sensitivities =
            SensitivityMatrix::zeros(vec!["pos-A".into()], vec![FactorId::new("Rates")]);
        sensitivities.set_delta(0, 0, 100.0);

        let covariance = FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04])?;
        let decomposer = SimulationDecomposer::new(50, 123);
        let result = decomposer.decompose(
            &sensitivities,
            &covariance,
            &RiskMeasure::VaR { confidence: 0.99 },
        );

        assert!(result.is_err());

        Ok(())
    }
}
