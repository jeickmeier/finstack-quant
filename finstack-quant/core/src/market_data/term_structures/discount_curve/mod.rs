//! Discount factor curves for present value calculations.
//!
//! A discount curve represents the time value of money, mapping curve times and
//! calendar dates to finite strictly positive discount factors `D(t) > 0`.
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::dates::{Date, DayCount};
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
//! use time::Month;
//!
//! let curve = DiscountCurve::builder("USD-OIS")
//!     .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
//!     .day_count(DayCount::Act365F)
//!     .knots([(0.0, 1.0), (5.0, 0.9)])
//!     .interp(InterpStyle::MonotoneConvex)
//!     .extrapolation(ExtrapolationPolicy::Extend)
//!     .build()
//!     .expect("DiscountCurve builder should succeed");
//! assert!(curve.df(3.0).expect("valid df") < 1.0);
//! ```

mod builder;
mod curve;
mod evaluation;
mod traits;
mod transform;

pub use builder::DiscountCurveBuilder;

use super::common::CurveCalendar;
use crate::math::interp::{ExtrapolationPolicy, InterpStyle, exact_f64_eq};
use crate::{
    dates::{Date, DayCount},
    math::interp::Interp,
    types::CurveId,
};

/// Piece-wise discount factor curve supporting explicit interpolation and tails.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawDiscountCurve", into = "RawDiscountCurve")]
pub struct DiscountCurve {
    pub(crate) id: CurveId,
    pub(crate) base: Date,
    /// Day-count basis used to convert dates to time for discounting.
    pub(crate) day_count: DayCount,
    /// Business-day calendar of the time axis; required by Bus/252.
    pub(crate) calendar: Option<CurveCalendar>,
    /// Knot times in **years**.
    pub(crate) knots: Box<[f64]>,
    /// Discount factors (unitless).
    pub(crate) dfs: Box<[f64]>,
    pub(crate) interp: Interp,
    /// Exact accumulated roll and continuous zero-rate shock evaluation.
    transform: Option<evaluation::CurveTransform>,
    /// Exact typed recipe used to replay calibration after quote shocks.
    pub(crate) rate_calibration: Option<super::RateCalibrationRecipe>,
    /// Opaque FX policy stamp when bootstrap used cross-currency assumptions.
    pub(crate) fx_policy: Option<String>,
}

/// Raw serializable state of DiscountCurve.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawDiscountCurve {
    /// Curve identifier
    pub id: String,
    /// Base date
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub base: Date,
    /// Day count convention for discount time basis
    pub day_count: DayCount,
    /// Registered holiday calendar id of the business-day time axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<String>,
    /// Time/value pairs stored on the curve (must already contain origin `(0.0, 1.0)`).
    pub knot_points: Vec<(f64, f64)>,
    /// Interpolation style
    pub interp_style: InterpStyle,
    /// Extrapolation policy
    pub extrapolation: ExtrapolationPolicy,
    /// Exact typed calibration replay recipe.
    pub rate_calibration: Option<super::RateCalibrationRecipe>,
    /// Opaque FX policy stamp; see [`DiscountCurve::fx_policy`].
    pub fx_policy: Option<String>,
    /// Canonical source interpolation and accumulated continuous transformations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<evaluation::CurveTransform>,
}

impl From<DiscountCurve> for RawDiscountCurve {
    fn from(curve: DiscountCurve) -> Self {
        let knot_points: Vec<(f64, f64)> = curve
            .knots
            .iter()
            .zip(curve.dfs.iter())
            .map(|(&t, &df)| (t, df))
            .collect();

        RawDiscountCurve {
            id: curve.id.to_string(),
            base: curve.base,
            day_count: curve.day_count,
            calendar_id: curve.calendar.map(|calendar| calendar.id),
            knot_points,
            interp_style: curve.interp.style(),
            extrapolation: curve.interp.extrapolation(),
            rate_calibration: curve.rate_calibration,
            fx_policy: curve.fx_policy,
            transform: curve.transform,
        }
    }
}

impl TryFrom<RawDiscountCurve> for DiscountCurve {
    type Error = crate::Error;

    fn try_from(state: RawDiscountCurve) -> crate::Result<Self> {
        let first = state
            .knot_points
            .first()
            .ok_or(crate::InputError::TooFewPoints)?;
        if !exact_f64_eq(first.0, 0.0) || !exact_f64_eq(first.1, 1.0) {
            return Err(crate::Error::Validation(
                "canonical DiscountCurve state must already start at (0.0, 1.0)".to_string(),
            ));
        }
        if let Some(ref tr) = state.transform {
            let src_first = tr
                .source_points()
                .first()
                .ok_or(crate::InputError::TooFewPoints)?;
            if !exact_f64_eq(src_first.0, 0.0)
                || !exact_f64_eq(src_first.1, 1.0)
                || tr.source_points().len() < 2
            {
                return Err(crate::Error::Validation(
                    "canonical DiscountCurve transform source_points must contain origin (0.0, 1.0) and at least one positive pillar".to_string(),
                ));
            }
        } else if state.knot_points.len() < 2 {
            return Err(crate::InputError::TooFewPoints.into());
        }

        let mut builder = DiscountCurve::builder(state.id)
            .base_date(state.base)
            .day_count(state.day_count)
            .calendar_id_opt(state.calendar_id)
            .knots(state.knot_points)
            .interp(state.interp_style)
            .extrapolation(state.extrapolation)
            .rate_calibration_opt(state.rate_calibration)
            .fx_policy_opt(state.fx_policy);
        builder.transform = state.transform;
        builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::calendar_by_id_strict;

    #[test]
    fn flat_curve_uses_continuous_rate_at_all_maturities() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");
        let curve =
            DiscountCurve::flat("USD-OIS", base, 0.04, DayCount::Act365F).expect("flat curve");
        for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
            let expected = crate::math::Compounding::Continuous
                .df_from_rate(0.04, t)
                .expect("valid df");
            assert!((curve.df(t).expect("df") - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn flat_curve_supports_zero_and_negative_continuous_rates() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");

        for rate in [0.0, -0.01] {
            let curve = DiscountCurve::flat("EUR-OIS", base, rate, DayCount::Act365F)
                .expect("flat discount curve");
            for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
                let expected = crate::math::Compounding::Continuous
                    .df_from_rate(rate, t)
                    .expect("valid df");
                assert!((curve.df(t).expect("df") - expected).abs() < 1e-12);
            }
            assert!((curve.forward(2.0, 9.0).expect("flat forward") - rate).abs() < 1e-12);
        }
    }

    #[test]
    fn flat_curve_matches_manually_built_continuous_curve() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");
        let rate = 0.04;
        let one_year_df = crate::math::Compounding::Continuous
            .df_from_rate(rate, 1.0)
            .expect("valid df");
        let flat = DiscountCurve::flat("USD-OIS", base, rate, DayCount::Act365F)
            .expect("flat discount curve");
        let manual = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, one_year_df)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .expect("manual continuous curve");

        for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
            assert!((flat.df(t).expect("df") - manual.df(t).expect("df")).abs() < 1e-12);
        }
    }

    #[test]
    fn optional_rate_calibration_round_trips_as_none() {
        let canonical = serde_json::json!({
            "id": "USD-OIS",
            "base": "2025-01-02",
            "day_count": "act_365f",
            "knot_points": [[0.0, 1.0], [5.0, 0.8]],
            "interp_style": "linear",
            "extrapolation": "extend",
            "rate_calibration": null,
            "fx_policy": null
        });

        let curve: DiscountCurve =
            serde_json::from_value(canonical).expect("canonical serialized curve");
        let serialized = serde_json::to_value(curve).expect("serialize curve");
        let restored: DiscountCurve =
            serde_json::from_value(serialized.clone()).expect("round-trip curve");

        assert!(serialized["rate_calibration"].is_null());
        assert!(restored.rate_calibration().is_none());
        assert!(serialized.get("calendar_id").is_none());
    }

    #[test]
    fn bus252_curve_calendar_round_trips_and_survives_roll_and_bump() {
        use crate::dates::DayCountContext;
        use crate::market_data::bumps::{BumpSpec, Bumpable};
        use crate::market_data::traits::Discounting;

        let base = Date::from_calendar_date(2025, time::Month::January, 6).expect("base");
        let pay = Date::from_calendar_date(2025, time::Month::February, 3).expect("pay");
        let curve = DiscountCurve::builder("BRL-CDI")
            .base_date(base)
            .day_count(DayCount::Bus252)
            .calendar_id("brbd")
            .knots([(0.0, 1.0), (1.0, 0.9)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .expect("Bus/252 curve with calendar");

        let brbd = calendar_by_id_strict("brbd").expect("brbd");
        let business_days = brbd
            .count_business_days(base, pay)
            .expect("calendar lookup");
        let t = DayCount::Bus252
            .year_fraction(
                base,
                pay,
                DayCountContext {
                    calendar: Some(brbd),
                    ..DayCountContext::default()
                },
            )
            .expect("bus/252 time");
        assert_eq!(t, f64::from(business_days) / 252.0);
        let df = curve
            .df_between_dates(base, pay)
            .expect("curve-owned calendar");
        assert!((df - curve.df(t).expect("df")).abs() < 1e-15);

        let json = serde_json::to_value(&curve).expect("serialize");
        assert_eq!(json["calendar_id"], "brbd");
        let restored: DiscountCurve = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored.calendar_id(), Some("brbd"));
        assert_eq!(restored.df_between_dates(base, pay).expect("restored"), df);

        let rolled = curve
            .condition(
                Date::from_calendar_date(2025, time::Month::January, 13).expect("reference date"),
            )
            .expect("condition");
        assert_eq!(rolled.calendar_id(), Some("brbd"));
        let bumped = curve
            .apply_bump(BumpSpec::parallel_bp(1.0))
            .expect("bump keeps calendar");
        assert_eq!(bumped.calendar_id(), Some("brbd"));
        assert!(Discounting::day_count_context(&bumped).calendar.is_some());
    }
}
