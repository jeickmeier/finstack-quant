//! Credit hazard rate curves for default probability modeling.
//!
//! A hazard curve represents the piecewise-constant default intensity `λ(t)`
//! on ending intervals `(tᵢ₋₁, tᵢ]` with `t₀ = 0` and constant tail intensity
//! `λₙ` beyond `tₙ`.
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::dates::{Date, DayCount};
//! use finstack_quant_core::market_data::term_structures::HazardCurve;
//! use time::Month;
//!
//! let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let hc = HazardCurve::builder("USD-CREDIT")
//!     .base_date(base)
//!     .day_count(DayCount::Act365F)
//!     .recovery_rate(0.40)
//!     .knots([(1.0, 0.01), (10.0, 0.015)])
//!     .build()
//!     .expect("HazardCurve builder should succeed");
//! assert!(hc.sp(5.0).expect("valid survival") < 1.0);
//! ```
//!
//! # References
//!
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*.
//!   Wiley Finance. Chapters 3-5. `docs/REFERENCES.md#o-kane-2008`
//! - ISDA (2009). "ISDA CDS Standard Model." Version 1.8.2.
//!   `docs/REFERENCES.md#isda-cds-standard-model`

use super::common::CurveCalendar;
use crate::{
    currency::Currency,
    dates::{Date, DayCount},
    error::InputError,
    math::interp::exact_f64_eq,
    types::CurveId,
};

/// Source coordinate anchor and original piecewise-constant hazard schedule
/// retained across [`HazardCurve::condition`] transformations so dated survival
/// integrates directly on the retained source timeline.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(super) struct HazardConditionSource {
    /// Original untransformed curve base date.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub(super) base_date: Date,
    /// Original `(time_years, hazard_rate)` pillars from `base_date`.
    pub(super) knot_points: Vec<(f64, f64)>,
    /// Offset in curve day-count years from `base_date` to the conditioned `base`.
    #[serde(skip)]
    pub(super) offset: f64,
}

/// Piecewise-constant credit hazard curve for default probability modeling.
///
/// Stores non-negative intensities `λᵢ ≥ 0` on strictly positive ending knots
/// `0 < t₁ < ... < tₙ` with left-continuous intervals `(tᵢ₋₁, tᵢ]` (`λ(0) = λ₁`)
/// and constant tail `λₙ` beyond `tₙ`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawHazardCurve", into = "RawHazardCurve")]
pub struct HazardCurve {
    id: CurveId,
    base: Date,
    /// Strictly positive strictly increasing ending knot times in source-clock years.
    knots: Box<[f64]>,
    /// Piecewise-constant non-negative hazard intensities `λ ≥ 0`; same length as `knots`.
    lambdas: Box<[f64]>,
    /// Recovery rate used during calibration/reporting metadata (`[0, 1]`).
    recovery_rate: f64,
    /// Optional issuer metadata.
    issuer: Option<String>,
    /// Debt seniority metadata.
    seniority: Option<Seniority>,
    /// Currency of protection leg (metadata).
    currency: Option<Currency>,
    /// Day count convention for converting dates to source-clock years.
    day_count: DayCount,
    /// Business-day calendar of the time axis; required by `Bus252`.
    calendar: Option<CurveCalendar>,
    /// Exact typed recipe used to replay calibration after quote shocks.
    hazard_calibration: Option<crate::market_data::term_structures::HazardCalibrationRecipe>,
    /// Opaque FX policy stamp; see [`crate::market_data::term_structures::DiscountCurve::fx_policy`].
    fx_policy: Option<String>,
    /// Source coordinate anchor when conditioned in time.
    source: Option<HazardConditionSource>,
}

/// Seniority level for credit exposures.
///
/// Order is **not** total — `SeniorSecured` is strictly senior to `Senior`,
/// `Subordinated`, and `Junior`, but the relative ordering of `Subordinated`
/// vs. `Junior` is jurisdiction-dependent. Do not rely on `Ord` semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Seniority {
    /// Senior secured debt.
    SeniorSecured,
    /// Senior unsecured debt.
    Senior,
    /// Subordinated debt.
    Subordinated,
    /// Junior/mezzanine debt.
    Junior,
}

impl core::fmt::Display for Seniority {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Seniority::SeniorSecured => write!(f, "senior_secured"),
            Seniority::Senior => write!(f, "senior"),
            Seniority::Subordinated => write!(f, "subordinated"),
            Seniority::Junior => write!(f, "junior"),
        }
    }
}

impl core::str::FromStr for Seniority {
    type Err = crate::Error;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        match s {
            "senior_secured" => Ok(Self::SeniorSecured),
            "senior" => Ok(Self::Senior),
            "subordinated" => Ok(Self::Subordinated),
            "junior" => Ok(Self::Junior),
            other => Err(crate::Error::Validation(format!(
                "unknown seniority {other:?}; expected one of senior_secured, senior, \
                 subordinated, junior"
            ))),
        }
    }
}

mod builder;
mod curve;
#[cfg(test)]
mod tests;
mod transform;
mod wire;
pub use builder::HazardCurveBuilder;
use wire::RawHazardCurve;
