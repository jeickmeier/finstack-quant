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
    roll_knots, triangular_weight, validate_pillars, year_fraction_to, InterpRollState,
};
use crate::dates::DateExt;
use crate::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle, ValidationPolicy};
use crate::{
    dates::{Date, DayCount},
    error::InputError,
    math::interp::Interp,
    types::CurveId,
};

/// Level family a [`PriceCurve`] represents.
///
/// The kind selects the validation contract, the wire field carrying the spot
/// value, and the curve type a [`super::super::context::MarketContext`]
/// reports and checks on lookup (`Price` versus `VolIndex`).
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

impl PriceCurveKind {
    /// Canonical label: `"price"` or `"vol_index"`.
    ///
    /// This is the name host bindings accept and report for the kind; the
    /// JSON wire form carries the kind through the spot field name instead.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Price => "price",
            Self::VolIndex => "vol_index",
        }
    }
}

impl std::fmt::Display for PriceCurveKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for PriceCurveKind {
    type Err = crate::Error;

    /// Parse the canonical label `"price"` or `"vol_index"` (exact match).
    fn from_str(s: &str) -> crate::Result<Self> {
        match s {
            "price" => Ok(Self::Price),
            "vol_index" => Ok(Self::VolIndex),
            other => Err(crate::Error::Validation(format!(
                "Invalid price curve kind {other:?}: expected \"price\" or \"vol_index\""
            ))),
        }
    }
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
    /// Source interpolation; the unrolled source when `roll` is set.
    interp: Interp,
    /// Source grid and origin of a rolled curve.
    roll: Option<InterpRollState>,
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
    /// Source interpolation and accumulated time origin for a rolled curve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roll_transform: Option<InterpRollState>,
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
            roll_transform: curve.roll,
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
            .roll_transform(state.roll_transform)
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
            base: None,
            day_count: DayCount::Act365F,
            spot_price: None,
            points: Vec::new(),
            style: InterpStyle::Linear,
            extrapolation: ExtrapolationPolicy::FlatZero,
            roll_transform: None,
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
        match &self.roll {
            Some(state) => state.interp(&self.interp, t),
            None => self.interp.interp(t),
        }
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
    ///
    /// The builder keeps the curve's pillars, spot and any roll state, so
    /// `build()` reproduces the curve under `new_id`; supplying more pillars
    /// through [`PriceCurveBuilder::knots`] drops the roll state.
    ///
    /// # Arguments
    ///
    /// * `new_id` - Identifier of the rebuilt curve, the `MarketContext` lookup key.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> PriceCurveBuilder {
        self.metadata_builder(new_id)
            .spot_price(self.spot_price)
            .knots(self.knots.iter().copied().zip(self.prices.iter().copied()))
            .roll_transform(self.roll.clone())
    }

    /// Builder pre-populated with this curve's metadata (kind, base date,
    /// day count, interpolation, extrapolation) but **no** knots or spot.
    /// Shared by every rebuild-style operation so no field is dropped.
    fn metadata_builder(&self, new_id: impl Into<CurveId>) -> PriceCurveBuilder {
        PriceCurve::builder(new_id)
            .kind(self.kind)
            .base_date(self.base)
            .day_count(self.day_count)
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
    }

    /// Rebuild with level-shocked pillars.
    ///
    /// An unrolled curve takes `pillars` and `spot` as given. A rolled curve
    /// shocks its source pillars with `map(time, price)` instead and reads its
    /// current pillars and spot back from the shocked source, so the shocked
    /// and base curves share one interpolation (off-pillar levels follow the
    /// interpolation style, as they do on an unrolled curve).
    fn rebuild_shocked(
        &self,
        new_id: impl Into<CurveId>,
        spot: f64,
        pillars: Vec<(f64, f64)>,
        map: impl Fn(f64, f64) -> f64,
    ) -> crate::Result<Self> {
        let builder = self.metadata_builder(new_id);
        match &self.roll {
            Some(roll) => {
                let (state, prices) = roll.bump(
                    &self.interp,
                    ValidationPolicy::AllowNegative,
                    &self.knots,
                    map,
                )?;
                let spot = self
                    .knots
                    .iter()
                    .position(|&t| t == 0.0)
                    .map_or(spot, |origin| prices[origin]);
                builder
                    .spot_price(spot)
                    .knots(self.knots.iter().copied().zip(prices))
                    .roll_transform(Some(state))
                    .build()
            }
            None => builder.spot_price(spot).knots(pillars).build(),
        }
    }

    /// Rebuild this curve with replacement knots and spot while preserving
    /// its id, kind, base date, day count, interpolation and extrapolation.
    ///
    /// # Arguments
    ///
    /// * `knots` - Replacement `(time, price)` pillars: times in year
    ///   fractions from the base date under the curve's day count, prices in
    ///   the curve's native price unit.
    /// * `spot_price` - Replacement spot price in the same unit; it is the
    ///   curve's value at `t = 0`.
    ///
    /// # Errors
    ///
    /// Returns an error when the replacement knots or spot violate the
    /// curve's validation or interpolation constraints.
    pub fn rebuild_with_knots<I>(&self, knots: I, spot_price: f64) -> crate::Result<Self>
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.metadata_builder(self.id.clone())
            .spot_price(spot_price)
            .knots(knots)
            .build()
    }

    /// Rebuild with every pillar mapped through `map` and the given spot.
    fn shocked(
        &self,
        new_id: impl Into<CurveId>,
        spot: f64,
        map: impl Fn(f64, f64) -> f64,
    ) -> crate::Result<Self> {
        let pillars: Vec<(f64, f64)> = self
            .knots
            .iter()
            .zip(self.prices.iter())
            .map(|(&t, &price)| (t, map(t, price)))
            .collect();
        self.rebuild_shocked(new_id, spot, pillars, map)
    }

    /// Scale every pillar and the spot by `1 + pct`, preserving signed prices.
    fn scaled(&self, pct: f64) -> crate::Result<Self> {
        let factor = 1.0 + pct;
        self.shocked(
            format!("{}+{:.2}%", self.id.as_str(), pct * 100.0),
            self.spot_price * factor,
            |_, price| price * factor,
        )
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
    /// Returns a validation error if the rolled base date exceeds the supported calendar range.
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        if days == 0 {
            return Ok(self.clone());
        }
        let new_base = self.base.add_days(days)?;
        let dt_years = year_fraction_to(self.base, new_base, self.day_count)?;

        let mut rolled_points = roll_knots(&self.knots, &self.prices, dt_years);

        if rolled_points.is_empty() {
            return Err(crate::InputError::TooFewPoints.into());
        }

        // New spot is interpolated from old curve at dt_years
        let new_spot = self.price(dt_years);
        rolled_points.insert(0, (0.0, new_spot));

        self.metadata_builder(self.id.clone())
            .base_date(new_base)
            .spot_price(new_spot)
            .knots(rolled_points)
            .roll_transform(InterpRollState::advance(
                self.roll.as_ref(),
                dt_years,
                self.knots.iter().copied().zip(self.prices.iter().copied()),
            )?)
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
    base: Option<Date>,
    day_count: DayCount,
    spot_price: Option<f64>,
    points: Vec<(f64, f64)>,
    style: InterpStyle,
    extrapolation: ExtrapolationPolicy,
    roll_transform: Option<InterpRollState>,
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
        self.base = Some(d);
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
    /// An explicit spot is inserted as the zero-time interpolation knot when
    /// absent and must match any supplied zero-time knot.
    ///
    /// # Arguments
    ///
    /// * `price` - Finite spot in the same quote units as the forward knots;
    ///   signed values are allowed for price curves, while volatility-index
    ///   curves require a non-negative level.
    pub fn spot_price(mut self, price: f64) -> Self {
        self.spot_price = Some(price);
        self
    }

    /// Supply knot points `(t, forward_price)`.
    ///
    /// Supplied pillars define the curve's interpolation; any roll state
    /// carried over from [`PriceCurve::to_builder_with_id`] is dropped.
    ///
    /// # Arguments
    ///
    /// * `pts` - `(time, price)` pillars appended to any already supplied:
    ///   times in years from the base date under the curve day count, prices
    ///   in the curve's native unit.
    pub fn knots<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(pts);
        self.roll_transform = None;
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

    fn roll_transform(mut self, state: Option<InterpRollState>) -> Self {
        self.roll_transform = state;
        self
    }

    /// Validate input and build the [`PriceCurve`].
    ///
    /// A base date is mandatory. Provide at least two strictly increasing,
    /// finite time knots measured in years, and one finite forward price per
    /// knot; prices may be signed because some commodity/power markets trade
    /// through zero. If spot is omitted, the first knot must be exactly at
    /// `t = 0` so the builder can infer it without extrapolation. A
    /// supplied spot anchors interpolation at zero; a missing zero-time knot
    /// is inserted, and a conflicting zero-time price is rejected. A
    /// [`PriceCurveKind::VolIndex`] curve additionally rejects negative knot
    /// levels and a negative spot (zero is permitted).
    ///
    /// # Errors
    ///
    /// Returns an input, validation, or interpolation error if the base date
    /// was not set, fewer than two points were supplied, knots are non-finite
    /// or not strictly increasing, any price/spot is non-finite (or negative
    /// for a vol-index curve), spot cannot be inferred or differs from a
    /// zero-time knot, or the selected
    /// interpolation/extrapolation combination rejects the grid.
    pub fn build(self) -> crate::Result<PriceCurve> {
        let base = self
            .base
            .ok_or_else(|| crate::Error::Validation("PriceCurve requires base_date".to_string()))?;
        if self.points.len() < 2 {
            return Err(InputError::TooFewPoints.into());
        }

        let (mut kvec, mut pvec): (Vec<f64>, Vec<f64>) = self.points.into_iter().unzip();
        crate::math::interp::validate_knots(&kvec)?;

        if pvec.iter().any(|price| !price.is_finite()) {
            return Err(crate::Error::Validation(
                "price curve pillar values must be finite".to_string(),
            ));
        }

        // Infer spot price only when the first knot is explicitly anchored at t=0.
        let spot_price = match self.spot_price {
            Some(spot) => spot,
            None => infer_spot_from_knots(&kvec, &pvec).ok_or_else(|| {
                crate::Error::Validation(
                    "PriceCurve requires spot_price unless the first knot is at t = 0".to_string(),
                )
            })?,
        };

        if !spot_price.is_finite() {
            return Err(crate::Error::Validation(
                "price curve spot must be finite".to_string(),
            ));
        }

        let origin = kvec.partition_point(|&t| t < 0.0);
        if kvec.get(origin).is_some_and(|&t| t == 0.0) {
            if pvec[origin].partial_cmp(&spot_price) != Some(core::cmp::Ordering::Equal) {
                return Err(crate::Error::Validation(
                    "Price knot at t=0 must equal spot_price".to_string(),
                ));
            }
        } else {
            kvec.insert(origin, 0.0);
            pvec.insert(origin, spot_price);
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

        let mut interp = self.style.build_enum(
            knots.clone(),
            prices.clone(),
            self.extrapolation,
            ValidationPolicy::AllowNegative,
        )?;

        if let Some(state) = &self.roll_transform {
            if self.kind == PriceCurveKind::VolIndex {
                let (source_knots, source_levels): (Vec<f64>, Vec<f64>) =
                    state.source_points.iter().copied().unzip();
                validate_non_negative_knots(
                    &source_knots,
                    &source_levels,
                    "Volatility index level",
                )?;
            }
            interp = state.restore(
                self.style,
                self.extrapolation,
                ValidationPolicy::AllowNegative,
            )?;
            validate_pillars(
                knots.iter().copied().zip(prices.iter().copied()),
                false,
                "rolled interpolation",
                |t| state.interp(&interp, t),
            )?;
        }

        Ok(PriceCurve {
            id: self.id,
            kind: self.kind,
            base,
            day_count: self.day_count,
            spot_price,
            knots,
            prices,
            interp,
            roll: self.roll_transform,
        })
    }
}

/// Bumps rebuild the curve through one shocked-pillar path. Additive
/// `Fraction` shocks are in price units and move the stored spot with the
/// pillars; `Percent`/`Factor` shocks scale pillars and spot. A key-rate shock
/// applies the triangular weight to pillars after the origin only; the spot
/// level remains unchanged.
impl Bumpable for PriceCurve {
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        spec.validate()?;
        let id = self.id.as_str();
        match spec.bump_type {
            BumpType::Parallel => match (spec.mode, spec.units) {
                (BumpMode::Additive, BumpUnits::Fraction) => {
                    let bump = spec.value;
                    self.shocked(
                        crate::market_data::bumps::id_bump_bp(id, bump * 100.0),
                        self.spot_price + bump,
                        |_, price| price + bump,
                    )
                }
                (BumpMode::Additive, BumpUnits::Percent)
                | (BumpMode::Multiplicative, BumpUnits::Percent) => self.scaled(spec.value / 100.0),
                (BumpMode::Multiplicative, BumpUnits::Factor) => self.scaled(spec.value - 1.0),
                _ => Err(InputError::UnsupportedBump {
                    reason: format!(
                        "PriceCurve parallel bump: unsupported mode/units {:?}/{:?}. \
                         Use Additive/{{Fraction,Percent}} or Multiplicative/{{Factor,Percent}}",
                        spec.mode, spec.units
                    ),
                }
                .into()),
            },
            BumpType::TriangularKeyRate {
                prev_bucket,
                target_bucket,
                next_bucket,
            } => match (spec.mode, spec.units) {
                (BumpMode::Additive, BumpUnits::Fraction) => {
                    let bump = spec.value;
                    let new_id = crate::market_data::bumps::id_bump_bp(id, bump * 100.0);
                    if self.knots.len() < 2 {
                        return self
                            .shocked(new_id, self.spot_price + bump, |_, price| price + bump);
                    }
                    // Spot is not bumped in a key-rate shock; neither are rolled
                    // source pillars at or before the current origin.
                    self.shocked(new_id, self.spot_price, |t, price| {
                        if t <= 0.0 {
                            price
                        } else {
                            price
                                + bump
                                    * triangular_weight(t, prev_bucket, target_bucket, next_bucket)
                        }
                    })
                }
                (BumpMode::Additive, BumpUnits::Percent) => {
                    let percentage = spec.value / 100.0;
                    // Spot is not bumped in a key-rate shock; neither are
                    // rolled source pillars at or before the current origin.
                    self.shocked(
                        spec.standard_bump_id(self.id()),
                        self.spot_price,
                        |time, price| {
                            if time <= 0.0 {
                                price
                            } else {
                                let weight = triangular_weight(
                                    time,
                                    prev_bucket,
                                    target_bucket,
                                    next_bucket,
                                );
                                price * (1.0 + percentage * weight)
                            }
                        },
                    )
                }
                _ => Err(InputError::UnsupportedBump {
                    reason: format!(
                        "PriceCurve key-rate bump requires Additive mode, got {:?}/{:?}",
                        spec.mode, spec.units
                    ),
                }
                .into()),
            },
        }
    }
}

// Knot helpers specific to signed price pillars.

/// Validate that all values in a knot slice are non-negative.
///
/// Returns a descriptive error that includes the tenor and value for quick diagnosis.
fn validate_non_negative_knots(
    knots: &[f64],
    values: &[f64],
    value_label: &str,
) -> crate::Result<()> {
    for (i, (&t, &v)) in knots.iter().zip(values.iter()).enumerate() {
        if v < 0.0 {
            return Err(crate::Error::Validation(format!(
                "{value_label} must be non-negative at t={t:.6}: value={v:.8} (index {i})"
            )));
        }
    }
    Ok(())
}

/// Infer the spot value from a knot set when the first knot is at t≈0.
///
/// Returns `Some(v)` when the first knot is at t=0 (within 1e-14), otherwise `None`.
#[inline]
fn infer_spot_from_knots(knots: &[f64], values: &[f64]) -> Option<f64> {
    knots
        .first()
        .filter(|&&t| t.abs() <= 1e-14)
        .map(|_| values[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentage_bucket_shocks_scale_local_signed_levels_and_preserve_spot() {
        for spot in [100.0, 0.0, -50.0] {
            let curve = PriceCurve::builder("LOCAL-PERCENT")
                .base_date(Date::from_calendar_date(2025, time::Month::January, 1).expect("date"))
                .knots([
                    (0.0, spot),
                    (0.5, 80.0),
                    (1.0, 200.0),
                    (1.5, -100.0),
                    (2.0, 300.0),
                ])
                .build()
                .expect("price curve");
            let bumped = curve
                .apply_bump(BumpSpec {
                    mode: BumpMode::Additive,
                    units: BumpUnits::Percent,
                    value: 10.0,
                    bump_type: BumpType::TriangularKeyRate {
                        prev_bucket: Some(0.0),
                        target_bucket: 1.0,
                        next_bucket: Some(2.0),
                    },
                })
                .expect("percentage bucket shock");
            for (time, expected) in [
                (0.0, spot),
                (0.5, 84.0),
                (1.0, 220.0),
                (1.5, -105.0),
                (2.0, 300.0),
            ] {
                assert!((bumped.price(time) - expected).abs() < 1e-12);
            }
        }
    }

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
    fn future_only_quotes_interpolate_from_explicit_spot() {
        for kind in [PriceCurveKind::Price, PriceCurveKind::VolIndex] {
            let curve = PriceCurve::builder("PRICE-ORIGIN")
                .kind(kind)
                .base_date(Date::from_calendar_date(2025, time::Month::January, 1).unwrap())
                .spot_price(75.0)
                .knots([(1.0, 80.0), (2.0, 100.0)])
                .build()
                .unwrap();
            assert_eq!(curve.knots(), &[0.0, 1.0, 2.0]);
            assert!((curve.price(0.5) - 77.5).abs() < 1e-12);
            assert!((curve.price(1e-6) - 75.000005).abs() < 1e-12);
            assert!((curve.price(1.0) - 80.0).abs() < 1e-12);

            let restored: PriceCurve =
                serde_json::from_str(&serde_json::to_string(&curve).unwrap()).unwrap();
            assert_eq!(restored.knots(), curve.knots());
            assert_eq!(restored.price(0.5), curve.price(0.5));
        }
    }

    #[test]
    fn conflicting_zero_time_price_is_rejected() {
        let result = PriceCurve::builder("PRICE-CONFLICT")
            .base_date(Date::from_calendar_date(2025, time::Month::January, 1).unwrap())
            .spot_price(75.0)
            .knots([(0.0, 100.0), (1.0, 110.0)])
            .build();
        assert!(result.is_err());
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
        let bumped = curve
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: 2.0,
                bump_type: BumpType::Parallel,
            })
            .expect("Bump should succeed");
        assert!((bumped.spot_price() - 77.0).abs() < 1e-10);
        assert!((bumped.price(0.25) - 78.5).abs() < 1e-10);
    }

    #[test]
    fn percentage_bump() {
        let curve = sample_wti_curve();
        let bumped = curve
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Percent,
                value: 10.0,
                bump_type: BumpType::Parallel,
            })
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
    fn triangular_bumps_preserve_signed_prices() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let curve = PriceCurve::builder("POWER")
            .base_date(base)
            .spot_price(-10.0)
            .knots([(0.25, -5.0), (0.5, -3.0), (1.0, 5.0)])
            .build()
            .unwrap();
        for bump in [0.0, -2.0, 6.0] {
            let bumped = curve
                .apply_bump(BumpSpec {
                    mode: BumpMode::Additive,
                    units: BumpUnits::Fraction,
                    value: bump,
                    bump_type: BumpType::TriangularKeyRate {
                        prev_bucket: None,
                        target_bucket: 0.25,
                        next_bucket: Some(0.5),
                    },
                })
                .unwrap();
            assert_eq!(bumped.price(0.25), -5.0 + bump);
            assert_eq!(bumped.price(0.5), -3.0);
            assert_eq!(bumped.price(1.0), 5.0);
            assert_eq!(bumped.kind(), PriceCurveKind::Price);
        }
    }

    #[test]
    fn triangular_bumps_preserve_spot_anchor() {
        for curve in [sample_wti_curve(), sample_vix_curve()] {
            for bump in [-5.0, 0.0, 5.0] {
                let bumped = curve
                    .apply_bump(BumpSpec {
                        mode: BumpMode::Additive,
                        units: BumpUnits::Fraction,
                        value: bump,
                        bump_type: BumpType::TriangularKeyRate {
                            prev_bucket: None,
                            target_bucket: 0.25,
                            next_bucket: Some(0.5),
                        },
                    })
                    .unwrap();
                assert_eq!(bumped.knots(), curve.knots());
                assert_eq!(bumped.spot_price(), curve.spot_price());
                assert_eq!(bumped.prices()[0], curve.spot_price());
                assert_eq!(bumped.price(0.0), curve.spot_price());
                assert!((bumped.price(1e-8) - curve.spot_price()).abs() < 1e-5);
                assert_eq!(bumped.price(0.25), curve.price(0.25) + bump);
                assert_eq!(bumped.price(0.5), curve.price(0.5));
            }
        }
        let bumped = sample_vix_curve()
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: -20.0,
                bump_type: BumpType::TriangularKeyRate {
                    prev_bucket: None,
                    target_bucket: 0.25,
                    next_bucket: Some(0.5),
                },
            })
            .expect("zero delivery level is valid while spot stays unchanged");
        assert_eq!(bumped.price(0.25), 0.0);
        assert_eq!(bumped.prices()[0], 18.5);
    }

    #[test]
    fn triangular_bumps_reject_negative_vol_index_levels() {
        let curve = sample_vix_curve();
        let error = curve
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: -21.0,
                bump_type: BumpType::TriangularKeyRate {
                    prev_bucket: None,
                    target_bucket: 0.25,
                    next_bucket: Some(0.5),
                },
            })
            .expect_err("negative volatility index level must fail");
        assert!(error.to_string().contains("non-negative"), "{error}");
    }

    #[test]
    fn triangular_bumps_reject_malformed_bucket_grids() {
        let curve = sample_wti_curve();
        for (previous, target, next) in [
            (None, f64::NAN, None),
            (Some(f64::NEG_INFINITY), 1.0, None),
            (None, 1.0, Some(f64::INFINITY)),
            (Some(2.0), 1.0, Some(3.0)),
            (Some(0.0), 1.0, Some(1.0)),
        ] {
            assert!(curve
                .apply_bump(BumpSpec {
                    mode: BumpMode::Additive,
                    units: BumpUnits::Fraction,
                    value: 10.0,
                    bump_type: BumpType::TriangularKeyRate {
                        prev_bucket: previous,
                        target_bucket: target,
                        next_bucket: next
                    }
                })
                .is_err());
        }
        assert!(curve
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: f64::NAN,
                bump_type: BumpType::TriangularKeyRate {
                    prev_bucket: None,
                    target_bucket: 1.0,
                    next_bucket: None
                }
            })
            .is_err());
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

    #[test]
    fn rolls_preserve_source_interpolation_through_rebuilds_bumps_and_serde() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let source = PriceCurve::builder("PRICE-ROLL")
            .base_date(base)
            .spot_price(100.0)
            .knots([(0.0, 100.0), (1.0, 110.0), (2.0, 130.0), (3.0, 160.0)])
            .interp(InterpStyle::CubicHermite)
            .build()
            .unwrap();
        let unchanged = source.roll_forward(0).unwrap();
        assert_eq!(
            serde_json::to_value(&source).unwrap(),
            serde_json::to_value(&unchanged).unwrap()
        );

        let rolled = source.roll_forward(365).unwrap();
        for time in [0.25, 0.5, 1.5] {
            assert!((rolled.price(time) - source.price(time + 1.0)).abs() < 1e-12);
        }
        let rebuilt = rolled
            .to_builder_with_id("PRICE-ROLL-REBUILT")
            .build()
            .unwrap();
        assert!((rebuilt.price(0.5) - source.price(1.5)).abs() < 1e-12);

        let restored: PriceCurve =
            serde_json::from_str(&serde_json::to_string(&rolled).unwrap()).unwrap();
        assert!((restored.price(0.5) - source.price(1.5)).abs() < 1e-12);
        let twice = restored.roll_forward(180).unwrap();
        assert!((twice.price(0.5) - source.price(1.5 + 180.0 / 365.0)).abs() < 1e-12);

        let bumped = restored
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: 2.0,
                bump_type: BumpType::Parallel,
            })
            .unwrap();
        assert!((bumped.price(0.5) - (restored.price(0.5) + 2.0)).abs() < 1e-12);

        // A zero shock leaves the rolled Hermite shape untouched off-pillar.
        let flat = rolled
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: 0.0,
                bump_type: BumpType::Parallel,
            })
            .unwrap();
        for time in [0.1, 0.25, 0.75] {
            assert_eq!(flat.price(time), rolled.price(time));
        }
    }

    #[test]
    fn rolled_log_linear_curve_takes_additive_and_key_rate_shocks() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let rolled = PriceCurve::builder("PRICE-LOGLIN")
            .base_date(base)
            .spot_price(100.0)
            .knots([(0.0, 100.0), (1.0, 110.0), (2.0, 130.0)])
            .interp(InterpStyle::LogLinear)
            .build()
            .unwrap()
            .roll_forward(182)
            .unwrap();

        let bumped = rolled
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: 2.0,
                bump_type: BumpType::Parallel,
            })
            .expect("additive shock on a rolled curve");
        assert!((bumped.spot_price() - (rolled.spot_price() + 2.0)).abs() < 1e-12);
        for &time in rolled.knots() {
            assert!((bumped.price(time) - (rolled.price(time) + 2.0)).abs() < 1e-12);
        }

        // A key-rate shock leaves the spot where it is.
        let key_rate = rolled
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: 1.0,
                bump_type: BumpType::TriangularKeyRate {
                    prev_bucket: None,
                    target_bucket: 0.5,
                    next_bucket: Some(1.5),
                },
            })
            .expect("key-rate shock on a rolled curve");
        assert!((key_rate.spot_price() - rolled.spot_price()).abs() < 1e-12);
        assert!(key_rate.price(rolled.knots()[1]) > rolled.price(rolled.knots()[1]));
    }

    #[test]
    fn percent_key_rate_bump_keeps_roll_state_and_matches_fraction_path() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let rolled = PriceCurve::builder("PRICE-ROLL-PCT")
            .base_date(base)
            .spot_price(100.0)
            .knots([(0.0, 100.0), (1.0, 110.0), (2.0, 130.0), (3.0, 160.0)])
            .interp(InterpStyle::CubicHermite)
            .build()
            .unwrap()
            .roll_forward(182)
            .unwrap();
        assert!(rolled.roll.is_some());

        // Bucket the pre-roll 2y pillar (now at knots()[2]) between its neighbours,
        // so a +10% shock there equals an absolute +13.0 shock on every pillar.
        let knots = rolled.knots();
        let (prev, target, next) = (Some(knots[1]), knots[2], Some(knots[3]));
        let percent = rolled
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Percent,
                value: 10.0,
                bump_type: BumpType::TriangularKeyRate {
                    prev_bucket: prev,
                    target_bucket: target,
                    next_bucket: next,
                },
            })
            .expect("percent key-rate shock on a rolled curve");
        let fraction = rolled
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: 13.0,
                bump_type: BumpType::TriangularKeyRate {
                    prev_bucket: prev,
                    target_bucket: target,
                    next_bucket: next,
                },
            })
            .expect("fraction key-rate shock on a rolled curve");

        assert!(percent.roll.is_some());
        assert_eq!(percent.knots(), rolled.knots());
        assert!((percent.price(target) - 143.0).abs() < 1e-12);
        assert!((percent.spot_price() - rolled.spot_price()).abs() < 1e-12);
        for time in [0.1, 0.25, knots[1], 0.75, target, 1.9, knots[3], 2.4] {
            assert!(
                (percent.price(time) - fraction.price(time)).abs() < 1e-12,
                "t={time}: percent {} vs fraction {}",
                percent.price(time),
                fraction.price(time)
            );
        }
    }

    #[test]
    fn backward_roll_rebuilds_from_rolled_pillars() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let curve = PriceCurve::builder("PRICE-BACK")
            .base_date(base)
            .spot_price(100.0)
            .knots([(0.0, 100.0), (1.0, 110.0)])
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .build()
            .unwrap();
        let back = curve.roll_forward(-365).expect("backward roll");
        assert_eq!(back.spot_price(), 100.0);
        assert!((back.price(2.0) - 110.0).abs() < 1e-12);
    }

    #[test]
    fn replacement_knots_drop_a_carried_roll_state() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let rolled = PriceCurve::builder("PRICE-ROLL")
            .base_date(base)
            .spot_price(100.0)
            .knots([(0.0, 100.0), (1.0, 110.0), (2.0, 130.0)])
            .interp(InterpStyle::CubicHermite)
            .build()
            .unwrap()
            .roll_forward(90)
            .unwrap();
        let extended = rolled
            .to_builder_with_id("PRICE-EXTENDED")
            .knots([(3.0, 140.0)])
            .build()
            .expect("new pillars define a fresh interpolation");
        assert!((extended.price(3.0) - 140.0).abs() < 1e-12);
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
    fn kind_labels_round_trip_and_reject_unknown_names() {
        for kind in [PriceCurveKind::Price, PriceCurveKind::VolIndex] {
            assert_eq!(kind.to_string().parse::<PriceCurveKind>().ok(), Some(kind));
        }
        assert_eq!(PriceCurveKind::VolIndex.as_str(), "vol_index");
        assert!("Price".parse::<PriceCurveKind>().is_err());
    }

    #[test]
    fn vol_index_bumps_preserve_kind() {
        let curve = sample_vix_curve();
        let bumped = curve
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: 2.0,
                bump_type: BumpType::Parallel,
            })
            .expect("Bump should succeed");
        assert_eq!(bumped.kind(), PriceCurveKind::VolIndex);
        assert!((bumped.spot_price() - 20.5).abs() < 1e-10);
        assert!((bumped.price(0.25) - 22.0).abs() < 1e-10);
        let scaled = curve
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Percent,
                value: 10.0,
                bump_type: BumpType::Parallel,
            })
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
