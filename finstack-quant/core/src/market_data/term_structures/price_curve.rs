//! Forward price curves for commodity and other price-based derivatives.
//!
//! A price curve represents expected future price levels for commodities,
//! indices, or other assets that are quoted in absolute prices (not rates).
//! These curves are essential for pricing commodity derivatives including
//! forwards, swaps, and options.
//!
//! # Financial Concept
//!
//! The forward price F(t) represents the expected delivery price at time t:
//! ```text
//! F(t) = market expectation of asset price at time t
//! ```
//!
//! For commodities, forward prices embed storage costs, convenience yields,
//! and interest rates:
//! ```text
//! F(T) = S × exp((r - y + u) × T)
//! ```
//! where S is spot, r is risk-free rate, y is convenience yield, and u is storage cost.
//!
//! # Market Construction
//!
//! Price curves are typically bootstrapped from:
//! - **Futures**: Exchange-traded commodity futures (WTI, Brent, NG, etc.)
//! - **Forwards**: OTC forward contracts
//! - **Spot prices**: Current market prices with cost-of-carry adjustments
//!
//! # Term Structure Characteristics
//!
//! Commodity forward curves exhibit various shapes:
//! - **Contango**: Forward prices higher than spot (normal market)
//! - **Backwardation**: Forward prices lower than spot (supply constraints)
//! - **Seasonal patterns**: Energy and agricultural commodities
//!
//! # Use Cases
//!
//! - **Commodity forward pricing**: Mark-to-market vs contract price
//! - **Commodity swap valuation**: Project floating leg prices
//! - **Commodity option pricing**: Forward level for Black-76
//! - **Risk management**: Delta and scenario analysis
//!
//! # Volatility index curves
//!
//! The same type also carries volatility-index term structures (CBOE VIX,
//! VXN, VSTOXX) when built with [`PriceCurveKind::VolIndex`]: knot values are
//! then expected futures/forward *levels* of the index and must be
//! non-negative. Downstream pricers (e.g. `VolatilityIndexFuture` in
//! `valuations`) treat `F(t)` as the fair futures level `E[VIX_t]` with **no
//! convexity adjustment**, which is exact only when the knots are quoted
//! futures/forward levels. Feeding vols derived from variance-swap strikes or
//! option replication supplies `√(E[VIX_t²])`, which by Jensen's inequality
//! overstates every future by the vol-of-vol convexity (roughly +8–28% for
//! typical VIX vol-of-vol); apply the concavity adjustment first.
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::market_data::term_structures::PriceCurve;
//! use finstack_quant_core::math::interp::InterpStyle;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let curve = PriceCurve::builder("WTI-FORWARD")
//!     .base_date(base)
//!     .spot_price(75.0)
//!     .knots([(0.0, 75.0), (0.25, 76.5), (0.5, 77.2), (1.0, 78.0)])
//!     .interp(InterpStyle::Linear)
//!     .build()
//!     .expect("PriceCurve builder should succeed");
//! assert!(curve.price(0.25) > 0.0);
//! ```
//!
//! # References
//!
//! - Black, F. (1976). "The Pricing of Commodity Contracts." Journal of
//!   Financial Economics, 3(1-2), 167-179. `docs/REFERENCES.md#black-1976`
//! - Schwartz, E. S. (1997). "The Stochastic Behavior of Commodity Prices."
//!   Journal of Finance, 52(3), 923-973.
//! - Whaley, R. E. (2009). "Understanding the VIX." *Journal of Portfolio Management*,
//!   35(3), 98-105. `docs/REFERENCES.md#whaley-2009-vix`
//! - CBOE (2019). "VIX White Paper." CBOE Global Markets. `docs/REFERENCES.md#cboe-vix-white-paper`

use super::common::{
    build_interp_allow_any_values, bump_knots_parallel, bump_knots_percentage,
    bump_knots_triangular, default_curve_base_date, infer_spot_from_knots, roll_knots,
    split_points, validate_non_negative_knots, year_fraction_to,
};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
use crate::{
    dates::{Date, DayCount},
    error::InputError,
    math::interp::types::Interp,
    types::CurveId,
};

/// Level family a [`PriceCurve`] represents.
///
/// The kind selects the validation contract, the wire field carrying the spot
/// value, and the [`super::super::context::CurveStorage`] variant a curve is
/// inserted under (`Price` versus `VolIndex`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PriceCurveKind {
    /// Signed forward prices (commodities, power). Any finite value is valid;
    /// serialised as `spot_price`.
    #[default]
    Price,
    /// Volatility-index forward levels (VIX, VXN, VSTOXX). Knot levels and
    /// spot must be non-negative; serialised as `spot_level`.
    VolIndex,
}

/// Forward price curve for commodities, other price-based assets and
/// volatility indices.
///
/// Represents expected future price levels. Stores forward prices at
/// knot times and interpolates between them. A [`PriceCurveKind::VolIndex`]
/// curve holds volatility-index levels with a non-negativity contract; see the
/// module docs for the convexity caveat on vol-index inputs.
///
/// # Price Characteristics
///
/// - **Spot price**: Current market price at t=0
/// - **Forward prices**: Expected future prices
/// - **Units**: Absolute prices (e.g., USD per barrel, USD per MMBtu) or
///   index points for volatility indices
///
/// # Thread Safety
///
/// Immutable after construction; safe to share via `Arc<PriceCurve>`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawPriceCurve", into = "RawPriceCurve")]
pub struct PriceCurve {
    id: CurveId,
    kind: PriceCurveKind,
    base: Date,
    /// Day-count basis used for time calculations.
    day_count: DayCount,
    /// Spot price (t=0).
    spot_price: f64,
    /// Knot times in **years** (strictly increasing, first should be 0.0).
    knots: Box<[f64]>,
    /// Forward prices at each knot.
    prices: Box<[f64]>,
    interp: Interp,
}

/// Raw serializable state of PriceCurve.
///
/// Exactly one of `spot_price` (kind `Price`) or `spot_level` (kind
/// `VolIndex`) is present on the wire; the field name carries the kind.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawPriceCurve {
    /// Curve identifier
    pub id: String,
    /// Base date
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub base: Date,
    /// Day count convention
    pub day_count: DayCount,
    /// Spot price (signed price curves)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot_price: Option<f64>,
    /// Spot index level (volatility index curves)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot_level: Option<f64>,
    /// Time/value pairs used to construct the curve
    pub knot_points: Vec<(f64, f64)>,
    /// Interpolation style
    pub interp_style: InterpStyle,
    /// Extrapolation policy
    pub extrapolation: ExtrapolationPolicy,
}

impl From<PriceCurve> for RawPriceCurve {
    fn from(curve: PriceCurve) -> Self {
        let knot_points: Vec<(f64, f64)> = curve
            .knots
            .iter()
            .zip(curve.prices.iter())
            .map(|(&t, &price)| (t, price))
            .collect();

        let (spot_price, spot_level) = match curve.kind {
            PriceCurveKind::Price => (Some(curve.spot_price), None),
            PriceCurveKind::VolIndex => (None, Some(curve.spot_price)),
        };
        RawPriceCurve {
            id: curve.id.to_string(),
            base: curve.base,
            day_count: curve.day_count,
            spot_price,
            spot_level,
            knot_points,
            interp_style: curve.interp.style(),
            extrapolation: curve.interp.extrapolation(),
        }
    }
}

impl TryFrom<RawPriceCurve> for PriceCurve {
    type Error = crate::Error;

    fn try_from(state: RawPriceCurve) -> crate::Result<Self> {
        let (kind, spot) = match (state.spot_price, state.spot_level) {
            (Some(spot), None) => (PriceCurveKind::Price, spot),
            (None, Some(spot)) => (PriceCurveKind::VolIndex, spot),
            _ => {
                return Err(crate::Error::Validation(
                    "PriceCurve requires exactly one of `spot_price` or `spot_level`".to_string(),
                ))
            }
        };
        PriceCurve::builder(state.id)
            .kind(kind)
            .base_date(state.base)
            .day_count(state.day_count)
            .spot_price(spot)
            .knots(state.knot_points)
            .interp(state.interp_style)
            .extrapolation(state.extrapolation)
            .build()
    }
}

impl PriceCurve {
    /// Start building a price curve for the given `id`.
    ///
    /// **Defaults:** [`PriceCurveKind::Price`], linear interpolation with Flat
    /// extrapolation maintains stable tail prices consistent with typical
    /// commodity curve behavior. Use [`PriceCurveBuilder::kind`] to build a
    /// volatility-index curve.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> PriceCurveBuilder {
        PriceCurveBuilder {
            id: id.into(),
            kind: PriceCurveKind::Price,
            base: default_curve_base_date(),
            base_is_set: false,
            day_count: DayCount::Act365F,
            spot_price: None,
            points: Vec::new(),
            style: InterpStyle::Linear,
            extrapolation: ExtrapolationPolicy::FlatZero,
        }
    }

    /// Forward price at time `t` (in years from base date).
    ///
    /// # Returns
    /// The interpolated forward price at time `t`.
    #[must_use]
    #[inline]
    pub fn price(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return self.spot_price;
        }
        self.interp.interp(t)
    }

    /// Forward price on a specific calendar date.
    ///
    /// # Errors
    /// Returns an error if the date is before the base date.
    pub fn price_on_date(&self, date: Date) -> crate::Result<f64> {
        if date < self.base {
            return Err(crate::Error::Validation(format!(
                "Date {} is before curve base date {}",
                date, self.base
            )));
        }
        if date == self.base {
            return Ok(self.spot_price);
        }
        let t = year_fraction_to(self.base, date, self.day_count)?;
        Ok(self.price(t))
    }

    /// Current spot price (or spot index level for a vol-index curve).
    #[must_use]
    #[inline]
    pub fn spot_price(&self) -> f64 {
        self.spot_price
    }

    /// Level family of this curve.
    #[must_use]
    #[inline]
    pub fn kind(&self) -> PriceCurveKind {
        self.kind
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

    /// Forward prices at each knot.
    #[inline]
    pub fn prices(&self) -> &[f64] {
        &self.prices
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
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> PriceCurveBuilder {
        PriceCurve::builder(new_id)
            .kind(self.kind)
            .base_date(self.base)
            .day_count(self.day_count)
            .spot_price(self.spot_price)
            .knots(self.knots.iter().copied().zip(self.prices.iter().copied()))
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
    }

    /// Create a new curve with a parallel bump applied (additive, in price units).
    ///
    /// # Arguments
    /// * `bump` - Bump size in price units (e.g., 1.0 adds $1 to all prices)
    ///
    /// # Returns
    /// A new price curve with all prices shifted.
    ///
    /// Prices are bumped additively in the curve's native price unit (for
    /// example, USD/bbl), including the stored spot. Finite negative prices are
    /// valid for markets such as power and are preserved. Volatility-index
    /// curves reject shocks that produce negative levels.
    ///
    /// # Errors
    ///
    /// Propagates reconstruction errors if `bump` produces non-finite knot
    /// prices or an invalid interpolation configuration. A successful result
    /// preserves base date, day count, interpolation, and extrapolation.
    pub fn with_parallel_bump(&self, bump: f64) -> crate::Result<Self> {
        if !bump.is_finite() {
            return Err(InputError::Invalid.into());
        }
        let bumped_points = bump_knots_parallel(&self.knots, &self.prices, bump);
        let new_id = crate::market_data::bumps::id_bump_bp(self.id.as_str(), bump * 100.0);

        PriceCurve::builder(new_id)
            .kind(self.kind)
            .base_date(self.base)
            .day_count(self.day_count)
            .spot_price(self.spot_price + bump)
            .knots(bumped_points)
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
            .build()
    }

    /// Create a new curve with a percentage bump applied (multiplicative).
    ///
    /// # Arguments
    /// * `pct` - Percentage bump (e.g., 0.01 = +1%, -0.05 = -5%)
    ///
    /// # Returns
    /// A new price curve with all prices scaled.
    ///
    /// Each knot is multiplied by `1 + pct`; the stored spot uses the same
    /// multiplier, preserving signed prices. Volatility-index curves reject
    /// shocks that produce negative levels. The method changes price levels,
    /// not delivery dates or interpolation mechanics.
    ///
    /// # Errors
    ///
    /// Propagates reconstruction errors if `pct` produces non-finite knot
    /// prices or invalid interpolation inputs. No range is imposed on a finite
    /// percentage bump because signed forward-price markets can require shocks
    /// outside conventional equity-style bounds.
    pub fn with_percentage_bump(&self, pct: f64) -> crate::Result<Self> {
        if !pct.is_finite() {
            return Err(InputError::Invalid.into());
        }
        let bumped_points = bump_knots_percentage(&self.knots, &self.prices, pct);
        let new_id = format!("{}+{:.2}%", self.id.as_str(), pct * 100.0);

        PriceCurve::builder(new_id)
            .kind(self.kind)
            .base_date(self.base)
            .day_count(self.day_count)
            .spot_price(self.spot_price * (1.0 + pct))
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
    /// * `bump` - Bump size in price units
    ///
    /// # Returns
    /// A new price curve with the triangular key-rate bump applied.
    ///
    /// The bump is zero outside the neighbor interval and reaches `bump` at
    /// `target_bucket`; when the curve has fewer than two knots, this degrades
    /// to [`with_parallel_bump`](Self::with_parallel_bump). Spot is deliberately
    /// unchanged for a key-rate shock, reflecting a sensitivity to future
    /// delivery buckets rather than a spot-price scenario.
    ///
    /// # Errors
    ///
    /// Propagates builder/interpolation validation errors after applying the
    /// triangular knot shock, including non-finite resulting prices or invalid
    /// interpolation input. A successful result retains all curve conventions
    /// except for its bumped identifier and knot values.
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
            &self.prices,
            prev_bucket,
            target_bucket,
            next_bucket,
            bump,
        );
        let new_id = crate::market_data::bumps::id_bump_bp(self.id.as_str(), bump * 100.0);
        PriceCurve::builder(new_id)
            .kind(self.kind)
            .base_date(self.base)
            .day_count(self.day_count)
            .spot_price(self.spot_price) // Spot typically not bumped in key-rate
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
    /// - Forward prices are preserved
    ///
    /// # Arguments
    /// * `days` - Number of days to roll forward
    ///
    /// # Returns
    /// A new price curve with updated base date and shifted knots.
    ///
    /// # Errors
    /// Returns an error if no future knot remains after filtering expired points.
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        let new_base = self.base + time::Duration::days(days);
        let dt_years = year_fraction_to(self.base, new_base, self.day_count)?;

        let mut rolled_points = roll_knots(&self.knots, &self.prices, dt_years);

        if rolled_points.is_empty() {
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        // New spot is interpolated from old curve at dt_years
        let new_spot = self.price(dt_years);
        rolled_points.insert(0, (0.0, new_spot));

        PriceCurve::builder(self.id.clone())
            .kind(self.kind)
            .base_date(new_base)
            .day_count(self.day_count)
            .spot_price(new_spot)
            .knots(rolled_points)
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
            .build()
    }
}

/// Fluent builder for [`PriceCurve`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::term_structures::PriceCurve;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = PriceCurve::builder("WTI-FORWARD")
///     .base_date(base)
///     .spot_price(75.0)
///     .knots([(0.0, 75.0), (0.25, 76.5), (0.5, 77.2), (1.0, 78.0)])
///     .build()
///     .expect("PriceCurve builder should succeed");
/// assert!(curve.price(0.5) > 75.0);
/// ```
pub struct PriceCurveBuilder {
    id: CurveId,
    kind: PriceCurveKind,
    base: Date,
    base_is_set: bool,
    day_count: DayCount,
    spot_price: Option<f64>,
    points: Vec<(f64, f64)>,
    style: InterpStyle,
    extrapolation: ExtrapolationPolicy,
}

impl PriceCurveBuilder {
    /// Select the level family (signed prices or non-negative vol-index levels).
    ///
    /// # Arguments
    ///
    /// * `kind` - Quote family controlling whether negative levels are accepted when the curve is built.
    pub fn kind(mut self, kind: PriceCurveKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set the curve's valuation **base date**.
    pub fn base_date(mut self, d: Date) -> Self {
        self.base = d;
        self.base_is_set = true;
        self
    }

    /// Choose the **day-count** convention.
    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = day_count;
        self
    }

    /// Set the **spot price**.
    ///
    /// If not set, will be inferred from the first knot point (at t=0).
    pub fn spot_price(mut self, price: f64) -> Self {
        self.spot_price = Some(price);
        self
    }

    /// Supply knot points `(t, forward_price)`.
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

    /// Validate input and build the [`PriceCurve`].
    ///
    /// A base date is mandatory. Provide at least two strictly increasing,
    /// finite time knots measured in years, and one finite forward price per
    /// knot; prices may be signed because some commodity/power markets trade
    /// through zero. If spot is omitted, the first knot must be exactly at
    /// `t = 0` so the builder can infer it without extrapolation. A
    /// [`PriceCurveKind::VolIndex`] curve additionally rejects negative knot
    /// levels and a negative spot (zero is permitted).
    ///
    /// # Errors
    ///
    /// Returns an input, validation, or interpolation error if the base date
    /// was not set, fewer than two points were supplied, knots are non-finite
    /// or not strictly increasing, any price/spot is non-finite (or negative
    /// for a vol-index curve), spot cannot be inferred, or the selected
    /// interpolation/extrapolation combination rejects the grid.
    pub fn build(self) -> crate::Result<PriceCurve> {
        if !self.base_is_set {
            return Err(InputError::Invalid.into());
        }
        if self.points.len() < 2 {
            return Err(InputError::TooFewPoints.into());
        }

        let (kvec, pvec): (Vec<f64>, Vec<f64>) = split_points(self.points);
        crate::math::interp::utils::validate_knots(&kvec)?;

        if pvec.iter().any(|price| !price.is_finite()) {
            return Err(InputError::Invalid.into());
        }

        // Infer spot price only when the first knot is explicitly anchored at t=0.
        let spot_price = match self.spot_price {
            Some(spot) => spot,
            None => infer_spot_from_knots(&kvec, &pvec).ok_or(InputError::Invalid)?,
        };

        if !spot_price.is_finite() {
            return Err(InputError::Invalid.into());
        }

        if self.kind == PriceCurveKind::VolIndex {
            validate_non_negative_knots(&kvec, &pvec, "Volatility index level")?;
            if spot_price < 0.0 {
                return Err(crate::Error::Validation(format!(
                    "Spot level must be non-negative: {:.8}",
                    spot_price
                )));
            }
        }

        let knots = kvec.into_boxed_slice();
        let prices = pvec.into_boxed_slice();

        let interp = build_interp_allow_any_values(
            self.style,
            knots.clone(),
            prices.clone(),
            self.extrapolation,
        )?;

        Ok(PriceCurve {
            id: self.id,
            kind: self.kind,
            base: self.base,
            day_count: self.day_count,
            spot_price,
            knots,
            prices,
            interp,
        })
    }
}

// Trait implementations

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_wti_curve() -> PriceCurve {
        PriceCurve::builder("WTI-FORWARD")
            .base_date(
                Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
            )
            .knots([(0.0, 75.0), (0.25, 76.5), (0.5, 77.2), (1.0, 78.0)])
            .spot_price(75.0)
            .build()
            .expect("PriceCurve builder should succeed with valid test data")
    }

    #[test]
    fn interpolates_forward_price() {
        let curve = sample_wti_curve();
        // At 0.25Y, should be 76.5
        assert!((curve.price(0.25) - 76.5).abs() < 1e-10);
        // At 0.125Y (midpoint of first segment), should be ~75.75
        let mid = curve.price(0.125);
        assert!((mid - 75.75).abs() < 0.01, "Expected ~75.75, got {}", mid);
    }

    #[test]
    fn spot_price_at_zero() {
        let curve = sample_wti_curve();
        assert!((curve.price(0.0) - 75.0).abs() < 1e-10);
        assert!((curve.spot_price() - 75.0).abs() < 1e-10);
    }

    #[test]
    fn builder_requires_explicit_base_date() {
        let result = PriceCurve::builder("WTI-FWD")
            .spot_price(75.0)
            .knots([(0.0, 75.0), (1.0, 78.0)])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn builder_rejects_missing_spot_when_first_knot_is_not_zero() {
        let result = PriceCurve::builder("WTI-FWD")
            .base_date(
                Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
            )
            .knots([(0.25, 76.5), (1.0, 78.0)])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn parallel_bump() {
        let curve = sample_wti_curve();
        let bumped = curve.with_parallel_bump(2.0).expect("Bump should succeed");
        assert!((bumped.spot_price() - 77.0).abs() < 1e-10);
        assert!((bumped.price(0.25) - 78.5).abs() < 1e-10);
    }

    #[test]
    fn percentage_bump() {
        let curve = sample_wti_curve();
        let bumped = curve
            .with_percentage_bump(0.10)
            .expect("Bump should succeed");
        // 75.0 * 1.10 = 82.5
        assert!((bumped.spot_price() - 82.5).abs() < 1e-10);
    }

    #[test]
    fn permits_finite_signed_futures_prices() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let curve = PriceCurve::builder("POWER")
            .base_date(base)
            .spot_price(-10.0)
            .knots([(0.0, -10.0), (1.0, -20.0)])
            .build()
            .expect("finite signed prices are valid market data");
        assert!((curve.price(0.5) + 15.0).abs() < 1e-12);
    }

    #[test]
    fn requires_at_least_two_points() {
        let result = PriceCurve::builder("WTI").knots([(0.0, 75.0)]).build();
        assert!(result.is_err());
    }

    #[test]
    fn serde_round_trip() {
        let curve = sample_wti_curve();
        let json = serde_json::to_string(&curve).expect("Serialize should succeed");
        let recovered: PriceCurve =
            serde_json::from_str(&json).expect("Deserialize should succeed");
        assert_eq!(curve.id(), recovered.id());
        assert!((curve.spot_price() - recovered.spot_price()).abs() < 1e-10);
        assert!((curve.price(0.5) - recovered.price(0.5)).abs() < 1e-10);
    }

    fn sample_vix_curve() -> PriceCurve {
        PriceCurve::builder("VIX")
            .kind(PriceCurveKind::VolIndex)
            .base_date(
                Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
            )
            .knots([(0.0, 18.5), (0.25, 20.0), (0.5, 21.5), (1.0, 22.0)])
            .spot_price(18.5)
            .build()
            .expect("vol-index PriceCurve builder should succeed with valid test data")
    }

    #[test]
    fn vol_index_rejects_negative_levels() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let result = PriceCurve::builder("VIX")
            .kind(PriceCurveKind::VolIndex)
            .base_date(base)
            .knots([(0.0, 18.5), (0.5, -5.0)])
            .build();
        assert!(result.is_err());
        let result = PriceCurve::builder("VIX")
            .kind(PriceCurveKind::VolIndex)
            .base_date(base)
            .spot_price(-1.0)
            .knots([(0.0, 18.5), (0.5, 20.0)])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn vol_index_serde_uses_spot_level_and_round_trips() {
        let curve = sample_vix_curve();
        let json = serde_json::to_value(&curve).expect("Serialize should succeed");
        assert!((json["spot_level"].as_f64().unwrap() - 18.5).abs() < 1e-12);
        assert!(json.get("spot_price").is_none());
        let recovered: PriceCurve =
            serde_json::from_value(json).expect("Deserialize should succeed");
        assert_eq!(recovered.kind(), PriceCurveKind::VolIndex);
        assert!((curve.price(0.5) - recovered.price(0.5)).abs() < 1e-10);

        let price_json = serde_json::to_value(sample_wti_curve()).unwrap();
        assert!(price_json.get("spot_level").is_none());
        assert!(price_json.get("spot_price").is_some());
        let both = serde_json::json!({
            "id": "X", "base": "2025-01-01", "day_count": "act_365f",
            "spot_price": 1.0, "spot_level": 1.0,
            "knot_points": [[0.0, 1.0], [1.0, 1.0]],
            "interp_style": "linear", "extrapolation": "flat_zero"
        });
        assert!(serde_json::from_value::<PriceCurve>(both).is_err());
    }

    #[test]
    fn vol_index_bumps_preserve_kind() {
        let curve = sample_vix_curve();
        let bumped = curve.with_parallel_bump(2.0).expect("Bump should succeed");
        assert_eq!(bumped.kind(), PriceCurveKind::VolIndex);
        assert!((bumped.spot_price() - 20.5).abs() < 1e-10);
        assert!((bumped.price(0.25) - 22.0).abs() < 1e-10);
        let scaled = curve
            .with_percentage_bump(0.1)
            .expect("Bump should succeed");
        assert_eq!(scaled.kind(), PriceCurveKind::VolIndex);
        assert!((scaled.spot_price() - 20.35).abs() < 1e-10);
    }

    #[test]
    fn price_on_date() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid date");
        let curve = PriceCurve::builder("WTI")
            .base_date(base)
            .knots([(0.0, 75.0), (1.0, 78.0)])
            .spot_price(75.0)
            .build()
            .expect("Should build");

        // At base date, should return spot
        let spot = curve.price_on_date(base).expect("Should succeed");
        assert!((spot - 75.0).abs() < 1e-10);

        // 6 months forward
        let six_months = Date::from_calendar_date(2025, time::Month::July, 1).expect("Valid date");
        let fwd_price = curve.price_on_date(six_months).expect("Should succeed");
        assert!(fwd_price > 75.0 && fwd_price < 78.0);
    }
}
