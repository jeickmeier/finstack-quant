//! Base correlation curves for CDO and CDS tranche pricing.
//!
//! Base correlation is a market-standard model for pricing credit index tranches
//! (CDX, iTraxx). It represents the implied correlation of each tranche with a
//! notional equity tranche (0% attachment), providing a one-factor framework for
//! consistent tranche pricing across different attachment/detachment points.
//!
//! # Financial Concept
//!
//! Base correlation maps detachment points to implied correlations:
//! ```text
//! For a tranche [K₁, K₂]:
//! - Price [0, K₂] tranche using base correlation β(K₂)
//! - Price [0, K₁] tranche using base correlation β(K₁)
//! - Tranche price = Price[0,K₂] - Price[0,K₁]
//!   ```
//!
//! # Why Base Correlation?
//!
//! Base correlation solved the "correlation smile" problem where compound
//! correlation (single correlation for each tranche) produced arbitrage:
//! - **Monotonic**: Base correlation increases with detachment point
//! - **No arbitrage**: Ensures consistent pricing across tranches
//! - **Market standard**: Universally adopted post-2004
//!
//! # Market Construction
//!
//! Base correlation curves are calibrated from:
//! - **Tranche spreads**: Market quotes for standardized tranches (0-3%, 3-7%, etc.)
//! - **Index CDS**: Par spread for the underlying credit index
//! - **Recovery assumptions**: Typically 40% for senior unsecured
//! - **Copula model**: Usually one-factor Gaussian copula
//!
//! # Use Cases
//!
//! - **CDO tranche pricing**: Synthetic CDOs on credit indices
//! - **Bespoke tranche pricing**: Custom attachment/detachment points
//! - **Index tranche trading**: CDX/iTraxx tranche strategies
//! - **Correlation trading**: Long/short different tranches
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;
//!
//! let curve = BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
//!     .knots(vec![(3.0, 0.25), (7.0, 0.45), (10.0, 0.60)])
//!     .build()
//!     .expect("BaseCorrelationCurve builder should succeed");
//! assert!(curve.correlation(5.0) > 0.25);
//! ```
//!
//! # References
//!
//! - **Base Correlation Framework**:
//!   - McGinty, L., Beinstein, E., Ahluwalia, R., & Watts, M. (2004). "Introducing
//!   Base Correlations." JPMorgan Credit Derivatives Strategy.
//! - O'Kane, D., & Livesey, M. (2004). "Base Correlation Explained." Lehman Brothers
//!   Quantitative Credit Research Quarterly, Q1 2004. `docs/REFERENCES.md#o-kane-2008`
//!
//! - **Copula Models**:
//! - Li, D. X. (2000). "On Default Correlation: A Copula Function Approach."
//!   *Journal of Fixed Income*, 9(4), 43-54. `docs/REFERENCES.md#li-2000-gaussian-copula`
//!   - Hull, J., & White, A. (2004). "Valuation of a CDO and an nth to Default CDS
//!   Without Monte Carlo Simulation." *Journal of Derivatives*, 12(2), 8-23.
//!
//! - **Textbooks**:
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*.
//!   Wiley Finance. Chapters 6-8 (Tranche pricing and base correlation). `docs/REFERENCES.md#o-kane-2008`

use crate::error::InputError;
use crate::market_data::bumps::{BumpSpec, Bumpable};
use crate::math::interp::{types::Interp, ExtrapolationPolicy, InterpStyle};
use crate::types::CurveId;
use crate::Result;

/// Absolute detachment tolerance for bucket shocks.
///
/// Scenario detachment filters are already normalized to the same unit as
/// curve detachments before they reach the curve, so bucket matching should be
/// exact apart from floating-point roundoff.
pub const BASE_CORR_DETACHMENT_MATCH_TOLERANCE: f64 = 1.0e-10;

// Arbitrage Validation Types

/// Result of arbitrage-free validation for a base correlation curve.
///
/// Contains detailed information about any violations found during validation,
/// allowing calibration systems to diagnose and fix curve issues.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ArbitrageCheckResult {
    /// Whether the curve passes all arbitrage checks.
    pub is_arbitrage_free: bool,
    /// List of specific violations found.
    pub violations: Vec<ArbitrageViolation>,
    /// Non-fatal warnings (e.g., high correlation near boundaries).
    pub warnings: Vec<String>,
    /// Maximum absolute violation magnitude (for severity assessment).
    pub max_violation_magnitude: f64,
}

impl ArbitrageCheckResult {
    /// Create a passing result with no violations.
    pub fn pass() -> Self {
        Self {
            is_arbitrage_free: true,
            violations: Vec::new(),
            warnings: Vec::new(),
            max_violation_magnitude: 0.0,
        }
    }

    /// Create a failing result with violations.
    pub fn fail(violations: Vec<ArbitrageViolation>) -> Self {
        let max_mag = violations.iter().map(|v| v.magnitude()).fold(0.0, f64::max);
        Self {
            is_arbitrage_free: false,
            violations,
            warnings: Vec::new(),
            max_violation_magnitude: max_mag,
        }
    }

    /// Add multiple warnings.
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings.extend(warnings);
        self
    }
}

/// Specific arbitrage violation in a base correlation curve.
///
/// Each violation type captures the relevant data points involved.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ArbitrageViolation {
    /// Correlation decreases as detachment increases (violates monotonicity).
    ///
    /// Base correlation must be non-decreasing: senior tranches cannot have
    /// lower correlation than junior tranches.
    NonMonotonicCorrelation {
        /// Lower detachment point (percent)
        k1: f64,
        /// Correlation at lower detachment
        corr1: f64,
        /// Higher detachment point (percent)
        k2: f64,
        /// Correlation at higher detachment (should be >= corr1)
        corr2: f64,
    },

    /// Correlation outside valid range [0, 1].
    InvalidCorrelationBounds {
        /// Detachment point (percent)
        detachment: f64,
        /// Invalid correlation value
        correlation: f64,
    },

    /// Correlation too close to boundary (may cause numerical issues).
    BoundaryCorrelation {
        /// Detachment point (percent)
        detachment: f64,
        /// Correlation value near boundary
        correlation: f64,
        /// Whether near 0 or 1
        near_zero: bool,
    },
}

impl ArbitrageViolation {
    /// Get the magnitude of the violation for severity assessment.
    pub fn magnitude(&self) -> f64 {
        match self {
            ArbitrageViolation::NonMonotonicCorrelation { corr1, corr2, .. } => {
                (corr1 - corr2).abs()
            }
            ArbitrageViolation::InvalidCorrelationBounds { correlation, .. } => {
                if *correlation < 0.0 {
                    -correlation
                } else {
                    correlation - 1.0
                }
            }
            ArbitrageViolation::BoundaryCorrelation { correlation, .. } => {
                if *correlation < 0.05 {
                    0.05 - correlation
                } else {
                    correlation - 0.95
                }
            }
        }
    }
}

impl core::fmt::Display for ArbitrageViolation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ArbitrageViolation::NonMonotonicCorrelation {
                k1,
                corr1,
                k2,
                corr2,
            } => {
                write!(
                    f,
                    "Non-monotonic: β({:.1}%) = {:.4} > β({:.1}%) = {:.4}",
                    k1, corr1, k2, corr2
                )
            }
            ArbitrageViolation::InvalidCorrelationBounds {
                detachment,
                correlation,
            } => {
                write!(
                    f,
                    "Invalid correlation {:.4} at K={:.1}% (must be in [0, 1])",
                    correlation, detachment
                )
            }
            ArbitrageViolation::BoundaryCorrelation {
                detachment,
                correlation,
                near_zero,
            } => {
                let boundary = if *near_zero { "0" } else { "1" };
                write!(
                    f,
                    "Boundary correlation {:.4} near {} at K={:.1}%",
                    correlation, boundary, detachment
                )
            }
        }
    }
}

/// Base correlation curve for CDO/CDS index tranche pricing.
///
/// Maps tranche detachment points (in percent) to implied base correlations
/// used in one-factor Gaussian copula models. Base correlation is the market
/// standard for quoting credit index tranches (CDX, iTraxx).
///
/// # Model
///
/// Base correlation β(K) is defined such that:
/// ```text
/// Price of [0, K] tranche = f(β(K), other parameters)
///
/// For tranche [K₁, K₂]:
/// Tranche value = Price[0,K₂](β(K₂)) - Price[0,K₁](β(K₁))
/// ```
///
/// # Interpolation
///
/// - Linear interpolation between quoted detachment points
/// - Flat extrapolation beyond curve boundaries
/// - Ensures base correlation is monotonically increasing (validated at construction)
///
/// # Invariants
///
/// - Detachment points are strictly increasing
/// - Correlations ∈ [0, 1]
/// - Base correlation typically increases with detachment (equity < mezzanine < senior)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawBaseCorrelationCurve", into = "RawBaseCorrelationCurve")]
pub struct BaseCorrelationCurve {
    /// Curve identifier (typically index name + maturity)
    pub id: CurveId,
    /// Detachment points in percent (e.g., 3.0 for a 0-3% tranche)
    pub detachment_points: Vec<f64>,
    /// Base correlation values corresponding to each detachment point
    pub correlations: Vec<f64>,
    /// Interpolator for base correlations
    interp: Interp,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawBaseCorrelationCurve {
    id: CurveId,
    detachment_points: Vec<f64>,
    correlations: Vec<f64>,
}

impl From<BaseCorrelationCurve> for RawBaseCorrelationCurve {
    fn from(curve: BaseCorrelationCurve) -> Self {
        RawBaseCorrelationCurve {
            id: curve.id,
            detachment_points: curve.detachment_points,
            correlations: curve.correlations,
        }
    }
}

impl TryFrom<RawBaseCorrelationCurve> for BaseCorrelationCurve {
    type Error = crate::Error;

    fn try_from(raw: RawBaseCorrelationCurve) -> crate::Result<Self> {
        let points: Vec<(f64, f64)> = raw
            .detachment_points
            .iter()
            .copied()
            .zip(raw.correlations.iter().copied())
            .collect();

        BaseCorrelationCurve::builder(raw.id).knots(points).build()
    }
}

impl BaseCorrelationCurve {
    /// Create a new base correlation curve builder.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> BaseCorrelationCurveBuilder {
        BaseCorrelationCurveBuilder::new(id)
    }

    /// Get the interpolated correlation for a given detachment point.
    ///
    /// Uses linear interpolation between points and flat extrapolation
    /// beyond the curve boundaries.
    pub fn correlation(&self, detachment_pct: f64) -> f64 {
        self.interp.interp(detachment_pct)
    }

    /// Raw detachment points (percent) used to build the curve.
    ///
    /// Returns the detachment points in ascending order, corresponding 1:1
    /// with the values returned by [`correlations()`](Self::correlations).
    pub fn detachment_points(&self) -> &[f64] {
        &self.detachment_points
    }

    /// Raw correlation values at each detachment point.
    ///
    /// Returns base correlation values in `[0, 1]`, corresponding 1:1 with
    /// the detachment points from [`detachment_points()`](Self::detachment_points).
    pub fn correlations(&self) -> &[f64] {
        &self.correlations
    }

    /// Number of knot points (detachment/correlation pairs) in the curve.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.detachment_points.len()
    }

    /// Returns `true` if the curve has no knot points.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.detachment_points.is_empty()
    }

    /// Interpolation style used by this curve (always Linear for base correlation).
    #[inline]
    pub fn interp_style(&self) -> InterpStyle {
        self.interp.style()
    }

    /// Extrapolation policy used by this curve.
    #[inline]
    pub fn extrapolation(&self) -> ExtrapolationPolicy {
        self.interp.extrapolation()
    }
}

impl BaseCorrelationCurve {
    /// Curve identifier (typically index name + maturity, e.g. "CDX.NA.IG.42_5Y").
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Apply a filtered bucket bump to matching detachment points (additive points).
    pub fn apply_bucket_bump(
        &self,
        detachment_filter: Option<&[f64]>,
        points: f64,
    ) -> Option<Self> {
        let new_points: Vec<(f64, f64)> = self
            .detachment_points
            .iter()
            .copied()
            .zip(self.correlations.iter().copied())
            .map(|(det, corr)| {
                let matches = detachment_filter
                    .map(|flt| {
                        flt.iter()
                            .any(|d| (d - det).abs() <= BASE_CORR_DETACHMENT_MATCH_TOLERANCE)
                    })
                    .unwrap_or(true);
                if matches {
                    (det, (corr + points).clamp(0.0, 1.0))
                } else {
                    (det, corr)
                }
            })
            .collect();
        // Bucket bumps may legitimately break monotonicity (e.g., shocking
        // only a subset of detachment points in a stress test).  Allow
        // non-monotonic construction here; callers should check
        // `validate_arbitrage_free` before pricing off a bumped curve.
        BaseCorrelationCurve::builder(self.id.clone())
            .knots(new_points)
            .allow_non_monotonic()
            .build()
            .ok()
    }

    // Arbitrage Validation

    /// Validate that the curve is arbitrage-free.
    ///
    /// Checks for:
    /// 1. Monotonicity: β(K₁) ≤ β(K₂) for K₁ < K₂
    /// 2. Valid bounds: 0 ≤ β(K) ≤ 1 for all K
    /// 3. Boundary warnings: correlations very close to 0 or 1
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;
    ///
    /// let curve = BaseCorrelationCurve::builder("CDX")
    ///     .knots(vec![(3.0, 0.25), (7.0, 0.45), (10.0, 0.60)])
    ///     .build()
    ///     .expect("Valid curve");
    ///
    /// let result = curve.validate_arbitrage_free();
    /// assert!(result.is_arbitrage_free);
    /// ```
    #[must_use = "arbitrage check result should be inspected"]
    pub fn validate_arbitrage_free(&self) -> ArbitrageCheckResult {
        let mut violations = Vec::new();
        let mut warnings = Vec::new();

        for (i, (&det, &corr)) in self
            .detachment_points
            .iter()
            .zip(&self.correlations)
            .enumerate()
        {
            if !(0.0..=1.0).contains(&corr) {
                violations.push(ArbitrageViolation::InvalidCorrelationBounds {
                    detachment: det,
                    correlation: corr,
                });
            }

            if corr < 0.02 {
                warnings.push(format!(
                    "Low correlation {:.4} at K={:.1}% may cause numerical issues",
                    corr, det
                ));
            } else if corr > 0.98 {
                warnings.push(format!(
                    "High correlation {:.4} at K={:.1}% may cause numerical issues",
                    corr, det
                ));
            }

            if i > 0 {
                let prev_corr = self.correlations[i - 1];
                if corr < prev_corr - 1e-9 {
                    violations.push(ArbitrageViolation::NonMonotonicCorrelation {
                        k1: self.detachment_points[i - 1],
                        corr1: prev_corr,
                        k2: det,
                        corr2: corr,
                    });
                }
            }
        }

        if violations.is_empty() {
            ArbitrageCheckResult::pass().with_warnings(warnings)
        } else {
            ArbitrageCheckResult::fail(violations).with_warnings(warnings)
        }
    }
}

/// Builder for creating base correlation curves.
///
/// # Examples
/// ```rust
/// use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;
///
/// let curve = BaseCorrelationCurve::builder("CDX")
///     .knots([(3.0, 0.25), (7.0, 0.45)])
///     .build()
///     .expect("BaseCorrelationCurve builder should succeed");
/// assert!(curve.correlation(5.0) > 0.25);
/// ```
pub struct BaseCorrelationCurveBuilder {
    id: CurveId,
    points: Vec<(f64, f64)>, // (detachment_pct, correlation)
    /// When `true`, skip the post-build monotonicity / bounds check.
    /// Default is `false`, meaning the builder rejects non-monotonic curves.
    allow_non_monotonic: bool,
}

impl BaseCorrelationCurveBuilder {
    /// Create a new builder with the given curve ID.
    pub fn new(id: impl Into<CurveId>) -> Self {
        Self {
            id: id.into(),
            points: Vec::new(),
            allow_non_monotonic: false,
        }
    }

    /// Allow the curve to be non-monotonic (skip arbitrage-free validation on build).
    ///
    /// By default, `build()` rejects curves that violate monotonicity or
    /// correlation bounds.  Call this method to bypass that check, for example
    /// when a bucket bump shocks only a subset of detachment points.  Curves
    /// built this way should be checked with
    /// [`BaseCorrelationCurve::validate_arbitrage_free`] before pricing.
    pub fn allow_non_monotonic(mut self) -> Self {
        self.allow_non_monotonic = true;
        self
    }

    /// Add a single point (detachment_pct, correlation).
    pub fn add_point(mut self, detachment_pct: f64, correlation: f64) -> Self {
        self.points.push((detachment_pct, correlation));
        self
    }

    /// Set all knot points at once.
    pub fn knots<I>(mut self, points: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(points);
        self
    }

    /// Build the base correlation curve.
    ///
    /// Unless [`allow_non_monotonic`](Self::allow_non_monotonic) has been called,
    /// the builder validates that the resulting curve is arbitrage-free
    /// (monotonic correlations within `[0, 1]`).
    pub fn build(self) -> Result<BaseCorrelationCurve> {
        if self.points.len() < 2 {
            return Err(InputError::TooFewPoints.into());
        }

        let allow_non_monotonic = self.allow_non_monotonic;

        // Sort by detachment using total_cmp so NaNs cannot panic.
        let mut sorted_points = self.points;
        sorted_points.sort_by(|a, b| a.0.total_cmp(&b.0));

        for (detachment, corr) in &sorted_points {
            if !detachment.is_finite() || *detachment < 0.0 {
                return Err(InputError::Invalid.into());
            }
            if !corr.is_finite() || *corr < 0.0 || *corr > 1.0 {
                return Err(InputError::Invalid.into());
            }
        }

        let (kvec, cvec): (Vec<f64>, Vec<f64>) = sorted_points.into_iter().unzip();

        let interp = super::common::build_interp(
            InterpStyle::Linear,
            kvec.clone().into_boxed_slice(),
            cvec.clone().into_boxed_slice(),
            ExtrapolationPolicy::FlatZero,
        )?;

        let curve = BaseCorrelationCurve {
            id: self.id,
            detachment_points: kvec,
            correlations: cvec,
            interp,
        };

        // Arbitrage-free validation (unless explicitly opted out)
        if !allow_non_monotonic {
            let check = curve.validate_arbitrage_free();
            let hard_violations: Vec<_> = check
                .violations
                .iter()
                .filter(|v| {
                    matches!(
                        v,
                        ArbitrageViolation::NonMonotonicCorrelation { .. }
                            | ArbitrageViolation::InvalidCorrelationBounds { .. }
                    )
                })
                .collect();

            if !hard_violations.is_empty() {
                let descriptions: Vec<String> =
                    hard_violations.iter().map(|v| v.to_string()).collect();
                return Err(crate::Error::Validation(format!(
                    "Base correlation curve is not arbitrage-free: {}. \
                     Use .allow_non_monotonic() to bypass this check.",
                    descriptions.join("; ")
                )));
            }
        }

        Ok(curve)
    }
}

impl Bumpable for BaseCorrelationCurve {
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        spec.validate_parallel("BaseCorrelationCurve")?;

        let (raw_val, is_multiplicative) = spec.resolve_standard_values_or_error(
            "BaseCorrelationCurve",
            "only supports Additive/{Percent,Fraction} or Multiplicative/Factor",
        )?;
        let (add, mul) = if is_multiplicative {
            (0.0, raw_val)
        } else {
            (raw_val, 1.0)
        };

        let bumped_id = spec.standard_bump_id(self.id());

        let mut bumped_points = Vec::with_capacity(self.detachment_points().len());
        for (&d, &c) in self
            .detachment_points()
            .iter()
            .zip(self.correlations().iter())
        {
            bumped_points.push((d, (c * mul + add).clamp(0.0, 1.0)));
        }

        BaseCorrelationCurve::builder(bumped_id)
            .knots(bumped_points)
            .build()
    }
}
