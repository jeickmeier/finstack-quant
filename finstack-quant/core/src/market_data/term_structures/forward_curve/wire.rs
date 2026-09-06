//! Validated serialization adapters.

use super::*;

/// Raw serializable state of ForwardCurve
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(super) struct RawForwardCurve {
    /// Curve identifier
    pub id: String,
    /// Base date
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub base: Date,
    /// Reset lag in business days
    pub reset_lag: i32,
    /// Day count convention
    pub day_count: DayCount,
    /// Index tenor in years
    pub tenor: f64,
    /// Time/value pairs used to construct the curve
    pub knot_points: Vec<(f64, f64)>,
    /// Optional contractual reset/end-date boundaries.
    ///
    /// `None` selects fixed numeric-tenor discount-factor stepping.
    pub projection_grid: Option<Vec<f64>>,
    /// Interpolation style
    pub interp_style: InterpStyle,
    /// Extrapolation policy
    pub extrapolation: ExtrapolationPolicy,
    /// Exact typed calibration replay recipe.
    pub rate_calibration: Option<crate::market_data::term_structures::RateCalibrationRecipe>,
    /// Opaque FX policy stamp; see [`crate::market_data::term_structures::DiscountCurve::fx_policy`].
    pub fx_policy: Option<String>,
}

impl From<ForwardCurve> for RawForwardCurve {
    fn from(curve: ForwardCurve) -> Self {
        let knot_points: Vec<(f64, f64)> = curve
            .knots
            .iter()
            .zip(curve.forwards.iter())
            .map(|(&t, &fwd)| (t, fwd))
            .collect();

        RawForwardCurve {
            id: curve.id.to_string(),
            base: curve.base,
            reset_lag: curve.reset_lag,
            day_count: curve.day_count,
            tenor: curve.tenor,
            knot_points,
            projection_grid: curve.projection_grid.map(Vec::from),
            interp_style: curve.interp.style(),
            extrapolation: curve.interp.extrapolation(),
            rate_calibration: curve.rate_calibration,
            fx_policy: curve.fx_policy,
        }
    }
}

impl TryFrom<RawForwardCurve> for ForwardCurve {
    type Error = crate::Error;

    fn try_from(state: RawForwardCurve) -> crate::Result<Self> {
        ForwardCurve::builder(state.id, state.tenor)
            .base_date(state.base)
            .reset_lag(state.reset_lag)
            .day_count(state.day_count)
            .knots(state.knot_points)
            .projection_grid_opt(state.projection_grid)
            .interp(state.interp_style)
            .extrapolation(state.extrapolation)
            .rate_calibration_opt(state.rate_calibration)
            .fx_policy_opt(state.fx_policy)
            .build()
    }
}
