//! Fractional Brownian motion (fBM) increment generators.
//!
//! This module provides generators that transform i.i.d. standard normal samples
//! into correlated fBM increments ΔB_H\[i\] = B_H(t\_{i+1}) − B_H(tᵢ) for a
//! given Hurst exponent H ∈ (0, 1).
//!
//! Two algorithms are available:
//!
//! - [`CholeskyFbm`] — exact generation via Cholesky decomposition of the
//!   increment covariance matrix. O(n³) one-time setup, O(n²) per path.
//!   Best for validation and short grids (n ≲ 500).
//!
//! - [`HybridFbm`] — windowed conditional-Gaussian recursion. Uses exact
//!   Cholesky on the first b increments, then generates each later increment
//!   from its exact conditional distribution given the previous b increments
//!   (long-range dependence beyond the window is truncated). O(n·b) per path,
//!   enabling generation on large grids.
//!
//! Both implement [`FractionalNoiseGenerator`], which decouples the correlation
//! structure from the source of randomness: callers supply i.i.d. N(0,1) draws and
//! receive correlated fBM increments back.
//!
//! # References
//!
//! - Bennedsen, M., Lunde, A. & Pakkanen, M. S. (2017). Hybrid scheme for
//!   Brownian semistationary processes. *Finance and Stochastics*, 21(4), 931–965.
//! - Bayer, C., Friz, P. & Gatheral, J. (2016). Pricing under rough volatility.
//!   *Quantitative Finance*, 16(6), 887–904.

use finstack_quant_core::math::fractional::{
    fbm_increment_covariance, fbm_increment_covariance_matrix, HurstExponent,
};
use finstack_quant_core::{Error, Result};
use nalgebra::DMatrix;

// ---------------------------------------------------------------------------
// FractionalNoiseGenerator trait
// ---------------------------------------------------------------------------

/// Generates correlated fractional Brownian motion increments from i.i.d. normals.
///
/// Unlike [`crate::traits::RandomStream`] (which produces independent samples),
/// fBM increments are correlated across time steps. A generator encapsulates
/// the precomputed correlation structure for a specific time grid and Hurst
/// exponent.
pub trait FractionalNoiseGenerator: Send + Sync {
    /// Transform i.i.d. standard normals into correlated fractional increments.
    ///
    /// # Arguments
    ///
    /// * `normals` — input slice of
    ///   [`num_steps()`](Self::num_steps)·[`normals_per_step()`](Self::normals_per_step)
    ///   i.i.d. N(0,1) values
    /// * `out` — output slice of [`num_steps()`](Self::num_steps) correlated
    ///   fractional increments
    fn generate(&self, normals: &[f64], out: &mut [f64]);

    /// Number of time steps (length of the `out` slice).
    fn num_steps(&self) -> usize;

    /// Number of i.i.d. N(0,1) inputs consumed per time step.
    ///
    /// The input `normals` slice passed to [`generate`](Self::generate) must
    /// have length `num_steps() * normals_per_step()`. The default is 1: the
    /// Cholesky and windowed-conditional fBM generators draw exactly one
    /// normal per step. The Bennedsen-Lunde-Pakkanen hybrid scheme for the
    /// Riemann-Liouville Volterra process draws two per step (one for the
    /// driving Brownian increment, one for the exact near-field innovation),
    /// so [`RiemannLiouvilleVolterra`](super::volterra::RiemannLiouvilleVolterra)
    /// overrides this to 2.
    fn normals_per_step(&self) -> usize {
        1
    }

    /// Hurst exponent H used by this generator.
    fn hurst(&self) -> f64;
}

// ---------------------------------------------------------------------------
// CholeskyFbm
// ---------------------------------------------------------------------------

/// Exact fBM generator via Cholesky decomposition of the increment covariance matrix.
///
/// Precomputes the lower-triangular Cholesky factor L of the n × n covariance
/// matrix of fBM increments. Each path is generated as `out = L · normals`,
/// giving an O(n²) matrix–vector multiply per path.
///
/// # Complexity
///
/// - Setup: O(n³) for the Cholesky decomposition
/// - Per path: O(n²) for the triangular matrix–vector multiply
/// - Memory: O(n²) for the stored factor
///
/// Use this generator for validation or when the number of time steps is small
/// (n ≲ 500). For longer grids, consider [`HybridFbm`].
pub struct CholeskyFbm {
    /// Hurst exponent value.
    hurst_val: f64,
    /// Lower-triangular Cholesky factor of the increment covariance matrix.
    cholesky_factor: DMatrix<f64>,
    /// Number of time steps (n = len(times) − 1).
    num_steps: usize,
}

impl CholeskyFbm {
    /// Create a new Cholesky fBM generator for the given time grid and Hurst exponent.
    ///
    /// The time grid must contain at least two monotonically increasing, finite,
    /// non-negative values. The Hurst exponent must lie in (0, 1).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if:
    /// - The Hurst exponent is outside (0, 1)
    /// - The time grid has fewer than two points
    /// - The time grid is not strictly increasing
    /// - The Cholesky decomposition fails (non-positive-definite covariance)
    pub fn new(times: &[f64], hurst: f64) -> Result<Self> {
        let h = HurstExponent::new(hurst)?;
        validate_time_grid(times)?;

        let cov = fbm_increment_covariance_matrix(times, h.value());
        let chol = nalgebra::linalg::Cholesky::new(cov).ok_or_else(|| {
            Error::Validation(
                "Cholesky decomposition failed: increment covariance matrix is not \
                 positive-definite"
                    .to_string(),
            )
        })?;

        let n = times.len() - 1;
        Ok(Self {
            hurst_val: h.value(),
            cholesky_factor: chol.l(),
            num_steps: n,
        })
    }
}

impl FractionalNoiseGenerator for CholeskyFbm {
    fn generate(&self, normals: &[f64], out: &mut [f64]) {
        let n = self.num_steps;
        debug_assert_eq!(normals.len(), n);
        debug_assert_eq!(out.len(), n);

        // out = L * normals  (lower-triangular matrix–vector multiply)
        for (i, out_i) in out.iter_mut().enumerate().take(n) {
            let mut sum = 0.0;
            for (j, &z) in normals.iter().enumerate().take(i + 1) {
                sum += self.cholesky_factor[(i, j)] * z;
            }
            *out_i = sum;
        }
    }

    fn num_steps(&self) -> usize {
        self.num_steps
    }

    fn hurst(&self) -> f64 {
        self.hurst_val
    }
}

// ---------------------------------------------------------------------------
// HybridFbm
// ---------------------------------------------------------------------------

/// Configuration for the hybrid fBM generator.
#[derive(Debug, Clone, Default)]
pub struct HybridFbmConfig {
    /// Near-field window size b. When `None`, an automatic default is chosen
    /// based on the grid size: `min(max(10, √n), 50)`.
    pub near_field_size: Option<usize>,
}

/// Windowed conditional-Gaussian fBM generator.
///
/// Generates each increment beyond the exact start-up window from its
/// conditional Gaussian distribution given the previous b increments,
/// truncating long-range dependence beyond the window. (Despite the legacy
/// name, this is *not* the Bennedsen-Lunde-Pakkanen hybrid scheme — that
/// scheme generates the Riemann-Liouville Volterra process from driving
/// Brownian increments and lives in
/// [`RiemannLiouvilleVolterra`](super::volterra::RiemannLiouvilleVolterra).)
///
/// # Algorithm
///
/// For the first b steps, generation is exact (identical to [`CholeskyFbm`]
/// on a b × b sub-grid). For each subsequent step i ≥ b, increment i is drawn
/// from its exact conditional distribution given the previous b increments:
/// a conditional-mean weight vector (solved from the true increment
/// covariance) applied to the window, plus an innovation with the matching
/// residual standard deviation. Dependence on increments older than the
/// window is truncated; the truncation error decays with the window size and
/// with H → 0.5 (where increments become independent).
///
/// This yields O(n·b) per-path cost instead of O(n²). The marginal variance
/// of every increment is preserved up to the window-truncation drift, and
/// covariances at lags ≤ b are matched against the achieved covariance of
/// the generated window.
///
/// # Complexity
///
/// - Setup: O(b³ + n·b)
/// - Per path: O(n·b)
/// - Memory: O(b² + n·b)
pub struct HybridFbm {
    /// Hurst exponent value.
    hurst_val: f64,
    /// Number of time steps (n).
    num_steps: usize,
    /// Near-field window size (b).
    near_field_size: usize,
    /// Lower-triangular Cholesky factor for the first b increments (b × b).
    near_cholesky: DMatrix<f64>,
    /// For each step i in b..n, the conditional-mean weights on the preceding
    /// b increments and the residual standard deviation.
    /// `cond_weights[i - b]` = (`Vec<f64>` of length b, residual_std: `f64`).
    cond_weights: Vec<(Vec<f64>, f64)>,
}

impl HybridFbm {
    /// Create a new hybrid fBM generator.
    ///
    /// # Arguments
    ///
    /// * `times` — time grid with at least two strictly increasing non-negative values
    /// * `hurst` — Hurst exponent in (0, 1)
    /// * `config` — optional configuration (near-field window size)
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the inputs are invalid or the near-field
    /// Cholesky decomposition fails.
    pub fn new(times: &[f64], hurst: f64, config: HybridFbmConfig) -> Result<Self> {
        let h = HurstExponent::new(hurst)?;
        validate_time_grid(times)?;

        let n = times.len() - 1;
        let b = config
            .near_field_size
            .unwrap_or_else(|| auto_near_field_size(n))
            .min(n);

        if b == 0 {
            return Err(Error::Validation(
                "near-field window size must be at least 1".to_string(),
            ));
        }

        // Build the near-field Cholesky factor (first b increments).
        let near_times = &times[..=b];
        let near_cov = fbm_increment_covariance_matrix(near_times, h.value());
        let near_chol = nalgebra::linalg::Cholesky::new(near_cov).ok_or_else(|| {
            Error::Validation(
                "Near-field Cholesky decomposition failed: covariance not positive-definite"
                    .to_string(),
            )
        })?;
        let near_cholesky = near_chol.l();

        // Build conditional-mean weights for steps b..n. Each later increment
        // is generated from its conditional Gaussian distribution given the
        // previous b increments; dependence beyond the window is truncated.
        let mut cond_weights = Vec::with_capacity(n.saturating_sub(b));

        for i in b..n {
            // Covariance of increment i with the previous b increments.
            let mut cov_vec = vec![0.0; b];
            for (k, cov_k) in cov_vec.iter_mut().enumerate() {
                let j = i - b + k;
                *cov_k = fbm_increment_covariance(
                    times[j],
                    times[j + 1],
                    times[i],
                    times[i + 1],
                    h.value(),
                );
            }

            // Covariance matrix of the b preceding increments.
            let prev_times = &times[i - b..=i];
            let prev_cov = fbm_increment_covariance_matrix(prev_times, h.value());

            // Solve prev_cov * w = cov_vec for the conditional mean weights.
            let prev_chol = nalgebra::linalg::Cholesky::new(prev_cov).ok_or_else(|| {
                Error::Validation(format!(
                    "Cholesky decomposition failed for conditional block at step {i}"
                ))
            })?;
            let cov_dvec = nalgebra::DVector::from_column_slice(&cov_vec);
            let w = prev_chol.solve(&cov_dvec);

            // Conditional variance = var(increment i) - cov_vec . w
            let var_i =
                fbm_increment_covariance(times[i], times[i + 1], times[i], times[i + 1], h.value());
            let explained: f64 = cov_vec.iter().zip(w.iter()).map(|(c, wi)| c * wi).sum();
            let raw_cond_var = var_i - explained;
            // Conditional variance must be non-negative in exact arithmetic;
            // small negative values arise from floating-point roundoff and are
            // clamped to zero. Larger negatives indicate the conditional-block
            // covariance solve is numerically unstable (typically a poorly
            // conditioned near-field block) and would silently zero the
            // innovation term, so we surface them via tracing.
            let abs_var_i = var_i.abs();
            if raw_cond_var < -1e-9 * abs_var_i.max(1.0) {
                tracing::warn!(
                    step = i,
                    var_i,
                    explained,
                    raw_cond_var,
                    "HybridFbm: conditional variance clamped to zero from a significantly negative \
                     value; near-field Cholesky / linear-solve may be ill-conditioned and the \
                     innovation term has been suppressed for this step"
                );
            }
            let cond_var = raw_cond_var.max(0.0);
            let cond_std = cond_var.sqrt();

            cond_weights.push((w.as_slice().to_vec(), cond_std));
        }

        Ok(Self {
            hurst_val: h.value(),
            num_steps: n,
            near_field_size: b,
            near_cholesky,
            cond_weights,
        })
    }
}

impl FractionalNoiseGenerator for HybridFbm {
    fn generate(&self, normals: &[f64], out: &mut [f64]) {
        let n = self.num_steps;
        let b = self.near_field_size;
        debug_assert_eq!(normals.len(), n);
        debug_assert_eq!(out.len(), n);

        // Phase 1: exact Cholesky for the first b increments.
        for (i, out_i) in out.iter_mut().enumerate().take(b.min(n)) {
            let mut sum = 0.0;
            for (j, &z) in normals.iter().enumerate().take(i + 1) {
                sum += self.near_cholesky[(i, j)] * z;
            }
            *out_i = sum;
        }

        // Phase 2: windowed conditional recursion for steps b..n.
        for i in b..n {
            let idx = i - b;
            let (ref w, cond_std) = self.cond_weights[idx];

            // Conditional mean from the previous b increments. Dependence on
            // increments older than the window is truncated — adding an extra
            // "far-field" term on top of the conditional mean would double
            // count it (the window increments already carry that
            // information) and break the increment variance.
            let mut val = 0.0;
            for k in 0..b {
                val += w[k] * out[i - b + k];
            }

            // Innovation: residual std * independent normal.
            val += cond_std * normals[i];

            out[i] = val;
        }
    }

    fn num_steps(&self) -> usize {
        self.num_steps
    }

    fn hurst(&self) -> f64 {
        self.hurst_val
    }
}

// ---------------------------------------------------------------------------
// Canonical auto-selecting factory
// ---------------------------------------------------------------------------

/// Number of steps below which the auto factory uses exact Cholesky generation.
pub const FBM_AUTO_CHOLESKY_MAX_STEPS: usize = 199;

/// Create a fBM generator for the given time grid and Hurst exponent.
///
/// Uses [`CholeskyFbm`] for grids with at most
/// [`FBM_AUTO_CHOLESKY_MAX_STEPS`] steps and [`HybridFbm`] otherwise. Use the
/// concrete constructors directly when an explicit algorithm is required.
///
/// # Arguments
///
/// * `times` - Strictly increasing, finite, non-negative time grid in years;
///   at least two nodes are required.
/// * `hurst` - Fractional-Brownian-motion Hurst exponent in `(0, 1)` that
///   controls path roughness and increment dependence.
///
/// # Errors
///
/// Returns [`Error::Validation`] if the time grid or Hurst exponent is invalid.
pub fn create_fbm_generator(
    times: &[f64],
    hurst: f64,
) -> Result<Box<dyn FractionalNoiseGenerator>> {
    let n = if times.len() >= 2 { times.len() - 1 } else { 0 };

    if n <= FBM_AUTO_CHOLESKY_MAX_STEPS {
        Ok(Box::new(CholeskyFbm::new(times, hurst)?))
    } else {
        Ok(Box::new(HybridFbm::new(
            times,
            hurst,
            HybridFbmConfig::default(),
        )?))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Validate that a time grid is strictly increasing with at least two finite
/// non-negative values.
fn validate_time_grid(times: &[f64]) -> Result<()> {
    if times.len() < 2 {
        return Err(Error::Validation(
            "time grid must have at least 2 points".to_string(),
        ));
    }
    if !times.iter().all(|t| t.is_finite() && *t >= 0.0) {
        return Err(Error::Validation(
            "all time grid values must be finite and non-negative".to_string(),
        ));
    }
    for w in times.windows(2) {
        if w[1] <= w[0] {
            return Err(Error::Validation(format!(
                "time grid must be strictly increasing, but found {} >= {}",
                w[0], w[1]
            )));
        }
    }
    Ok(())
}

/// Default near-field window size: min(max(10, √n), 50).
fn auto_near_field_size(n: usize) -> usize {
    let sqrt_n = (n as f64).sqrt() as usize;
    sqrt_n.clamp(10, 50)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a uniform time grid from 0 to T with n steps.
    fn uniform_grid(t_end: f64, n: usize) -> Vec<f64> {
        let dt = t_end / n as f64;
        (0..=n).map(|i| i as f64 * dt).collect()
    }

    // -- CholeskyFbm -------------------------------------------------------

    #[test]
    fn cholesky_output_dimensions() {
        let times = uniform_grid(1.0, 10);
        let gen = CholeskyFbm::new(&times, 0.3).unwrap();
        assert_eq!(gen.num_steps(), 10);
        assert!((gen.hurst() - 0.3).abs() < 1e-14);

        let normals = vec![1.0; 10];
        let mut out = vec![0.0; 10];
        gen.generate(&normals, &mut out);

        // All outputs should be finite
        assert!(out.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn cholesky_h_half_independent_increments() {
        // H = 0.5 means standard Brownian motion: increment covariance is diagonal.
        let n = 20;
        let times = uniform_grid(1.0, n);
        let gen = CholeskyFbm::new(&times, 0.5).unwrap();

        // When H = 0.5, L should be approximately diagonal (identity times sqrt(dt)).
        // A unit vector e_3 should produce output nonzero only in position 3.
        for k in 0..n {
            let mut normals = vec![0.0; n];
            normals[k] = 1.0;
            let mut out = vec![0.0; n];
            gen.generate(&normals, &mut out);

            for (i, val) in out.iter().enumerate() {
                if i == k {
                    // Should be L[k,k] ≈ sqrt(dt) = sqrt(1/20)
                    assert!(val.abs() > 1e-10, "diagonal entry should be nonzero");
                } else {
                    assert!(
                        val.abs() < 1e-10,
                        "off-diagonal entry [{i}] should be zero for H=0.5, got {val}"
                    );
                }
            }
        }
    }

    #[test]
    fn cholesky_rejects_bad_hurst() {
        let times = uniform_grid(1.0, 5);
        assert!(CholeskyFbm::new(&times, 0.0).is_err());
        assert!(CholeskyFbm::new(&times, 1.0).is_err());
        assert!(CholeskyFbm::new(&times, -0.1).is_err());
        assert!(CholeskyFbm::new(&times, f64::NAN).is_err());
    }

    #[test]
    fn cholesky_rejects_short_grid() {
        assert!(CholeskyFbm::new(&[0.0], 0.3).is_err());
        assert!(CholeskyFbm::new(&[], 0.3).is_err());
    }

    #[test]
    fn cholesky_rejects_non_increasing_grid() {
        assert!(CholeskyFbm::new(&[0.0, 0.5, 0.3], 0.3).is_err());
        assert!(CholeskyFbm::new(&[0.0, 0.5, 0.5], 0.3).is_err());
    }

    // -- HybridFbm ---------------------------------------------------------

    #[test]
    fn hybrid_matches_cholesky_short_path() {
        // For a short path where near-field covers everything, hybrid should
        // produce identical results to Cholesky.
        let n = 8;
        let times = uniform_grid(1.0, n);
        let h = 0.3;

        let chol = CholeskyFbm::new(&times, h).unwrap();
        let hybrid = HybridFbm::new(
            &times,
            h,
            HybridFbmConfig {
                near_field_size: Some(n),
            },
        )
        .unwrap();

        let normals: Vec<f64> = (0..n).map(|i| 0.1 * (i as f64 + 1.0)).collect();
        let mut out_chol = vec![0.0; n];
        let mut out_hyb = vec![0.0; n];

        chol.generate(&normals, &mut out_chol);
        hybrid.generate(&normals, &mut out_hyb);

        for (i, (&a, &b)) in out_chol.iter().zip(out_hyb.iter()).enumerate() {
            assert!(
                (a - b).abs() < 1e-10,
                "step {i}: cholesky={a}, hybrid={b}, diff={}",
                (a - b).abs()
            );
        }
    }

    #[test]
    fn hybrid_output_finite() {
        let n = 50;
        let times = uniform_grid(1.0, n);
        let gen = HybridFbm::new(&times, 0.1, HybridFbmConfig::default()).unwrap();
        assert_eq!(gen.num_steps(), n);

        let normals: Vec<f64> = (0..n).map(|i| (-1.0_f64).powi(i as i32) * 0.5).collect();
        let mut out = vec![0.0; n];
        gen.generate(&normals, &mut out);

        assert!(out.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn hybrid_approximate_agreement_with_cholesky() {
        // For a grid longer than the near-field window, the hybrid scheme
        // should agree with Cholesky approximately (not exactly, since the
        // far-field is an approximation).
        let n = 30;
        let times = uniform_grid(1.0, n);
        let h = 0.2;

        let chol = CholeskyFbm::new(&times, h).unwrap();
        let hybrid = HybridFbm::new(
            &times,
            h,
            HybridFbmConfig {
                near_field_size: Some(10),
            },
        )
        .unwrap();

        let normals: Vec<f64> = (0..n).map(|i| 0.3 * (i as f64 - 15.0) / 15.0).collect();
        let mut out_chol = vec![0.0; n];
        let mut out_hyb = vec![0.0; n];

        chol.generate(&normals, &mut out_chol);
        hybrid.generate(&normals, &mut out_hyb);

        // First 10 steps should match exactly (both use Cholesky).
        for i in 0..10 {
            assert!(
                (out_chol[i] - out_hyb[i]).abs() < 1e-10,
                "near-field step {i} should match exactly"
            );
        }

        // Later steps should be in the same ballpark (rough tolerance).
        let max_diff: f64 = out_chol
            .iter()
            .zip(out_hyb.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f64, f64::max);
        assert!(
            max_diff < 0.5,
            "hybrid should approximate cholesky, max_diff={max_diff}"
        );
    }

    #[test]
    fn hybrid_covariance_matches_exact_beyond_near_field() {
        // Reconstruct the linear map L implied by the generator by feeding
        // unit vectors, then compare Cov = L·Lᵀ against the exact fBM
        // increment covariance. The removed far-field construction inflated
        // the variance of increments beyond the window by O(1) factors
        // (prior fix), which this test would have caught.
        for &h in &[0.1, 0.3, 0.7] {
            let n = 60;
            let b = 20;
            let times = uniform_grid(1.0, n);
            let gen = HybridFbm::new(
                &times,
                h,
                HybridFbmConfig {
                    near_field_size: Some(b),
                },
            )
            .unwrap();

            // Column k of L is generate(e_k); the map is lower-triangular.
            let mut l = vec![vec![0.0_f64; n]; n];
            for k in 0..n {
                let mut normals = vec![0.0; n];
                normals[k] = 1.0;
                let mut out = vec![0.0; n];
                gen.generate(&normals, &mut out);
                for (i, &v) in out.iter().enumerate() {
                    l[i][k] = v;
                }
            }

            let exact = fbm_increment_covariance_matrix(&times, h);
            let mut max_rel_var: f64 = 0.0;
            let mut max_norm_cov: f64 = 0.0;
            for i in 0..n {
                for j in 0..=i {
                    let cov_ij: f64 = (0..n).map(|k| l[i][k] * l[j][k]).sum();
                    let scale = (exact[(i, i)] * exact[(j, j)]).sqrt();
                    if i == j {
                        let rel = (cov_ij - exact[(i, i)]).abs() / exact[(i, i)];
                        max_rel_var = max_rel_var.max(rel);
                    } else {
                        let norm = (cov_ij - exact[(i, j)]).abs() / scale;
                        max_norm_cov = max_norm_cov.max(norm);
                    }
                }
            }

            // Variances must be preserved tightly; covariances at all lags
            // must be close in correlation units. The window truncation
            // drops correlations at lags > b, which decay like
            // H(2H−1)·lag^{2H−2}, so the persistent-memory case (H > 0.5)
            // gets a looser bound.
            let cov_tol = if h > 0.5 { 0.08 } else { 0.03 };
            assert!(
                max_rel_var < 0.02,
                "H={h}: max relative variance error {max_rel_var} >= 2%"
            );
            assert!(
                max_norm_cov < cov_tol,
                "H={h}: max normalized covariance error {max_norm_cov} >= {cov_tol}"
            );
        }
    }

    // -- Factory -----------------------------------------------------------

    #[test]
    fn factory_auto_selects_cholesky_for_small_grid() {
        let times = uniform_grid(1.0, 50);
        let gen = create_fbm_generator(&times, 0.3).unwrap();
        assert_eq!(gen.num_steps(), 50);
    }

    #[test]
    fn factory_auto_selects_hybrid_for_large_grid() {
        let times = uniform_grid(1.0, 300);
        let gen = create_fbm_generator(&times, 0.1).unwrap();
        assert_eq!(gen.num_steps(), 300);
    }

    #[test]
    fn explicit_cholesky_constructor() {
        let times = uniform_grid(1.0, 20);
        let gen = CholeskyFbm::new(&times, 0.4).unwrap();
        assert_eq!(gen.num_steps(), 20);
        assert!((gen.hurst() - 0.4).abs() < 1e-14);
    }

    #[test]
    fn explicit_hybrid_constructor() {
        let times = uniform_grid(1.0, 100);
        let gen = HybridFbm::new(
            &times,
            0.15,
            HybridFbmConfig {
                near_field_size: Some(15),
            },
        )
        .unwrap();
        assert_eq!(gen.num_steps(), 100);
    }
}
