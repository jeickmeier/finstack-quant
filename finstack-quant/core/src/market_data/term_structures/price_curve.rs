//! Native price and volatility-index level curves.
//!
//! A source interpolates its quoted price or index levels in the original
//! curve clock. The market or contract identifies the asset, units and
//! delivery convention. An origin pillar is optional. Observed spot and
//! historical settlement fixings are separate market data.
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
//!     .kind(finstack_quant_core::market_data::term_structures::PriceCurveKind::Price)
//!     .day_count(finstack_quant_core::dates::DayCount::Act365F)
//!     .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
//!     .base_date(base)
//!     .knots([(0.0, 75.0), (0.25, 76.5), (0.5, 77.2), (1.0, 78.0)])
//!     .interp(InterpStyle::Linear)
//!     .build()
//!     .expect("PriceCurve builder should succeed");
//! assert!(curve.price(0.25).expect("valid pillar") > 0.0);
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

use crate::math::interp::exact_f64_eq;

use super::common::{CurveCalendar, signed_curve_year_fraction, validate_curve_clock};
use crate::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
use crate::{
    dates::{Date, DayCount},
    error::InputError,
    math::interp::Interp,
    types::CurveId,
};

/// Level family a [`PriceCurve`] represents.
///
/// The kind determines level validation and the typed market-context lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PriceCurveKind {
    /// Signed, finite forward prices, for example in USD per barrel.
    Price,
    /// Nonnegative volatility-index levels in index points.
    VolIndex,
}

impl PriceCurveKind {
    /// Canonical label: `"price"` or `"vol_index"`.
    ///
    /// This label is shared by the wire format and both host bindings.
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
/// Interpolates native quoted levels on an explicit time grid. Price levels
/// can be signed. Volatility-index levels are nonnegative index points.
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
    /// Resolved business-day calendar when required by the clock.
    calendar: Option<CurveCalendar>,
    /// Knot times in **years** (strictly increasing and nonnegative).
    knots: Box<[f64]>,
    /// Forward prices at each knot.
    prices: Box<[f64]>,
    /// Underlying interpolator.
    interp: Interp,
}

/// Raw serializable state of PriceCurve.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawPriceCurve {
    /// Curve identifier
    pub id: String,
    /// Base date
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub base: Date,
    /// Day count convention
    pub day_count: DayCount,
    /// Native level family.
    pub level_kind: PriceCurveKind,
    /// Registered holiday calendar for the source clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<String>,
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

        RawPriceCurve {
            id: curve.id.to_string(),
            base: curve.base,
            day_count: curve.day_count,
            level_kind: curve.kind,
            calendar_id: curve.calendar.map(|cal| cal.id),
            knot_points,
            interp_style: curve.interp.style(),
            extrapolation: curve.interp.extrapolation(),
        }
    }
}

impl TryFrom<RawPriceCurve> for PriceCurve {
    type Error = crate::Error;

    fn try_from(state: RawPriceCurve) -> crate::Result<Self> {
        PriceCurve::builder(state.id)
            .kind(state.level_kind)
            .base_date(state.base)
            .day_count(state.day_count)
            .calendar_id_opt(state.calendar_id)
            .knots(state.knot_points)
            .interp(state.interp_style)
            .extrapolation(state.extrapolation)
            .build()
    }
}

impl PriceCurve {
    /// Start building a price curve for the given `id`.
    ///
    /// # Arguments
    ///
    /// * `id` - Source identifier for typed price or volatility-index lookup.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> PriceCurveBuilder {
        PriceCurveBuilder {
            id: id.into(),
            kind: None,
            base: None,
            day_count: None,
            calendar_id: None,
            points: Vec::new(),
            style: None,
            extrapolation: None,
        }
    }

    /// Forward price at time `t` (in years from base date).
    ///
    /// An exact pillar returns its stored level. Between and beyond pillars,
    /// the explicit interpolation and tail policy govern the result.
    ///
    /// # Arguments
    /// * `t` - Finite nonnegative time in years from the source base date under its day count.
    ///
    /// # Returns
    /// The interpolated forward price at time `t`.
    ///
    /// # Errors
    /// Rejects negative or non-finite times, non-finite levels, unsupported extrapolation and
    /// negative volatility-index levels.
    #[inline]
    pub fn price(&self, t: f64) -> crate::Result<f64> {
        if !t.is_finite() || t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "PriceCurve '{}' requires a finite nonnegative time, got {t}",
                self.id
            )));
        }
        let price = self.interp.interp(t)?;
        if !price.is_finite()
            || (self.kind == PriceCurveKind::VolIndex && price < 0.0)
            || (self.interp.style() == InterpStyle::LogLinear && price <= 0.0)
        {
            return Err(crate::Error::Validation(format!(
                "Invalid {} level {price} at time {t} on curve '{}'",
                self.kind, self.id
            )));
        }
        Ok(price)
    }

    /// Forward price on a specific calendar date.
    ///
    /// # Arguments
    /// * `date` - Target calendar date evaluated under [`Self::day_count`].
    ///
    /// # Errors
    /// Returns an error if `date` precedes the source base or if evaluation fails under [`Self::price`].
    pub fn price_on_date(&self, date: Date) -> crate::Result<f64> {
        self.price(self.source_time(date)?)
    }

    /// Map a supported date to original source time without evaluating a level.
    pub(crate) fn source_time(&self, date: Date) -> crate::Result<f64> {
        if date < self.base {
            return Err(crate::Error::Validation(format!(
                "PriceCurve '{}' cannot evaluate {date} before its base {}",
                self.id, self.base
            )));
        }
        signed_curve_year_fraction(self.base, date, self.day_count, self.calendar.as_ref())
    }

    /// Registered calendar identifier used for dated curve queries.
    pub fn calendar_id(&self) -> Option<&str> {
        self.calendar.as_ref().map(|calendar| calendar.id.as_str())
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
    /// The builder keeps the curve's pillars so `build()` reproduces
    /// the curve under `new_id`.
    ///
    /// # Arguments
    ///
    /// * `new_id` - Identifier of the rebuilt curve, the `MarketContext` lookup key.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> PriceCurveBuilder {
        self.metadata_builder(new_id)
            .knots(self.knots.iter().copied().zip(self.prices.iter().copied()))
    }

    /// Builder pre-populated with this curve's metadata (kind, base date,
    /// day count, interpolation, extrapolation) without knots.
    /// Shared by every rebuild-style operation so no field is dropped.
    fn metadata_builder(&self, new_id: impl Into<CurveId>) -> PriceCurveBuilder {
        PriceCurve::builder(new_id)
            .kind(self.kind)
            .base_date(self.base)
            .day_count(self.day_count)
            .calendar_id_opt(self.calendar_id().map(str::to_owned))
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
    }

    /// Rebuild this curve with replacement knots while preserving
    /// its id, kind, base date, day count, interpolation and extrapolation.
    ///
    /// # Arguments
    ///
    /// * `knots` - Replacement `(time, price)` pillars: times in year
    ///   fractions from the base date under the curve's day count, prices in
    ///   the curve's native price unit.
    ///
    /// # Errors
    ///
    /// Returns an error when the replacement knots violate the
    /// curve's validation or interpolation constraints.
    pub fn rebuild_with_knots<I>(&self, knots: I) -> crate::Result<Self>
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.metadata_builder(self.id.clone()).knots(knots).build()
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
///     .kind(finstack_quant_core::market_data::term_structures::PriceCurveKind::Price)
///     .day_count(finstack_quant_core::dates::DayCount::Act365F)
///     .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
///     .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
///     .base_date(base)
///     .knots([(0.0, 75.0), (0.25, 76.5), (0.5, 77.2), (1.0, 78.0)])
///     .build()
///     .expect("PriceCurve builder should succeed");
/// assert!(curve.price(0.5).expect("valid pillar") > 75.0);
/// ```
pub struct PriceCurveBuilder {
    id: CurveId,
    kind: Option<PriceCurveKind>,
    base: Option<Date>,
    day_count: Option<DayCount>,
    calendar_id: Option<String>,
    points: Vec<(f64, f64)>,
    style: Option<InterpStyle>,
    extrapolation: Option<ExtrapolationPolicy>,
}

impl PriceCurveBuilder {
    /// Select the level family (signed prices or non-negative vol-index levels).
    ///
    /// # Arguments
    ///
    /// * `kind` - Quote family controlling whether negative levels are accepted when the curve is built.
    pub fn kind(mut self, kind: PriceCurveKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Set the source valuation base date.
    ///
    /// # Arguments
    ///
    /// * `d` - First admissible query date, corresponding to zero on the source clock.
    pub fn base_date(mut self, d: Date) -> Self {
        self.base = Some(d);
        self
    }

    /// Choose the day-count convention for dated level queries.
    ///
    /// # Arguments
    ///
    /// * `day_count` - Source clock for mapping dates to year fractions. Bus/252 also requires a registered calendar.
    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = Some(day_count);
        self
    }

    /// Set the holiday calendar for the source day count.
    ///
    /// # Arguments
    ///
    /// * `calendar_id` - Registered calendar id or joint id required for Bus/252.
    pub fn calendar_id(mut self, calendar_id: impl Into<String>) -> Self {
        self.calendar_id = Some(calendar_id.into());
        self
    }

    /// Set an optional holiday calendar for the source day count.
    ///
    /// # Arguments
    ///
    /// * `calendar_id` - Registered calendar id, or `None` for a clock independent of holidays.
    pub fn calendar_id_opt(mut self, calendar_id: Option<String>) -> Self {
        self.calendar_id = calendar_id;
        self
    }

    /// Supply knot points `(t, forward_price)`.
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
        self
    }

    /// Select interpolation style for this curve.
    ///
    /// # Arguments
    ///
    /// * `style` - Native level interpolation, one of linear, cubic Hermite or log-linear. Log-linear requires positive quotes.
    pub fn interp(mut self, style: InterpStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Set the extrapolation policy for out-of-bounds evaluation.
    ///
    /// # Arguments
    ///
    /// * `policy` - Hold the boundary level, extend the interpolant or reject queries outside the source pillars.
    pub fn extrapolation(mut self, policy: ExtrapolationPolicy) -> Self {
        self.extrapolation = Some(policy);
        self
    }

    /// Validate input and build the [`PriceCurve`].
    ///
    /// The kind, base date, clock, scalar interpolation style and tail
    /// policy must all be supplied. At least two strictly increasing
    /// nonnegative times with finite levels are required.
    ///
    /// # Errors
    ///
    /// Returns an error for missing source metadata, an incomplete clock,
    /// invalid pillars, a factor-only interpolation style or negative
    /// volatility-index levels.
    pub fn build(self) -> crate::Result<PriceCurve> {
        let missing =
            |field: &str| crate::Error::Validation(format!("PriceCurve requires {field}"));
        let kind = self.kind.ok_or_else(|| missing("kind"))?;
        let day_count = self.day_count.ok_or_else(|| missing("day_count"))?;
        let style = self.style.ok_or_else(|| missing("interp"))?;
        let extrapolation = self.extrapolation.ok_or_else(|| missing("extrapolation"))?;
        let calendar = validate_curve_clock(
            "PriceCurve",
            self.id.as_str(),
            day_count,
            self.calendar_id.as_deref(),
        )?;
        let base = self
            .base
            .ok_or_else(|| crate::Error::Validation("PriceCurve requires base_date".to_string()))?;
        let (kvec, pvec): (Vec<f64>, Vec<f64>) = self.points.into_iter().unzip();
        crate::math::interp::validate_knots(&kvec)?;

        if pvec.iter().any(|price| !price.is_finite()) {
            return Err(crate::Error::Validation(
                "price curve pillar values must be finite".to_string(),
            ));
        }

        if kvec[0] < 0.0 {
            return Err(crate::Error::Validation(
                "price curve pillar times must be nonnegative".into(),
            ));
        }
        if kind == PriceCurveKind::VolIndex {
            validate_non_negative_knots(&kvec, &pvec, "Volatility index level")?;
        }
        let knots = kvec.into_boxed_slice();
        let prices = pvec.into_boxed_slice();

        if matches!(
            style,
            InterpStyle::MonotoneConvex | InterpStyle::PiecewiseQuadraticForward
        ) {
            return Err(crate::Error::Validation(
                "Discount-factor forward interpolation does not apply to price levels".to_string(),
            ));
        }
        let interp = style.build_enum(knots.clone(), prices.clone(), extrapolation)?;

        Ok(PriceCurve {
            id: self.id,
            kind,
            base,
            day_count,
            calendar,
            knots,
            prices,
            interp,
        })
    }
}

/// Direct parameter shocks change original source pillars in native units.
impl Bumpable for PriceCurve {
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        spec.validate()?;
        let relative = match (spec.mode, spec.units) {
            (BumpMode::Additive, BumpUnits::Fraction) => false,
            (BumpMode::Multiplicative, BumpUnits::Percent | BumpUnits::Factor) => true,
            _ => {
                return Err(InputError::UnsupportedBump {
                    reason: format!(
                        "unsupported price-curve shock {:?}/{:?}",
                        spec.mode, spec.units
                    ),
                }
                .into());
            }
        };
        if self.kind == PriceCurveKind::VolIndex
            && relative
            && ((spec.units == BumpUnits::Factor && spec.value < 0.0)
                || (spec.units == BumpUnits::Percent && spec.value < -100.0))
        {
            return Err(crate::Error::Validation(
                "relative shock would reverse volatility-index levels".into(),
            ));
        }
        if let BumpType::TriangularKeyRate {
            prev_bucket,
            target_bucket,
            next_bucket,
        } = spec.bump_type
            && (prev_bucket.is_some_and(|t| t < 0.0)
                || target_bucket < 0.0
                || next_bucket.is_some_and(|t| t < 0.0))
        {
            return Err(crate::Error::Validation(
                "price shock bucket times must be nonnegative".into(),
            ));
        }
        let mut supported = false;
        let mut points = Vec::with_capacity(self.len());
        let magnitude = if spec.units == BumpUnits::Percent {
            let normalized = spec.value / 100.0;
            if normalized == 0.0 && spec.value != 0.0 {
                return Err(crate::Error::Validation(
                    "relative price shock underflow".into(),
                ));
            }
            normalized
        } else {
            spec.value
        };
        let identity = if spec.units == BumpUnits::Factor {
            1.0
        } else {
            0.0
        };
        for (&time, &price) in self.knots.iter().zip(&self.prices) {
            let w = spec.bump_type.weight(time);
            supported |= w > 0.0;
            let candidate = if w == 0.0 || exact_f64_eq(spec.value, identity) {
                price
            } else if !relative {
                magnitude.mul_add(w, price)
            } else {
                let multiplier = if spec.units == BumpUnits::Factor {
                    if exact_f64_eq(w, 1.0) {
                        magnitude
                    } else {
                        magnitude.mul_add(w, 1.0 - w)
                    }
                } else {
                    spec.value.mul_add(w, 100.0) / 100.0
                };
                let value = if spec.units == BumpUnits::Percent && magnitude.abs() < 0.5 {
                    price.mul_add(magnitude * w, price)
                } else {
                    price * multiplier
                };
                if value == 0.0 && price != 0.0 && multiplier != 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "price shock underflow at {time}"
                    )));
                }
                value
            };
            points.push((time, candidate));
        }
        if !supported {
            return Err(crate::Error::Validation(
                "price shock has no support on any source pillar".into(),
            ));
        }
        self.rebuild_with_knots(points)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_vol_index_extension_is_rejected_for_numeric_and_dated_queries() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let expiry = Date::from_calendar_date(2026, time::Month::January, 1).unwrap();
        for policy in [ExtrapolationPolicy::Extend, ExtrapolationPolicy::Error] {
            let curve = PriceCurve::builder("VIX")
                .day_count(DayCount::Act365F)
                .interp(InterpStyle::Linear)
                .kind(PriceCurveKind::VolIndex)
                .base_date(base)
                .knots([(0.0, 30.0), (0.25, 20.0)])
                .extrapolation(policy)
                .build()
                .unwrap();
            assert_eq!(curve.price(0.0).unwrap(), 30.0);
            assert_eq!(curve.price(0.25).unwrap(), 20.0);
            assert!(curve.price(1.0).is_err());
            assert!(curve.price_on_date(expiry).is_err());
        }
    }

    #[test]
    fn percentage_bucket_shocks_scale_local_signed_levels() {
        for spot in [100.0, 0.0, -50.0] {
            let curve = PriceCurve::builder("LOCAL-PERCENT")
                .kind(PriceCurveKind::Price)
                .day_count(DayCount::Act365F)
                .interp(InterpStyle::Linear)
                .extrapolation(ExtrapolationPolicy::Flat)
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
                    mode: BumpMode::Multiplicative,
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
                assert!((bumped.price(time).unwrap() - expected).abs() < 1e-12);
            }
        }
    }

    fn sample_wti_curve() -> PriceCurve {
        PriceCurve::builder("WTI-FORWARD")
            .kind(PriceCurveKind::Price)
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .base_date(
                Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
            )
            .knots([(0.0, 75.0), (0.25, 76.5), (0.5, 77.2), (1.0, 78.0)])
            .build()
            .expect("PriceCurve builder should succeed with valid test data")
    }

    #[test]
    fn interpolates_forward_price() {
        let curve = sample_wti_curve();
        // At 0.25Y, should be 76.5
        assert!((curve.price(0.25).unwrap() - 76.5).abs() < 1e-10);
        // At 0.125Y (midpoint of first segment), should be ~75.75
        let mid = curve.price(0.125).unwrap();
        assert!((mid - 75.75).abs() < 0.01, "Expected ~75.75, got {}", mid);
    }

    #[test]
    fn explicit_origin_is_a_price_pillar() {
        let curve = sample_wti_curve();
        assert!((curve.price(0.0).unwrap() - 75.0).abs() < 1e-10);
    }

    #[test]
    fn future_only_sources_preserve_the_grid_and_apply_declared_tails() {
        for first in [1e-16, 1.0] {
            let base = time::macros::date!(2025 - 01 - 01);
            let curve = PriceCurve::builder("WTI")
                .kind(PriceCurveKind::Price)
                .day_count(DayCount::Act365F)
                .interp(InterpStyle::Linear)
                .base_date(base)
                .knots([(first, 80.0), (2.0, 100.0)])
                .extrapolation(ExtrapolationPolicy::Error)
                .build()
                .unwrap();
            assert_eq!(curve.knots(), &[first, 2.0]);
            assert!(curve.price(0.0).is_err());
            assert!(curve.price_on_date(base).is_err());
            assert_eq!(curve.price(first).unwrap(), 80.0);
            let restored: PriceCurve =
                serde_json::from_value(serde_json::to_value(&curve).unwrap()).unwrap();
            assert_eq!(restored.knots(), curve.knots());
        }
    }

    #[test]
    fn generic_construction_requires_explicit_source_conventions() {
        let builder = || {
            PriceCurve::builder("P")
                .base_date(time::macros::date!(2025 - 01 - 01))
                .knots([(0.0, 1.0), (1.0, 2.0)])
        };
        assert!(builder().build().is_err());
        assert!(
            builder()
                .kind(PriceCurveKind::Price)
                .interp(InterpStyle::Linear)
                .extrapolation(ExtrapolationPolicy::Flat)
                .build()
                .is_err()
        );
        assert!(
            builder()
                .kind(PriceCurveKind::Price)
                .day_count(DayCount::Act365F)
                .extrapolation(ExtrapolationPolicy::Flat)
                .build()
                .is_err()
        );
        assert!(
            builder()
                .kind(PriceCurveKind::Price)
                .day_count(DayCount::Act365F)
                .interp(InterpStyle::Linear)
                .build()
                .is_err()
        );
    }

    #[test]
    fn complete_clock_and_native_interpolation_are_required() {
        let builder = || {
            PriceCurve::builder("WTI")
                .kind(PriceCurveKind::Price)
                .day_count(DayCount::Act365F)
                .interp(InterpStyle::Linear)
                .extrapolation(ExtrapolationPolicy::Flat)
                .base_date(time::macros::date!(2025 - 01 - 01))
                .knots([(0.5, 1.0), (1.0, 2.0)])
        };
        for day_count in [
            DayCount::Bus252,
            DayCount::ActActIsma,
            DayCount::Act365L,
            DayCount::ThirtyE360Isda,
        ] {
            assert!(builder().day_count(day_count).build().is_err());
        }
        for style in [
            InterpStyle::MonotoneConvex,
            InterpStyle::PiecewiseQuadraticForward,
        ] {
            assert!(builder().interp(style).build().is_err());
        }
        let curve = builder()
            .day_count(DayCount::Bus252)
            .calendar_id("weekends_only")
            .build()
            .unwrap();
        assert_eq!(curve.price_on_date(curve.base_date()).unwrap(), 1.0);
        assert_eq!(curve.calendar_id(), Some("weekends_only"));
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
        assert!((bumped.price(0.0).unwrap() - 77.0).abs() < 1e-10);
        assert!((bumped.price(0.25).unwrap() - 78.5).abs() < 1e-10);
    }

    #[test]
    fn percentage_bump() {
        let curve = sample_wti_curve();
        let bumped = curve
            .apply_bump(BumpSpec {
                mode: BumpMode::Multiplicative,
                units: BumpUnits::Percent,
                value: 10.0,
                bump_type: BumpType::Parallel,
            })
            .expect("Bump should succeed");
        // 75.0 * 1.10 = 82.5
        assert!((bumped.price(0.0).unwrap() - 82.5).abs() < 1e-10);
    }

    #[test]
    fn permits_finite_signed_futures_prices() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let curve = PriceCurve::builder("POWER")
            .kind(PriceCurveKind::Price)
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .base_date(base)
            .knots([(0.0, -10.0), (1.0, -20.0)])
            .build()
            .expect("finite signed prices are valid market data");
        assert!((curve.price(0.5).unwrap() + 15.0).abs() < 1e-12);
    }

    #[test]
    fn triangular_bumps_preserve_signed_prices() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let curve = PriceCurve::builder("POWER")
            .kind(PriceCurveKind::Price)
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .base_date(base)
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
            assert_eq!(bumped.price(0.25).unwrap(), -5.0 + bump);
            assert_eq!(bumped.price(0.5).unwrap(), -3.0);
            assert_eq!(bumped.price(1.0).unwrap(), 5.0);
            assert_eq!(bumped.kind(), PriceCurveKind::Price);
        }
    }

    #[test]
    fn triangular_bumps_include_the_origin_when_the_wing_has_support() {
        for curve in [sample_wti_curve(), sample_vix_curve()] {
            let bumped = curve
                .apply_bump(BumpSpec {
                    mode: BumpMode::Additive,
                    units: BumpUnits::Fraction,
                    value: 2.0,
                    bump_type: BumpType::TriangularKeyRate {
                        prev_bucket: None,
                        target_bucket: 0.25,
                        next_bucket: Some(0.5),
                    },
                })
                .unwrap();
            assert_eq!(bumped.price(0.0).unwrap(), curve.price(0.0).unwrap() + 2.0);
            assert_eq!(
                bumped.price(0.25).unwrap(),
                curve.price(0.25).unwrap() + 2.0
            );
            assert_eq!(bumped.price(0.5).unwrap(), curve.price(0.5).unwrap());
        }
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
            assert!(
                curve
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
                    .is_err()
            );
        }
        assert!(
            curve
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
                .is_err()
        );
    }

    #[test]
    fn requires_at_least_two_points() {
        let result = PriceCurve::builder("WTI")
            .base_date(sample_wti_curve().base_date())
            .kind(PriceCurveKind::Price)
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .knots([(0.0, 75.0)])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn serde_round_trip() {
        let curve = sample_wti_curve();
        let json = serde_json::to_string(&curve).expect("Serialize should succeed");
        let recovered: PriceCurve =
            serde_json::from_str(&json).expect("Deserialize should succeed");
        assert_eq!(curve.id(), recovered.id());
        assert!((curve.price(0.0).unwrap() - recovered.price(0.0).unwrap()).abs() < 1e-10);
        assert!((curve.price(0.5).unwrap() - recovered.price(0.5).unwrap()).abs() < 1e-10);
    }

    fn sample_vix_curve() -> PriceCurve {
        PriceCurve::builder("VIX")
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .kind(PriceCurveKind::VolIndex)
            .base_date(
                Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
            )
            .knots([(0.0, 18.5), (0.25, 20.0), (0.5, 21.5), (1.0, 22.0)])
            .build()
            .expect("vol-index PriceCurve builder should succeed with valid test data")
    }

    #[test]
    fn vol_index_rejects_negative_levels() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let result = PriceCurve::builder("VIX")
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .kind(PriceCurveKind::VolIndex)
            .base_date(base)
            .knots([(0.0, 18.5), (0.5, -5.0)])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn kind_is_explicit_in_wire_state() {
        for curve in [sample_wti_curve(), sample_vix_curve()] {
            let mut wire = serde_json::to_value(&curve).unwrap();
            assert_eq!(wire["level_kind"], curve.kind().as_str());
            assert_eq!(
                serde_json::from_value::<PriceCurve>(wire.clone())
                    .unwrap()
                    .kind(),
                curve.kind()
            );
            wire.as_object_mut().unwrap().remove("level_kind");
            assert!(serde_json::from_value::<PriceCurve>(wire).is_err());
        }
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
        assert!((bumped.price(0.0).unwrap() - 20.5).abs() < 1e-10);
        assert!((bumped.price(0.25).unwrap() - 22.0).abs() < 1e-10);
        let scaled = curve
            .apply_bump(BumpSpec {
                mode: BumpMode::Multiplicative,
                units: BumpUnits::Percent,
                value: 10.0,
                bump_type: BumpType::Parallel,
            })
            .expect("Bump should succeed");
        assert_eq!(scaled.kind(), PriceCurveKind::VolIndex);
        assert!((scaled.price(0.0).unwrap() - 20.35).abs() < 1e-10);
    }

    #[test]
    fn price_on_date() {
        let base = Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid date");
        let curve = PriceCurve::builder("WTI")
            .kind(PriceCurveKind::Price)
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .base_date(base)
            .knots([(0.0, 75.0), (1.0, 78.0)])
            .build()
            .expect("Should build");

        assert_eq!(curve.price_on_date(base).unwrap(), 75.0);

        // 6 months forward
        let six_months = Date::from_calendar_date(2025, time::Month::July, 1).expect("Valid date");
        let fwd_price = curve.price_on_date(six_months).expect("Should succeed");
        assert!(fwd_price > 75.0 && fwd_price < 78.0);
    }

    #[test]
    fn pre_base_pillars_and_queries_are_rejected() {
        let curve = sample_wti_curve();
        assert!(curve.price(-0.25).is_err());
        assert!(
            curve
                .price_on_date(curve.base_date().previous_day().unwrap())
                .is_err()
        );
        assert!(
            curve
                .rebuild_with_knots([(-0.25, 1.0), (0.0, 2.0)])
                .is_err()
        );
    }

    #[test]
    fn identity_bumps_preserve_source_levels_bit_for_bit() {
        let curve = sample_wti_curve()
            .rebuild_with_knots([(0.0, -0.0), (1.0, 1.0)])
            .unwrap();
        for (mode, units, value) in [
            (BumpMode::Additive, BumpUnits::Fraction, 0.0),
            (BumpMode::Multiplicative, BumpUnits::Percent, 0.0),
            (BumpMode::Multiplicative, BumpUnits::Factor, 1.0),
        ] {
            let bumped = curve
                .apply_bump(BumpSpec {
                    mode,
                    units,
                    value,
                    bump_type: BumpType::Parallel,
                })
                .unwrap();
            assert_eq!(bumped.prices()[0].to_bits(), (-0.0_f64).to_bits());
            assert_eq!(bumped.prices()[1], 1.0);
        }
    }

    #[test]
    fn factor_shocks_preserve_tiny_factors_and_signed_price_stresses() {
        let curve = sample_wti_curve()
            .rebuild_with_knots([(0.0, 1e300), (1.0, -1e300)])
            .unwrap();
        let mut spec = BumpSpec {
            mode: BumpMode::Multiplicative,
            units: BumpUnits::Factor,
            value: 1e-300,
            bump_type: BumpType::Parallel,
        };
        let tiny = curve.apply_bump(spec).unwrap();
        assert_eq!(tiny.prices(), &[1.0, -1.0]);
        assert_eq!(tiny.id(), curve.id());
        spec.value = -1.0;
        assert_eq!(curve.apply_bump(spec).unwrap().prices(), &[-1e300, 1e300]);
        spec.bump_type = BumpType::TriangularKeyRate {
            prev_bucket: Some(2.0),
            target_bucket: 3.0,
            next_bucket: Some(4.0),
        };
        assert!(curve.apply_bump(spec).is_err());
        spec.bump_type = BumpType::TriangularKeyRate {
            prev_bucket: Some(0.0),
            target_bucket: 0.5,
            next_bucket: Some(1.5),
        };
        assert_eq!(curve.apply_bump(spec).unwrap().price(1.0).unwrap(), 0.0);
        spec.mode = BumpMode::Additive;
        spec.units = BumpUnits::Percent;
        assert!(curve.apply_bump(spec).is_err());
    }
}
