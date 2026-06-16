//! Basis spread curves for cross-currency and multi-curve frameworks.
//!
//! A basis spread curve stores `(time, spread)` knots representing the
//! continuously compounded spread between two discount curves, typically
//! arising from cross-currency basis swap calibration.
//!
//! # Financial Concept
//!
//! The basis spread `s(t)` captures the funding cost differential between
//! two currencies or two discount curves:
//! ```text
//! spread(t) = z_foreign(t) - z_domestic(t)
//!
//! where z(t) is the continuously compounded zero rate
//! ```
//!
//! # Market Construction
//!
//! Basis spread curves are typically derived as a byproduct of cross-currency
//! basis curve bootstrapping:
//! 1. Bootstrap the foreign discount curve from XCCY basis swaps or FX forwards
//! 2. Extract `spread(T) = z_foreign(T) - z_domestic(T)` at each pillar
//! 3. Interpolate between pillars for intermediate tenors
//!
//! # Use Cases
//!
//! - **Cross-currency valuation**: FX-implied discount factor adjustments
//! - **Basis risk analytics**: Monitor cross-currency funding costs
//! - **Multi-curve framework**: Spread decomposition across curves
//!
//! # References
//!
//! - Andersen, L., & Piterbarg, V. (2010). *Interest Rate Modeling* (3 vols).
//!   Atlantic Financial Press. Volume 1, Chapter 4.
//! - Fujii, M., Shimada, Y., & Takahashi, A. (2011). "A Note on Construction of
//!   Multiple Swap Curves with and without Collateral." *FSA Research Review*, 7.

use super::common::{
    build_interp_allow_any_values, default_curve_base_date, roll_knots, split_points,
    year_fraction_to,
};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
use crate::{
    dates::{Date, DayCount},
    error::InputError,
    market_data::traits::TermStructure,
    math::interp::types::Interp,
    types::CurveId,
};

/// Basis spread curve storing continuously compounded spread values.
///
/// Represents the spread between two zero-rate curves at discrete pillar
/// points. Spread values may be positive or negative.
///
/// # Thread Safety
///
/// Immutable after construction; safe to share via `Arc<BasisSpreadCurve>`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawBasisSpreadCurve", into = "RawBasisSpreadCurve")]
pub struct BasisSpreadCurve {
    id: CurveId,
    base: Date,
    /// Day-count basis used for time calculations.
    day_count: DayCount,
    /// Knot times in years (strictly increasing).
    knots: Box<[f64]>,
    /// Continuously compounded spread values at each knot.
    spreads: Box<[f64]>,
    interp: Interp,
}

/// Raw serializable state of BasisSpreadCurve.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBasisSpreadCurve {
    /// Curve identifier.
    pub id: String,
    /// Base date.
    pub base: Date,
    /// Day count convention.
    pub day_count: DayCount,
    /// Time/value pairs used to construct the curve.
    pub knot_points: Vec<(f64, f64)>,
    /// Interpolation style.
    pub interp_style: InterpStyle,
    /// Extrapolation policy.
    pub extrapolation: ExtrapolationPolicy,
}

impl From<BasisSpreadCurve> for RawBasisSpreadCurve {
    fn from(curve: BasisSpreadCurve) -> Self {
        let knot_points: Vec<(f64, f64)> = curve
            .knots
            .iter()
            .zip(curve.spreads.iter())
            .map(|(&t, &s)| (t, s))
            .collect();

        RawBasisSpreadCurve {
            id: curve.id.to_string(),
            base: curve.base,
            day_count: curve.day_count,
            knot_points,
            interp_style: curve.interp.style(),
            extrapolation: curve.interp.extrapolation(),
        }
    }
}

impl TryFrom<RawBasisSpreadCurve> for BasisSpreadCurve {
    type Error = crate::Error;

    fn try_from(state: RawBasisSpreadCurve) -> crate::Result<Self> {
        BasisSpreadCurve::builder(state.id)
            .base_date(state.base)
            .day_count(state.day_count)
            .knots(state.knot_points)
            .interp(state.interp_style)
            .extrapolation(state.extrapolation)
            .build()
    }
}

impl BasisSpreadCurve {
    /// Start building a basis spread curve for the given `id`.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> BasisSpreadCurveBuilder {
        BasisSpreadCurveBuilder {
            id: id.into(),
            base: default_curve_base_date(),
            base_is_set: false,
            day_count: DayCount::Act365F,
            points: Vec::new(),
            style: InterpStyle::Linear,
            extrapolation: ExtrapolationPolicy::FlatZero,
        }
    }

    /// Continuously compounded spread at time `t` (in years from base date).
    #[must_use]
    #[inline]
    pub fn spread(&self, t: f64) -> f64 {
        self.interp.interp(t)
    }

    /// Curve identifier.
    #[must_use]
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Base date of the curve.
    #[must_use]
    #[inline]
    pub fn base_date(&self) -> Date {
        self.base
    }

    /// Day count convention used for time calculations.
    #[must_use]
    #[inline]
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Interpolation style used by this curve.
    #[must_use]
    #[inline]
    pub fn interp_style(&self) -> InterpStyle {
        self.interp.style()
    }

    /// Extrapolation policy used by this curve.
    #[must_use]
    #[inline]
    pub fn extrapolation(&self) -> ExtrapolationPolicy {
        self.interp.extrapolation()
    }

    /// Knot times (in years).
    #[must_use]
    #[inline]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Spread values at each knot.
    #[must_use]
    #[inline]
    pub fn spreads(&self) -> &[f64] {
        &self.spreads
    }

    /// Roll the curve forward by `days` calendar days.
    ///
    /// The time shift uses the curve's own `day_count`, matching
    /// `DiscountCurve::roll_forward` and siblings.
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        let new_base = self.base + time::Duration::days(days);
        let dt = year_fraction_to(self.base, new_base, self.day_count)?;
        let rolled = roll_knots(&self.knots, &self.spreads, dt);
        if rolled.is_empty() {
            return Err(crate::Error::Validation(
                "All knots expired after rolling forward".to_string(),
            ));
        }
        Self::builder(self.id.clone())
            .base_date(new_base)
            .day_count(self.day_count)
            .knots(rolled)
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
            .build()
    }

    /// Create a builder from this curve with a new ID (for rebuildable-with-id pattern).
    pub fn to_builder_with_id(&self, id: CurveId) -> BasisSpreadCurveBuilder {
        let points: Vec<(f64, f64)> = self
            .knots
            .iter()
            .zip(self.spreads.iter())
            .map(|(&t, &s)| (t, s))
            .collect();
        BasisSpreadCurveBuilder {
            id,
            base: self.base,
            base_is_set: true,
            day_count: self.day_count,
            points,
            style: self.interp.style(),
            extrapolation: self.interp.extrapolation(),
        }
    }
}

impl TermStructure for BasisSpreadCurve {
    fn id(&self) -> &CurveId {
        &self.id
    }
}

/// Builder for [`BasisSpreadCurve`].
pub struct BasisSpreadCurveBuilder {
    id: CurveId,
    base: Date,
    base_is_set: bool,
    day_count: DayCount,
    points: Vec<(f64, f64)>,
    style: InterpStyle,
    extrapolation: ExtrapolationPolicy,
}

impl BasisSpreadCurveBuilder {
    /// Set the base date.
    pub fn base_date(mut self, date: Date) -> Self {
        self.base = date;
        self.base_is_set = true;
        self
    }

    /// Set the day count convention.
    pub fn day_count(mut self, dc: DayCount) -> Self {
        self.day_count = dc;
        self
    }

    /// Set the knot points as `(time, spread)` pairs.
    pub fn knots(mut self, pts: impl IntoIterator<Item = (f64, f64)>) -> Self {
        self.points = pts.into_iter().collect();
        self
    }

    /// Set the interpolation style.
    pub fn interp(mut self, style: InterpStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the extrapolation policy.
    pub fn extrapolation(mut self, policy: ExtrapolationPolicy) -> Self {
        self.extrapolation = policy;
        self
    }

    /// Build the curve, validating inputs.
    pub fn build(self) -> crate::Result<BasisSpreadCurve> {
        if !self.base_is_set {
            return Err(InputError::Invalid.into());
        }
        if self.points.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }

        let (knots_vec, spreads_vec) = split_points(self.points);
        let knots: Box<[f64]> = knots_vec.into();
        let spreads: Box<[f64]> = spreads_vec.into();

        let interp = build_interp_allow_any_values(
            self.style,
            knots.clone(),
            spreads.clone(),
            self.extrapolation,
        )?;

        Ok(BasisSpreadCurve {
            id: self.id,
            base: self.base,
            day_count: self.day_count,
            knots,
            spreads,
            interp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn base_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).unwrap()
    }

    #[test]
    fn round_trip_serde() {
        let curve = BasisSpreadCurve::builder("USD-EUR-BASIS")
            .base_date(base_date())
            .knots([(1.0, 0.001), (5.0, 0.002), (10.0, 0.003)])
            .build()
            .unwrap();

        let json = serde_json::to_string(&curve).unwrap();
        let restored: BasisSpreadCurve = serde_json::from_str(&json).unwrap();
        assert_eq!(curve.id(), restored.id());
        assert!((curve.spread(3.0) - restored.spread(3.0)).abs() < 1e-12);
    }

    #[test]
    fn spread_interpolation() {
        let curve = BasisSpreadCurve::builder("TEST")
            .base_date(base_date())
            .knots([(0.0, 0.0), (10.0, 0.01)])
            .build()
            .unwrap();

        assert!((curve.spread(0.0)).abs() < 1e-12);
        assert!((curve.spread(5.0) - 0.005).abs() < 1e-12);
        assert!((curve.spread(10.0) - 0.01).abs() < 1e-12);
    }

    #[test]
    fn negative_spreads_allowed() {
        let curve = BasisSpreadCurve::builder("TEST")
            .base_date(base_date())
            .knots([(0.0, -0.005), (10.0, -0.002)])
            .build();
        assert!(curve.is_ok());
    }
}
