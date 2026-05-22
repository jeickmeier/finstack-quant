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
//! ```
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
//! use finstack_core::market_data::term_structures::BaseCorrelationCurve;
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
//!     Base Correlations." JPMorgan Credit Derivatives Strategy.
//!   - O'Kane, D., & Livesey, M. (2004). "Base Correlation Explained." Lehman Brothers
//!     Quantitative Credit Research Quarterly, Q1 2004.
//!
//! - **Copula Models**:
//!   - Li, D. X. (2000). "On Default Correlation: A Copula Function Approach."
//!     *Journal of Fixed Income*, 9(4), 43-54.
//!   - Hull, J., & White, A. (2004). "Valuation of a CDO and an nth to Default CDS
//!     Without Monte Carlo Simulation." *Journal of Derivatives*, 12(2), 8-23.
//!
//! - **Textbooks**:
//!   - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*.
//!     Wiley Finance. Chapters 6-8 (Tranche pricing and base correlation).

use crate::error::InputError;
use crate::math::interp::{types::Interp, ExtrapolationPolicy, InterpStyle};
use crate::types::CurveId;
use crate::Result;

/// Absolute detachment tolerance for bucket shocks.
///
/// Scenario detachment filters are already normalized to the same unit as
/// curve detachments before they reach the curve, so bucket matching should be
/// exact apart from floating-point roundoff.
pub const BASE_CORR_DETACHMENT_MATCH_TOLERANCE: f64 = 1.0e-10;

// ============================================================================
// Arbitrage Validation Types
// ============================================================================

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

    /// Add a warning to the result.
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
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

// ============================================================================
// Smoothing Methods
// ============================================================================

/// Smoothing method for enforcing arbitrage-free base correlation curves.
///
/// When calibration produces non-monotonic correlations, these methods
/// can be used to create an arbitrage-free curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SmoothingMethod {
    /// No smoothing - use raw calibrated values.
    #[default]
    None,

    /// Pool Adjacent Violators Algorithm (isotonic regression).
    ///
    /// Finds the closest monotonically increasing curve to the raw data
    /// in a least-squares sense. Fast and theoretically optimal.
    IsotonicRegression,

    /// Enforce strict monotonicity by adjusting violations.
    ///
    /// Each point is set to max(current, previous + epsilon).
    /// Simple but may accumulate adjustments.
    StrictMonotonic,

    /// Weighted moving average with monotonicity constraint.
    ///
    /// Smooths the curve while ensuring non-decreasing correlations.
    WeightedSmoothing,
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
        // non-monotonic construction here; callers can re-validate if needed.
        BaseCorrelationCurve::builder(self.id.clone())
            .knots(new_points)
            .allow_non_monotonic()
            .build()
            .ok()
    }

    // ========================================================================
    // Arbitrage Validation
    // ========================================================================

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
    /// use finstack_core::market_data::term_structures::BaseCorrelationCurve;
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

        // Check correlation bounds
        for (i, (&det, &corr)) in self
            .detachment_points
            .iter()
            .zip(&self.correlations)
            .enumerate()
        {
            // Check valid range
            if !(0.0..=1.0).contains(&corr) {
                violations.push(ArbitrageViolation::InvalidCorrelationBounds {
                    detachment: det,
                    correlation: corr,
                });
            }

            // Check boundary proximity (warning only)
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

            // Check monotonicity
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

    /// Check if the curve is monotonically non-decreasing.
    #[must_use]
    pub fn is_monotonic(&self) -> bool {
        self.correlations.windows(2).all(|w| w[1] >= w[0] - 1e-9)
    }

    // ========================================================================
    // Smoothing Methods
    // ========================================================================

    /// Apply smoothing to create an arbitrage-free curve.
    ///
    /// If the curve is already arbitrage-free, returns a clone.
    /// Otherwise, applies the specified smoothing method.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_core::market_data::term_structures::{
    ///     BaseCorrelationCurve, SmoothingMethod
    /// };
    ///
    /// // Create a non-monotonic curve (opt in to allow non-monotonic data)
    /// let raw = BaseCorrelationCurve::builder("TEST")
    ///     .knots(vec![(3.0, 0.50), (7.0, 0.40), (10.0, 0.60)])
    ///     .allow_non_monotonic()
    ///     .build()
    ///     .expect("Valid curve");
    ///
    /// let smoothed = raw.apply_smoothing(SmoothingMethod::IsotonicRegression)
    ///     .expect("Smoothing should succeed");
    ///
    /// assert!(smoothed.is_monotonic());
    /// ```
    pub fn apply_smoothing(&self, method: SmoothingMethod) -> Result<Self> {
        match method {
            SmoothingMethod::None => Ok(self.clone()),
            SmoothingMethod::IsotonicRegression => self.apply_isotonic_regression(),
            SmoothingMethod::StrictMonotonic => self.apply_strict_monotonic(),
            SmoothingMethod::WeightedSmoothing => self.apply_weighted_smoothing(),
        }
    }

    /// Apply Pool Adjacent Violators Algorithm (PAVA) for isotonic regression.
    ///
    /// This finds the closest monotonically non-decreasing sequence to the
    /// input correlations in the L2 sense.
    fn apply_isotonic_regression(&self) -> Result<Self> {
        if self.correlations.is_empty() {
            return Ok(self.clone());
        }

        let n = self.correlations.len();
        let mut smoothed = self.correlations.clone();

        // PAVA: Pool Adjacent Violators Algorithm (single-pass pool-merge)
        // Each pool is (start_index, element_count, value_sum).
        let mut pools: Vec<(usize, usize, f64)> = Vec::with_capacity(n);
        for (i, &val) in smoothed.iter().enumerate() {
            pools.push((i, 1, val));
            while pools.len() > 1 {
                let tail = pools.len() - 1;
                let avg_curr = pools[tail].2 / pools[tail].1 as f64;
                let avg_prev = pools[tail - 1].2 / pools[tail - 1].1 as f64;
                if avg_prev > avg_curr {
                    if let Some(absorbed) = pools.pop() {
                        if let Some(prev) = pools.last_mut() {
                            prev.1 += absorbed.1;
                            prev.2 += absorbed.2;
                        }
                    }
                } else {
                    break;
                }
            }
        }
        for &(start, count, sum) in &pools {
            let avg = sum / count as f64;
            for item in smoothed.iter_mut().skip(start).take(count) {
                *item = avg;
            }
        }

        // Clamp to valid range
        for c in &mut smoothed {
            *c = c.clamp(0.001, 0.999);
        }

        // Enforce final monotonicity (should already be satisfied, but ensure)
        for i in 1..n {
            if smoothed[i] < smoothed[i - 1] {
                smoothed[i] = smoothed[i - 1];
            }
        }

        let points: Vec<(f64, f64)> = self
            .detachment_points
            .iter()
            .copied()
            .zip(smoothed)
            .collect();

        BaseCorrelationCurve::builder(self.id.clone())
            .knots(points)
            .build()
    }

    /// Apply strict monotonicity enforcement.
    ///
    /// Each correlation is set to max(current, previous + epsilon).
    fn apply_strict_monotonic(&self) -> Result<Self> {
        const EPSILON: f64 = 1e-6;

        if self.correlations.is_empty() {
            return Ok(self.clone());
        }

        let mut smoothed = self.correlations.clone();

        // Forward pass: ensure non-decreasing
        for i in 1..smoothed.len() {
            let min_val = smoothed[i - 1] + EPSILON;
            if smoothed[i] < min_val {
                smoothed[i] = min_val;
            }
        }

        // Clamp to valid range
        for c in &mut smoothed {
            *c = c.clamp(0.001, 0.999);
        }

        let points: Vec<(f64, f64)> = self
            .detachment_points
            .iter()
            .copied()
            .zip(smoothed)
            .collect();

        BaseCorrelationCurve::builder(self.id.clone())
            .knots(points)
            .build()
    }

    /// Apply weighted smoothing with monotonicity constraint.
    ///
    /// Uses a weighted average that respects the monotonicity requirement.
    fn apply_weighted_smoothing(&self) -> Result<Self> {
        if self.correlations.len() < 3 {
            return self.apply_strict_monotonic();
        }

        let n = self.correlations.len();
        let mut smoothed = vec![0.0; n];

        // First and last points: keep original (or apply light smoothing)
        smoothed[0] = self.correlations[0];
        smoothed[n - 1] = self.correlations[n - 1];

        // Interior points: weighted average of neighbors
        for (i, smoothed_val) in smoothed.iter_mut().enumerate().take(n - 1).skip(1) {
            // Weights: 0.25 * prev + 0.5 * current + 0.25 * next
            let avg = 0.25 * self.correlations[i - 1]
                + 0.5 * self.correlations[i]
                + 0.25 * self.correlations[i + 1];
            *smoothed_val = avg;
        }

        // Enforce monotonicity
        for i in 1..n {
            if smoothed[i] < smoothed[i - 1] {
                smoothed[i] = smoothed[i - 1] + 1e-6;
            }
        }

        // Clamp to valid range
        for c in &mut smoothed {
            *c = c.clamp(0.001, 0.999);
        }

        let points: Vec<(f64, f64)> = self
            .detachment_points
            .iter()
            .copied()
            .zip(smoothed)
            .collect();

        BaseCorrelationCurve::builder(self.id.clone())
            .knots(points)
            .build()
    }

    /// Create an arbitrage-free version of this curve.
    ///
    /// Convenience method that validates and applies smoothing if needed.
    pub fn make_arbitrage_free(&self, method: SmoothingMethod) -> Result<Self> {
        let validation = self.validate_arbitrage_free();
        if validation.is_arbitrage_free {
            Ok(self.clone())
        } else {
            self.apply_smoothing(method)
        }
    }
}

/// Builder for creating base correlation curves.
///
/// # Examples
/// ```rust
/// use finstack_core::market_data::term_structures::BaseCorrelationCurve;
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
    /// when constructing a curve that will subsequently be smoothed.
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

        // Sort by detachment point deterministically (panic-free even with NaNs).
        let mut sorted_points = self.points;
        sorted_points.sort_by(|a, b| a.0.total_cmp(&b.0));

        // Validate points
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
