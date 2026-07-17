//! Volatility index forward curves for VIX and similar indices.
//!
//! A volatility index curve represents expected future levels of implied volatility
//! indices such as the CBOE VIX (S&P 500), VXN (NASDAQ-100), or VSTOXX (EURO STOXX 50).
//! These curves are essential for pricing volatility derivatives including VIX futures
//! and options.
//!
//! # Financial Concept
//!
//! Volatility indices measure market expectations of near-term volatility, typically
//! calculated from options prices on an underlying index. The VIX, for example,
//! represents the market's expectation of 30-day forward-looking volatility.
//!
//! The forward volatility level F(t) represents the expected index level at time t:
//! ```text
//! F(t) = market expectation of volatility index at time t
//! ```
//!
//! # Market Construction
//!
//! Volatility index curves are typically bootstrapped from:
//! - **VIX Futures**: Monthly and weekly futures on the volatility index
//! - **VIX Options**: OTC and exchange-traded options on the index
//! - **Spot level**: Current index value (typically around 12-30 for VIX)
//!
//! # Term Structure Characteristics
//!
//! Unlike interest rate curves, volatility index curves typically exhibit:
//! - **Contango**: Forward levels higher than spot (normal market conditions)
//! - **Backwardation**: Forward levels lower than spot (during volatility spikes)
//! - **Mean reversion**: Long-dated forwards converge to historical average
//!
//! # Use Cases
//!
//! - **VIX futures pricing**: Calculate fair value vs. quoted price
//! - **VIX option pricing**: Forward level is the underlying for Black model
//! - **Volatility swap valuation**: Project future volatility levels
//! - **Risk management**: Stress testing and scenario analysis
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::market_data::term_structures::VolatilityIndexCurve;
//! use finstack_quant_core::math::interp::InterpStyle;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let curve = VolatilityIndexCurve::builder("VIX")
//!     .base_date(base)
//!     .spot_level(18.5)
//!     .knots([(0.0, 18.5), (0.25, 20.0), (0.5, 21.5), (1.0, 22.0)])
//!     .interp(InterpStyle::Linear)
//!     .build()
//!     .expect("VolatilityIndexCurve builder should succeed");
//! assert!(curve.forward_level(0.25) > 0.0);
//! ```
//!
//! # References
//!
//! - Whaley, R. E. (2009). "Understanding the VIX." *Journal of Portfolio Management*,
//!   35(3), 98-105.
//! - CBOE (2019). "VIX White Paper." CBOE Global Markets.
//! - Carr, P., & Wu, L. (2006). "A Tale of Two Indices." *Journal of Derivatives*,
//!   13(3), 13-29.

use super::common::{
    build_interp_allow_any_values, bump_knots_parallel, bump_knots_percentage,
    bump_knots_triangular, default_curve_base_date, infer_spot_from_knots, roll_knots,
    split_points, validate_non_negative_knots, year_fraction_to,
};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
use crate::{
    dates::{Date, DayCount},
    error::InputError,
    market_data::traits::TermStructure,
    math::interp::types::Interp,
    types::CurveId,
};

/// Volatility index forward curve for indices like VIX, VXN, VSTOXX.
///
/// Represents expected future volatility index levels. Stores forward levels at
/// knot times and interpolates between them.
///
/// # Index Characteristics
///
/// - **Spot level**: Current index value (e.g., VIX = 18.5)
/// - **Forward levels**: Expected future index values
/// - **Mean reversion**: Long-dated forwards typically revert to ~20-25 for VIX
///
/// # Thread Safety
///
/// Immutable after construction; safe to share via `Arc<VolatilityIndexCurve>`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawVolatilityIndexCurve", into = "RawVolatilityIndexCurve")]
pub struct VolatilityIndexCurve {
    id: CurveId,
    base: Date,
    /// Day-count basis used for time calculations.
    day_count: DayCount,
    /// Spot index level (t=0).
    spot_level: f64,
    /// Knot times in **years** (strictly increasing, first should be 0.0).
    knots: Box<[f64]>,
    /// Forward volatility index levels at each knot.
    levels: Box<[f64]>,
    interp: Interp,
}

/// Raw serializable state of VolatilityIndexCurve
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVolatilityIndexCurve {
    /// Curve identifier
    pub id: String,
    /// Base date
    pub base: Date,
    /// Day count convention
    pub day_count: DayCount,
    /// Spot index level
    pub spot_level: f64,
    /// Time/value pairs used to construct the curve
    pub knot_points: Vec<(f64, f64)>,
    /// Interpolation style
    pub interp_style: InterpStyle,
    /// Extrapolation policy
    pub extrapolation: ExtrapolationPolicy,
}

impl From<VolatilityIndexCurve> for RawVolatilityIndexCurve {
    fn from(curve: VolatilityIndexCurve) -> Self {
        let knot_points: Vec<(f64, f64)> = curve
            .knots
            .iter()
            .zip(curve.levels.iter())
            .map(|(&t, &lvl)| (t, lvl))
            .collect();

        RawVolatilityIndexCurve {
            id: curve.id.to_string(),
            base: curve.base,
            day_count: curve.day_count,
            spot_level: curve.spot_level,
            knot_points,
            interp_style: curve.interp.style(),
            extrapolation: curve.interp.extrapolation(),
        }
    }
}

impl TryFrom<RawVolatilityIndexCurve> for VolatilityIndexCurve {
    type Error = crate::Error;

    fn try_from(state: RawVolatilityIndexCurve) -> crate::Result<Self> {
        VolatilityIndexCurve::builder(state.id)
            .base_date(state.base)
            .day_count(state.day_count)
            .spot_level(state.spot_level)
            .knots(state.knot_points)
            .interp(state.interp_style)
            .extrapolation(state.extrapolation)
            .build()
    }
}

impl VolatilityIndexCurve {
    /// Start building a volatility index curve for the given `id`.
    ///
    /// **Defaults:** Linear interpolation with Flat extrapolation maintains
    /// stable tail levels consistent with mean reversion expectations.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> VolatilityIndexCurveBuilder {
        VolatilityIndexCurveBuilder {
            id: id.into(),
            base: default_curve_base_date(),
            base_is_set: false,
            day_count: DayCount::Act365F,
            spot_level: None,
            points: Vec::new(),
            style: InterpStyle::Linear,
            extrapolation: ExtrapolationPolicy::FlatZero,
        }
    }

    /// Forward volatility index level at time `t` (in years from base date).
    ///
    /// # Returns
    /// The interpolated forward index level at time `t`.
    #[must_use]
    #[inline]
    pub fn forward_level(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return self.spot_level;
        }
        self.interp.interp(t)
    }

    /// Forward volatility index level on a specific calendar date.
    ///
    /// # Errors
    /// Returns an error if the date is before the base date.
    pub fn forward_level_on_date(&self, date: Date) -> crate::Result<f64> {
        if date < self.base {
            return Err(crate::Error::Validation(format!(
                "Date {} is before curve base date {}",
                date, self.base
            )));
        }
        if date == self.base {
            return Ok(self.spot_level);
        }
        let t = year_fraction_to(self.base, date, self.day_count)?;
        Ok(self.forward_level(t))
    }

    /// Current spot level of the volatility index.
    #[must_use]
    #[inline]
    pub fn spot_level(&self) -> f64 {
        self.spot_level
    }

    /// Day-count convention used for this curve.
    #[inline]
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Raw knot times used to construct the curve.
    #[inline]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Forward index levels at each knot.
    #[inline]
    pub fn levels(&self) -> &[f64] {
        &self.levels
    }

    /// Curve identifier.
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Valuation **base date**.
    #[inline]
    pub fn base_date(&self) -> Date {
        self.base
    }

    /// Interpolation style used by this curve.
    #[inline]
    pub fn interp_style(&self) -> InterpStyle {
        self.interp.style()
    }

    /// Extrapolation policy used by this curve.
    #[inline]
    pub fn extrapolation(&self) -> ExtrapolationPolicy {
        self.interp.extrapolation()
    }

    /// Number of knot points in the curve.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.knots.len()
    }

    /// Returns `true` if the curve has no knot points.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.knots.is_empty()
    }

    /// Create a builder pre-populated with this curve's data but a new ID.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> VolatilityIndexCurveBuilder {
        VolatilityIndexCurve::builder(new_id)
            .base_date(self.base)
            .day_count(self.day_count)
            .spot_level(self.spot_level)
            .knots(self.knots.iter().copied().zip(self.levels.iter().copied()))
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
    }

    /// Create a new curve with a parallel bump applied (additive, in index points).
    ///
    /// # Arguments
    /// * `bump` - Bump size in index points (e.g., 1.0 adds 1 point to all levels)
    ///
    /// # Returns
    /// A new volatility index curve with all levels shifted.
    ///
    /// `bump` is expressed in index points, not a percentage or implied-vol
    /// decimal. It shifts every forward level and the spot level; spot is
    /// floored at zero, while the rebuilt curve also rejects any negative knot
    /// level. This is a level scenario for VIX-like indices, not a shock to an
    /// option-implied volatility surface.
    ///
    /// # Errors
    ///
    /// Propagates reconstruction errors if the shock produces a negative or
    /// non-finite forward level, invalid knots, or an interpolation failure.
    /// The returned curve otherwise preserves the base date, day count, and
    /// interpolation/extrapolation conventions.
    pub fn with_parallel_bump(&self, bump: f64) -> crate::Result<Self> {
        let bumped_points = bump_knots_parallel(&self.knots, &self.levels, bump);
        let new_id = crate::market_data::bumps::id_bump_bp(self.id.as_str(), bump * 100.0);

        VolatilityIndexCurve::builder(new_id)
            .base_date(self.base)
            .day_count(self.day_count)
            .spot_level((self.spot_level + bump).max(0.0))
            .knots(bumped_points)
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
            .build()
    }

    /// Create a new curve with a percentage bump applied (multiplicative).
    ///
    /// # Arguments
    /// * `pct` - Percentage bump (e.g., 0.10 = +10%, -0.05 = -5%)
    ///
    /// # Returns
    /// A new volatility index curve with all levels scaled.
    ///
    /// Each forward level is multiplied by `1 + pct`; spot uses the same
    /// multiplier and is floored at zero. A finite shock below `-100%` can make
    /// knot levels negative and is therefore rejected during rebuilding rather
    /// than silently converted to a valid but different curve.
    ///
    /// # Errors
    ///
    /// Propagates reconstruction errors if `pct` produces negative/non-finite
    /// levels, invalid knots, or an interpolation failure. No separate range is
    /// imposed on `pct`; the non-negative-level invariant is the authoritative
    /// validation boundary.
    pub fn with_percentage_bump(&self, pct: f64) -> crate::Result<Self> {
        let bumped_points = bump_knots_percentage(&self.knots, &self.levels, pct);
        let new_id = format!("{}+{:.2}%", self.id.as_str(), pct * 100.0);

        VolatilityIndexCurve::builder(new_id)
            .base_date(self.base)
            .day_count(self.day_count)
            .spot_level((self.spot_level * (1.0 + pct)).max(0.0))
            .knots(bumped_points)
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
            .build()
    }

    /// Create a new curve with a triangular key-rate bump at a specific tenor.
    ///
    /// # Arguments
    /// * `prev_bucket` - Previous bucket time in years; `None` for the first bucket
    /// * `target_bucket` - Target bucket time in years (peak of the triangle)
    /// * `next_bucket` - Next bucket time in years; `None` for the last bucket
    /// * `bump` - Bump size in index points
    ///
    /// # Returns
    /// A new volatility index curve with the triangular key-rate bump applied.
    ///
    /// The shock reaches `bump` at `target_bucket`, fades linearly to zero at
    /// the neighbor buckets, and is zero outside that interval. It leaves spot
    /// unchanged because this is a forward-bucket sensitivity. With fewer than
    /// two knots it falls back to [`with_parallel_bump`](Self::with_parallel_bump).
    ///
    /// # Errors
    ///
    /// Propagates reconstruction errors if the triangular shock creates a
    /// negative/non-finite level, invalid interpolation input, or other builder
    /// failure. The result uses a bumped identifier and retains all other curve
    /// conventions.
    pub fn with_triangular_key_rate_bump_neighbors(
        &self,
        prev_bucket: Option<f64>,
        target_bucket: f64,
        next_bucket: Option<f64>,
        bump: f64,
    ) -> crate::Result<Self> {
        if self.knots.len() < 2 {
            return self.with_parallel_bump(bump);
        }

        let bumped_points = bump_knots_triangular(
            &self.knots,
            &self.levels,
            prev_bucket,
            target_bucket,
            next_bucket,
            bump,
        );
        let new_id = crate::market_data::bumps::id_bump_bp(self.id.as_str(), bump * 100.0);
        VolatilityIndexCurve::builder(new_id)
            .base_date(self.base)
            .day_count(self.day_count)
            .spot_level(self.spot_level) // Spot typically not bumped in key-rate
            .knots(bumped_points)
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
            .build()
    }

    /// Roll the curve forward by a specified number of days.
    ///
    /// This creates a new curve with:
    /// - Base date advanced by `days`
    /// - Knot times shifted backwards (t' = t - dt_years)
    /// - Points with t' <= 0 are filtered out (expired)
    /// - Forward levels are preserved
    ///
    /// # Arguments
    /// * `days` - Number of days to roll forward
    ///
    /// # Returns
    /// A new volatility index curve with updated base date and shifted knots.
    ///
    /// # Errors
    /// Returns an error if fewer than 2 knot points remain after filtering expired points.
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        let new_base = self.base + time::Duration::days(days);
        let dt_years = year_fraction_to(self.base, new_base, self.day_count)?;

        let rolled_points = roll_knots(&self.knots, &self.levels, dt_years);

        if rolled_points.len() < 2 {
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        // New spot is interpolated from old curve at dt_years
        let new_spot = self.forward_level(dt_years);

        VolatilityIndexCurve::builder(self.id.clone())
            .base_date(new_base)
            .day_count(self.day_count)
            .spot_level(new_spot)
            .knots(rolled_points)
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
            .build()
    }
}

/// Fluent builder for [`VolatilityIndexCurve`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::term_structures::VolatilityIndexCurve;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = VolatilityIndexCurve::builder("VIX")
///     .base_date(base)
///     .spot_level(18.5)
///     .knots([(0.0, 18.5), (0.25, 20.0), (0.5, 21.5), (1.0, 22.0)])
///     .build()
///     .expect("VolatilityIndexCurve builder should succeed");
/// assert!(curve.forward_level(0.5) > 18.5);
/// ```
pub struct VolatilityIndexCurveBuilder {
    id: CurveId,
    base: Date,
    base_is_set: bool,
    day_count: DayCount,
    spot_level: Option<f64>,
    points: Vec<(f64, f64)>,
    style: InterpStyle,
    extrapolation: ExtrapolationPolicy,
}

impl VolatilityIndexCurveBuilder {
    /// Set the curve's valuation **base date**.
    pub fn base_date(mut self, d: Date) -> Self {
        self.base = d;
        self.base_is_set = true;
        self
    }

    /// Choose the **day-count** convention.
    pub fn day_count(mut self, dc: DayCount) -> Self {
        self.day_count = dc;
        self
    }

    /// Set the **spot level** of the volatility index.
    ///
    /// If not set, will be inferred from the first knot point (at t=0).
    pub fn spot_level(mut self, level: f64) -> Self {
        self.spot_level = Some(level);
        self
    }

    /// Supply knot points `(t, forward_level)`.
    pub fn knots<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(pts);
        self
    }

    /// Select interpolation style for this curve.
    pub fn interp(mut self, style: InterpStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the extrapolation policy for out-of-bounds evaluation.
    pub fn extrapolation(mut self, policy: ExtrapolationPolicy) -> Self {
        self.extrapolation = policy;
        self
    }

    /// Validate input and build the [`VolatilityIndexCurve`].
    ///
    /// A valid curve needs an explicit base date, at least two strictly
    /// increasing finite time knots in years, and a finite non-negative level
    /// at every knot. If spot is omitted, the first knot must be exactly zero
    /// so it can be inferred without extrapolation. The builder permits zero
    /// levels but not negative levels, reflecting an index-level—not a signed
    /// commodity-price—contract.
    ///
    /// # Errors
    ///
    /// Returns an input, validation, or interpolation error if the base date is
    /// absent, fewer than two points are supplied, knots are invalid, a level
    /// or spot is non-finite/negative, spot cannot be inferred, or the selected
    /// interpolation/extrapolation combination rejects the grid.
    pub fn build(self) -> crate::Result<VolatilityIndexCurve> {
        if !self.base_is_set {
            return Err(InputError::Invalid.into());
        }
        if self.points.len() < 2 {
            return Err(InputError::TooFewPoints.into());
        }

        let (kvec, lvec): (Vec<f64>, Vec<f64>) = split_points(self.points);
        crate::math::interp::utils::validate_knots(&kvec)?;

        // Validate all levels are non-negative
        validate_non_negative_knots(&kvec, &lvec, "Volatility index level")?;

        // Infer spot level only when the first knot is explicitly anchored at t=0.
        let spot_level = match self.spot_level {
            Some(spot) => spot,
            None => infer_spot_from_knots(&kvec, &lvec).ok_or(InputError::Invalid)?,
        };

        if spot_level < 0.0 {
            return Err(crate::Error::Validation(format!(
                "Spot level must be non-negative: {:.8}",
                spot_level
            )));
        }

        let knots = kvec.into_boxed_slice();
        let levels = lvec.into_boxed_slice();

        let interp = build_interp_allow_any_values(
            self.style,
            knots.clone(),
            levels.clone(),
            self.extrapolation,
        )?;

        Ok(VolatilityIndexCurve {
            id: self.id,
            base: self.base,
            day_count: self.day_count,
            spot_level,
            knots,
            levels,
            interp,
        })
    }
}

// -----------------------------------------------------------------------------
// Trait implementations
// -----------------------------------------------------------------------------

impl TermStructure for VolatilityIndexCurve {
    #[inline]
    fn id(&self) -> &CurveId {
        &self.id
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_vix_curve() -> VolatilityIndexCurve {
        VolatilityIndexCurve::builder("VIX")
            .base_date(
                Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
            )
            .knots([(0.0, 18.5), (0.25, 20.0), (0.5, 21.5), (1.0, 22.0)])
            .spot_level(18.5)
            .build()
            .expect("VolatilityIndexCurve builder should succeed with valid test data")
    }

    #[test]
    fn interpolates_forward_level() {
        let curve = sample_vix_curve();
        // At 0.25Y, should be 20.0
        assert!((curve.forward_level(0.25) - 20.0).abs() < 1e-10);
        // At 0.125Y (midpoint of first segment), should be ~19.25
        let mid = curve.forward_level(0.125);
        assert!((mid - 19.25).abs() < 0.01, "Expected ~19.25, got {}", mid);
    }

    #[test]
    fn spot_level_at_zero() {
        let curve = sample_vix_curve();
        assert!((curve.forward_level(0.0) - 18.5).abs() < 1e-10);
        assert!((curve.spot_level() - 18.5).abs() < 1e-10);
    }

    #[test]
    fn builder_requires_explicit_base_date() {
        let result = VolatilityIndexCurve::builder("VIX")
            .spot_level(18.5)
            .knots([(0.0, 18.5), (0.5, 21.0)])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn builder_rejects_missing_spot_when_first_knot_is_not_zero() {
        let result = VolatilityIndexCurve::builder("VIX")
            .base_date(
                Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
            )
            .knots([(0.25, 20.0), (0.5, 21.5)])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn parallel_bump() {
        let curve = sample_vix_curve();
        let bumped = curve.with_parallel_bump(2.0).expect("Bump should succeed");
        assert!((bumped.spot_level() - 20.5).abs() < 1e-10);
        assert!((bumped.forward_level(0.25) - 22.0).abs() < 1e-10);
    }

    #[test]
    fn percentage_bump() {
        let curve = sample_vix_curve();
        let bumped = curve
            .with_percentage_bump(0.10)
            .expect("Bump should succeed");
        // 18.5 * 1.10 = 20.35
        assert!((bumped.spot_level() - 20.35).abs() < 1e-10);
    }

    #[test]
    fn rejects_negative_levels() {
        let result = VolatilityIndexCurve::builder("VIX")
            .knots([(0.0, 18.5), (0.5, -5.0)])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn requires_at_least_two_points() {
        let result = VolatilityIndexCurve::builder("VIX")
            .knots([(0.0, 18.5)])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn serde_round_trip() {
        let curve = sample_vix_curve();
        let json = serde_json::to_string(&curve).expect("Serialize should succeed");
        let recovered: VolatilityIndexCurve =
            serde_json::from_str(&json).expect("Deserialize should succeed");
        assert_eq!(curve.id(), recovered.id());
        assert!((curve.spot_level() - recovered.spot_level()).abs() < 1e-10);
        assert!((curve.forward_level(0.5) - recovered.forward_level(0.5)).abs() < 1e-10);
    }
}
