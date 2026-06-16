//! Multi-factor Gaussian copula: global + one shared sector factor.
//!
//! # Implementation Scope
//!
//! Two complementary APIs are provided:
//!
//! 1. **[`Copula::conditional_default_prob`]** (the trait method): integrates
//!    over a single `(Z_G, Z_S)` pair shared by every name in the portfolio.
//!    Mathematically equivalent to a reparameterized two-factor Gaussian
//!    copula in which every pair of entities exhibits the same pairwise
//!    correlation `ρᵢⱼ = β_G² + β_S²`. Use this for single-sector pricing
//!    or as a fast approximation when sector resolution is not needed.
//!
//! 2. **[`MultiFactorCopula::conditional_default_prob_with_sector`]** (inherent
//!    method): per-name sector routing using an explicit `sector_idx`:
//!    - `sector_idx == 0` — name is **cross-sector only** (sector loading
//!      zeroed, `ρᵢⱼ = β_G²`).
//!    - `sector_idx ∈ [1, num_sectors]` — picks the corresponding slot of
//!      `factor_realization` as the sector shock; idiosyncratic loading is
//!      `γᵢ = √(1 − β_G² − β_S²)`.
//!    - `sector_idx > num_sectors` — out-of-range falls back to zero sector
//!      shock without panicking (treated like `sector_idx == 0`).
//!    - Length mismatches between `factor_realization` and `num_factors_count`
//!      trigger `debug_assert_eq!`; in release builds, return the unconditional
//!      PD `Φ(c)` (a structured fallback rather than a biased zeroed-shock).
//!
//! Use `Copula::num_factors == 2` as a signal that the caller is supplying
//! `[Z_G, Z_S]`. Pricers that need per-name sector resolution should call the
//! inherent method directly rather than the trait method.
//!
//! # Mathematical Model
//!
//! Latent variable for entity i with sector s(i):
//! ```text
//! Aᵢ = β_G · Z_G + β_S(i) · Z_S(i) + γᵢ · εᵢ
//! ```
//!
//! where:
//! - Z_G ~ N(0,1) is the global systematic factor
//! - Z_S ~ N(0,1) are sector-specific factors (independent of Z_G)
//! - εᵢ ~ N(0,1) is the idiosyncratic factor
//! - β_G is the global loading, β_S is the sector loading
//! - γᵢ = √(1 - β_G² - β_S²) is the idiosyncratic loading
//!
//! # Realized Correlation Structure
//!
//! Per-name sector is resolved by `conditional_default_prob_with_sector`:
//!
//! ```text
//! ρᵢⱼ = β_G² + β_S² · 1{s(i)=s(j) ≠ 0}  (same non-zero sector)
//! ρᵢⱼ = β_G²                              (different sectors, or sector_idx = 0)
//! ```
//!
//! The single-shared-factor trait method (`Copula::conditional_default_prob`)
//! prices every name as if it belonged to the same sector — the intra-sector
//! correlation is then realized for all pairs.
//!
//! # Use Cases
//!
//! - Single-sector pricing via the trait method (fast path)
//! - Per-name sector resolution via `conditional_default_prob_with_sector`
//!   for bespoke CDOs and sector-aware basket pricing
//! - Two-factor sensitivity analysis on global vs. sector loadings
//!
//! # References
//!
//! - Multi-factor basket and bespoke CDO modeling:
//!   `docs/REFERENCES.md#andersen-sidenius-basu-2003`
//! - Analytical correlation-product valuation:
//!   `docs/REFERENCES.md#hull-white-2004-cdo`

use super::{select_quadrature, Copula};
use finstack_quant_core::math::{norm_cdf, GaussHermiteQuadrature};

/// CDF argument clipping to prevent overflow.
const CDF_CLIP: f64 = 10.0;
/// Default quadrature order for multi-dimensional integration.
const MULTI_FACTOR_QUADRATURE_ORDER: u8 = 10;

/// Multi-factor Gaussian copula with sector structure.
///
/// Uses a global factor plus one sector-specific factor to model
/// intra-sector vs. inter-sector correlation differences.
///
/// # Factor Limit
///
/// Supports 1 to 5 total factors: one global factor plus up to four sector
/// factors. Beyond that cap, Monte Carlo integration is more appropriate than
/// nested quadrature.
///
/// # Default Parameters
///
/// - Global loading: 0.4 (gives ~16% inter-sector correlation)
/// - Sector loading: 0.3 (gives ~25% additional intra-sector correlation)
/// - Sector fraction: 0.4 (40% of total correlation from sector factor)
/// - Quadrature order: 10 (better accuracy while remaining cheap for two factors)
///
/// # References
///
/// - `docs/REFERENCES.md#andersen-sidenius-basu-2003`
/// - `docs/REFERENCES.md#hull-white-2004-cdo`
pub struct MultiFactorCopula {
    /// Number of systematic factors (1 to 5, capped)
    num_factors_count: usize,
    /// Global factor loading (default for all entities)
    default_global_loading: f64,
    /// Sector factor loading (default for all entities)
    default_sector_loading: f64,
    /// Fraction of total correlation attributed to sector factor in decompose_correlation
    sector_fraction: f64,
    /// Cached quadrature for integration
    quadrature: GaussHermiteQuadrature,
}

impl Clone for MultiFactorCopula {
    fn clone(&self) -> Self {
        // Preserve the configured quadrature order (recover it from the
        // stored node count) — rebuilding at the fixed internal default
        // would silently downgrade copulas built via `with_quadrature_order`.
        let order =
            u8::try_from(self.quadrature.points.len()).unwrap_or(MULTI_FACTOR_QUADRATURE_ORDER);
        Self {
            num_factors_count: self.num_factors_count,
            default_global_loading: self.default_global_loading,
            default_sector_loading: self.default_sector_loading,
            sector_fraction: self.sector_fraction,
            quadrature: select_quadrature(order),
        }
    }
}

impl std::fmt::Debug for MultiFactorCopula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiFactorCopula")
            .field("num_factors_count", &self.num_factors_count)
            .field("default_global_loading", &self.default_global_loading)
            .field("default_sector_loading", &self.default_sector_loading)
            .field("sector_fraction", &self.sector_fraction)
            .finish()
    }
}

/// Maximum supported total factors (global + sector slots).
///
/// The cap of 5 (1 global + up to 4 sector factors) covers standard
/// bespoke CDO structures (e.g. financials / industrials / consumer /
/// sovereign + global) while keeping the nested Gauss-Hermite
/// quadrature cost bounded at `n_q^{num_factors}` (10^5 = 100 000
/// evaluations at the default order). Beyond 5 factors the quadrature
/// surface becomes too expensive and callers should switch to Monte
/// Carlo integration.
const MAX_FACTORS: usize = 5;

/// Maximum number of sector factors (excluding the global factor).
const MAX_SECTORS: usize = MAX_FACTORS - 1;

impl MultiFactorCopula {
    /// Create a multi-factor copula with specified number of factors.
    ///
    /// Uses default loadings: β_G=0.4, β_S=0.3, sector_fraction=0.4
    ///
    /// # Arguments
    /// * `num_factors` - Number of factors (1 to 5; capped at `MAX_FACTORS`)
    ///
    /// # Returns
    ///
    /// A multi-factor Gaussian copula with default loadings.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::correlation::{Copula, MultiFactorCopula};
    ///
    /// let copula = MultiFactorCopula::new(2);
    /// assert_eq!(copula.num_factors(), 2);
    /// assert!(copula.intra_sector_correlation() >= copula.inter_sector_correlation());
    /// ```
    #[must_use]
    pub fn new(num_factors: usize) -> Self {
        Self::with_quadrature_order(num_factors, MULTI_FACTOR_QUADRATURE_ORDER)
    }

    /// Create a multi-factor copula with an explicit per-dimension quadrature
    /// order.
    ///
    /// Integration cost is `order^{num_factors}`, so callers that configure a
    /// pricer-level `quadrature_order` should pass it through here rather
    /// than silently getting the fixed internal default — but be aware that
    /// orders tuned for 1-dimensional copulas (e.g. 20) are expensive in
    /// 4-5 dimensions.
    ///
    /// # Arguments
    /// * `num_factors` - Number of factors (1 to 5; capped at `MAX_FACTORS`)
    /// * `quadrature_order` - Gauss-Hermite points per dimension
    #[must_use]
    pub fn with_quadrature_order(num_factors: usize, quadrature_order: u8) -> Self {
        let num_factors = num_factors.clamp(1, MAX_FACTORS);
        Self {
            num_factors_count: num_factors,
            default_global_loading: 0.4,
            default_sector_loading: 0.3,
            sector_fraction: 0.4,
            quadrature: select_quadrature(quadrature_order),
        }
    }

    /// Create with custom loadings.
    ///
    /// Loadings are clamped to ensure β_G² + β_S² ≤ 1 (valid variance).
    ///
    /// # Note — loadings are descriptive parameters
    ///
    /// The stored default loadings parameterize the copula's *reporting*
    /// accessors ([`Self::inter_sector_correlation`],
    /// [`Self::intra_sector_correlation`]). The conditional-PD pricing path
    /// ([`Copula::conditional_default_prob`]) derives its loadings from the
    /// **correlation argument** supplied per call (typically base
    /// correlation) via [`Self::decompose_correlation`] with this copula's
    /// `sector_fraction` — it does not read the default loadings. Use
    /// [`Self::with_loadings_and_sector_fraction`] to control how the
    /// pricing correlation splits between global and sector factors.
    ///
    /// # Arguments
    /// * `num_factors` - Number of factors (1 to 5; capped at `MAX_FACTORS`)
    /// * `global_loading` - Loading on global factor (β_G), clamped to [0, 0.99]
    /// * `sector_loading` - Loading on sector factor (β_S), clamped to maintain variance constraint
    ///
    /// # Returns
    ///
    /// A multi-factor Gaussian copula with bounded loadings.
    #[must_use]
    pub fn with_loadings(num_factors: usize, global_loading: f64, sector_loading: f64) -> Self {
        let gl = global_loading.clamp(0.0, 0.99);
        let max_sector = (1.0 - gl * gl).sqrt();
        let sl = sector_loading.clamp(0.0, max_sector * 0.99);

        Self {
            num_factors_count: num_factors.clamp(1, MAX_FACTORS),
            default_global_loading: gl,
            default_sector_loading: sl,
            sector_fraction: 0.4,
            quadrature: select_quadrature(MULTI_FACTOR_QUADRATURE_ORDER),
        }
    }

    /// Create with custom loadings and sector fraction.
    ///
    /// # Arguments
    /// * `num_factors` - Number of factors (1 or 2; capped at 2)
    /// * `global_loading` - Loading on global factor (β_G), clamped to [0, 0.99]
    /// * `sector_loading` - Loading on sector factor (β_S), clamped to maintain variance constraint
    /// * `sector_fraction` - Fraction of total correlation from sector factor, clamped to [0, 1]
    ///
    /// # Returns
    ///
    /// A multi-factor Gaussian copula with explicit sector-fraction decomposition.
    #[must_use]
    pub fn with_loadings_and_sector_fraction(
        num_factors: usize,
        global_loading: f64,
        sector_loading: f64,
        sector_fraction: f64,
    ) -> Self {
        let mut copula = Self::with_loadings(num_factors, global_loading, sector_loading);
        copula.sector_fraction = sector_fraction.clamp(0.0, 1.0);
        copula
    }

    /// Create a `K`-sector copula with a shared global loading and a
    /// shared sector loading. The total number of factors is `1 + K`.
    ///
    /// Names are identified by a sector index (`sector_idx`) passed to
    /// [`Copula::conditional_default_prob_with_sector`]:
    ///
    /// * `sector_idx = 0` → cross-sector name (global factor only, no
    ///   sector shock).
    /// * `sector_idx = k ∈ [1, K]` → the `k`-th sector factor from the
    ///   realization vector is used alongside the global factor.
    ///
    /// `num_sectors` is clamped to `[1, MAX_SECTORS]` (currently 4);
    /// larger portfolios should switch to Monte Carlo integration.
    ///
    /// Constructor for a true multi-sector Gaussian copula with `K`
    /// independent sector factors.
    #[must_use]
    pub fn with_k_sectors(num_sectors: usize, global_loading: f64, sector_loading: f64) -> Self {
        let k = num_sectors.clamp(1, MAX_SECTORS);
        let gl = global_loading.clamp(0.0, 0.99);
        let max_sector = (1.0 - gl * gl).sqrt();
        let sl = sector_loading.clamp(0.0, max_sector * 0.99);

        Self {
            num_factors_count: 1 + k,
            default_global_loading: gl,
            default_sector_loading: sl,
            sector_fraction: 0.4,
            quadrature: select_quadrature(MULTI_FACTOR_QUADRATURE_ORDER),
        }
    }

    /// Number of sector factors (excluding the global factor).
    ///
    /// Returns `num_factors_count - 1`. Valid `sector_idx` values
    /// passed to [`Copula::conditional_default_prob_with_sector`] are
    /// `1..=num_sectors()`, plus `0` for the cross-sector case.
    #[must_use]
    pub fn num_sectors(&self) -> usize {
        self.num_factors_count.saturating_sub(1)
    }

    /// Get the parameter-level inter-sector correlation (β_G²).
    ///
    /// Returns the correlation `β_G²` that would apply to a pair of names in
    /// different sectors under the intended multi-factor model. See the module
    /// documentation: this implementation does not currently resolve per-name
    /// sector assignments, so every simulated pair behaves as
    /// [`Self::intra_sector_correlation`] regardless of this value.
    ///
    /// # Returns
    ///
    /// The implied *parameter* correlation for cross-sector pairs, `β_G²`.
    #[must_use]
    pub fn inter_sector_correlation(&self) -> f64 {
        self.default_global_loading * self.default_global_loading
    }

    /// Get the parameter-level intra-sector correlation (β_G² + β_S²).
    ///
    /// This is the realized pairwise correlation produced by the current
    /// implementation for *every* pair of names (see module docs — sector
    /// resolution is not wired through the [`Copula`] trait yet).
    ///
    /// # Returns
    ///
    /// The implied correlation between names in the same sector, `β_G² + β_S²`.
    #[must_use]
    pub fn intra_sector_correlation(&self) -> f64 {
        let gl = self.default_global_loading;
        let sl = self.default_sector_loading;
        gl * gl + sl * sl
    }

    /// Compute idiosyncratic loading given factor loadings.
    ///
    /// γ = √(1 - β_G² - β_S²) to ensure Var(Aᵢ) = 1
    fn idiosyncratic_loading(&self, global_loading: f64, sector_loading: f64) -> f64 {
        let sum_sq = global_loading * global_loading + sector_loading * sector_loading;
        (1.0 - sum_sq).max(0.0).sqrt()
    }

    /// Recursive nested Gauss-Hermite integration over all
    /// `num_factors_count` systematic factors.
    ///
    /// `scratch` is the current factor vector being filled in; `depth`
    /// is the index of the next slot to integrate over. When `depth ==
    /// num_factors_count` the scratch vector is complete and `f` is
    /// evaluated at that point.
    ///
    /// The quadrature stored on the copula follows the physicists'
    /// convention (weight `e^{-z²}`). To compute a standard-normal
    /// expectation at each dimension we apply the transform
    /// `x = √2 · z` and divide by `√π`, matching
    /// [`GaussHermiteQuadrature::integrate`]'s normalization. The
    /// per-dimension `1/√π` factor composes multiplicatively across
    /// `num_factors_count` nested integrals, so the final result is the
    /// multivariate expectation `E[f(Z)]` for `Z ~ N(0, I_d)`.
    ///
    /// Generalizes the earlier hand-rolled 1- and 2-dimensional
    /// integration paths to arbitrary `K`.
    fn integrate_recursive(
        &self,
        f: &dyn Fn(&[f64]) -> f64,
        scratch: &mut [f64],
        depth: usize,
    ) -> f64 {
        const SQRT_2: f64 = std::f64::consts::SQRT_2;
        if depth == scratch.len() {
            return f(scratch);
        }
        // Collect nodes so we can mutate `scratch` inside the loop
        // without the quadrature driver's closure keeping a borrow.
        let nodes: Vec<(f64, f64)> = self
            .quadrature
            .points
            .iter()
            .zip(self.quadrature.weights.iter())
            .map(|(&z, &w)| (SQRT_2 * z, w))
            .collect();
        let mut acc = 0.0_f64;
        for (x, w) in nodes {
            scratch[depth] = x;
            acc += w * self.integrate_recursive(f, scratch, depth + 1);
        }
        acc / std::f64::consts::PI.sqrt()
    }

    /// Decompose total correlation into global and sector components.
    ///
    /// Given total correlation ρ and sector fraction f:
    /// - β_G² = ρ · (1 - f)
    /// - β_S² = ρ · f
    ///
    /// # Arguments
    /// * `total_correlation` - Total correlation, clamped to [0, 0.99]
    /// * `sector_fraction` - Fraction of correlation from sector factor, clamped to [0, 1]
    ///
    /// # Returns
    ///
    /// A pair `(global_loading, sector_loading)` whose squared values reconstruct
    /// the bounded total correlation.
    #[must_use]
    pub fn decompose_correlation(
        &self,
        total_correlation: f64,
        sector_fraction: f64,
    ) -> (f64, f64) {
        let rho = total_correlation.clamp(0.0, 0.99);
        let f = sector_fraction.clamp(0.0, 1.0);

        let global_sq = rho * (1.0 - f);
        let sector_sq = rho * f;

        (global_sq.sqrt(), sector_sq.sqrt())
    }
}

impl Copula for MultiFactorCopula {
    fn conditional_default_prob(
        &self,
        default_threshold: f64,
        factor_realization: &[f64],
        correlation: f64,
    ) -> f64 {
        // Single-sector shortcut: every name is treated as belonging to the
        // one available sector factor (`sector_idx = 1`). Cross-sector
        // behavior is only exposed through `conditional_default_prob_with_sector`
        // with `sector_idx = 0`.
        self.conditional_default_prob_with_sector(
            default_threshold,
            factor_realization,
            correlation,
            1,
        )
    }

    fn conditional_default_prob_with_sector(
        &self,
        default_threshold: f64,
        factor_realization: &[f64],
        correlation: f64,
        sector_idx: usize,
    ) -> f64 {
        // Length mismatch is a programmer error. In debug, fail loudly; in
        // release, return the unconditional PD Φ(c) (the no-information
        // answer) rather than silently zeroing missing factors — a zeroed
        // factor produces biased conditional PDs under positive correlation.
        debug_assert_eq!(
            factor_realization.len(),
            self.num_factors_count,
            "MultiFactorCopula expects exactly {} factors, got {}",
            self.num_factors_count,
            factor_realization.len()
        );
        if factor_realization.len() != self.num_factors_count {
            tracing::error!(
                expected = self.num_factors_count,
                actual = factor_realization.len(),
                "MultiFactorCopula: factor length mismatch; returning unconditional PD"
            );
            return norm_cdf(default_threshold);
        }

        let (global_loading, sector_loading_full) =
            self.decompose_correlation(correlation, self.sector_fraction);

        // `sector_idx = 0` marks the name as sector-unaffected
        // (cross-sector only), so we zero its sector loading. Any
        // non-zero `sector_idx` keeps the sector loading and, for
        // copulas with `num_sectors >= 1`, picks the `sector_idx`-th
        // slot of `factor_realization` as the sector shock. Non-zero
        // `sector_idx` on a degenerate 1-factor copula
        // (`num_factors_count == 1`) still applies the loading but the
        // sector shock is absent.
        let num_sectors = self.num_sectors();
        let sector_loading = if sector_idx == 0 {
            0.0
        } else {
            sector_loading_full
        };

        // Resolve the global and sector-shock values. The global factor
        // always lives in slot 0. For `sector_idx >= 1` we index directly
        // into the factor vector when a slot exists; otherwise the shock
        // defaults to zero.
        let z_global = factor_realization.first().copied().unwrap_or(0.0);
        let z_sector = if sector_idx == 0 || sector_idx > num_sectors {
            0.0
        } else {
            factor_realization.get(sector_idx).copied().unwrap_or(0.0)
        };

        let gamma = self.idiosyncratic_loading(global_loading, sector_loading);

        if gamma < 1e-10 {
            // Near-perfect correlation
            let systematic = global_loading * z_global + sector_loading * z_sector;
            return norm_cdf(default_threshold - systematic);
        }

        // P(default | Z_G, Z_S) = Φ((threshold - β_G·Z_G - β_S·Z_S) / γ)
        let systematic = global_loading * z_global + sector_loading * z_sector;
        let conditional_threshold = (default_threshold - systematic) / gamma;

        norm_cdf(conditional_threshold.clamp(-CDF_CLIP, CDF_CLIP))
    }

    fn conditional_default_prob_given_systematic_and_mixing(
        &self,
        default_threshold: f64,
        systematic: f64,
        mixing: f64,
        correlation: f64,
    ) -> f64 {
        // Single-Z conditional with the sector factor(s) integrated out. The
        // latent variable is Aᵢ = β_G·Z_G + β_S·Z_S + γ·εᵢ; conditioning on
        // Z_G only, the remainder β_S·Z_S + γ·εᵢ is N(0, 1 − β_G²), so
        //   P(default | Z_G) = Φ((c − β_G·Z_G)/√(1−β_G²)).
        // There is no mixing variable. Note this is the GLOBAL-factor
        // conditional only — per-name engines that cannot supply sector
        // factor realizations must not simulate this copula name-by-name
        // (see `PerNameCopulaDefault::new`, which rejects multi-factor specs).
        let _ = mixing;
        let (global_loading, _) = self.decompose_correlation(correlation, self.sector_fraction);
        let residual_sd = (1.0 - global_loading * global_loading).max(0.0).sqrt();
        if residual_sd < 1e-10 {
            return norm_cdf(default_threshold - global_loading * systematic);
        }
        let conditional_threshold = (default_threshold - global_loading * systematic) / residual_sd;
        norm_cdf(conditional_threshold.clamp(-CDF_CLIP, CDF_CLIP))
    }

    fn integrate_fn(&self, f: &dyn Fn(&[f64]) -> f64) -> f64 {
        // Multi-dimensional Gauss-Hermite quadrature via recursive
        // nested integration over `num_factors_count` dimensions
        // (1 global + `num_sectors` sector slots). Cost is
        // `n_q^{num_factors}` at quadrature order `n_q`, which is why
        // `MAX_FACTORS` is capped at 5 — beyond that Monte Carlo is
        // cheaper and more flexible.
        //
        // The recursive helper supports any total factor count in
        // `[1, MAX_FACTORS]`.
        let d = self.num_factors_count;
        let mut scratch = vec![0.0_f64; d];
        self.integrate_recursive(f, &mut scratch, 0)
    }

    fn num_factors(&self) -> usize {
        self.num_factors_count
    }

    fn model_name(&self) -> &'static str {
        "Multi-Factor Gaussian Copula"
    }

    fn tail_dependence(&self, _correlation: f64) -> f64 {
        // Multi-factor Gaussian still has zero tail dependence
        // (sum of Gaussians is Gaussian)
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::super::GaussianCopula;
    use super::*;
    use finstack_quant_core::math::standard_normal_inv_cdf;

    #[test]
    fn test_multi_factor_creation() {
        let copula = MultiFactorCopula::new(2);
        assert_eq!(copula.num_factors(), 2);
        assert_eq!(copula.model_name(), "Multi-Factor Gaussian Copula");
    }

    #[test]
    fn test_multi_factor_capped_at_max_factors() {
        // MAX_FACTORS (currently 5 = 1 global + 4 sector factors)
        // supports bespoke CDO pricing with K > 1 sector factors.
        let copula = MultiFactorCopula::new(100);
        assert_eq!(
            copula.num_factors(),
            MAX_FACTORS,
            "Factors should be capped at MAX_FACTORS ({MAX_FACTORS})"
        );
    }

    #[test]
    fn test_correlation_decomposition() {
        let copula = MultiFactorCopula::new(2);

        // With 50% sector fraction
        let (global, sector) = copula.decompose_correlation(0.36, 0.5);

        // Total correlation should reconstruct
        let reconstructed = global * global + sector * sector;
        assert!(
            (reconstructed - 0.36).abs() < 1e-6,
            "Reconstructed {} should equal original 0.36",
            reconstructed
        );
    }

    #[test]
    fn test_inter_vs_intra_sector_correlation() {
        let copula = MultiFactorCopula::with_loadings(2, 0.4, 0.4);

        let inter = copula.inter_sector_correlation();
        let intra = copula.intra_sector_correlation();

        // Intra-sector should be higher than inter-sector
        assert!(
            intra > inter,
            "Intra-sector {} should exceed inter-sector {}",
            intra,
            inter
        );

        // Inter-sector = β_G² = 0.16
        assert!((inter - 0.16).abs() < 1e-6);

        // Intra-sector = β_G² + β_S² = 0.32
        assert!((intra - 0.32).abs() < 1e-6);
    }

    #[test]
    fn test_single_factor_equals_gaussian() {
        let multi_copula = MultiFactorCopula::new(1);
        let gaussian_copula = GaussianCopula::new();

        let threshold = standard_normal_inv_cdf(0.05);
        let correlation = 0.30;

        // Single-factor multi should behave like Gaussian
        let multi_prob = multi_copula.conditional_default_prob(threshold, &[0.5], correlation);
        let gaussian_prob =
            gaussian_copula.conditional_default_prob(threshold, &[0.5], correlation);

        // They use different loading decomposition, so won't be exactly equal
        // but should be in the same ballpark
        assert!(
            (multi_prob - gaussian_prob).abs() < 0.05,
            "Single-factor multi {} should be close to Gaussian {}",
            multi_prob,
            gaussian_prob
        );
    }

    #[test]
    fn test_zero_tail_dependence() {
        let copula = MultiFactorCopula::new(2);
        assert_eq!(copula.tail_dependence(0.5), 0.0);
    }

    #[test]
    fn test_integration_recovers_unconditional() {
        let copula = MultiFactorCopula::new(2);
        let pd = 0.05;
        let threshold = standard_normal_inv_cdf(pd);
        let correlation = 0.30;

        let integrated_prob = copula.integrate_fn(&|factors| {
            copula.conditional_default_prob(threshold, factors, correlation)
        });

        // Should be close to unconditional
        assert!(
            (integrated_prob - pd).abs() < 0.01,
            "Integrated probability {} should be close to unconditional {}",
            integrated_prob,
            pd
        );
    }

    #[test]
    fn test_factor_length_mismatch_contract() {
        let copula = MultiFactorCopula::new(2);
        let pd = 0.05;
        let threshold = standard_normal_inv_cdf(pd);
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
                    (result - pd).abs() < 1e-9,
                    "factor length mismatch should return unconditional PD ({pd}), got {result}"
                );
            }
        };

        assert_contract(&[-1.0]);
        assert_contract(&[0.5, 1.0, -0.3]);
        assert_contract(&[]);
    }

    #[test]
    fn test_loading_constraints() {
        // Loadings should satisfy β_G² + β_S² ≤ 1
        let copula = MultiFactorCopula::with_loadings(2, 0.8, 0.8);

        // Should be clamped
        let intra = copula.intra_sector_correlation();
        assert!(
            intra <= 1.0,
            "Intra-sector correlation {} should be ≤ 1",
            intra
        );
    }

    /// `conditional_default_prob_with_sector(..., sector_idx=0)` must
    /// drop the sector-factor contribution, so a cross-sector name
    /// effectively sees only the global-factor correlation `β_G²`.
    /// This produces a different conditional PD than the default
    /// `sector_idx=1` path whenever the sector loading is non-zero.
    #[test]
    fn test_sector_aware_conditional_prob_distinguishes_cross_sector() {
        let copula = MultiFactorCopula::with_loadings_and_sector_fraction(2, 0.5, 0.4, 0.5);
        let threshold = standard_normal_inv_cdf(0.05);
        let total_corr = 0.40;

        // Pick a shock where the sector factor matters: z_global small, z_sector large.
        let factors = [0.1, 1.5];

        let intra = copula.conditional_default_prob_with_sector(threshold, &factors, total_corr, 1);
        let cross = copula.conditional_default_prob_with_sector(threshold, &factors, total_corr, 0);
        let legacy = copula.conditional_default_prob(threshold, &factors, total_corr);

        // Cross-sector must differ from intra-sector when the sector shock is non-zero.
        assert!(
            (intra - cross).abs() > 1e-4,
            "sector_idx=0 (cross) vs sector_idx=1 (intra) must differ on a non-zero sector shock: intra={intra:.6}, cross={cross:.6}"
        );
        // Legacy `conditional_default_prob` must continue to match the
        // intra-sector branch (sector_idx = 1) so existing pricing paths
        // are unaffected.
        assert!(
            (legacy - intra).abs() < 1e-12,
            "legacy conditional_default_prob must match sector_idx=1: legacy={legacy:.9}, intra={intra:.9}"
        );
    }

    /// The default `Copula` trait method must ignore `sector_idx` for
    /// sector-unaware copulas, preserving backwards compatibility
    /// across Gaussian / Student-t / RFL implementations.
    #[test]
    fn test_sector_aware_default_ignores_sector_idx_for_gaussian() {
        use super::super::GaussianCopula;
        let gaussian = GaussianCopula::new();
        let threshold = standard_normal_inv_cdf(0.05);
        let factors = [0.25];
        let corr = 0.30;

        let base = gaussian.conditional_default_prob(threshold, &factors, corr);
        for sector_idx in [0, 1, 2, 99] {
            let s = gaussian
                .conditional_default_prob_with_sector(threshold, &factors, corr, sector_idx);
            assert!(
                (s - base).abs() < 1e-12,
                "GaussianCopula sector_idx={sector_idx} must match base PD: base={base:.9}, s={s:.9}"
            );
        }
    }

    /// A true `K`-sector copula must accept `1 + K` factor realizations
    /// and route `sector_idx` to the correct slot of the factor vector.
    /// Two sectors with equal (zero) shocks must produce the same PD;
    /// a sector with a non-zero shock must produce a distinctly
    /// different PD, proving sector_idx actually indexes into the
    /// factor vector.
    #[test]
    fn test_k_sector_copula_distinguishes_sectors() {
        let copula = MultiFactorCopula::with_k_sectors(3, 0.4, 0.4);
        assert_eq!(copula.num_factors(), 4); // 1 global + 3 sectors
        assert_eq!(copula.num_sectors(), 3);

        let threshold = standard_normal_inv_cdf(0.05);
        let total_corr = 0.40;

        // Global shock = 0, sector 1 shock = +1.5, others zero.
        let factors = [0.0, 1.5, 0.0, 0.0];

        let pd_sector_1 =
            copula.conditional_default_prob_with_sector(threshold, &factors, total_corr, 1);
        let pd_sector_2 =
            copula.conditional_default_prob_with_sector(threshold, &factors, total_corr, 2);
        let pd_sector_3 =
            copula.conditional_default_prob_with_sector(threshold, &factors, total_corr, 3);

        // Sectors 2 and 3 both have zero shocks in the factor vector and
        // share the same sector loading, so their PDs must agree.
        assert!(
            (pd_sector_2 - pd_sector_3).abs() < 1e-12,
            "sectors with equal zero shocks must give equal PDs: s2={pd_sector_2} s3={pd_sector_3}"
        );
        // Sector 1 sees the +1.5σ shock and must differ noticeably — if
        // sector_idx weren't routed to factors[1], this invariant would
        // fail.
        assert!(
            (pd_sector_1 - pd_sector_2).abs() > 1e-4,
            "sector_idx=1 shock must produce a distinct PD: s1={pd_sector_1} s2={pd_sector_2}"
        );
    }

    /// `integrate_fn` on a `K`-sector copula must recover the
    /// unconditional PD when applied to
    /// `conditional_default_prob_with_sector` for every sector index.
    /// The marginal distribution of a single name is Φ(c) = PD by
    /// construction regardless of its sector assignment.
    #[test]
    fn test_k_sector_integration_recovers_unconditional_for_each_sector() {
        let copula = MultiFactorCopula::with_k_sectors(3, 0.4, 0.4);
        let pd = 0.05;
        let threshold = standard_normal_inv_cdf(pd);
        let corr = 0.30;

        for sector_idx in 0..=copula.num_sectors() {
            let integrated = copula.integrate_fn(&|factors| {
                copula.conditional_default_prob_with_sector(threshold, factors, corr, sector_idx)
            });
            assert!(
                (integrated - pd).abs() < 0.01,
                "sector_idx={sector_idx}: integrated PD {integrated} must converge to unconditional {pd}"
            );
        }
    }

    /// `with_k_sectors` must clamp to `MAX_SECTORS` so callers asking
    /// for an unreasonably large portfolio fall back to the largest
    /// supported `K` rather than silently constructing a quadrature
    /// grid too expensive to run.
    #[test]
    fn test_k_sector_clamps_at_max_sectors() {
        let copula = MultiFactorCopula::with_k_sectors(100, 0.4, 0.3);
        assert_eq!(copula.num_sectors(), MAX_SECTORS);
        assert_eq!(copula.num_factors(), 1 + MAX_SECTORS);
    }
}
