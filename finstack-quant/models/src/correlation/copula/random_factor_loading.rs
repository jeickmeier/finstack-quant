//! Random Factor Loading (RFL) copula with stochastic correlation.
//!
//! Models factor loading as random, representing uncertainty in common-factor
//! exposure. Larger loading realizations strengthen joint-default clustering.
//!
//! # Mathematical Model
//!
//! The factor loading β is random rather than fixed:
//! ```text
//! β = clamp(μ + σ_β · η, 0, 1),    η ~ N(0, 1)
//! Aᵢ = β · Z + √(1-β²) · εᵢ
//! ```
//!
//! This means effective correlation ρ(β) = β² is stochastic, with:
//! - Scenarios with larger β have stronger common-factor dependence.
//! - The loading shock η is independent of the market factor Z.
//!
//! # Parameterization
//!
//! The `correlation` argument is treated as the **realized pairwise
//! correlation**: `E[β²] = ρ`. The normal location μ is calibrated to this
//! clipped second moment; it can be negative when low correlation and large
//! loading volatility are combined. Correlation zero and one give the exact
//! deterministic loadings zero and one. With zero loading volatility,
//! β = √ρ and the model is exactly Gaussian.
//!
//! # Integration Approach
//!
//! Two-dimensional integration:
//! 1. Outer: over the random loading β (or equivalently, loading shock η)
//! 2. Inner: over the market factor Z given β
//!
//! # Tail-Dependence Interpretation
//!
//! The loading distribution has an atom at β = 1, which contributes perfect
//! dependence. Its probability `Φ((μ - 1) / σ_β)` is the strict lower-tail
//! dependence coefficient. The separate
//! [`RandomFactorLoadingCopula::stress_correlation_proxy`] is a heuristic
//! diagnostic rather than this asymptotic coefficient.
//!
//! # Impact on Tranches
//!
//! - **Equity tranches**: Less affected (already high-risk)
//! - **Mezzanine tranches**: Moderately affected
//! - **Senior tranches**: Significantly affected (correlation uncertainty matters)
//!
//! # References
//!
//! - Random recovery and random-factor-loading extensions: `docs/REFERENCES.md#andersen-sidenius-2005-rfl`
//!

use super::{select_quadrature, Copula, DEFAULT_QUADRATURE_ORDER};
use finstack_quant_core::math::{
    norm_cdf, norm_pdf, standard_normal_inv_cdf, BrentSolver, GaussHermiteQuadrature,
};
use std::sync::Mutex;

/// Minimum loading, giving the exact independent Gaussian limit.
const MIN_LOADING: f64 = 0.0;
/// Maximum loading, giving the exact comonotonic limit.
const MAX_LOADING: f64 = 1.0;
/// CDF argument clipping to prevent overflow.
const CDF_CLIP: f64 = 10.0;

/// Random Factor Loading copula with stochastic correlation.
///
/// The factor loading is drawn from a distribution at each scenario,
/// creating uncertainty in the effective correlation level.
///
/// # Numerical Stability
///
/// - Loading volatility is clamped to [0, 0.5]
/// - Effective loading is clamped to [0, 1], with exact boundary conditionals
/// - The normal location is calibrated to the actual clipped second moment
/// - CDF arguments are clipped to prevent overflow
/// - Quadrature is cached for performance
///
/// # References
///
/// - `docs/REFERENCES.md#andersen-sidenius-2005-rfl`
pub struct RandomFactorLoadingCopula {
    /// Volatility of the factor loading, clamped to [0, 0.5]
    loading_volatility: f64,
    /// Quadrature order for integration
    quadrature_order: u8,
    /// Cached quadrature for outer integration (loading shock η)
    outer_quadrature: GaussHermiteQuadrature,
    /// Cached quadrature for inner integration (market factor Z)
    inner_quadrature: GaussHermiteQuadrature,
    /// Last correlation/location pair, reused across conditional evaluations.
    /// The bounded cache avoids repeated calibration inside quadrature loops.
    location_cache: Mutex<Option<(u64, f64)>>,
}

impl Clone for RandomFactorLoadingCopula {
    fn clone(&self) -> Self {
        let quadrature = select_quadrature(self.quadrature_order);
        Self {
            loading_volatility: self.loading_volatility,
            quadrature_order: self.quadrature_order,
            outer_quadrature: select_quadrature(self.quadrature_order),
            inner_quadrature: quadrature,
            location_cache: Mutex::new(self.location_cache.lock().ok().and_then(|cache| *cache)),
        }
    }
}

impl std::fmt::Debug for RandomFactorLoadingCopula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RandomFactorLoadingCopula")
            .field("loading_volatility", &self.loading_volatility)
            .field("quadrature_order", &self.quadrature_order)
            .finish()
    }
}

impl RandomFactorLoadingCopula {
    /// Create a Random Factor Loading copula.
    ///
    /// # Arguments
    /// * `loading_vol` - Volatility of factor loading, clamped to [0.0, 0.5].
    ///   Typical values: 0.05-0.20. Higher values increase correlation uncertainty.
    ///
    /// # Returns
    ///
    /// An RFL copula using the default quadrature order.
    ///
    /// # Panics
    ///
    /// Panics if `loading_vol` is not finite.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_models::correlation::{Copula, RandomFactorLoadingCopula};
    /// use finstack_quant_core::math::standard_normal_inv_cdf;
    ///
    /// let copula = RandomFactorLoadingCopula::new(0.15);
    /// let threshold = standard_normal_inv_cdf(0.05);
    /// let cond_pd = copula.conditional_default_prob(threshold, &[0.0, 1.0], 0.30);
    ///
    /// assert!(cond_pd > 0.0 && cond_pd < 1.0);
    /// ```
    #[must_use]
    pub fn new(loading_vol: f64) -> Self {
        assert!(
            loading_vol.is_finite(),
            "RFL loading volatility must be finite"
        );
        let order = DEFAULT_QUADRATURE_ORDER;
        Self {
            loading_volatility: loading_vol.clamp(0.0, 0.5),
            quadrature_order: order,
            outer_quadrature: select_quadrature(order),
            inner_quadrature: select_quadrature(order),
            location_cache: Mutex::new(None),
        }
    }

    /// Create with custom quadrature order for higher precision.
    ///
    /// # Arguments
    /// * `loading_vol` - Volatility of factor loading, clamped to [0.0, 0.5]
    /// * `order` - Requested quadrature order for both integration dimensions
    ///
    /// # Returns
    ///
    /// An RFL copula using the requested quadrature order.
    ///
    /// # Panics
    ///
    /// Panics if `loading_vol` is not finite.
    #[must_use]
    pub fn with_quadrature_order(loading_vol: f64, order: u8) -> Self {
        assert!(
            loading_vol.is_finite(),
            "RFL loading volatility must be finite"
        );
        Self {
            loading_volatility: loading_vol.clamp(0.0, 0.5),
            quadrature_order: order,
            outer_quadrature: select_quadrature(order),
            inner_quadrature: select_quadrature(order),
            location_cache: Mutex::new(None),
        }
    }

    /// Get the loading volatility.
    ///
    /// # Returns
    ///
    /// The bounded loading-volatility parameter in decimal units.
    #[must_use]
    pub fn loading_volatility(&self) -> f64 {
        self.loading_volatility
    }

    /// Realized pairwise correlation `E[β²]` actually produced by the model
    /// for the supplied requested `correlation`.
    ///
    /// Evaluates the clipped-normal second moment at the calibrated location.
    /// It agrees with `correlation` to numerical root-finding precision.
    ///
    /// # Arguments
    ///
    /// * `correlation` - Target pairwise latent correlation in `[0, 1]`.
    ///
    /// # Returns
    ///
    /// The realized correlation, or NaN for an invalid target or failed solve.
    #[must_use]
    pub fn realized_correlation(&self, correlation: f64) -> f64 {
        let location = self.loading_location(correlation);
        self.clipped_second_moment(location)
    }

    /// Stress-correlation proxy: a heuristic gauge of how much extra
    /// correlation mass appears in the high-loading tail (`η > 2`).
    ///
    /// **This is NOT the strict copula lower-tail-dependence coefficient
    /// `λ_L`.** RFL collapses to a Gaussian copula in the `σ_β → 0` limit
    /// (where `λ_L = 0`); this proxy correctly vanishes there but has no
    /// formal interpretation as a copula tail-dependence limit.
    ///
    /// The strict coefficient is available separately through
    /// [`Copula::tail_dependence`].
    ///
    /// # Arguments
    ///
    /// * `correlation` - Target pairwise latent correlation in `[0, 1]`.
    ///
    /// # Returns
    ///
    /// A non-negative stress-correlation proxy. Vanishes when
    /// `loading_volatility == 0`.
    #[must_use]
    pub fn stress_correlation_proxy(&self, correlation: f64) -> f64 {
        let mean_loading = self.loading_location(correlation);
        if mean_loading.is_nan() {
            return f64::NAN;
        }
        if self.loading_volatility <= 0.0 {
            return 0.0;
        }

        // Stress scenario: β̄ + 2σ_β (η = 2 tail reference).
        let beta_stress = self.effective_loading(mean_loading, 2.0);
        let rho_stress = beta_stress * beta_stress;

        // Extra correlation mass in the stress tail vs mean loading.
        let delta_rho = (rho_stress - correlation).max(0.0);
        // Mass of the loading-shock tail (η > 2).
        let tail_mass = 1.0 - norm_cdf(2.0);

        // Vanishes linearly in σ_β so the Gaussian (σ_β = 0) limit is exact.
        tail_mass * delta_rho * self.loading_volatility
    }

    /// Compute effective loading given mean and shock.
    ///
    /// β(η) = β̄ + σ_β · η where η ~ N(0,1)
    ///
    /// Result is clamped to the full admissible interval [0, 1].
    fn effective_loading(&self, mean_loading: f64, loading_shock: f64) -> f64 {
        let beta = mean_loading + self.loading_volatility * loading_shock;
        beta.clamp(MIN_LOADING, MAX_LOADING)
    }

    /// Second moment of `clamp(location + sigma * N(0, 1), 0, 1)`.
    fn clipped_second_moment(&self, location: f64) -> f64 {
        if location == f64::NEG_INFINITY {
            return 0.0;
        }
        if location == f64::INFINITY {
            return 1.0;
        }
        let sigma = self.loading_volatility;
        if sigma == 0.0 {
            return location.clamp(0.0, 1.0).powi(2);
        }
        let lower = -location / sigma;
        let upper = (1.0 - location) / sigma;
        // Subtract survival probabilities in the positive tail rather than
        // two CDF values rounded to one. The final term is the atom at beta=1.
        let interior_mass = if lower >= 0.0 {
            norm_cdf(-lower) - norm_cdf(-upper)
        } else {
            norm_cdf(upper) - norm_cdf(lower)
        };
        ((location * location + sigma * sigma) * interior_mass + location * sigma * norm_pdf(lower)
            - sigma * (location + 1.0) * norm_pdf(upper)
            + norm_cdf(-upper))
        .clamp(0.0, 1.0)
    }

    /// Calibrate the location to the actual clipped second moment.
    fn loading_location(&self, correlation: f64) -> f64 {
        if !correlation.is_finite() || !(0.0..=1.0).contains(&correlation) {
            return f64::NAN;
        }
        if correlation == 0.0 {
            return f64::NEG_INFINITY;
        }
        if correlation >= 1.0 {
            return f64::INFINITY;
        }
        if self.loading_volatility == 0.0 {
            return correlation.sqrt();
        }
        if let Some((cached_correlation, location)) =
            self.location_cache.lock().ok().and_then(|cache| *cache)
        {
            if cached_correlation == correlation.to_bits() {
                return location;
            }
        }

        // Forty normal standard deviations bracket all representable
        // correlation targets. Brent works in location space, with a bounded
        // scale since the loading standard deviation is at most 0.5.
        let tail = 40.0 * self.loading_volatility;
        let result = BrentSolver::new().tolerance(1e-13).solve_in_bracket(
            |location| self.clipped_second_moment(location) - correlation,
            -tail,
            1.0 + tail,
        );
        match result {
            Ok(location) => {
                if let Ok(mut cache) = self.location_cache.lock() {
                    *cache = Some((correlation.to_bits(), location));
                }
                location
            }
            Err(error) => {
                tracing::error!(%error, correlation, "RFL clipped-loading calibration failed");
                f64::NAN
            }
        }
    }

    /// Compute idiosyncratic loading given factor loading.
    ///
    /// γ = √(1 - β²) to ensure Var(Aᵢ) = 1
    fn idiosyncratic_loading(&self, factor_loading: f64) -> f64 {
        (1.0 - factor_loading * factor_loading).max(0.0).sqrt()
    }
}

impl Copula for RandomFactorLoadingCopula {
    fn conditional_default_prob(
        &self,
        default_threshold: f64,
        factor_realization: &[f64],
        correlation: f64,
    ) -> f64 {
        // Length mismatch is a programmer error. In debug, fail loudly so
        // integration tests catch it immediately — consistent with
        // GaussianCopula, StudentTCopula, and MultiFactorCopula. In release,
        // fall through with η defaulted to 0.0 (mean-loading scenario) and
        // emit a one-time tracing::warn! so the discrepancy appears in logs.
        debug_assert_eq!(
            factor_realization.len(),
            2,
            "RandomFactorLoadingCopula expects exactly 2 factors [Z, η], got {}",
            factor_realization.len()
        );
        if factor_realization.len() != 2 {
            tracing::warn!(
                expected = 2,
                actual = factor_realization.len(),
                "RandomFactorLoadingCopula: factor_realization length mismatch, defaulting missing to 0.0"
            );
        }
        // factor_realization[0] = Z (market factor)
        // factor_realization[1] = η (loading shock), optional
        let z = factor_realization.first().copied().unwrap_or(0.0);
        let eta = factor_realization.get(1).copied().unwrap_or(0.0);

        let mean_loading = self.loading_location(correlation);
        let beta = self.effective_loading(mean_loading, eta);
        let gamma = self.idiosyncratic_loading(beta);

        if beta >= 1.0 {
            return f64::from(z <= default_threshold);
        }

        // P(default | Z, β) = Φ((threshold - β·Z) / γ)
        let conditional_threshold = (default_threshold - beta * z) / gamma;
        norm_cdf(conditional_threshold.clamp(-CDF_CLIP, CDF_CLIP))
    }

    fn conditional_default_prob_given_systematic_and_mixing(
        &self,
        default_threshold: f64,
        systematic: f64,
        mixing: f64,
        correlation: f64,
    ) -> f64 {
        // For RFL the mixing variable is the shared loading shock η (drawn
        // via `sample_mixing`). The (Z, η) conditional is exactly the
        // 2-factor conditional: P(default | Z, η) = Φ((c − β(η)·Z)/√(1−β(η)²)).
        self.conditional_default_prob(default_threshold, &[systematic, mixing], correlation)
    }

    fn integrate_fn(&self, f: &dyn Fn(&[f64]) -> f64) -> f64 {
        // Double integral: outer over loading shock η, inner over market Z
        self.outer_quadrature
            .integrate(|eta| self.inner_quadrature.integrate(|z| f(&[z, eta])))
    }

    fn latent_variable(
        &self,
        systematic: f64,
        idiosyncratic: f64,
        mixing: f64,
        correlation: f64,
    ) -> f64 {
        // Andersen-Sidenius RFL latent variable, the sampling counterpart of
        // `conditional_default_prob`:
        //   β(η) = clamp(β̄ + σ_β·η),  Aᵢ = β(η)·Z + √(1−β(η)²)·εᵢ
        // with the loading shock η (= `mixing`) drawn ONCE per period via
        // `sample_mixing` and shared across the pool. Without this override
        // the trait-default Gaussian construction √ρ·Z + √(1−ρ)·εᵢ silently
        // degenerates the finite-pool MC to a plain Gaussian copula —
        // dropping the stochastic-correlation channel the quadrature engine
        // prices.
        let mean_loading = self.loading_location(correlation);
        let beta = self.effective_loading(mean_loading, mixing);
        let gamma = self.idiosyncratic_loading(beta);
        beta * systematic + gamma * idiosyncratic
    }

    fn sample_mixing(&self, u01: f64) -> finstack_quant_core::Result<f64> {
        // The shared per-period mixing variable for RFL is the loading shock
        // η ~ N(0,1): one inverse-CDF draw keeps the realization
        // deterministic and order-stable, mirroring the Student-t W draw.
        super::validate_mixing_uniform(u01)?;
        let mixing = standard_normal_inv_cdf(u01);
        if !mixing.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "RFL mixing transform must produce a finite loading shock".into(),
            ));
        }
        Ok(mixing)
    }

    fn num_factors(&self) -> usize {
        2 // Market factor Z and loading shock η
    }

    fn model_name(&self) -> &'static str {
        "Random Factor Loading Copula"
    }

    fn stress_correlation_proxy(&self, correlation: f64) -> finstack_quant_core::Result<f64> {
        let proxy = RandomFactorLoadingCopula::stress_correlation_proxy(self, correlation);
        if proxy.is_finite() {
            Ok(proxy)
        } else {
            Err(finstack_quant_core::Error::Validation(
                "RFL stress correlation requires a finite correlation in [0, 1] and a converged loading calibration".into(),
            ))
        }
    }

    fn tail_dependence(&self, correlation: f64) -> f64 {
        let location = self.loading_location(correlation);
        if self.loading_volatility == 0.0 {
            return if location.is_nan() {
                f64::NAN
            } else {
                f64::from(correlation >= 1.0)
            };
        }
        // For every beta < 1 the conditional Gaussian copula has zero
        // tail dependence. The atom at beta=1 contributes its entire mass.
        norm_cdf((location - 1.0) / self.loading_volatility)
    }
}

#[cfg(test)]
mod tests {
    use super::super::GaussianCopula;
    use super::*;
    use finstack_quant_core::math::standard_normal_inv_cdf;

    #[test]
    fn test_rfl_creation() {
        let copula = RandomFactorLoadingCopula::new(0.15);
        assert_eq!(copula.num_factors(), 2);
        assert!((copula.loading_volatility() - 0.15).abs() < 1e-10);
        assert_eq!(copula.model_name(), "Random Factor Loading Copula");
    }

    #[test]
    fn test_rfl_loading_volatility_clamped() {
        let copula_high = RandomFactorLoadingCopula::new(1.0);
        assert!(copula_high.loading_volatility() <= 0.5);

        let copula_neg = RandomFactorLoadingCopula::new(-0.1);
        assert!(copula_neg.loading_volatility() >= 0.0);
    }

    #[test]
    fn test_effective_loading_bounds() {
        let copula = RandomFactorLoadingCopula::new(0.15);

        // Even with extreme shocks, loading should stay bounded
        let loading_extreme_neg = copula.effective_loading(0.5, -10.0);
        let loading_extreme_pos = copula.effective_loading(0.5, 10.0);

        assert_eq!(loading_extreme_neg, 0.0);
        assert_eq!(loading_extreme_pos, 1.0);
    }

    #[test]
    fn test_conditional_prob_varies_with_loading_shock() {
        let copula = RandomFactorLoadingCopula::new(0.15);
        let threshold = standard_normal_inv_cdf(0.05);
        let correlation = 0.30;

        // Same market factor Z=0, different loading shocks
        let prob_low_loading = copula.conditional_default_prob(
            threshold,
            &[0.0, -2.0], // Low loading (η = -2)
            correlation,
        );
        let prob_mean_loading = copula.conditional_default_prob(
            threshold,
            &[0.0, 0.0], // Mean loading
            correlation,
        );
        let prob_high_loading = copula.conditional_default_prob(
            threshold,
            &[0.0, 2.0], // High loading (η = +2)
            correlation,
        );

        // All should be around the unconditional probability
        // (loading shock mainly affects joint behavior, not individual marginal)
        assert!(prob_low_loading > 0.0 && prob_low_loading < 1.0);
        assert!(prob_mean_loading > 0.0 && prob_mean_loading < 1.0);
        assert!(prob_high_loading > 0.0 && prob_high_loading < 1.0);
    }

    #[test]
    fn test_integration_recovers_unconditional() {
        let copula = RandomFactorLoadingCopula::new(0.15);
        let pd = 0.05;
        let threshold = standard_normal_inv_cdf(pd);
        let correlation = 0.30;

        // E[P(default|Z,η)] should equal P(default)
        let integrated_prob = copula.integrate_fn(&|factors| {
            copula.conditional_default_prob(threshold, factors, correlation)
        });

        assert!(
            (integrated_prob - pd).abs() < 0.01,
            "Integrated probability {} should be close to unconditional {}",
            integrated_prob,
            pd
        );
    }

    #[test]
    fn test_stress_proxy_small_but_positive() {
        let copula = RandomFactorLoadingCopula::new(0.15);
        let lambda = copula.stress_correlation_proxy(0.5);

        // RFL stress proxy is small and non-negative.
        assert!(lambda >= 0.0);
        assert!(lambda < 0.1);
    }

    #[test]
    fn test_stress_proxy_zero_loading_vol_is_gaussian_limit() {
        let copula = RandomFactorLoadingCopula::new(0.0);
        assert_eq!(copula.stress_correlation_proxy(0.0), 0.0);
        assert_eq!(copula.stress_correlation_proxy(0.5), 0.0);
        assert_eq!(copula.stress_correlation_proxy(0.99), 0.0);
    }

    #[test]
    fn tail_dependence_tracks_the_perfect_loading_atom() {
        let copula = RandomFactorLoadingCopula::new(0.5);
        assert_eq!(copula.tail_dependence(0.0), 0.0);
        assert_eq!(copula.tail_dependence(1.0), 1.0);
        let rho = 0.9;
        let location = copula.loading_location(rho);
        let eta_at_perfect_loading = (1.0 - location) / copula.loading_volatility();
        assert_eq!(
            copula.latent_variable(1.0, 0.0, eta_at_perfect_loading + 1e-8, rho),
            1.0
        );
        assert!(copula.latent_variable(1.0, 0.0, eta_at_perfect_loading - 1e-8, rho) < 1.0);
        assert!((copula.tail_dependence(rho) - norm_cdf(-eta_at_perfect_loading)).abs() < 1e-14);

        let zero_vol = RandomFactorLoadingCopula::new(0.0);
        assert_eq!(zero_vol.tail_dependence(0.5), 0.0);
        assert_eq!(zero_vol.tail_dependence(1.0), 1.0);
    }

    #[test]
    fn loading_calibration_preserves_correlations_below_the_loading_variance() {
        let copula = RandomFactorLoadingCopula::new(0.5);
        for rho in [1e-10, 1e-6, 0.001, 0.01, 0.04] {
            assert!((copula.realized_correlation(rho) - rho).abs() < 1e-12);
        }
        assert!(copula.loading_location(0.01) < 0.0);
    }

    /// W-49: `RandomFactorLoadingCopula` must panic in debug builds (via
    /// `debug_assert_eq!`) when the caller passes a factor vector of the wrong
    /// length, consistent with `GaussianCopula`, `StudentTCopula`, and
    /// `MultiFactorCopula`.  Before this fix, only a `tracing::warn!` was
    /// emitted and `η` silently defaulted to `0.0`, providing no test-time signal.
    #[test]
    fn test_factor_length_mismatch_panics_in_debug() {
        let copula = RandomFactorLoadingCopula::new(0.15);
        let threshold = standard_normal_inv_cdf(0.05);
        let correlation = 0.30;

        let assert_contract = |factors: &[f64]| {
            if cfg!(debug_assertions) {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    copula.conditional_default_prob(threshold, factors, correlation)
                }));
                assert!(
                    outcome.is_err(),
                    "debug build should panic on factor length mismatch for {:?}",
                    factors
                );
            }
            // Release build: no panic contract tested here (warn-only).
        };

        // A 1-element vector is the most likely caller mistake (Z passed
        // without η, analogous to passing a Gaussian factor to the RFL copula).
        assert_contract(&[0.0]);
        // Empty vector also exercises the guard.
        assert_contract(&[]);
    }

    #[test]
    fn test_realized_correlation_matches_input() {
        use finstack_quant_core::math::integration::adaptive_simpson;

        // Integrate the loading that the production latent-variable kernel
        // actually uses, independently of its analytic calibration objective.
        for sigma in [0.0, 0.15, 0.5] {
            let copula = RandomFactorLoadingCopula::new(sigma);
            for rho in [0.0, 0.01, 0.04, 0.30, 0.90, 0.999, 1.0] {
                let realized = adaptive_simpson(
                    |eta| copula.latent_variable(1.0, 0.0, eta, rho).powi(2) * norm_pdf(eta),
                    -10.0,
                    10.0,
                    1e-11,
                    24,
                )
                .expect("bounded loading moment integral");
                assert!(
                    (realized - rho).abs() < 1e-9,
                    "σ_β={sigma}, ρ={rho}: actual E[β²]={realized}"
                );
                assert!((copula.realized_correlation(rho) - realized).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn conditional_default_prob_matches_latent_loading_including_atoms() {
        let copula = RandomFactorLoadingCopula::new(0.5);
        let threshold = -0.5;
        let z = -0.25;
        for rho in [0.0, 0.01, 0.3, 0.9, 1.0] {
            for eta in [-10.0, -1.0, 0.0, 1.0, 10.0] {
                let beta = copula.latent_variable(1.0, 0.0, eta, rho);
                let expected = if beta == 1.0 {
                    f64::from(z <= threshold)
                } else {
                    norm_cdf((threshold - beta * z) / (1.0 - beta * beta).sqrt())
                };
                let actual = copula
                    .conditional_default_prob_checked(threshold, &[z, eta], rho)
                    .expect("valid conditional inputs");
                assert!((actual - expected).abs() < 1e-14);
            }
        }
    }

    #[test]
    fn test_zero_volatility_equals_gaussian() {
        let rfl_copula = RandomFactorLoadingCopula::new(0.0);
        let gaussian_copula = GaussianCopula::new();

        let threshold = standard_normal_inv_cdf(0.05);
        let correlation = 0.30;

        // With zero loading vol, RFL should behave like Gaussian
        // (when only passing market factor)
        let rfl_prob = rfl_copula.conditional_default_prob(
            threshold,
            &[0.5, 0.0], // Z=0.5, η=0 (no loading shock)
            correlation,
        );
        let gaussian_prob =
            gaussian_copula.conditional_default_prob(threshold, &[0.5], correlation);

        assert!(
            (rfl_prob - gaussian_prob).abs() < 0.01,
            "Zero-vol RFL {} should equal Gaussian {}",
            rfl_prob,
            gaussian_prob
        );
    }
}
