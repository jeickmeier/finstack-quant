//! Validated serialization adapters.

use super::*;

/// Raw serializable state of a HazardCurve
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(super) struct RawHazardCurve {
    /// Curve identifier
    pub id: String,
    /// Base date
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub base: Date,
    /// Time/value pairs used to construct the curve
    pub knot_points: Vec<(f64, f64)>,
    /// Recovery rate
    pub recovery_rate: f64,
    /// Optional issuer
    pub issuer: Option<String>,
    /// Seniority
    pub seniority: Option<Seniority>,
    /// Currency
    pub currency: Option<Currency>,
    /// Day count convention
    pub day_count: DayCount,
    /// Par spread points for reporting
    pub par_points: Vec<(f64, f64)>,
    /// Par interpolation method
    #[serde(default = "default_par_interp")]
    pub par_interp: ParInterp,
    /// Survival-probability interpolation style between pillars
    #[serde(default = "default_survival_interp")]
    pub survival_interp: InterpStyle,
    /// Exact calibration replay inputs.
    #[serde(default)]
    pub hazard_calibration: Option<crate::market_data::term_structures::HazardCalibrationRecipe>,
    /// Opaque FX policy stamp; see [`crate::market_data::term_structures::DiscountCurve::fx_policy`].
    #[serde(default)]
    pub fx_policy: Option<String>,
}

fn default_par_interp() -> ParInterp {
    ParInterp::Linear
}

fn default_survival_interp() -> InterpStyle {
    InterpStyle::LogLinear
}

impl From<HazardCurve> for RawHazardCurve {
    fn from(curve: HazardCurve) -> Self {
        let knot_points: Vec<(f64, f64)> = curve
            .knots
            .iter()
            .zip(curve.lambdas.iter())
            .map(|(&t, &lambda)| (t, lambda))
            .collect();
        let par_points: Vec<(f64, f64)> = curve
            .par_tenors
            .iter()
            .zip(curve.par_spreads_bp.iter())
            .map(|(&t, &spread)| (t, spread))
            .collect();

        RawHazardCurve {
            id: curve.id.to_string(),
            base: curve.base,
            knot_points,
            recovery_rate: curve.recovery_rate,
            issuer: curve.issuer,
            seniority: curve.seniority,
            currency: curve.currency,
            day_count: curve.day_count,
            par_points,
            par_interp: curve.par_interp,
            survival_interp: curve.survival_interp_style,
            hazard_calibration: curve.hazard_calibration,
            fx_policy: curve.fx_policy,
        }
    }
}

impl TryFrom<RawHazardCurve> for HazardCurve {
    type Error = crate::Error;

    fn try_from(state: RawHazardCurve) -> crate::Result<Self> {
        HazardCurve::builder(state.id)
            .base_date(state.base)
            .recovery_rate(state.recovery_rate)
            .day_count(state.day_count)
            .knots(state.knot_points)
            .par_spreads(state.par_points)
            .par_interp(state.par_interp)
            .interp(state.survival_interp)
            .hazard_calibration_opt(state.hazard_calibration)
            .issuer_opt(state.issuer)
            .seniority_opt(state.seniority)
            .currency_opt(state.currency)
            .fx_policy_opt(state.fx_policy)
            .build()
    }
}
