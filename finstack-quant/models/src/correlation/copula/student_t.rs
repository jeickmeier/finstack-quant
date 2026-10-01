//! Student-t copula for tail dependence modeling in credit portfolio pricing.
//!
//! The Student-t copula addresses the "Gaussian copula killed Wall Street" critique
//! by modeling tail dependence - the empirically observed phenomenon that joint
//! defaults cluster in stressed markets more than Gaussian correlation predicts.
//!
//! # Mathematical Model (Standard Multivariate t-Copula)
//!
//! All entities share a common mixing variable W ~ Gamma(ν/2, ν/2):
//! ```text
//! M  = Z_M / √W     (systematic factor, t(ν)-distributed)
//! εᵢ = Zᵢ  / √W     (idiosyncratic, t(ν)-distributed, same W)
//! Aᵢ = √ρ · M + √(1-ρ) · εᵢ
//! ```
//!
//! The shared W creates tail dependence: when W is small (heavy-tail event),
//! ALL variables are simultaneously large in magnitude.
//!
//! # Conditional Default Probability
//!
//! Given the systematic factor M = m:
//! ```text
//! P(default | M=m) = t_{ν+1}( (c - √ρ·m) / √(1-ρ) · √((ν+1)/(ν + m²)) )
//! ```
//!
//! where c = t_ν⁻¹(PD) is the default threshold and the ν+1 degrees of freedom
//! arise from conditioning on M in the multivariate t-distribution.
//!
//! # Tail Dependence
//!
//! Lower tail dependence coefficient:
//! ```text
//! λ_L = 2 · t_{ν+1}(-√((ν+1)(1-ρ)/(1+ρ)))
//! ```
//!
//! - As ν → ∞, converges to Gaussian (λ_L → 0)
//! - Lower ν = higher tail dependence
//! - Typical market calibration: ν ∈ [4, 10] for CDX tranches
//!
//! # Integration Approach
//!
//! Uses the variance-gamma mixing representation with **two factor slots**
//! `[Z, W]` so the semi-analytic engine prices the true shared-W t-copula:
//! - Outer integral over W ~ Gamma(ν/2, ν/2) using Gauss-Laguerre quadrature
//! - Inner Gaussian integration over Z conditional on W
//! - [`Copula::integrate_fn`] passes `[z, w]` to the integrand and
//!   [`Copula::conditional_default_prob`] dispatches a 2-length realization
//!   to the (Z, W)-conditional `Φ((c·√W − √ρ·Z)/√(1−ρ))`
//!
//! Conditional independence across names holds only given **both** Z and W.
//! Conditioning on the t-variate `M = Z/√W` alone (a 1-factor quadrature)
//! understates joint-default clustering: it integrates W out before imposing
//! conditional independence, which prices a different model than the
//! per-name MC path samples.
//!
//! # References
//!
//! - Student-t copula theory: `docs/REFERENCES.md#demarta-mcneil-2005-t-copula` `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`
//!
//! - Correlation-dependent credit valuation: `docs/REFERENCES.md#hull-predescu-white-2005`
//!

use super::{get_cached_quadrature, Copula, DEFAULT_QUADRATURE_ORDER};
use finstack_quant_core::math::distributions::chi_squared_quantile;
#[cfg(test)]
use finstack_quant_core::math::student_t_inv_cdf;
use finstack_quant_core::math::{norm_cdf, student_t_cdf, GaussHermiteQuadrature};
use nalgebra::{DMatrix, SymmetricEigen};
use std::sync::Arc;

/// Evaluate the Student-t CDF, returning a loudly-logged NaN when the
/// distribution itself is invalid.
///
/// `student_t_cdf` only fails for non-finite / non-positive degrees of
/// freedom — unreachable for a constructed [`StudentTCopula`]. If it ever
/// fires, NaN is the safer failure mode: it poisons downstream tranche-loss
/// quadrature unmistakably, whereas a normal-CDF substitute would produce a
/// plausible-but-wrong probability that understates the t distribution's
/// tails.
fn t_cdf_or_nan(x: f64, nu: f64) -> f64 {
    student_t_cdf(x, nu).unwrap_or_else(|err| {
        tracing::error!(%err, nu, "Student-t CDF evaluation failed; propagating NaN");
        f64::NAN
    })
}

/// Student-t copula with configurable degrees of freedom.
///
/// Captures tail dependence - the tendency for defaults to cluster
/// during market stress more than Gaussian correlation predicts.
///
/// Implements the standard multivariate t-copula (shared mixing variable)
/// per Demarta & McNeil (2005), with proper ν+1 conditional degrees of freedom.
///
/// # References
///
/// - `docs/REFERENCES.md#demarta-mcneil-2005-t-copula` `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`
/// - `docs/REFERENCES.md#hull-predescu-white-2005`
pub struct StudentTCopula {
    /// Degrees of freedom (ν > 2 required for finite variance)
    degrees_of_freedom: f64,
    /// Quadrature order for integration
    quadrature_order: u8,
    /// Cached inner quadrature for Gaussian integration given W (Arc for cheap clone)
    inner_quadrature: Arc<GaussHermiteQuadrature>,
    /// Cached Gauss-Laguerre quadrature nodes and weights for Gamma(ν/2, ν/2)
    gamma_quadrature: Vec<(f64, f64)>,
}

impl Clone for StudentTCopula {
    fn clone(&self) -> Self {
        Self {
            degrees_of_freedom: self.degrees_of_freedom,
            quadrature_order: self.quadrature_order,
            inner_quadrature: Arc::clone(&self.inner_quadrature),
            gamma_quadrature: self.gamma_quadrature.clone(),
        }
    }
}

impl std::fmt::Debug for StudentTCopula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StudentTCopula")
            .field("degrees_of_freedom", &self.degrees_of_freedom)
            .field("quadrature_order", &self.quadrature_order)
            .field("gamma_points", &self.gamma_quadrature.len())
            .finish()
    }
}

impl StudentTCopula {
    /// Create a Student-t copula with specified degrees of freedom.
    ///
    /// # Arguments
    /// * `df` - Finite degrees of freedom, strictly greater than 2 for finite variance.
    ///
    /// # Returns
    ///
    /// A Student-t copula using the default quadrature order.
    ///
    /// # Panics
    /// Panics if `df` is not finite or is at most 2.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_models::correlation::{Copula, StudentTCopula};
    ///
    /// let copula = StudentTCopula::new(5.0);
    /// let lambda = copula.tail_dependence(0.50);
    ///
    /// assert!(lambda > 0.0);
    /// ```
    #[must_use]
    pub fn new(df: f64) -> Self {
        assert!(
            df.is_finite() && df > 2.0,
            "Student-t df must be > 2 and finite for finite variance"
        );
        let order = DEFAULT_QUADRATURE_ORDER;
        Self {
            degrees_of_freedom: df,
            quadrature_order: order,
            inner_quadrature: get_cached_quadrature(order),
            gamma_quadrature: Self::compute_gamma_quadrature(df, order as usize),
        }
    }

    /// Create with custom quadrature order for higher precision.
    ///
    /// # Arguments
    /// * `df` - Finite degrees of freedom, strictly greater than 2.
    /// * `order` - Requested quadrature order for the inner Gaussian integration;
    ///   the Gamma mixing rule uses this order bounded to 10 through 64 points.
    ///
    /// # Returns
    ///
    /// A Student-t copula using the requested quadrature order.
    ///
    /// # Panics
    ///
    /// Panics if `df` is not finite or is at most 2.
    #[must_use]
    pub fn with_quadrature_order(df: f64, order: u8) -> Self {
        assert!(
            df.is_finite() && df > 2.0,
            "Student-t df must be > 2 and finite"
        );
        Self {
            degrees_of_freedom: df,
            quadrature_order: order,
            inner_quadrature: get_cached_quadrature(order),
            gamma_quadrature: Self::compute_gamma_quadrature(df, order as usize),
        }
    }

    /// Get the degrees of freedom.
    ///
    /// # Returns
    ///
    /// The Student-t degrees of freedom used by this copula.
    #[must_use]
    pub fn df(&self) -> f64 {
        self.degrees_of_freedom
    }

    /// Compute quadrature for W ~ Gamma(ν/2, ν/2) integration.
    ///
    /// W = χ²(ν)/ν has a Gamma(ν/2, 2/ν) distribution (shape=ν/2, scale=2/ν).
    ///
    /// The density is: f(w) = (ν/2)^{ν/2} / Γ(ν/2) · w^{ν/2-1} · exp(-νw/2)
    ///
    /// Using the substitution u = νw/2 (so w = 2u/ν, dw = 2/ν du):
    /// ∫ g(w) f(w) dw = ∫ g(2u/ν) · u^{ν/2-1} · exp(-u) / Γ(ν/2) du
    ///
    /// Generalized Laguerre polynomials absorb the full Gamma weight. Their
    /// Golub-Welsch Jacobi matrix is formed directly in `W = u / shape`
    /// coordinates, keeping its entries finite even for very large ν. Squared
    /// first components of its orthonormal eigenvectors are probability weights:
    /// no Gamma function, density reweighting, or node filtering is required.
    fn compute_gamma_quadrature(nu: f64, n: usize) -> Vec<(f64, f64)> {
        let effective_n = n.clamp(MIN_LAGUERRE_ORDER, MAX_LAGUERRE_ORDER);
        let shape = nu / 2.0;
        let mut jacobi = DMatrix::<f64>::zeros(effective_n, effective_n);
        for k in 0..effective_n {
            let k_float = k as f64;
            jacobi[(k, k)] = 1.0 + 2.0 * k_float / shape;
            if k > 0 {
                let off_diagonal = ((k_float / shape) * (1.0 + (k_float - 1.0) / shape)).sqrt();
                jacobi[(k, k - 1)] = off_diagonal;
                jacobi[(k - 1, k)] = off_diagonal;
            }
        }
        let eigen = SymmetricEigen::new(jacobi);
        let mut nodes_weights: Vec<_> = (0..effective_n)
            .map(|index| {
                let first_component = eigen.eigenvectors[(0, index)];
                (eigen.eigenvalues[index], first_component * first_component)
            })
            .collect();
        nodes_weights.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        nodes_weights
    }
}

/// Floor on the Student-t copula's Gauss-Laguerre outer-integration order.
///
/// At least ten nodes resolve the mixing distribution for low-degree rules;
/// callers can increase the order to check nonlinear loss-integral convergence.
const MIN_LAGUERRE_ORDER: usize = 10;

/// Upper bound on the Gauss-Laguerre order accepted by the Student-t
/// copula. `O(n²)` eigendecomposition inside
/// [`GaussLaguerreQuadrature::new`] remains cheap below this bound;
/// above it, numerical conditioning of the Jacobi matrix starts to
/// erode reliable weight recovery for the highest-index nodes.
const MAX_LAGUERRE_ORDER: usize = 64;

impl Copula for StudentTCopula {
    fn conditional_default_prob(
        &self,
        default_threshold: f64,
        factor_realization: &[f64],
        correlation: f64,
    ) -> f64 {
        // Two accepted shapes:
        //
        // * `[z, w]` — the canonical 2-factor realization produced by
        //   `integrate_fn` (Gaussian Z plus shared mixing W). Dispatches the
        //   exact (Z, W)-conditional so the quadrature engine prices the same
        //   shared-W model the per-name MC samples.
        // * `[m]` — the t-distributed systematic factor M = Z/√W with W
        //   integrated out. This is the correct *single-name* conditional
        //   P(default | M) (Demarta & McNeil ν+1 form), retained for scalar-Z
        //   engines; it must NOT be used as a conditional-independence factor
        //   for pool-loss integration.
        //
        // Anything else is a programmer error: fail loudly in debug, return
        // the unconditional PD t_ν(c) in release.
        match factor_realization {
            [z, w] => {
                return self.conditional_default_prob_given_systematic_and_mixing(
                    default_threshold,
                    *z,
                    *w,
                    correlation,
                );
            }
            [_] => {}
            _ => {
                debug_assert!(
                    false,
                    "StudentTCopula expects [z, w] or [m], got {} factors",
                    factor_realization.len()
                );
                tracing::error!(
                    actual = factor_realization.len(),
                    "StudentTCopula: factor length mismatch; returning unconditional PD"
                );
                return t_cdf_or_nan(default_threshold, self.degrees_of_freedom);
            }
        }
        let [m] = factor_realization else {
            return t_cdf_or_nan(default_threshold, self.degrees_of_freedom);
        };
        let m = *m;
        let nu = self.degrees_of_freedom;
        if !correlation.is_finite() || !(0.0..=1.0).contains(&correlation) {
            return f64::NAN;
        }
        if correlation >= 1.0 {
            return f64::from(m <= default_threshold);
        }

        let sqrt_rho = correlation.sqrt();
        let sqrt_1mr = (1.0 - correlation).sqrt();

        // Standard multivariate t-copula conditional (Demarta & McNeil 2005):
        // P(default | M=m) = t_{ν+1}( (c - √ρ·m)/√(1-ρ) · √((ν+1)/(ν+m²)) )
        // Even at rho = 0, observing M changes the shared scale-mixture
        // distribution. Zero linear correlation does not imply independence.
        // Scale before subtracting so large finite M cannot overflow M² or
        // produce the indeterminate product infinity * zero.
        let conditional_scale = nu.sqrt().hypot(m);
        let conditional_threshold = ((nu + 1.0).sqrt() / sqrt_1mr)
            * (default_threshold / conditional_scale - sqrt_rho * (m / conditional_scale));

        t_cdf_or_nan(conditional_threshold, nu + 1.0)
    }

    fn conditional_default_prob_given_systematic_and_mixing(
        &self,
        default_threshold: f64,
        systematic: f64,
        mixing: f64,
        correlation: f64,
    ) -> f64 {
        // LHP (N → ∞) limit of the per-name Student-t latent construction
        //   Aᵢ = (√ρ·Z + √(1−ρ)·εᵢ) / √W,   default ⟺ Aᵢ ≤ c = t_ν⁻¹(PD)
        // conditioned on the SAME (Z, W) as `latent_variable`. With εᵢ ~ N(0,1):
        //   Aᵢ ≤ c  ⟺  √(1−ρ)·εᵢ ≤ c·√W − √ρ·Z  ⟺  εᵢ ≤ (c·√W − √ρ·Z)/√(1−ρ)
        // so the conditional default fraction is
        //   P(default | Z, W) = Φ( (c·√W − √ρ·Z) / √(1−ρ) ).
        //
        // This is NOT `conditional_default_prob`: that method conditions on
        // the t(ν) systematic factor M = Z/√W (with W integrated out via the
        // ν+1 scaling), whereas the per-name engine draws a Gaussian Z and an
        // explicit shared W. Feeding Z into the M-slot is a distribution and
        // a sigma-algebra mismatch — it biases the pool default rate low.
        let z = systematic;
        if !mixing.is_finite()
            || mixing <= 0.0
            || !correlation.is_finite()
            || !(0.0..=1.0).contains(&correlation)
        {
            return f64::NAN;
        }
        let w = mixing;

        if correlation == 0.0 {
            // No systematic channel: Aᵢ = εᵢ/√W, so default ⟺ εᵢ ≤ c·√W.
            return norm_cdf(default_threshold * w.sqrt());
        }
        if correlation >= 1.0 {
            return f64::from(z / w.sqrt() <= default_threshold);
        }

        let sqrt_rho = correlation.sqrt();
        let sqrt_1mr = (1.0 - correlation).sqrt();

        let conditional_threshold = (default_threshold * w.sqrt() - sqrt_rho * z) / sqrt_1mr;

        norm_cdf(conditional_threshold)
    }

    fn integrate_fn(&self, f: &dyn Fn(&[f64]) -> f64) -> f64 {
        // Two-layer integration over BOTH latent factor slots [Z, W]:
        //
        // E[g(Z, W)] = E_W[ E_Z[ g(Z, W) | W ] ]
        //
        // Outer: over W ~ Gamma(ν/2, ν/2) using its normalized generalized
        // Laguerre rule; inner: over Z ~ N(0,1) using
        // Gauss-Hermite. The integrand receives the raw pair `[z, w]` —
        // NOT the collapsed t-variate `m = z/√w` — so pool-loss engines
        // impose conditional independence in the correct (Z, W)
        // sigma-algebra (names are NOT conditionally independent given M
        // alone).
        let mut result = 0.0;
        for &(w_val, w_weight) in &self.gamma_quadrature {
            let inner = self
                .inner_quadrature
                .integrate(|z_gauss| f(&[z_gauss, w_val]));

            result += w_weight * inner;
        }

        result
    }

    fn try_integrate_pair(
        &self,
        f: &super::FactorPairIntegrand<'_>,
    ) -> finstack_quant_core::Result<(f64, f64)> {
        let error = std::cell::RefCell::new(None);
        let sqrt_2 = std::f64::consts::SQRT_2;
        let inv_sqrt_pi = 1.0 / std::f64::consts::PI.sqrt();
        let mut first = 0.0;
        let mut second = 0.0;
        for &(w_val, w_weight) in &self.gamma_quadrature {
            let mut inner_first = 0.0;
            let mut inner_second = 0.0;
            for (i, &z) in self.inner_quadrature.points.iter().enumerate() {
                if error.borrow().is_some() {
                    break;
                }
                match f(&[sqrt_2 * z, w_val]) {
                    Ok((a, b)) => {
                        let weight = self.inner_quadrature.weights[i];
                        inner_first += weight * a;
                        inner_second += weight * b;
                    }
                    Err(err) => {
                        *error.borrow_mut() = Some(err);
                    }
                }
            }
            first += w_weight * inner_first * inv_sqrt_pi;
            second += w_weight * inner_second * inv_sqrt_pi;
        }
        match error.into_inner() {
            Some(err) => Err(err),
            None => Ok((first, second)),
        }
    }

    fn sample_mixing(&self, u01: f64) -> finstack_quant_core::Result<f64> {
        // Variance-mixture representation of the multivariate t-copula:
        // every name shares W = χ²(ν)/ν. Inverse-CDF sampling from a single
        // uniform keeps the draw deterministic and order-stable (no rejection
        // loop), which is required for bit-identical serial/parallel results.
        super::validate_mixing_uniform(u01)?;
        let nu = self.degrees_of_freedom;
        let mixing = chi_squared_quantile(u01, nu)? / nu;
        if !mixing.is_finite() || mixing <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Student-t mixing transform must produce a finite positive value, got {mixing}"
            )));
        }
        Ok(mixing)
    }

    fn latent_variable(
        &self,
        systematic: f64,
        idiosyncratic: f64,
        mixing: f64,
        correlation: f64,
    ) -> f64 {
        // Standard multivariate t-copula latent variable (Demarta & McNeil
        // 2005), the sampling counterpart of `conditional_default_prob`:
        //   M  = Z_M / √W,  εᵢ = Zᵢ / √W,  Aᵢ = √ρ·M + √(1−ρ)·εᵢ
        // The shared mixing W (drawn once per period via `sample_mixing`)
        // induces tail dependence: a small W makes every name's |Aᵢ| large
        // simultaneously. Default occurs when Aᵢ ≤ t_ν⁻¹(PD).
        if !mixing.is_finite()
            || mixing <= 0.0
            || !correlation.is_finite()
            || !(0.0..=1.0).contains(&correlation)
        {
            return f64::NAN;
        }
        let w = mixing;
        let gaussian_part =
            correlation.sqrt() * systematic + (1.0 - correlation).sqrt() * idiosyncratic;
        gaussian_part / w.sqrt()
    }

    fn num_factors(&self) -> usize {
        // [Z, W]: Gaussian systematic factor plus the shared mixing variable.
        // Conditional independence across names requires conditioning on both.
        2
    }

    fn model_name(&self) -> &'static str {
        "Student-t Copula"
    }

    fn tail_dependence(&self, correlation: f64) -> f64 {
        if !correlation.is_finite() || !(0.0..=1.0).contains(&correlation) {
            return f64::NAN;
        }
        let rho = correlation;
        let nu = self.degrees_of_freedom;

        // λ_L = 2 · t_{ν+1}(-√((ν+1)(1-ρ)/(1+ρ)))
        let arg = -((nu + 1.0) * (1.0 - rho) / (1.0 + rho)).sqrt();
        2.0 * t_cdf_or_nan(arg, nu + 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::math::standard_normal_inv_cdf;

    #[test]
    fn mixing_lower_tail_is_positive_and_inversion_failures_propagate() {
        let mut copula = StudentTCopula::new(2.00001);
        for u in [1e-15, 1e-12, 1e-6, 0.5, 1.0 - 1e-12] {
            let mixing = copula.sample_mixing(u).expect("resolvable mixing tail");
            assert!(mixing.is_finite() && mixing > 0.0);
        }
        assert!(copula.sample_mixing(1e-15).expect("lower tail") < 1e-12);

        // Exercise the numerical-transform failure itself, separately from
        // uniform validation: this must not substitute the mean W=1.
        for unsupported in [f64::INFINITY, 1e8 + 1.0] {
            copula.degrees_of_freedom = unsupported;
            assert!(copula.sample_mixing(0.5).is_err());
        }
    }

    #[test]
    fn latent_and_conditional_mixing_preserve_small_values_and_reject_invalid_values() {
        let copula = StudentTCopula::new(5.0);
        let latent = copula.latent_variable(0.0, 1.0, 1e-20, 0.0);
        assert!((latent / 1e10 - 1.0).abs() < 1e-12);
        for mixing in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(copula.latent_variable(0.0, 1.0, mixing, 0.3).is_nan());
            assert!(copula
                .conditional_default_prob_given_systematic_and_mixing(0.0, 0.0, mixing, 0.3)
                .is_nan());
        }
    }

    #[test]
    fn test_student_t_creation() {
        let copula = StudentTCopula::new(5.0);
        // [Z, W]: the quadrature engine must integrate over both the Gaussian
        // systematic factor and the shared mixing variable.
        assert_eq!(copula.num_factors(), 2);
        assert!((copula.df() - 5.0).abs() < 1e-10);
        assert_eq!(copula.model_name(), "Student-t Copula");
    }

    #[test]
    #[should_panic(expected = "Student-t df must be > 2")]
    fn test_student_t_invalid_df() {
        let _ = StudentTCopula::new(2.0);
    }

    #[test]
    fn test_tail_dependence_positive() {
        let copula = StudentTCopula::new(5.0);
        let lambda = copula.tail_dependence(0.5);

        assert!(lambda > 0.0, "Tail dependence should be positive");
        assert!(lambda < 1.0, "Tail dependence should be < 1");
    }

    #[test]
    fn test_tail_dependence_increases_with_correlation() {
        let copula = StudentTCopula::new(5.0);

        let lambda_low = copula.tail_dependence(0.2);
        let lambda_mid = copula.tail_dependence(0.5);
        let lambda_high = copula.tail_dependence(0.8);

        assert!(
            lambda_mid > lambda_low,
            "Tail dependence should increase with correlation"
        );
        assert!(
            lambda_high > lambda_mid,
            "Tail dependence should increase with correlation"
        );
    }

    #[test]
    fn test_tail_dependence_decreases_with_df() {
        let copula_low_df = StudentTCopula::new(4.0);
        let copula_high_df = StudentTCopula::new(20.0);

        let lambda_low_df = copula_low_df.tail_dependence(0.5);
        let lambda_high_df = copula_high_df.tail_dependence(0.5);

        assert!(
            lambda_low_df > lambda_high_df,
            "Lower df should give higher tail dependence"
        );
    }

    #[test]
    fn test_converges_to_gaussian_for_high_df() {
        let copula_high_df = StudentTCopula::new(100.0);
        let lambda = copula_high_df.tail_dependence(0.5);

        assert!(
            lambda < 0.05,
            "High df should give near-zero tail dependence"
        );
    }

    #[test]
    fn test_conditional_prob_sensitive_to_factor() {
        let copula = StudentTCopula::new(5.0);
        let threshold = student_t_inv_cdf(0.05, 5.0).expect("valid Student-t inputs");
        let correlation = 0.3;

        let prob_neg = copula.conditional_default_prob(threshold, &[-2.0], correlation);
        let prob_zero = copula.conditional_default_prob(threshold, &[0.0], correlation);
        let prob_pos = copula.conditional_default_prob(threshold, &[2.0], correlation);

        assert!(prob_neg > prob_zero);
        assert!(prob_pos < prob_zero);
    }

    #[test]
    fn test_conditional_prob_uses_nu_plus_one_scaling() {
        // Verify the conditional formula from Demarta & McNeil (2005):
        //   P(default | M=m) = t_{ν+1}((c − √ρ·m)/√(1−ρ) · √((ν+1)/(ν+m²)))
        // Every interior correlation uses the same exact conditional formula.
        let copula = StudentTCopula::new(5.0);
        let nu: f64 = 5.0;
        let rho: f64 = 0.30;
        let sqrt_rho = rho.sqrt();
        let sqrt_1mr = (1.0 - rho).sqrt();

        for &factor in &[0.35_f64, 3.0, -2.5] {
            let threshold: f64 = -1.25;
            let base_arg = (threshold - sqrt_rho * factor) / sqrt_1mr;
            let scaling = ((nu + 1.0) / (nu + factor * factor)).sqrt();
            let expected =
                student_t_cdf(base_arg * scaling, nu + 1.0).expect("valid Student-t inputs");

            let prob = copula.conditional_default_prob(threshold, &[factor], rho);

            assert!(
                (prob - expected).abs() < 1e-12,
                "conditional formula mismatch: factor={factor}, expected {expected}, got {prob}"
            );
        }
    }

    #[test]
    fn test_high_correlation_saturates_toward_indicator() {
        // Regression: at ρ → 1, the copula should degenerate to
        //   P(default | M=m) → 1{m ≤ c}
        // because Aᵢ = M. The previous implementation dropped the 1/√(1−ρ)
        // factor and produced an overly smooth CDF.
        let copula = StudentTCopula::new(5.0);
        let threshold: f64 = -1.25;

        // m well below threshold ⇒ default virtually certain.
        let prob_below = copula.conditional_default_prob(threshold, &[-5.0], 1.0);
        assert!(
            prob_below > 0.999,
            "ρ→1, m≪c: expected near 1, got {prob_below}"
        );

        // m well above threshold ⇒ default virtually impossible.
        let prob_above = copula.conditional_default_prob(threshold, &[5.0], 1.0);
        assert!(
            prob_above < 1e-3,
            "ρ→1, m≫c: expected near 0, got {prob_above}"
        );
    }

    #[test]
    fn test_integration_recovers_unconditional() {
        // Critical self-consistency test: E[P(default|M)] must equal PD
        for &df in &[4.0, 5.0, 10.0, 30.0] {
            let copula = StudentTCopula::new(df);
            let pd = 0.05;
            let threshold = student_t_inv_cdf(pd, df).expect("valid Student-t inputs");
            let correlation = 0.30;

            let integrated_prob = copula
                .integrate_fn(&|z| copula.conditional_default_prob(threshold, z, correlation));

            assert!(
                (integrated_prob - pd).abs() < 0.005,
                "df={}: Integrated probability {} should equal unconditional {} (error={})",
                df,
                integrated_prob,
                pd,
                (integrated_prob - pd).abs()
            );
        }
    }

    #[test]
    fn test_integration_recovers_unconditional_various_pd() {
        let copula = StudentTCopula::new(5.0);

        for &pd in &[0.01, 0.05, 0.10, 0.20] {
            let threshold = student_t_inv_cdf(pd, 5.0).expect("valid Student-t inputs");
            let correlation = 0.30;

            let integrated_prob = copula
                .integrate_fn(&|z| copula.conditional_default_prob(threshold, z, correlation));

            assert!(
                (integrated_prob - pd).abs() < 0.005,
                "pd={}: Integrated probability {} (error={})",
                pd,
                integrated_prob,
                (integrated_prob - pd).abs()
            );
        }
    }

    #[test]
    fn test_tail_dependence_golden_values() {
        let test_cases = [(4.0, 0.5), (5.0, 0.5), (10.0, 0.5)];

        for (df, rho) in test_cases {
            let copula = StudentTCopula::new(df);
            let lambda = copula.tail_dependence(rho);

            assert!(
                (0.0..=1.0).contains(&lambda),
                "Tail dependence for df={}, ρ={}: got {}, expected in [0,1]",
                df,
                rho,
                lambda
            );

            assert!(
                lambda < 0.5,
                "Tail dependence {} seems too high for df={}, ρ={}",
                lambda,
                df,
                rho
            );
        }

        let copula_4 = StudentTCopula::new(4.0);
        let copula_10 = StudentTCopula::new(10.0);
        assert!(
            copula_4.tail_dependence(0.5) > copula_10.tail_dependence(0.5),
            "Lower df should give higher tail dependence"
        );
    }

    #[test]
    fn test_student_t_cdf_accuracy() {
        let cdf = student_t_cdf(-2.0, 5.0).expect("valid Student-t inputs");
        assert!(
            (cdf - 0.051).abs() < 0.002,
            "CDF(-2.0, df=5) = {}, expected ~0.051",
            cdf
        );

        let cdf_10 = student_t_cdf(-1.812, 10.0).expect("valid Student-t inputs");
        assert!(
            (cdf_10 - 0.05).abs() < 0.005,
            "CDF(-1.812, df=10) = {}, expected ~0.05",
            cdf_10
        );
    }

    #[test]
    fn test_student_t_inv_cdf_roundtrip() {
        let test_dfs = [3.0, 5.0, 10.0, 30.0];
        let test_probs = [0.05, 0.1, 0.25, 0.5, 0.75, 0.9, 0.95];

        for &df in &test_dfs {
            for &p in &test_probs {
                let x = student_t_inv_cdf(p, df).expect("valid Student-t inputs");
                let p_back = student_t_cdf(x, df).expect("valid Student-t inputs");
                assert!(
                    (p - p_back).abs() < 1e-6,
                    "Round-trip failed for df={}, p={}: got x={}, p_back={}",
                    df,
                    p,
                    x,
                    p_back
                );
            }
        }
    }

    #[test]
    fn test_gamma_quadrature_properties() {
        for df in [2.01, 4.0, 5.0, 10.0, 20.0, 200.0, 500.0, 1000.0, 1e8] {
            let copula = StudentTCopula::new(df);
            let points = &copula.gamma_quadrature;

            for &(x, w) in points {
                assert!(x > 0.0, "Quadrature node must be positive, got {}", x);
                assert!(
                    w >= 0.0,
                    "Quadrature weight must be non-negative, got {}",
                    w
                );
            }

            let weight_sum: f64 = points.iter().map(|&(_, w)| w).sum();
            assert!(
                (weight_sum - 1.0).abs() < 1e-12,
                "Gamma({}/2) weights sum to {}, expected ~1.0",
                df,
                weight_sum
            );

            assert!(
                points.len() >= 3,
                "Expected at least 3 quadrature points, got {}",
                points.len()
            );

            let mean: f64 = points.iter().map(|&(w, weight)| w * weight).sum();
            let second_moment: f64 = points.iter().map(|&(w, weight)| w * w * weight).sum();
            assert!((mean - 1.0).abs() < 1e-12, "df={df}: E[W]={mean}");
            assert!(
                (second_moment - (1.0 + 2.0 / df)).abs() < 1e-12,
                "df={df}: E[W²]={second_moment}"
            );
        }
    }

    #[test]
    fn high_df_quadrature_converges_to_gaussian_default_and_joint_probabilities() {
        use crate::correlation::GaussianCopula;

        let gaussian = GaussianCopula::new();
        let rho = 0.30;
        let pd = 0.05;
        let threshold = standard_normal_inv_cdf(pd);
        let gaussian_joint = gaussian.integrate_fn(&|factors| {
            gaussian
                .conditional_default_prob(threshold, factors, rho)
                .powi(2)
        });
        for df in [200.0, 500.0, 1000.0, 10_000.0] {
            let copula = StudentTCopula::new(df);
            let threshold = student_t_inv_cdf(pd, df).expect("valid Student-t inputs");
            let marginal = copula
                .integrate_fn(&|factors| copula.conditional_default_prob(threshold, factors, rho));
            let joint = copula.integrate_fn(&|factors| {
                copula
                    .conditional_default_prob(threshold, factors, rho)
                    .powi(2)
            });
            assert!((marginal - pd).abs() < 1e-6, "df={df}: PD={marginal}");
            assert!(
                (joint - gaussian_joint).abs() < 5e-4,
                "df={df}: joint={joint}, Gaussian={gaussian_joint}"
            );
            let pair = copula
                .try_integrate_pair(&|factors| Ok((1.0, factors[1])))
                .expect("finite Gamma moments");
            assert!((pair.0 - 1.0).abs() < 1e-12);
            assert!((pair.1 - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn conditional_and_latent_kernels_preserve_every_correlation() {
        let copula = StudentTCopula::new(5.0);
        for rho in [0.0_f64, 0.001, 0.01, 0.010_001, 0.3, 0.99, 0.999_999, 1.0] {
            for (threshold, z, mixing) in [(0.0_f64, 3.0_f64, 1.0_f64), (-0.7, -0.5, 0.4)] {
                let expected = if rho == 1.0 {
                    f64::from(z / mixing.sqrt() <= threshold)
                } else {
                    norm_cdf((threshold * mixing.sqrt() - rho.sqrt() * z) / (1.0 - rho).sqrt())
                };
                let actual = copula
                    .conditional_default_prob_checked(threshold, &[z, mixing], rho)
                    .expect("valid conditional inputs");
                assert!((actual - expected).abs() < 1e-14, "rho={rho}");
                if rho < 1.0 {
                    let epsilon_at_barrier =
                        (threshold * mixing.sqrt() - rho.sqrt() * z) / (1.0 - rho).sqrt();
                    assert!(
                        (copula.latent_variable(z, epsilon_at_barrier, mixing, rho) - threshold)
                            .abs()
                            < 1e-12
                    );
                }
            }
        }
        assert_eq!(copula.tail_dependence(1.0), 1.0);
        assert_eq!(copula.conditional_default_prob(0.0, &[0.05], 1.0), 0.0);
        // At zero linear correlation, observing M still conditions shared W.
        let expected =
            student_t_cdf(-(6.0_f64 / 14.0).sqrt(), 6.0).expect("valid conditional t distribution");
        assert!((copula.conditional_default_prob(-1.0, &[3.0], 0.0) - expected).abs() < 1e-14);
    }

    /// Requesting `with_quadrature_order(n)` with `n > 10` must
    /// actually produce a larger rule (the Golub-Welsch runtime
    /// generator has no fixed node cap).
    #[test]
    fn test_with_quadrature_order_uses_requested_order() {
        let df = 5.0;
        let copula10 = StudentTCopula::with_quadrature_order(df, 10);
        let copula30 = StudentTCopula::with_quadrature_order(df, 30);

        let n10 = copula10.gamma_quadrature.len();
        let n30 = copula30.gamma_quadrature.len();
        assert_eq!(n10, 10);
        assert_eq!(n30, 30);
        // Both must still integrate the constant 1 to approximately 1.
        for copula in [&copula10, &copula30] {
            let sum: f64 = copula.gamma_quadrature.iter().map(|(_, w)| w).sum();
            assert!(
                (sum - 1.0).abs() < 1e-12,
                "n={}: Σ w_i = {sum}, expected ~1",
                copula.gamma_quadrature.len()
            );
        }
    }

    #[test]
    fn test_factor_length_mismatch_contract() {
        let df = 5.0;
        let copula = StudentTCopula::new(df);
        let pd = 0.05;
        let threshold = student_t_inv_cdf(pd, df).expect("valid Student-t inputs");
        let correlation = 0.30;

        let assert_contract = |factors: &[f64]| {
            if cfg!(debug_assertions) {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    copula.conditional_default_prob(threshold, factors, correlation)
                }));
                assert!(
                    outcome.is_err(),
                    "debug builds should panic on factor length mismatch"
                );
            } else {
                let result = copula.conditional_default_prob(threshold, factors, correlation);
                assert!(
                    (result - pd).abs() < 1e-6,
                    "factor length mismatch should return unconditional PD ({pd}), got {result}"
                );
            }
        };

        // Valid shapes are [m] (single-name conditional given M) and [z, w]
        // (quadrature realization); everything else violates the contract.
        assert_contract(&[]);
        assert_contract(&[0.5, 1.0, 2.0]);
    }

    /// M2.3 anchor: the semi-analytic quadrature and a per-name Monte Carlo
    /// simulation must price the SAME shared-W t-copula. We compare the
    /// joint default probability of two names sharing (Z, W):
    ///
    ///   P(A₁ ≤ c, A₂ ≤ c) = E_{Z,W}[ P(default | Z, W)² ]
    ///
    /// Quadrature computes the right-hand side via `integrate_fn` (now over
    /// `[z, w]`); MC simulates pairs via `latent_variable` with a shared
    /// (Z, W) and independent ε₁, ε₂. A 1-factor quadrature
    /// (conditioning on M = Z/√W alone) understates this joint probability
    /// and fails the tolerance.
    #[test]
    fn quadrature_matches_per_name_mc_joint_default_prob() {
        use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
        use finstack_quant_models::monte_carlo::traits::RandomStream;

        let nu = 5.0;
        let copula = StudentTCopula::with_quadrature_order(nu, 40);
        let pd = 0.05;
        let threshold = student_t_inv_cdf(pd, nu).expect("valid Student-t inputs");
        let rho = 0.30;

        let quad_joint = copula.integrate_fn(&|factors| {
            let p = copula.conditional_default_prob(threshold, factors, rho);
            p * p
        });

        let mut rng = PhiloxRng::new(31337);
        let trials = 2_000_000usize;
        let mut joint = 0usize;
        for _ in 0..trials {
            let w = copula
                .sample_mixing(rng.next_u01())
                .expect("valid mixing draw");
            let z = rng.next_std_normal();
            let a1 = copula.latent_variable(z, rng.next_std_normal(), w, rho);
            let a2 = copula.latent_variable(z, rng.next_std_normal(), w, rho);
            if a1 <= threshold && a2 <= threshold {
                joint += 1;
            }
        }
        let mc_joint = joint as f64 / trials as f64;

        // p_joint ≈ 0.008; 3σ MC error at n=2e6 ≈ 1.9e-4.
        assert!(
            (quad_joint - mc_joint).abs() < 3e-4,
            "shared-W t-copula joint default prob: quadrature {quad_joint} must \
             match per-name MC {mc_joint} — a mismatch means the quadrature \
             engine prices a different model than the MC engine samples"
        );
    }

    #[test]
    fn test_latent_variable_marginal_recovers_pd() {
        // The per-name Student-t latent Aᵢ = (√ρ·Z + √(1−ρ)·εᵢ)/√W with
        // W = χ²(ν)/ν must be marginally t(ν): the fraction of draws below
        // t_ν⁻¹(PD) must equal PD. Each period draws one shared mixing W and
        // per-name idiosyncratic εᵢ.
        use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
        use finstack_quant_models::monte_carlo::traits::RandomStream;

        let nu = 6.0;
        let copula = StudentTCopula::new(nu);
        let pd = 0.05;
        let threshold = student_t_inv_cdf(pd, nu).expect("valid Student-t inputs");
        let rho = 0.30;

        let mut rng = PhiloxRng::new(11);
        // Outer loop = periods (one shared W each); inner = names.
        let periods = 8_000usize;
        let names = 64usize;
        let mut defaults = 0usize;
        for _ in 0..periods {
            let w = copula
                .sample_mixing(rng.next_u01())
                .expect("valid mixing draw");
            let z = rng.next_std_normal();
            for _ in 0..names {
                let eps = rng.next_std_normal();
                let a = copula.latent_variable(z, eps, w, rho);
                if a <= threshold {
                    defaults += 1;
                }
            }
        }
        let realized = defaults as f64 / (periods * names) as f64;
        // Heavier-tailed and correlated draws → wider MC error; 0.004 covers
        // the ~512k correlated sample at p=0.05.
        assert!(
            (realized - pd).abs() < 0.004,
            "Student-t latent marginal {realized} should recover PD {pd}"
        );
    }

    #[test]
    fn test_high_df_converges_to_gaussian() {
        use finstack_quant_core::math::norm_cdf;

        let df = 50.0;
        let copula = StudentTCopula::new(df);
        let pd = 0.05;
        let threshold = student_t_inv_cdf(pd, df).expect("valid Student-t inputs");
        let correlation = 0.30;

        let t_prob = copula.conditional_default_prob(threshold, &[0.0], correlation);

        let gauss_threshold = standard_normal_inv_cdf(pd);
        let sqrt_rho = correlation.sqrt();
        let sqrt_1mr = (1.0 - correlation).sqrt();
        let gauss_prob = norm_cdf((gauss_threshold - sqrt_rho * 0.0) / sqrt_1mr);

        assert!(
            (t_prob - gauss_prob).abs() < 0.02,
            "High-df t ({}) should be close to Gaussian ({})",
            t_prob,
            gauss_prob
        );
    }

    /// Compare default and high-order Gamma integration in a heavy-tail case.
    #[test]
    fn gamma_quadrature_bias_within_5bp_at_nu_4() {
        let nu = 4.0;
        let pd = 0.05; // senior-tranche regime: 5% unconditional PD
        let threshold = student_t_inv_cdf(pd, nu).expect("valid Student-t inputs");
        let correlation = 0.30;

        // Default-order copula (20 generalized Laguerre nodes).
        let copula_default = StudentTCopula::new(nu);

        // High-order reference: 64-node rule (MAX_LAGUERRE_ORDER).
        let copula_ref = StudentTCopula::with_quadrature_order(nu, 64);

        let integrate = |c: &StudentTCopula| {
            c.integrate_fn(&|z| c.conditional_default_prob(threshold, z, correlation))
        };

        let result_default = integrate(&copula_default);
        let result_ref = integrate(&copula_ref);

        assert!(
            (result_ref - pd).abs() < 0.005,
            "64-node reference: integrated PD {result_ref:.6} should equal {pd} (bias={:.6})",
            (result_ref - pd).abs()
        );
        assert!(
            (result_default - pd).abs() < 0.005,
            "default-order: integrated PD {result_default:.6} should equal {pd} (bias={:.6})",
            (result_default - pd).abs()
        );

        let bias = (result_default - result_ref).abs();
        assert!(
            bias < 0.0005,
            "ν={nu}: gamma-quadrature bias between 20-node and 64-node rules = {bias:.6} ({:.2} bp); \
             expected < 5 bp",
            bias * 10_000.0
        );
    }
}
