//! Copula models for portfolio default correlation.
//!
//! Provides a trait-based copula abstraction enabling pluggable correlation
//! models with the one-factor Gaussian copula as the default.
//!
//! # Supported Models
//!
//! - **Gaussian**: Standard one-factor Gaussian copula (market default)
//! - **Student-t**: Fat-tailed copula capturing tail dependence
//! - **Random Factor Loading (RFL)**: Stochastic correlation model
//! - **Multi-Factor**: Sector-based correlation structure
//!
//! # References
//!
//! - Gaussian copula background:
//!   `docs/REFERENCES.md#li-2000-gaussian-copula`
//! - Random recovery and random-factor-loading extensions:
//!   `docs/REFERENCES.md#andersen-sidenius-2005-rfl`
//! - Analytical CDO valuation context:
//!   `docs/REFERENCES.md#hull-white-2004-cdo`

mod gaussian;
mod multi_factor;
mod random_factor_loading;
mod student_t;

pub use gaussian::GaussianCopula;
pub use multi_factor::MultiFactorCopula;
pub use random_factor_loading::RandomFactorLoadingCopula;
pub use student_t::StudentTCopula;

use finstack_core::math::GaussHermiteQuadrature;

/// Copula model for portfolio default correlation.
///
/// Implementations provide the conditional default probability P(τᵢ ≤ t | M)
/// given the systematic factor(s) M, enabling integration over the factor space.
///
/// # Model Framework
///
/// All copula models follow the latent variable approach:
/// ```text
/// Aᵢ = f(M, εᵢ)  where M = systematic factors, εᵢ = idiosyncratic
/// Default: τᵢ ≤ t ⟺ Aᵢ ≤ threshold(PD(t))
/// ```
///
/// The copula determines the joint distribution of (M, εᵢ).
pub trait Copula: Send + Sync {
    /// Conditional default probability given factor realization(s).
    ///
    /// P(default | Z) = P(Aᵢ ≤ threshold | Z)
    ///
    /// # Arguments
    /// * `default_threshold` - Φ⁻¹(PD) or t⁻¹(PD) depending on copula
    /// * `factor_realization` - Systematic factor value(s)
    /// * `correlation` - Asset correlation parameter(s)
    ///
    /// # Returns
    ///
    /// A conditional default probability in `[0, 1]`.
    fn conditional_default_prob(
        &self,
        default_threshold: f64,
        factor_realization: &[f64],
        correlation: f64,
    ) -> f64;

    /// Sector-aware conditional default probability.
    ///
    /// For copulas that resolve per-name sector assignments (e.g. a multi-
    /// factor Gaussian copula with a global factor plus `K` sector factors),
    /// `sector_idx` selects which factor slot to pair with the systematic
    /// factor when computing the conditional PD for a given name.
    ///
    /// # Indexing convention
    ///
    /// - `sector_idx = 0` indicates the name has **no sector factor**; only
    ///   the global factor drives its latent variable. Correlation with other
    ///   names reduces to the inter-sector value (e.g. `β_G²` in a Gaussian
    ///   two-factor model).
    /// - `sector_idx ≥ 1` indicates membership in sector `k = sector_idx`;
    ///   the copula consumes the `k`-th sector-factor realization from
    ///   `factor_realization`. Pairs of names with the same non-zero
    ///   `sector_idx` share both the global and the sector factor.
    ///
    /// # Default implementation
    ///
    /// Sector-unaware copulas (Gaussian, Student-t, RFL, single-factor
    /// variants) ignore `sector_idx` and fall through to
    /// [`Self::conditional_default_prob`]. This keeps all existing callers
    /// compatible; only copulas that genuinely resolve sectors need to
    /// override.
    fn conditional_default_prob_with_sector(
        &self,
        default_threshold: f64,
        factor_realization: &[f64],
        correlation: f64,
        sector_idx: usize,
    ) -> f64 {
        let _ = sector_idx;
        self.conditional_default_prob(default_threshold, factor_realization, correlation)
    }

    /// LHP conditional default probability given the *Gaussian* systematic
    /// draw `z` and the shared mixing draw `w`.
    ///
    /// This is the large-homogeneous-pool (`N → ∞`) limit of the per-name
    /// latent construction [`Self::latent_variable`] — the sampling
    /// counterpart computed in the **same** `(Z, W)` sigma-algebra as
    /// [`Self::latent_variable`], so a per-name pool and the LHP fast-path
    /// converge as `N → ∞`.
    ///
    /// It differs from [`Self::conditional_default_prob`]: that method takes
    /// the copula's *own systematic factor* (for the Student-t copula the
    /// `t(ν)`-distributed `M = Z/√W`, with `W` already integrated out). Here
    /// the caller passes the raw Gaussian `Z` and the explicitly-drawn `W`,
    /// matching how [`Self::latent_variable`] is fed by the per-name engine.
    ///
    /// # Arguments
    ///
    /// * `default_threshold` — the per-name default barrier `c` (`Φ⁻¹(PD)`
    ///   for Gaussian-marginal copulas, `t_ν⁻¹(PD)` for the Student-t copula).
    /// * `systematic` — the Gaussian systematic draw `Z ~ N(0,1)` for the
    ///   period, shared by every name.
    /// * `mixing` — the shared mixing variable `W` drawn via
    ///   [`Self::sample_mixing`] (`1.0` for copulas without a mixing
    ///   variable). The default implementation ignores it.
    /// * `correlation` — the asset correlation `ρ`.
    ///
    /// # Default implementation
    ///
    /// Copulas whose latent variable is `Aᵢ = √ρ·Z + √(1−ρ)·εᵢ` with a
    /// Gaussian systematic `Z` (Gaussian, RFL, multi-factor) have
    /// `P(default | Z) = Φ((c − √ρ·Z)/√(1−ρ))`, which is exactly what
    /// [`Self::conditional_default_prob`] already computes from a single
    /// factor — so the default implementation delegates to it and ignores
    /// `mixing`. The Student-t copula, whose latent variable divides by `√W`,
    /// overrides this with the `W`-conditional closed form.
    fn conditional_default_prob_given_systematic_and_mixing(
        &self,
        default_threshold: f64,
        systematic: f64,
        mixing: f64,
        correlation: f64,
    ) -> f64 {
        let _ = mixing;
        self.conditional_default_prob(default_threshold, &[systematic], correlation)
    }

    /// Integrate expected value E[f(L)] over the factor distribution.
    ///
    /// Uses appropriate quadrature for the copula's factor distribution.
    /// The integrand receives factor values and returns a scalar.
    ///
    /// # Returns
    ///
    /// The factor-space expectation of the supplied integrand.
    fn integrate_fn(&self, f: &dyn Fn(&[f64]) -> f64) -> f64;

    /// Per-name latent variable `Aᵢ` for a finite-pool Monte Carlo draw.
    ///
    /// This realizes the copula's *own* latent-variable construction (the
    /// one documented in each model's module header) so a finite pool can be
    /// simulated name-by-name rather than collapsed to the large-homogeneous
    /// pool (LHP) limit. It is the sampling counterpart of
    /// [`Self::conditional_default_prob`], which only returns the analytic
    /// `P(default | Z)` and therefore cannot capture name-level lumpiness.
    ///
    /// # Arguments
    ///
    /// * `systematic` — a standard-normal draw `Z ~ N(0,1)` shared by every
    ///   name in the pool for this period.
    /// * `idiosyncratic` — a standard-normal draw `εᵢ ~ N(0,1)` unique to
    ///   name `i`.
    /// * `mixing` — the shared positive mixing variable `W` for copulas with
    ///   a variance-mixture representation (Student-t: `W ~ Gamma(ν/2, ν/2)`).
    ///   Pass `1.0` for copulas without a mixing variable (Gaussian); the
    ///   default implementation ignores it.
    /// * `correlation` — the asset correlation `ρ`.
    ///
    /// # Default implementation
    ///
    /// The Gaussian one-factor construction
    /// `Aᵢ = √ρ · Z + √(1−ρ) · εᵢ`.
    /// Copulas with a different latent structure (Student-t) override this.
    /// A name defaults when `Aᵢ ≤ threshold` where `threshold` is the copula's
    /// default threshold (`Φ⁻¹(PD)` for Gaussian, `t_ν⁻¹(PD)` for Student-t).
    fn latent_variable(
        &self,
        systematic: f64,
        idiosyncratic: f64,
        mixing: f64,
        correlation: f64,
    ) -> f64 {
        let _ = mixing;
        let rho = correlation.clamp(0.0, 1.0);
        rho.sqrt() * systematic + (1.0 - rho).sqrt() * idiosyncratic
    }

    /// Draw the shared mixing variable `W` for variance-mixture copulas.
    ///
    /// `u01` is a uniform `[0,1)` draw. The default implementation returns
    /// `1.0` (no mixing — Gaussian). The Student-t copula overrides this to
    /// sample `W ~ Gamma(ν/2, ν/2)` so that `M = Z/√W` is `t(ν)`-distributed
    /// and the shared `W` induces tail dependence across every name.
    fn sample_mixing(&self, u01: f64) -> f64 {
        let _ = u01;
        1.0
    }

    /// Number of systematic factors in the model.
    ///
    /// # Returns
    ///
    /// The length of the factor vector expected by
    /// [`Self::conditional_default_prob`].
    fn num_factors(&self) -> usize;

    /// Model description for diagnostics.
    ///
    /// # Returns
    ///
    /// A static human-readable model name.
    fn model_name(&self) -> &'static str;

    /// Lower-tail dependence coefficient at the given correlation.
    ///
    /// Strict definition:
    /// `λ_L = lim_{u→0} P(U₂ ≤ u | U₁ ≤ u)`.
    ///
    /// - Gaussian copula: λ_L = 0 (no tail dependence)
    /// - Student-t copula: λ_L > 0 (positive tail dependence)
    /// - Random Factor Loading copula: returns `f64::NAN`
    ///   (no closed-form λ_L; see
    ///   `RandomFactorLoadingCopula::stress_correlation_proxy`
    ///   for the heuristic stress gauge).
    ///
    /// Implementations that cannot supply a closed-form `λ_L` MUST return
    /// `f64::NAN` rather than a heuristic proxy. Callers should check
    /// `is_nan()` before using the result.
    ///
    /// # Returns
    ///
    /// The strict `λ_L`, or `f64::NAN` if the model has no closed form.
    fn tail_dependence(&self, correlation: f64) -> f64;
}

/// Copula model specification for configuration and serialization.
///
/// Allows copula selection without constructing the full model,
/// enabling deferred construction with market data.
#[derive(
    Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "type", deny_unknown_fields)]
#[non_exhaustive]
pub enum CopulaSpec {
    /// One-factor Gaussian copula (market standard default).
    ///
    /// Simple and fast, but lacks tail dependence.
    #[default]
    Gaussian,

    /// Student-t copula with specified degrees of freedom.
    ///
    /// Captures tail dependence - joint extreme defaults are more likely
    /// than Gaussian predicts. Lower df = more tail dependence.
    ///
    /// Typical calibration: df ∈ [4, 10] for CDX tranches.
    ///
    /// # Invariant
    ///
    /// `degrees_of_freedom` **must** be finite and `> 2`. The programmatic
    /// constructor [`CopulaSpec::student_t`] panics on invalid input, but
    /// deserialized specs (from config files, JSON, etc.) cannot panic —
    /// [`CopulaSpec::build`] silently clamps an out-of-range or non-finite
    /// value to `2.01` and emits a `tracing::warn!`. This is deliberate: it
    /// preserves forward
    /// compatibility for serialized specs but means callers that round-trip
    /// a spec may observe a changed `degrees_of_freedom`. Validate at the
    /// config-loading boundary if strict rejection is required.
    StudentT {
        /// Degrees of freedom (must be > 2 for finite variance)
        degrees_of_freedom: f64,
    },

    /// Random Factor Loading copula (stochastic correlation).
    ///
    /// Models correlation itself as random, capturing the empirical
    /// observation that correlation increases during market stress.
    RandomFactorLoading {
        /// Volatility of the factor loading (correlation vol proxy)
        loading_volatility: f64,
    },

    /// Multi-factor Gaussian copula with sector structure.
    ///
    /// Uses multiple systematic factors (global + sector-specific)
    /// to capture industry concentration effects.
    MultiFactor {
        /// Number of systematic factors
        num_factors: usize,
    },
}

impl CopulaSpec {
    /// Create a Gaussian copula specification.
    ///
    /// # Returns
    ///
    /// A [`CopulaSpec::Gaussian`] configuration.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::correlation::CopulaSpec;
    ///
    /// let spec = CopulaSpec::gaussian();
    /// assert!(spec.is_gaussian());
    /// ```
    pub fn gaussian() -> Self {
        CopulaSpec::Gaussian
    }

    /// Create a Student-t copula specification.
    ///
    /// # Arguments
    /// * `df` - Degrees of freedom (typically 4-10 for CDX)
    ///
    /// # Returns
    ///
    /// A [`CopulaSpec::StudentT`] configuration.
    ///
    /// # Panics
    /// Panics if df ≤ 2 (variance undefined)
    pub fn student_t(df: f64) -> Self {
        assert!(df > 2.0, "Student-t df must be > 2 for finite variance");
        CopulaSpec::StudentT {
            degrees_of_freedom: df,
        }
    }

    /// Create a Random Factor Loading specification.
    ///
    /// # Arguments
    /// * `loading_vol` - Volatility of factor loading (0.05-0.20 typical)
    ///
    /// # Returns
    ///
    /// A [`CopulaSpec::RandomFactorLoading`] configuration with bounded
    /// loading volatility.
    pub fn random_factor_loading(loading_vol: f64) -> Self {
        CopulaSpec::RandomFactorLoading {
            loading_volatility: loading_vol.clamp(0.0, 0.5),
        }
    }

    /// Create a Multi-factor copula specification.
    ///
    /// # Arguments
    ///
    /// * `num_factors` - Requested number of systematic factors.
    ///
    /// # Returns
    ///
    /// A [`CopulaSpec::MultiFactor`] configuration.
    pub fn multi_factor(num_factors: usize) -> Self {
        CopulaSpec::MultiFactor { num_factors }
    }

    /// Build a copula from this specification.
    ///
    /// # Returns
    ///
    /// A boxed [`Copula`] implementation matching the spec variant.
    #[must_use]
    pub fn build(&self) -> Box<dyn Copula> {
        match self {
            CopulaSpec::Gaussian => Box::new(GaussianCopula::new()),
            CopulaSpec::StudentT { degrees_of_freedom } => {
                if !degrees_of_freedom.is_finite() || *degrees_of_freedom <= 2.0 {
                    tracing::warn!(
                        df = degrees_of_freedom,
                        "Student-t degrees_of_freedom must be finite and > 2; clamping to 2.01"
                    );
                }
                let df = if degrees_of_freedom.is_finite() {
                    degrees_of_freedom.max(2.01)
                } else {
                    2.01
                };
                Box::new(StudentTCopula::new(df))
            }
            CopulaSpec::RandomFactorLoading { loading_volatility } => {
                Box::new(RandomFactorLoadingCopula::new(*loading_volatility))
            }
            CopulaSpec::MultiFactor { num_factors } => {
                Box::new(MultiFactorCopula::new(*num_factors))
            }
        }
    }

    /// Check if this is a Gaussian copula specification.
    ///
    /// # Returns
    ///
    /// `true` if this value is [`CopulaSpec::Gaussian`].
    pub fn is_gaussian(&self) -> bool {
        matches!(self, CopulaSpec::Gaussian)
    }

    /// Check if this is a Student-t copula specification.
    ///
    /// # Returns
    ///
    /// `true` if this value is [`CopulaSpec::StudentT`].
    pub fn is_student_t(&self) -> bool {
        matches!(self, CopulaSpec::StudentT { .. })
    }

    /// Check if this is a Random Factor Loading copula specification.
    ///
    /// # Returns
    ///
    /// `true` if this value is [`CopulaSpec::RandomFactorLoading`].
    pub fn is_rfl(&self) -> bool {
        matches!(self, CopulaSpec::RandomFactorLoading { .. })
    }

    /// Check if this is a Multi-factor copula specification.
    ///
    /// # Returns
    ///
    /// `true` if this value is [`CopulaSpec::MultiFactor`].
    pub fn is_multi_factor(&self) -> bool {
        matches!(self, CopulaSpec::MultiFactor { .. })
    }
}

/// Default quadrature order for copula integration.
///
/// Industry standard (QuantLib, Bloomberg) uses 20-50 points for tranche pricing.
pub(crate) const DEFAULT_QUADRATURE_ORDER: u8 = 20;

/// Global cache of Gauss-Hermite quadrature instances keyed by order.
/// Wrapped in `Arc` so copula clones are cheap (refcount bump instead of O(n²) recomputation).
static QUADRATURE_CACHE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<u8, std::sync::Arc<GaussHermiteQuadrature>>>,
> = std::sync::OnceLock::new();

fn get_cached_quadrature(order: u8) -> std::sync::Arc<GaussHermiteQuadrature> {
    let cache =
        QUADRATURE_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut map = match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    map.entry(order)
        .or_insert_with(|| std::sync::Arc::new(select_quadrature(order)))
        .clone()
}

/// Select quadrature based on order (uncached — used to populate the cache).
pub(crate) fn select_quadrature(order: u8) -> GaussHermiteQuadrature {
    GaussHermiteQuadrature::new(order as usize).unwrap_or_else(|_| {
        GaussHermiteQuadrature::new(DEFAULT_QUADRATURE_ORDER as usize).unwrap_or_else(|_| {
            unreachable!("DEFAULT_QUADRATURE_ORDER must be a valid Gauss-Hermite order")
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copula_spec_default() {
        let spec = CopulaSpec::default();
        assert_eq!(spec, CopulaSpec::Gaussian);
    }

    #[test]
    fn test_copula_spec_builders() {
        let gaussian = CopulaSpec::gaussian();
        assert!(matches!(gaussian, CopulaSpec::Gaussian));

        let student_t = CopulaSpec::student_t(5.0);
        assert!(matches!(
            student_t,
            CopulaSpec::StudentT {
                degrees_of_freedom: df
            } if (df - 5.0).abs() < 1e-10
        ));

        let rfl = CopulaSpec::random_factor_loading(0.15);
        assert!(matches!(
            rfl,
            CopulaSpec::RandomFactorLoading {
                loading_volatility: v
            } if (v - 0.15).abs() < 1e-10
        ));
    }

    #[test]
    #[should_panic(expected = "Student-t df must be > 2")]
    fn test_student_t_invalid_df() {
        CopulaSpec::student_t(2.0);
    }

    #[test]
    fn test_copula_build() {
        // Test Gaussian
        let gaussian = CopulaSpec::gaussian();
        assert!(gaussian.is_gaussian());
        let g_copula = gaussian.build();
        assert_eq!(g_copula.num_factors(), 1);

        // Test Student-t
        let student_t = CopulaSpec::student_t(5.0);
        assert!(student_t.is_student_t());
        let t_copula = student_t.build();
        assert_eq!(t_copula.num_factors(), 1);

        // Test RFL
        let rfl = CopulaSpec::random_factor_loading(0.1);
        assert!(rfl.is_rfl());
        let rfl_copula = rfl.build();
        assert_eq!(rfl_copula.num_factors(), 2);

        // Test Multi-factor
        let mf = CopulaSpec::multi_factor(2);
        assert!(mf.is_multi_factor());
        let mf_copula = mf.build();
        assert_eq!(mf_copula.num_factors(), 2);
    }

    #[test]
    fn test_deserialized_invalid_student_t_df_does_not_panic() {
        // Simulate config file with invalid df <= 2
        let spec: CopulaSpec =
            serde_json::from_str(r#"{"type":"StudentT","degrees_of_freedom":1.5}"#)
                .expect("should deserialize");
        // build() must not panic — it clamps df to 2.01
        let copula = spec.build();
        assert_eq!(copula.num_factors(), 1);
    }
}
