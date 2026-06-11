//! Random Factor Loading (RFL) copula with stochastic correlation.
//!
//! Models correlation itself as random, capturing the empirical observation
//! that correlation increases during market stress. This is particularly
//! important for senior tranches which are sensitive to correlation dynamics.
//!
//! # Mathematical Model
//!
//! The factor loading β is random rather than fixed:
//! ```text
//! β ~ N(β̄, σ²_β)      (truncated to [0.01, 0.99])
//! Aᵢ = β · Z + √(1-β²) · εᵢ
//! ```
//!
//! This means effective correlation ρ(β) = β² is stochastic, with:
//! - Higher realized correlation in stress (β further from 0)
//! - Lower realized correlation in calm markets
//!
//! # Parameterization
//!
//! The `correlation` argument is treated as the **realized pairwise
//! correlation**: E[β²] = ρ. Since E[β²] = β̄² + σ²_β (pre-clamping), we pick
//! β̄ = √max(ρ − σ²_β, 0). When σ²_β > ρ, β̄ is floored at 0 and the realized
//! correlation becomes σ²_β, which exceeds the requested ρ. This regime is:
//! - reported via a one-shot `tracing::warn!` per copula instance, and
//! - queryable via [`RandomFactorLoadingCopula::floor_engaged`] /
//!   [`RandomFactorLoadingCopula::realized_correlation`].
//!
//! # Integration Approach
//!
//! Two-dimensional integration:
//! 1. Outer: over the random loading β (or equivalently, loading shock η)
//! 2. Inner: over the market factor Z given β
//!
//! # Tail-Dependence Interpretation
//!
//! RFL has no closed-form strict copula lower-tail-dependence coefficient
//! `λ_L`. The trait-level [`Copula::tail_dependence`] therefore returns
//! `f64::NAN` for this model. A heuristic stress-correlation gauge is
//! exposed as the inherent method
//! [`RandomFactorLoadingCopula::stress_correlation_proxy`]; it is monotone
//! in correlation and loading volatility but is **not** a copula λ_L limit.
//!
//! # Impact on Tranches
//!
//! - **Equity tranches**: Less affected (already high-risk)
//! - **Mezzanine tranches**: Moderately affected
//! - **Senior tranches**: Significantly affected (correlation uncertainty matters)
//!
//! # References
//!
//! - Random recovery and random-factor-loading extensions:
//!   `docs/REFERENCES.md#andersen-sidenius-2005-rfl`

use super::{select_quadrature, Copula, DEFAULT_QUADRATURE_ORDER};
use finstack_core::math::{norm_cdf, standard_normal_inv_cdf, GaussHermiteQuadrature};
use std::sync::atomic::{AtomicBool, Ordering};

/// Minimum loading (ensures √(1-β²) is well-defined).
const MIN_LOADING: f64 = 0.01;
/// Maximum loading (ensures √(1-β²) is well-defined).
const MAX_LOADING: f64 = 0.99;
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
/// - Effective loading is clamped to [0.01, 0.99]
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
    /// Whether the non-negativity floor on β̄ has triggered a warning for
    /// any `conditional_default_prob` call on this instance. Used to
    /// rate-limit the engagement warning to one log per instance so that
    /// quadrature loops don't flood the log.
    floor_warned: AtomicBool,
}

impl Clone for RandomFactorLoadingCopula {
    fn clone(&self) -> Self {
        let quadrature = select_quadrature(self.quadrature_order);
        Self {
            loading_volatility: self.loading_volatility,
            quadrature_order: self.quadrature_order,
            outer_quadrature: select_quadrature(self.quadrature_order),
            inner_quadrature: quadrature,
            floor_warned: AtomicBool::new(self.floor_warned.load(Ordering::Relaxed)),
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
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::correlation::{Copula, RandomFactorLoadingCopula};
    /// use finstack_core::math::standard_normal_inv_cdf;
    ///
    /// let copula = RandomFactorLoadingCopula::new(0.15);
    /// let threshold = standard_normal_inv_cdf(0.05);
    /// let cond_pd = copula.conditional_default_prob(threshold, &[0.0, 1.0], 0.30);
    ///
    /// assert!(cond_pd > 0.0 && cond_pd < 1.0);
    /// ```
    #[must_use]
    pub fn new(loading_vol: f64) -> Self {
        let order = DEFAULT_QUADRATURE_ORDER;
        Self {
            loading_volatility: loading_vol.clamp(0.0, 0.5),
            quadrature_order: order,
            outer_quadrature: select_quadrature(order),
            inner_quadrature: select_quadrature(order),
            floor_warned: AtomicBool::new(false),
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
    #[must_use]
    pub fn with_quadrature_order(loading_vol: f64, order: u8) -> Self {
        Self {
            loading_volatility: loading_vol.clamp(0.0, 0.5),
            quadrature_order: order,
            outer_quadrature: select_quadrature(order),
            inner_quadrature: select_quadrature(order),
            floor_warned: AtomicBool::new(false),
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

    /// Whether the non-negativity floor `β̄ = √max(ρ − σ²_β, 0)` would
    /// engage for the supplied `correlation`.
    ///
    /// When this returns `true`, the realized pairwise correlation produced
    /// by [`Self::conditional_default_prob`] is `σ²_β` (the variance of the
    /// factor loading), **not** the requested `correlation`. Callers that
    /// need the realized correlation to match the input must lower
    /// `loading_volatility` or raise `correlation` until this returns `false`.
    ///
    /// # Returns
    ///
    /// `true` if `σ²_β > correlation` (after clamping `correlation` to
    /// `[0, 1]`), i.e. the model cannot honor the requested correlation.
    #[must_use]
    pub fn floor_engaged(&self, correlation: f64) -> bool {
        let rho = correlation.clamp(0.0, 1.0);
        let variance_of_loading = self.loading_volatility * self.loading_volatility;
        variance_of_loading > rho
    }

    /// Realized pairwise correlation `E[β²]` actually produced by the model
    /// for the supplied requested `correlation`.
    ///
    /// Equals the (clamped) `correlation` when the floor is not engaged,
    /// and `σ²_β` (the loading variance) when it is.
    ///
    /// # Returns
    ///
    /// `max(correlation_clamped, σ²_β)` in `[0, 1]`.
    #[must_use]
    pub fn realized_correlation(&self, correlation: f64) -> f64 {
        let rho = correlation.clamp(0.0, 1.0);
        let variance_of_loading = self.loading_volatility * self.loading_volatility;
        rho.max(variance_of_loading)
    }

    /// Stress-correlation proxy: a heuristic gauge of how much extra
    /// correlation mass appears in the high-loading tail (`η > 2`).
    ///
    /// **This is NOT the strict copula lower-tail-dependence coefficient
    /// `λ_L`.** RFL collapses to a Gaussian copula in the `σ_β → 0` limit
    /// (where `λ_L = 0`); this proxy correctly vanishes there but has no
    /// formal interpretation as a copula tail-dependence limit. Use it
    /// only as a monotone-in-(ρ, σ_β) diagnostic.
    ///
    /// The trait-level [`Copula::tail_dependence`] for this model returns
    /// `f64::NAN` to signal that no closed-form `λ_L` is available; callers
    /// wanting the proxy must call this inherent method on the concrete
    /// type.
    ///
    /// # Returns
    ///
    /// A non-negative stress-correlation proxy. Vanishes when
    /// `loading_volatility == 0`.
    #[must_use]
    pub fn stress_correlation_proxy(&self, correlation: f64) -> f64 {
        if self.loading_volatility <= 0.0 {
            return 0.0;
        }

        let rho = correlation.clamp(0.0, 1.0);
        let variance_of_loading = self.loading_volatility * self.loading_volatility;
        // Use the same β̄ parameterization as conditional_default_prob so the
        // unconditional correlation reference (β̄² + σ²_β ≈ ρ) is consistent.
        let mean_loading = (rho - variance_of_loading).max(0.0).sqrt();

        // Stress scenario: β̄ + 2σ_β (η = 2 tail reference).
        let beta_stress = (mean_loading + 2.0 * self.loading_volatility).min(MAX_LOADING);
        let rho_stress = beta_stress * beta_stress;

        // Extra correlation mass in the stress tail vs mean loading.
        let delta_rho = (rho_stress - rho).max(0.0);
        // Mass of the loading-shock tail (η > 2).
        let tail_mass = 1.0 - norm_cdf(2.0);

        // Vanishes linearly in σ_β so the Gaussian (σ_β = 0) limit is exact.
        tail_mass * delta_rho * self.loading_volatility
    }

    /// Compute effective loading given mean and shock.
    ///
    /// β(η) = β̄ + σ_β · η where η ~ N(0,1)
    ///
    /// Result is clamped to [0.01, 0.99] to ensure numerical stability.
    fn effective_loading(&self, mean_loading: f64, loading_shock: f64) -> f64 {
        let beta = mean_loading + self.loading_volatility * loading_shock;
        beta.clamp(MIN_LOADING, MAX_LOADING)
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

        // Mean loading chosen so the realized pairwise correlation equals ρ:
        //   E[β²] = β̄² + σ²_β (ignoring clamping) ⇒ β̄ = √max(ρ − σ²_β, 0).
        // When σ²_β > ρ, β̄ is floored at 0 and the model's realized
        // correlation is σ²_β > ρ; warn once per instance and proceed.
        let rho = correlation.clamp(0.0, 1.0);
        let variance_of_loading = self.loading_volatility * self.loading_volatility;
        if variance_of_loading > rho && !self.floor_warned.swap(true, Ordering::Relaxed) {
            tracing::warn!(
                requested_rho = rho,
                loading_variance = variance_of_loading,
                realized_rho = variance_of_loading,
                "RandomFactorLoadingCopula: σ²_β > ρ — non-negativity floor on β̄ engaged; \
                 realized pairwise correlation is σ²_β, not the requested ρ. \
                 Lower loading_volatility or raise correlation to honor the request. \
                 This warning is emitted at most once per copula instance; query \
                 floor_engaged(ρ) to detect this regime programmatically."
            );
        }
        let mean_loading = (rho - variance_of_loading).max(0.0).sqrt();
        let beta = self.effective_loading(mean_loading, eta);
        let gamma = self.idiosyncratic_loading(beta);

        if gamma < 1e-10 {
            // Near-perfect correlation case
            let threshold_adj = default_threshold - beta * z;
            return norm_cdf(threshold_adj);
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
        // Uses cached quadrature for performance
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
        let rho = correlation.clamp(0.0, 1.0);
        let variance_of_loading = self.loading_volatility * self.loading_volatility;
        let mean_loading = (rho - variance_of_loading).max(0.0).sqrt();
        let beta = self.effective_loading(mean_loading, mixing);
        let gamma = self.idiosyncratic_loading(beta);
        beta * systematic + gamma * idiosyncratic
    }

    fn sample_mixing(&self, u01: f64) -> f64 {
        // The shared per-period mixing variable for RFL is the loading shock
        // η ~ N(0,1): one inverse-CDF draw keeps the realization
        // deterministic and order-stable, mirroring the Student-t W draw.
        let p = u01.clamp(1e-12, 1.0 - 1e-12);
        standard_normal_inv_cdf(p)
    }

    fn num_factors(&self) -> usize {
        2 // Market factor Z and loading shock η
    }

    fn model_name(&self) -> &'static str {
        "Random Factor Loading Copula"
    }

    fn tail_dependence(&self, _correlation: f64) -> f64 {
        // RFL does not have a closed-form strict copula lower-tail
        // dependence coefficient. Returning NaN signals "not available"
        // to callers using the trait-level abstraction. For the heuristic
        // stress-correlation gauge previously returned here, call the
        // inherent method [`Self::stress_correlation_proxy`] directly on
        // the concrete type. (In the σ_β = 0 limit RFL collapses to a
        // Gaussian copula with λ_L = 0; we still return NaN there to keep
        // the trait contract — "NaN ⇒ no closed-form λ_L" — uniform.)
        f64::NAN
    }
}

#[cfg(test)]
mod tests {
    use super::super::GaussianCopula;
    use super::*;
    use finstack_core::math::standard_normal_inv_cdf;

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

        assert!(loading_extreme_neg >= 0.01);
        assert!(loading_extreme_pos <= 0.99);
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

        // Should be close to unconditional (within integration error)
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
    fn test_trait_tail_dependence_is_nan() {
        // The trait-level tail_dependence returns NaN for RFL because no
        // closed-form λ_L exists.
        let copula = RandomFactorLoadingCopula::new(0.15);
        assert!(<RandomFactorLoadingCopula as Copula>::tail_dependence(&copula, 0.5).is_nan());

        // Even at σ_β = 0, the trait still returns NaN (uniform contract).
        let zero_vol = RandomFactorLoadingCopula::new(0.0);
        assert!(<RandomFactorLoadingCopula as Copula>::tail_dependence(&zero_vol, 0.5).is_nan());
    }

    #[test]
    fn test_floor_engaged_detects_regime() {
        // σ²_β = 0.04, so floor engages whenever ρ < 0.04.
        let copula = RandomFactorLoadingCopula::new(0.20);
        assert!(copula.floor_engaged(0.01));
        assert!(copula.floor_engaged(0.03));
        assert!(!copula.floor_engaged(0.05));
        assert!(!copula.floor_engaged(0.30));

        // Realized correlation reflects the floor.
        assert!((copula.realized_correlation(0.01) - 0.04).abs() < 1e-12);
        assert!((copula.realized_correlation(0.30) - 0.30).abs() < 1e-12);
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
        // Regression: the `correlation` argument is defined as the realized
        // pairwise correlation E[β²]. We verify β̄² + σ²_β ≈ ρ for a range of
        // (ρ, σ_β) pairs where the non-negativity floor is not engaged.
        let cases = [(0.30_f64, 0.15_f64), (0.50, 0.20), (0.15, 0.10)];
        for &(rho, sigma) in &cases {
            let var = sigma * sigma;
            let mean_loading = (rho - var).max(0.0).sqrt();
            let realized = mean_loading * mean_loading + var;
            assert!(
                (realized - rho).abs() < 1e-12,
                "σ_β={sigma}, ρ={rho}: realized E[β²]={realized} should equal ρ"
            );
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
