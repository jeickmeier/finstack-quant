//! Validated serialization adapters for [`ForwardCurve`].

use super::*;
use crate::math::interp::exact_f64_eq;

/// Raw canonical serializable state of [`ForwardCurve`].
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(super) struct RawForwardCurve {
    /// Curve identifier.
    pub id: String,
    /// Base date.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub base: Date,
    /// Day-count convention for projection factor time basis.
    pub day_count: DayCount,
    /// Registered holiday calendar id of the business-day time axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<String>,
    /// Optional index accrual tenor metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_tenor: Option<Tenor>,
    /// Time/factor pairs stored on the curve (must already contain origin `(0.0, 1.0)`).
    pub knot_points: Vec<(f64, f64)>,
    /// Interpolation style.
    pub interp_style: InterpStyle,
    /// Extrapolation policy.
    pub extrapolation: ExtrapolationPolicy,
    /// Exact typed calibration replay recipe.
    pub rate_calibration: Option<crate::market_data::term_structures::RateCalibrationRecipe>,
    /// Opaque FX policy stamp; see [`ForwardCurve::fx_policy`].
    pub fx_policy: Option<String>,
    /// Canonical source interpolation and accumulated continuous transformations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<super::evaluation::CurveTransform>,
}

impl From<ForwardCurve> for RawForwardCurve {
    fn from(curve: ForwardCurve) -> Self {
        let knot_points: Vec<(f64, f64)> = curve
            .knots
            .iter()
            .zip(curve.factors.iter())
            .map(|(&t, &factor)| (t, factor))
            .collect();

        RawForwardCurve {
            id: curve.id.to_string(),
            base: curve.base,
            day_count: curve.day_count,
            calendar_id: curve.calendar.map(|calendar| calendar.id),
            index_tenor: curve.index_tenor,
            knot_points,
            interp_style: curve.interp.style(),
            extrapolation: curve.interp.extrapolation(),
            rate_calibration: curve.rate_calibration,
            fx_policy: curve.fx_policy,
            transform: curve.transform,
        }
    }
}

impl TryFrom<RawForwardCurve> for ForwardCurve {
    type Error = crate::Error;

    fn try_from(state: RawForwardCurve) -> crate::Result<Self> {
        let first = state
            .knot_points
            .first()
            .ok_or(crate::InputError::TooFewPoints)?;
        if !exact_f64_eq(first.0, 0.0) || !exact_f64_eq(first.1, 1.0) {
            return Err(crate::Error::Validation(
                "canonical ForwardCurve state must already start at (0.0, 1.0)".to_string(),
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
                    "canonical ForwardCurve transform source_points must contain origin (0.0, 1.0) and at least one positive pillar".to_string(),
                ));
            }
        } else if state.knot_points.len() < 2 {
            return Err(crate::InputError::TooFewPoints.into());
        }

        let mut builder = ForwardCurve::builder(state.id)
            .base_date(state.base)
            .day_count(state.day_count)
            .calendar_id_opt(state.calendar_id)
            .index_tenor_opt(state.index_tenor)
            .knots(state.knot_points)
            .interp(state.interp_style)
            .extrapolation(state.extrapolation)
            .rate_calibration_opt(state.rate_calibration)
            .fx_policy_opt(state.fx_policy);
        builder.transform = state.transform;
        builder.build()
    }
}
