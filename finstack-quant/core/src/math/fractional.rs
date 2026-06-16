//! Fractional Brownian motion (fBM) primitives and kernel functions.
//!
//! This module provides the mathematical building blocks for rough volatility
//! models, including:
//!
//! - **Hurst exponent** — validated parameter H ∈ (0, 1) controlling path roughness
//! - **Fractional kernels** — Riemann–Liouville power-law Volterra kernel
//! - **Covariance functions** — fBM covariance, variance, and covariance matrices for
//!   both fBM values and increments
//! - **Mittag-Leffler function** — generalized E_{α,β}(z) used in fractional calculus
//!   and the rough Heston characteristic function
//!
//! # Background
//!
//! Fractional Brownian motion B_H is a centered Gaussian process with covariance
//!
//! $$\operatorname{Cov}(B_H(t), B_H(s)) = \tfrac{1}{2}\bigl(|t|^{2H} + |s|^{2H} - |t-s|^{2H}\bigr)$$
//!
//! where H ∈ (0, 1) is the Hurst exponent. When H = 0.5 this reduces to standard
//! Brownian motion. When H < 0.5 the paths are rougher than Brownian motion, which
//! is the empirically observed regime for equity volatility.
//!
//! # References
//!
//! - Mandelbrot, B. & Van Ness, J. (1968). Fractional Brownian motions, fractional
//!   noises and applications. *SIAM Review*, 10(4), 422–437.
//! - Bayer, C., Friz, P. & Gatheral, J. (2016). Pricing under rough volatility.
//!   *Quantitative Finance*, 16(6), 887–904.
//! - El Euch, O. & Rosenbaum, M. (2019). The characteristic function of rough Heston
//!   models. *Mathematical Finance*, 29(1), 3–38.
//! - Gorenflo, R., Loutchko, J. & Luchko, Yu. (2002). Computation of the Mittag-Leffler
//!   function E_{α,β}(z) and its derivative. *Fractional Calculus and Applied Analysis*,
//!   5(4), 491–518.

use nalgebra::DMatrix;
use num_complex::Complex64;
use statrs::function::gamma::ln_gamma;

use crate::{Error, Result};

// ---------------------------------------------------------------------------
// HurstExponent
// ---------------------------------------------------------------------------

/// Validated Hurst exponent H ∈ (0, 1).
///
/// The Hurst exponent determines the roughness of fractional Brownian motion:
///
/// - H < 0.5 — rough (anti-persistent increments)
/// - H = 0.5 — standard Brownian motion
/// - H > 0.5 — smooth (persistent increments)
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HurstExponent {
    /// The Hurst parameter value.
    h: f64,
}

impl HurstExponent {
    /// Create a new Hurst exponent, validating that H ∈ (0, 1) and is finite.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `h` is not in the open interval (0, 1)
    /// or is not finite.
    pub fn new(h: f64) -> Result<Self> {
        if !h.is_finite() || h <= 0.0 || h >= 1.0 {
            return Err(Error::Validation(format!(
                "Hurst exponent must be in (0, 1), got {h}"
            )));
        }
        Ok(Self { h })
    }

    /// The raw Hurst parameter value.
    pub fn value(&self) -> f64 {
        self.h
    }

    /// The fractional index α = H + 0.5 used in Volterra-type representations.
    pub fn alpha(&self) -> f64 {
        self.h + 0.5
    }

    /// Returns `true` when the exponent describes a rough process (H < 0.5).
    pub fn is_rough(&self) -> bool {
        self.h < 0.5
    }
}

// ---------------------------------------------------------------------------
// RiemannLiouvilleKernel
// ---------------------------------------------------------------------------

/// Riemann–Liouville power-law kernel.
///
/// $$K(t,s) = c_H (t - s)^{H - 1/2}, \quad s < t$$
///
/// where c_H = √(2H). Returns 0 when t ≤ s.
///
/// This is the Riemann–Liouville (type II) Volterra kernel. The resulting
/// process matches the fBM *variance* `t^{2H}` but not the full fBM
/// covariance structure — the true Molchan–Golosov kernel additionally
/// involves an incomplete-beta/hypergeometric correction term. Rough
/// volatility models that require the Riemann-Liouville Volterra process
/// (e.g. rough Bergomi) use the dedicated
/// `finstack_quant_monte_carlo::rng::volterra` generator rather than this kernel.
#[derive(Debug, Clone, Copy)]
pub struct RiemannLiouvilleKernel {
    /// The Hurst exponent.
    hurst_exp: HurstExponent,
    /// Precomputed normalisation constant c_H = √(2H).
    c_h: f64,
}

impl RiemannLiouvilleKernel {
    /// Create a new Riemann–Liouville kernel for the given Hurst exponent.
    pub fn new(h: HurstExponent) -> Self {
        let c_h = (2.0 * h.value()).sqrt();
        Self { hurst_exp: h, c_h }
    }

    /// Evaluate the kernel K(t, s).
    pub fn evaluate(&self, t: f64, s: f64) -> f64 {
        if t <= s {
            return 0.0;
        }
        self.c_h * (t - s).powf(self.hurst_exp.value() - 0.5)
    }

    /// The Hurst exponent associated with this kernel.
    pub fn hurst(&self) -> HurstExponent {
        self.hurst_exp
    }
}

// ---------------------------------------------------------------------------
// Covariance utilities
// ---------------------------------------------------------------------------

/// Covariance of fractional Brownian motion.
///
/// $$\operatorname{Cov}(B_H(t), B_H(s)) = \tfrac{1}{2}\bigl(|t|^{2H} + |s|^{2H} - |t-s|^{2H}\bigr)$$
pub fn fbm_covariance(t: f64, s: f64, h: f64) -> f64 {
    let two_h = 2.0 * h;
    0.5 * (t.abs().powf(two_h) + s.abs().powf(two_h) - (t - s).abs().powf(two_h))
}

/// Variance of fractional Brownian motion at time t.
///
/// $$\operatorname{Var}(B_H(t)) = |t|^{2H}$$
pub fn fbm_variance(t: f64, h: f64) -> f64 {
    t.abs().powf(2.0 * h)
}

/// Covariance of fBM increments on arbitrary intervals.
///
/// $$\operatorname{Cov}\bigl(B_H(t_{i+1}) - B_H(t_i),\; B_H(t_{j+1}) - B_H(t_j)\bigr)$$
///
/// computed via the bilinearity relation on the fBM covariance function.
pub fn fbm_increment_covariance(ti: f64, ti1: f64, tj: f64, tj1: f64, h: f64) -> f64 {
    fbm_covariance(ti1, tj1, h) - fbm_covariance(ti1, tj, h) - fbm_covariance(ti, tj1, h)
        + fbm_covariance(ti, tj, h)
}

/// Full n × n covariance matrix of fBM at times t₁, …, tₙ.
///
/// Entry (i, j) = Cov(B_H(tᵢ), B_H(tⱼ)).
pub fn fbm_covariance_matrix(times: &[f64], h: f64) -> DMatrix<f64> {
    let n = times.len();
    DMatrix::from_fn(n, n, |i, j| fbm_covariance(times[i], times[j], h))
}

/// Covariance matrix of fBM increments on a time grid.
///
/// Given times t₀, t₁, …, tₙ the matrix is (n) × (n) with entry
/// (i, j) = Cov(B_H(t_{i+1}) − B_H(tᵢ), B_H(t_{j+1}) − B_H(tⱼ)).
///
/// Requires at least two time points. Returns an empty 0 × 0 matrix
/// when fewer than two points are supplied.
pub fn fbm_increment_covariance_matrix(times: &[f64], h: f64) -> DMatrix<f64> {
    if times.len() < 2 {
        return DMatrix::zeros(0, 0);
    }
    let n = times.len() - 1;
    DMatrix::from_fn(n, n, |i, j| {
        fbm_increment_covariance(times[i], times[i + 1], times[j], times[j + 1], h)
    })
}

// ---------------------------------------------------------------------------
// Mittag-Leffler function
// ---------------------------------------------------------------------------

/// Maximum number of series terms for Mittag-Leffler evaluation.
const ML_MAX_TERMS: usize = 200;

/// Relative convergence tolerance for Mittag-Leffler series.
const ML_TOL: f64 = 1e-15;

/// Series-validity bound on `|z|` for the Taylor evaluation when `α < 1`.
///
/// The series terms `|z|^k / Γ(αk + β)` grow before the Gamma function takes
/// over, peaking near `k* ≈ |z|^{1/α}/α` with peak magnitude roughly
/// `exp(α⁻¹·|z|^{1/α})`. For negative real `z` (the rough-Heston regime) the
/// result is O(1/|z|) while intermediate terms reach that peak, so roughly
/// `|z|^{1/α}/(α·ln 10)` decimal digits are lost to cancellation. For `α < 1`
/// the exponent `1/α > 1` makes this loss explode quickly; `|z| = 10` is a
/// conservative cutoff keeping the evaluation meaningful in `f64`. Larger
/// arguments require the Gorenflo–Loutchko–Luchko integral representation
/// (not implemented here).
const ML_SERIES_RADIUS_ALPHA_LT_1: f64 = 10.0;

/// Maximum tolerated absolute rounding-error estimate for the series sum.
///
/// The absolute error of the summation is bounded by roughly
/// `max_k |term_k| × ε` (machine epsilon); evaluations whose estimated
/// absolute error exceeds this cap are rejected as numerically meaningless.
const ML_MAX_ABS_ERROR: f64 = 1e-8;

/// Generalized Mittag-Leffler function E_{α,β}(z).
///
/// $$E_{\alpha,\beta}(z) = \sum_{k=0}^{\infty} \frac{z^k}{\Gamma(\alpha k + \beta)}$$
///
/// Computed via direct series summation using `ln_gamma` to avoid overflow
/// in the Gamma function. The series is truncated after an internal maximum
/// term count or when the relative contribution falls below an internal
/// tolerance (both defined as private constants in this module).
///
/// # Domain of validity
///
/// The raw Taylor series suffers catastrophic cancellation for moderately
/// large `|z|` with negative real part: intermediate terms peak near
/// `exp(α⁻¹·|z|^{1/α})` while the result is O(1/|z|). For `α < 1` (the rough
/// volatility regime) arguments with `|z| > 10` are rejected outright; for
/// any parameters, an evaluation whose estimated absolute rounding error
/// (peak term × machine ε) exceeds an internal cap (1e-8) is rejected.
/// Evaluating larger arguments requires the
/// integral-representation algorithm of Gorenflo, Loutchko & Luchko (2002),
/// which is not implemented here.
///
/// # Arguments
///
/// * `z` — complex argument
/// * `alpha` — first Mittag-Leffler parameter (must be > 0 and finite)
/// * `beta` — second Mittag-Leffler parameter (must be > 0 and finite)
///
/// # Errors
///
/// Returns [`Error::Validation`] when:
/// - `alpha` or `beta` is non-positive or non-finite, or `z` is non-finite
/// - `alpha < 1` and `|z|` exceeds the series-validity bound (10)
/// - the summation loses too many digits to cancellation to be meaningful
///
/// # References
///
/// - Gorenflo, R., Loutchko, J. & Luchko, Yu. (2002). Computation of the
///   Mittag-Leffler function E_{α,β}(z) and its derivative. *Fractional
///   Calculus and Applied Analysis*, 5(4), 491–518.
pub fn mittag_leffler(z: Complex64, alpha: f64, beta: f64) -> Result<Complex64> {
    if !alpha.is_finite() || alpha <= 0.0 || !beta.is_finite() || beta <= 0.0 {
        return Err(Error::Validation(format!(
            "Mittag-Leffler parameters must be finite and positive, got alpha={alpha}, beta={beta}"
        )));
    }
    if !z.re.is_finite() || !z.im.is_finite() {
        return Err(Error::Validation(format!(
            "Mittag-Leffler argument must be finite, got z={z}"
        )));
    }
    if alpha < 1.0 && z.norm() > ML_SERIES_RADIUS_ALPHA_LT_1 {
        return Err(Error::Validation(format!(
            "Mittag-Leffler Taylor series is unreliable for alpha < 1 with |z| = {:.3e} > {}; \
             the integral representation (Gorenflo et al. 2002) is required for this regime",
            z.norm(),
            ML_SERIES_RADIUS_ALPHA_LT_1
        )));
    }

    let mut sum = Complex64::new(0.0, 0.0);
    let mut z_pow = Complex64::new(1.0, 0.0); // z^0 = 1
    let mut max_term_norm = 0.0_f64;

    for k in 0..ML_MAX_TERMS {
        let log_gamma = ln_gamma(alpha * k as f64 + beta);
        let inv_gamma = (-log_gamma).exp();
        let term = z_pow * inv_gamma;
        max_term_norm = max_term_norm.max(term.norm());

        sum += term;

        // Convergence check on absolute contribution relative to running sum
        let sum_norm = sum.norm();
        if sum_norm > 0.0 && term.norm() / sum_norm < ML_TOL {
            break;
        }

        z_pow *= z;
    }

    // Catastrophic-cancellation guard: the absolute rounding error of the
    // summation is ~ max_k |term_k| × ε. If intermediate terms were so large
    // that this estimate exceeds the cap, the digits of the result are noise.
    let abs_error_estimate = max_term_norm * f64::EPSILON;
    if abs_error_estimate > ML_MAX_ABS_ERROR {
        return Err(Error::Validation(format!(
            "Mittag-Leffler series lost too many digits to cancellation for z={z}, \
             alpha={alpha}, beta={beta} (peak term {max_term_norm:.3e}, estimated \
             absolute error {abs_error_estimate:.3e})"
        )));
    }

    Ok(sum)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const TOL: f64 = 1e-10;

    // -- HurstExponent validation ------------------------------------------

    #[test]
    fn hurst_valid() {
        let h = HurstExponent::new(0.1).unwrap();
        assert!((h.value() - 0.1).abs() < TOL);
        assert!((h.alpha() - 0.6).abs() < TOL);
        assert!(h.is_rough());
    }

    #[test]
    fn hurst_half() {
        let h = HurstExponent::new(0.5).unwrap();
        assert!(!h.is_rough());
    }

    #[test]
    fn hurst_reject_zero() {
        assert!(HurstExponent::new(0.0).is_err());
    }

    #[test]
    fn hurst_reject_one() {
        assert!(HurstExponent::new(1.0).is_err());
    }

    #[test]
    fn hurst_reject_negative() {
        assert!(HurstExponent::new(-0.3).is_err());
    }

    #[test]
    fn hurst_reject_nan() {
        assert!(HurstExponent::new(f64::NAN).is_err());
    }

    #[test]
    fn hurst_reject_infinity() {
        assert!(HurstExponent::new(f64::INFINITY).is_err());
    }

    // -- fBM covariance ----------------------------------------------------

    #[test]
    fn fbm_cov_h_half_is_min() {
        // When H = 0.5, Cov(B(t), B(s)) = min(s, t) for s, t >= 0
        let h = 0.5;
        for &(t, s) in &[(1.0, 2.0), (3.0, 1.5), (0.5, 0.5)] {
            let cov = fbm_covariance(t, s, h);
            let expected = t.min(s);
            assert!(
                (cov - expected).abs() < TOL,
                "Cov({t},{s}) = {cov}, expected {expected}"
            );
        }
    }

    #[test]
    fn fbm_variance_matches() {
        let h = 0.3;
        let t = 2.5;
        let var = fbm_variance(t, h);
        let expected = t.powf(2.0 * h);
        assert!((var - expected).abs() < TOL);
    }

    #[test]
    fn fbm_variance_equals_self_covariance() {
        let h = 0.7;
        let t = 1.5;
        let var = fbm_variance(t, h);
        let cov = fbm_covariance(t, t, h);
        assert!((var - cov).abs() < TOL);
    }

    // -- Covariance matrix -------------------------------------------------

    #[test]
    fn covariance_matrix_symmetric() {
        let times = vec![0.1, 0.3, 0.5, 1.0];
        let h = 0.3;
        let cov = fbm_covariance_matrix(&times, h);
        for i in 0..cov.nrows() {
            for j in 0..cov.ncols() {
                assert!(
                    (cov[(i, j)] - cov[(j, i)]).abs() < TOL,
                    "Asymmetry at ({i},{j})"
                );
            }
        }
    }

    #[test]
    fn covariance_matrix_diagonal_is_variance() {
        let times = vec![0.2, 0.5, 1.0, 2.0];
        let h = 0.4;
        let cov = fbm_covariance_matrix(&times, h);
        for (i, &t) in times.iter().enumerate() {
            let expected = fbm_variance(t, h);
            assert!(
                (cov[(i, i)] - expected).abs() < TOL,
                "Diagonal mismatch at {i}"
            );
        }
    }

    #[test]
    fn increment_covariance_matrix_symmetric() {
        let times = vec![0.0, 0.25, 0.5, 0.75, 1.0];
        let h = 0.3;
        let cov = fbm_increment_covariance_matrix(&times, h);
        assert_eq!(cov.nrows(), 4);
        assert_eq!(cov.ncols(), 4);
        for i in 0..cov.nrows() {
            for j in 0..cov.ncols() {
                assert!(
                    (cov[(i, j)] - cov[(j, i)]).abs() < TOL,
                    "Asymmetry at ({i},{j})"
                );
            }
        }
    }

    #[test]
    fn increment_covariance_empty_for_single_point() {
        let cov = fbm_increment_covariance_matrix(&[1.0], 0.5);
        assert_eq!(cov.nrows(), 0);
    }

    // -- Kernel evaluation -------------------------------------------------

    #[test]
    fn riemann_liouville_zero_when_t_leq_s() {
        let h = HurstExponent::new(0.3).unwrap();
        let k = RiemannLiouvilleKernel::new(h);
        assert_eq!(k.evaluate(1.0, 1.0), 0.0);
        assert_eq!(k.evaluate(0.5, 1.0), 0.0);
    }

    #[test]
    fn riemann_liouville_positive_when_t_gt_s() {
        let h = HurstExponent::new(0.3).unwrap();
        let k = RiemannLiouvilleKernel::new(h);
        assert!(k.evaluate(1.0, 0.5) > 0.0);
    }

    // -- Mittag-Leffler ----------------------------------------------------

    #[test]
    fn mittag_leffler_e11_is_exp() {
        // E_{1,1}(z) = exp(z) for real z
        for &x in &[0.0, 0.5, 1.0, -1.0, 2.0] {
            let z = Complex64::new(x, 0.0);
            let ml = mittag_leffler(z, 1.0, 1.0).unwrap();
            let expected = x.exp();
            assert!(
                (ml.re - expected).abs() < 1e-8,
                "E_{{1,1}}({x}) = {}, expected {expected}",
                ml.re
            );
            assert!(ml.im.abs() < 1e-12);
        }
    }

    #[test]
    fn mittag_leffler_e21_is_cos() {
        // E_{2,1}(-z^2) = cos(z) for real z
        for &x in &[0.0, 0.5, 1.0, PI / 4.0, PI / 2.0] {
            let z = Complex64::new(-x * x, 0.0);
            let ml = mittag_leffler(z, 2.0, 1.0).unwrap();
            let expected = x.cos();
            assert!(
                (ml.re - expected).abs() < 1e-8,
                "E_{{2,1}}(-{x}^2) = {}, expected {expected}",
                ml.re
            );
        }
    }

    #[test]
    fn mittag_leffler_rejects_invalid_parameters() {
        let z = Complex64::new(1.0, 0.0);
        assert!(mittag_leffler(z, 0.0, 1.0).is_err());
        assert!(mittag_leffler(z, -0.5, 1.0).is_err());
        assert!(mittag_leffler(z, 1.0, 0.0).is_err());
        assert!(mittag_leffler(z, f64::NAN, 1.0).is_err());
        assert!(mittag_leffler(Complex64::new(f64::NAN, 0.0), 1.0, 1.0).is_err());
    }

    #[test]
    fn mittag_leffler_guard_fires_for_large_z_alpha_lt_1() {
        // The rough-volatility regime alpha < 1 with large negative real z is
        // exactly where the raw Taylor series produces garbage; the domain
        // guard must reject it rather than return a meaningless value.
        let z = Complex64::new(-50.0, 0.0);
        let result = mittag_leffler(z, 0.6, 1.0);
        assert!(
            result.is_err(),
            "expected domain guard error, got {result:?}"
        );
    }

    #[test]
    fn mittag_leffler_small_z_alpha_lt_1_still_works() {
        // E_{0.5,1}(z) for small real z: cross-check against the known identity
        // E_{1/2,1}(z) = exp(z^2) * erfc(-z), here just sanity-check the series
        // against a direct high-precision partial sum for |z| well inside the
        // validity bound.
        let z = Complex64::new(-0.5, 0.0);
        let ml = mittag_leffler(z, 0.5, 1.0).unwrap();
        // Direct reference sum (f64, small |z| so no cancellation issue).
        let mut reference = 0.0_f64;
        for k in 0..60 {
            reference += (-0.5_f64).powi(k) * (-ln_gamma(0.5 * f64::from(k) + 1.0)).exp();
        }
        assert!(
            (ml.re - reference).abs() < 1e-12,
            "series mismatch: {} vs {reference}",
            ml.re
        );
        assert!(ml.im.abs() < 1e-14);
    }
}
