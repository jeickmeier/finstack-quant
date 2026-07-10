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

use finstack_quant_core::math::GaussHermiteQuadrature;

use crate::correlation::{Error, Result};

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
    /// # Implementation requirement
    ///
    /// This method is deliberately **required** (no trait default). A default
    /// forwarding `&[systematic]` to [`Self::conditional_default_prob`] is
    /// only correct for single-slot Gaussian-systematic copulas; for any
    /// multi-slot copula (`num_factors() > 1`, e.g. RFL or multi-factor) it
    /// silently passes a wrong-length factor vector — debug panic / biased
    /// release pricing. Each copula must state its own `(Z, W)` conditional:
    ///
    /// - Gaussian: `Φ((c − √ρ·Z)/√(1−ρ))`, mixing ignored.
    /// - Student-t: `Φ((c·√W − √ρ·Z)/√(1−ρ))`.
    /// - RFL: `Φ((c − β(η)·Z)/√(1−β(η)²))` with `mixing = η`.
    /// - Multi-factor: sector factor integrated out,
    ///   `Φ((c − β_G·Z)/√(1−β_G²))`.
    fn conditional_default_prob_given_systematic_and_mixing(
        &self,
        default_threshold: f64,
        systematic: f64,
        mixing: f64,
        correlation: f64,
    ) -> f64;

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
    /// and the shared `W` induces tail dependence across every name. The RFL
    /// copula overrides it to sample the loading shock `η = Φ⁻¹(u01)`, drawn
    /// once per period and shared so the realized loading `β(η)` is common
    /// across the pool.
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

    /// Checked conditional default probability for public/host boundaries.
    ///
    /// Concrete copulas retain their infallible hot-path implementation, while
    /// this wrapper rejects factor-shape mistakes and non-finite inputs before
    /// they can silently decondition a portfolio calculation.
    fn conditional_default_prob_checked(
        &self,
        default_threshold: f64,
        factor_realization: &[f64],
        correlation: f64,
    ) -> finstack_quant_core::Result<f64> {
        let expected = self.num_factors();
        if factor_realization.len() != expected {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{} expects exactly {} systematic factors, got {}",
                self.model_name(),
                expected,
                factor_realization.len()
            )));
        }
        if !default_threshold.is_finite()
            || !correlation.is_finite()
            || factor_realization.iter().any(|value| !value.is_finite())
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{} conditional default probability requires finite threshold, correlation, and factors",
                self.model_name()
            )));
        }
        if !(0.0..=1.0).contains(&correlation) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{} correlation must lie within [0, 1], got {}",
                self.model_name(),
                correlation
            )));
        }
        let value =
            self.conditional_default_prob(default_threshold, factor_realization, correlation);
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{} produced invalid conditional default probability {}",
                self.model_name(),
                value
            )));
        }
        Ok(value)
    }

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
    /// `degrees_of_freedom` **must** be finite and `> 2`. Programmatic
    /// construction and [`CopulaSpec::build`] both reject invalid values.
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
    /// use finstack_quant_valuations::correlation::CopulaSpec;
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
    /// # Errors
    /// Returns [`crate::correlation::Error`] if `df` is not finite or `<= 2`.
    pub fn student_t(df: f64) -> Result<Self> {
        validate_student_t_degrees_of_freedom(df)?;
        Ok(CopulaSpec::StudentT {
            degrees_of_freedom: df,
        })
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
    ///
    /// # Errors
    /// Returns [`crate::correlation::Error`] if a Student-t spec has invalid
    /// degrees of freedom.
    pub fn build(&self) -> Result<Box<dyn Copula>> {
        Ok(match self {
            CopulaSpec::Gaussian => Box::new(GaussianCopula::new()),
            CopulaSpec::StudentT { degrees_of_freedom } => {
                validate_student_t_degrees_of_freedom(*degrees_of_freedom)?;
                Box::new(StudentTCopula::new(*degrees_of_freedom))
            }
            CopulaSpec::RandomFactorLoading { loading_volatility } => {
                Box::new(RandomFactorLoadingCopula::new(*loading_volatility))
            }
            CopulaSpec::MultiFactor { num_factors } => {
                Box::new(MultiFactorCopula::new(*num_factors))
            }
        })
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

fn validate_student_t_degrees_of_freedom(df: f64) -> Result<()> {
    if df.is_finite() && df > 2.0 {
        Ok(())
    } else {
        Err(Error::InvalidStudentTDegreesOfFreedom { value: df })
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
    if let Some(q) = map.get(&order) {
        return std::sync::Arc::clone(q);
    }
    // Resolve the order BEFORE caching: an unsupported order falls back to
    // the default, and the result must be cached under the *substituted*
    // order — caching the default-order quadrature under the requested key
    // would silently serve the wrong order to later callers that request it
    // once it becomes supported, and hides the substitution entirely.
    match GaussHermiteQuadrature::new(order as usize) {
        Ok(q) => {
            let q = std::sync::Arc::new(q);
            map.insert(order, std::sync::Arc::clone(&q));
            q
        }
        Err(_) => {
            warn_unsupported_quadrature_order(order);
            std::sync::Arc::clone(
                map.entry(DEFAULT_QUADRATURE_ORDER)
                    .or_insert_with(|| std::sync::Arc::new(default_quadrature())),
            )
        }
    }
}

fn warn_unsupported_quadrature_order(order: u8) {
    tracing::warn!(
        requested_order = order,
        substituted_order = DEFAULT_QUADRATURE_ORDER,
        "unsupported Gauss-Hermite quadrature order; substituting the default order"
    );
}

fn default_quadrature() -> GaussHermiteQuadrature {
    GaussHermiteQuadrature::new(DEFAULT_QUADRATURE_ORDER as usize).unwrap_or_else(|_| {
        unreachable!("DEFAULT_QUADRATURE_ORDER must be a valid Gauss-Hermite order")
    })
}

/// Select quadrature based on order (uncached — used to populate the cache).
///
/// Emits a warning and substitutes the default order when the requested
/// order is unsupported, so the substitution is never silent.
pub(crate) fn select_quadrature(order: u8) -> GaussHermiteQuadrature {
    GaussHermiteQuadrature::new(order as usize).unwrap_or_else(|_| {
        warn_unsupported_quadrature_order(order);
        default_quadrature()
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

        let student_t = CopulaSpec::student_t(5.0).expect("valid Student-t df");
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
    fn test_student_t_invalid_df_is_rejected() {
        assert!(matches!(
            CopulaSpec::student_t(2.0),
            Err(Error::InvalidStudentTDegreesOfFreedom { .. })
        ));
    }

    #[test]
    fn test_copula_build() {
        // Test Gaussian
        let gaussian = CopulaSpec::gaussian();
        assert!(gaussian.is_gaussian());
        let g_copula = gaussian.build().expect("Gaussian copula should build");
        assert_eq!(g_copula.num_factors(), 1);

        // Test Student-t
        let student_t = CopulaSpec::student_t(5.0).expect("valid Student-t df");
        assert!(student_t.is_student_t());
        let t_copula = student_t.build().expect("Student-t copula should build");
        // [Z, W]: Gaussian systematic factor plus the shared mixing variable.
        assert_eq!(t_copula.num_factors(), 2);

        // Test RFL
        let rfl = CopulaSpec::random_factor_loading(0.1);
        assert!(rfl.is_rfl());
        let rfl_copula = rfl.build().expect("RFL copula should build");
        assert_eq!(rfl_copula.num_factors(), 2);

        // Test Multi-factor
        let mf = CopulaSpec::multi_factor(2);
        assert!(mf.is_multi_factor());
        let mf_copula = mf.build().expect("multi-factor copula should build");
        assert_eq!(mf_copula.num_factors(), 2);
    }

    #[test]
    fn test_deserialized_invalid_student_t_df_is_rejected_on_build() {
        // Simulate config file with invalid df <= 2
        let spec: CopulaSpec =
            serde_json::from_str(r#"{"type":"StudentT","degrees_of_freedom":1.5}"#)
                .expect("should deserialize");
        assert!(matches!(
            spec.build(),
            Err(Error::InvalidStudentTDegreesOfFreedom { .. })
        ));
    }
}
