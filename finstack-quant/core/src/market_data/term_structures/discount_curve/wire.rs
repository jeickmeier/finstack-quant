//! Serializable wire representation for [`DiscountCurve`].

use super::super::common::define_factor_transform_wire;
use super::super::common::factor::validate_canonical_raw_origin;
use super::DiscountCurve;
use crate::dates::{Date, DayCount};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};

define_factor_transform_wire!(DiscountCurveTransformWire, "DiscountCurveTransform");

/// Raw serializable state of DiscountCurve.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(super) struct RawDiscountCurve {
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
    pub rate_calibration: Option<super::super::RateCalibrationRecipe>,
    /// Opaque FX policy stamp; see [`DiscountCurve::fx_policy`].
    pub fx_policy: Option<String>,
    /// Canonical source interpolation and accumulated continuous transformations.
    #[serde(skip_serializing_if = "Option::is_none")]
    transform: Option<DiscountCurveTransformWire>,
}

impl From<DiscountCurve> for RawDiscountCurve {
    fn from(curve: DiscountCurve) -> Self {
        RawDiscountCurve {
            id: curve.id.to_string(),
            base: curve.core.base,
            day_count: curve.core.day_count,
            calendar_id: curve
                .core
                .calendar
                .as_ref()
                .map(|calendar| calendar.id.clone()),
            knot_points: curve.core.knot_points(),
            interp_style: curve.core.interp.style(),
            extrapolation: curve.core.interp.extrapolation(),
            rate_calibration: curve.rate_calibration,
            fx_policy: curve.fx_policy,
            transform: curve.core.transform.map(DiscountCurveTransformWire::from),
        }
    }
}

impl TryFrom<RawDiscountCurve> for DiscountCurve {
    type Error = crate::Error;

    fn try_from(state: RawDiscountCurve) -> crate::Result<Self> {
        validate_canonical_raw_origin(
            "DiscountCurve",
            &state.knot_points,
            state
                .transform
                .as_ref()
                .map(|tr| tr.source_points.as_slice()),
        )?;

        let mut builder = DiscountCurve::builder(state.id)
            .base_date(state.base)
            .day_count(state.day_count)
            .calendar_id_opt(state.calendar_id)
            .knots(state.knot_points)
            .interp(state.interp_style)
            .extrapolation(state.extrapolation)
            .rate_calibration_opt(state.rate_calibration)
            .fx_policy_opt(state.fx_policy);
        builder.transform = state.transform.map(Into::into);
        builder.build()
    }
}
