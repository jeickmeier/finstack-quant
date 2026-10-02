//! Base correlation curves for CDO and CDS tranche pricing.
//!
//! Base correlation is a market-standard model for pricing credit index tranches
//! (CDX, iTraxx). It represents the implied correlation of each tranche with a
//! notional equity tranche (0% attachment), providing a one-factor framework for
//! quoting tranches across different attachment/detachment points.
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
//! Base correlation provides an interpolatable market quotation for equity
//! tranches. Correlation often increases with detachment, but that shape alone
//! does not imply arbitrage-free prices. Pricing must also check expected-loss
//! consistency under the portfolio default and recovery assumptions.
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

/// Shape diagnostics for a base correlation curve.
///
/// Reports decreases in quoted correlation and warnings near its boundaries.
/// A monotonic correlation curve can still imply negative tranche expected
/// losses. This report does not establish absence of financial arbitrage.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CorrelationShapeReport {
    /// Whether correlations are non-decreasing within a tolerance of `1e-9`.
    pub is_monotonic: bool,
    /// Adjacent knots whose correlations decrease beyond the tolerance.
    pub violations: Vec<CorrelationShapeViolation>,
    /// Non-fatal warnings for correlations near zero or one.
    pub warnings: Vec<String>,
    /// Largest decrease in decimal correlation units, or zero when none exists.
    pub max_violation_magnitude: f64,
}

/// An adjacent-knot decrease in a base correlation curve.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrelationShapeViolation {
    /// Lower detachment point in percent of portfolio notional.
    pub k1: f64,
    /// Decimal correlation at the lower detachment point.
    pub corr1: f64,
    /// Higher detachment point in percent of portfolio notional.
    pub k2: f64,
    /// Decimal correlation at the higher detachment point.
    pub corr2: f64,
}

impl CorrelationShapeViolation {
    /// Return the decrease in decimal correlation units.
    pub fn magnitude(&self) -> f64 {
        self.corr1 - self.corr2
    }
}

impl core::fmt::Display for CorrelationShapeViolation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Non-monotonic: β({:.1}%) = {:.4} > β({:.1}%) = {:.4}",
            self.k1, self.corr1, self.k2, self.corr2
        )
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
/// - Correlation shape is diagnostic only; decreasing curves remain representable
///
/// # Invariants
///
/// - Detachment points are finite, strictly increasing, and within `[0, 100]`
/// - Correlations are finite and within `[0, 1]`
/// - Immutable nodes and cached interpolation always describe the same curve
///
/// Quote changes require rebuilding the curve; callers cannot mutate its cache inputs.
///
/// ```compile_fail
/// use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;
/// let mut curve = BaseCorrelationCurve::builder("CDX")
///     .knots([(3.0, 0.25), (7.0, 0.45)]).build().unwrap();
/// curve.correlations[0] = 0.40;
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawBaseCorrelationCurve", into = "RawBaseCorrelationCurve")]
pub struct BaseCorrelationCurve {
    /// Curve identifier (typically index name + maturity)
    id: CurveId,
    /// Detachment points in percent (e.g., 3.0 for a 0-3% tranche)
    detachment_points: Vec<f64>,
    /// Base correlation values corresponding to each detachment point
    correlations: Vec<f64>,
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
        if raw.detachment_points.len() != raw.correlations.len() {
            return Err(crate::Error::Validation(format!(
                "Base correlation curve '{}': detachment_points length {} does not match correlations length {}",
                raw.id,
                raw.detachment_points.len(),
                raw.correlations.len()
            )));
        }
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
    ///
    /// # Arguments
    ///
    /// * `id` - Unique market-data identifier used to register and look up the curve.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> BaseCorrelationCurveBuilder {
        BaseCorrelationCurveBuilder::new(id)
    }

    /// Get the interpolated correlation for a given detachment point.
    ///
    /// Uses linear interpolation between points and flat extrapolation
    /// beyond the curve boundaries.
    ///
    /// # Arguments
    ///
    /// * `detachment_pct` - Tranche detachment in percent of portfolio notional;
    ///   finite out-of-grid coordinates use the nearest boundary correlation.
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

    /// Apply a filtered additive bump to matching detachment points.
    ///
    /// # Arguments
    ///
    /// * `detachment_filter` - Optional detachment points in percent of portfolio
    ///   notional; `None` selects every knot. Finite off-grid values match nothing.
    /// * `points` - Finite additive decimal correlation change, such as `0.01`
    ///   for one correlation point. Bumped correlations are clamped to `[0, 1]`.
    ///
    /// Returns `None` for non-finite inputs or an invalid reconstructed curve.
    /// The resulting correlation shape may be nonmonotonic.
    pub fn apply_bucket_bump(
        &self,
        detachment_filter: Option<&[f64]>,
        points: f64,
    ) -> Option<Self> {
        if !points.is_finite()
            || detachment_filter.is_some_and(|values| values.iter().any(|v| !v.is_finite()))
        {
            return None;
        }
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
        BaseCorrelationCurve::builder(self.id.clone())
            .knots(new_points)
            .build()
            .ok()
    }

    /// Diagnose monotonicity and boundary proximity of the quoted correlations.
    ///
    /// This checks only curve shape. Even a non-decreasing curve can imply
    /// negative tranche expected losses under its portfolio model; pricing and
    /// calibration must validate expected-loss consistency separately.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;
    ///
    /// let curve = BaseCorrelationCurve::builder("CDX")
    ///     .knots([(3.0, 0.25), (7.0, 0.45), (10.0, 0.60)])
    ///     .build()
    ///     .expect("Valid curve");
    /// assert!(curve.validate_shape().is_monotonic);
    /// ```
    #[must_use = "correlation shape diagnostics should be inspected"]
    pub fn validate_shape(&self) -> CorrelationShapeReport {
        let mut violations = Vec::new();
        let mut warnings = Vec::new();
        for (i, (&det, &corr)) in self
            .detachment_points
            .iter()
            .zip(&self.correlations)
            .enumerate()
        {
            if !(0.02..=0.98).contains(&corr) {
                warnings.push(format!(
                    "Correlation {corr:.4} at K={det:.1}% is near a boundary; the pricing model must support its limiting case"
                ));
            }
            if i > 0 && corr < self.correlations[i - 1] - 1e-9 {
                violations.push(CorrelationShapeViolation {
                    k1: self.detachment_points[i - 1],
                    corr1: self.correlations[i - 1],
                    k2: det,
                    corr2: corr,
                });
            }
        }
        CorrelationShapeReport {
            is_monotonic: violations.is_empty(),
            max_violation_magnitude: violations.iter().map(|v| v.magnitude()).fold(0.0, f64::max),
            violations,
            warnings,
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
}

impl BaseCorrelationCurveBuilder {
    /// Create a new builder with the given curve ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique market-data identifier stored on the completed curve.
    pub fn new(id: impl Into<CurveId>) -> Self {
        Self {
            id: id.into(),
            points: Vec::new(),
        }
    }

    /// Add a single detachment-correlation observation.
    ///
    /// # Arguments
    ///
    /// * `detachment_pct` - Detachment in percent of portfolio notional, within `[0, 100]`.
    /// * `correlation` - Decimal implied base correlation in `[0, 1]`.
    pub fn add_point(mut self, detachment_pct: f64, correlation: f64) -> Self {
        self.points.push((detachment_pct, correlation));
        self
    }

    /// Append detachment-correlation observations.
    ///
    /// # Arguments
    ///
    /// * `points` - Pairs of detachment in percent of portfolio notional and decimal
    ///   correlation. Build sorts by detachment and rejects duplicate coordinates.
    pub fn knots<I>(mut self, points: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(points);
        self
    }

    /// Build the base correlation curve.
    ///
    /// Enforces finite detachments in `[0, 100]`, unique knots, and finite
    /// correlations in `[0, 1]`. Decreasing correlations are supported for market
    /// quotes and stress scenarios; use [`BaseCorrelationCurve::validate_shape`]
    /// for a separate shape diagnostic.
    ///
    /// # Errors
    ///
    /// Returns an input error for fewer than two points, invalid coordinates or
    /// correlations, or duplicate detachment points.
    pub fn build(self) -> Result<BaseCorrelationCurve> {
        if self.points.len() < 2 {
            return Err(InputError::TooFewPoints.into());
        }

        // Sort by detachment using total_cmp so NaNs cannot panic.
        let mut sorted_points = self.points;
        sorted_points.sort_by(|a, b| a.0.total_cmp(&b.0));

        for (detachment, corr) in &sorted_points {
            if !detachment.is_finite() || !(0.0..=100.0).contains(detachment) {
                return Err(InputError::Invalid.into());
            }
            if !corr.is_finite() || *corr < 0.0 || *corr > 1.0 {
                return Err(InputError::Invalid.into());
            }
        }

        let (kvec, cvec): (Vec<f64>, Vec<f64>) = sorted_points.into_iter().unzip();

        let interp = super::common::build_interp_allow_any_values(
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
