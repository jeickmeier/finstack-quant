//! Validated serialization adapters for [`HazardCurve`].

use super::*;
use crate::currency::Currency;
use crate::dates::{Date, DayCount};

/// Raw serializable state of a [`HazardCurve`].
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(super) struct RawHazardCurve {
    /// Curve identifier.
    pub id: String,
    /// Base date.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub base: Date,
    /// Day count convention.
    pub day_count: DayCount,
    /// Registered holiday calendar id of the business-day time axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<String>,
    /// Recovery rate in `[0, 1]`.
    pub recovery_rate: f64,
    /// Strictly positive ending `(time_years, hazard_rate)` pillars.
    pub knot_points: Vec<(f64, f64)>,
    /// Optional issuer.
    pub issuer: Option<String>,
    /// Seniority.
    pub seniority: Option<Seniority>,
    /// Currency.
    pub currency: Option<Currency>,
    /// Exact calibration replay inputs.
    #[serde(default)]
    pub hazard_calibration: Option<crate::market_data::term_structures::HazardCalibrationRecipe>,
    /// Opaque FX policy stamp; see [`crate::market_data::term_structures::DiscountCurve::fx_policy`].
    #[serde(default)]
    pub fx_policy: Option<String>,
    /// Source coordinate anchor for conditioned curves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<HazardConditionSource>,
}

impl From<HazardCurve> for RawHazardCurve {
    fn from(curve: HazardCurve) -> Self {
        let knot_points: Vec<(f64, f64)> = curve
            .knots
            .iter()
            .zip(curve.lambdas.iter())
            .map(|(&t, &lambda)| (t, lambda))
            .collect();

        RawHazardCurve {
            id: curve.id.to_string(),
            base: curve.base,
            day_count: curve.day_count,
            calendar_id: curve.calendar.map(|cal| cal.id),
            recovery_rate: curve.recovery_rate,
            knot_points,
            issuer: curve.issuer,
            seniority: curve.seniority,
            currency: curve.currency,
            hazard_calibration: curve.hazard_calibration,
            fx_policy: curve.fx_policy,
            source: curve.source,
        }
    }
}

impl TryFrom<RawHazardCurve> for HazardCurve {
    type Error = crate::Error;

    fn try_from(state: RawHazardCurve) -> crate::Result<Self> {
        let mut builder = HazardCurve::builder(state.id)
            .base_date(state.base)
            .day_count(state.day_count)
            .calendar_id_opt(state.calendar_id)
            .recovery_rate(state.recovery_rate)
            .knots(state.knot_points)
            .hazard_calibration_opt(state.hazard_calibration)
            .issuer_opt(state.issuer)
            .seniority_opt(state.seniority)
            .currency_opt(state.currency)
            .fx_policy_opt(state.fx_policy);
        builder.source = state.source;
        builder.build()
    }
}
