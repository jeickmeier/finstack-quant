//! Discount factor curves for present value calculations.
//!
//! A discount curve represents the time value of money, mapping future dates to
//! present values. This is the fundamental building block for pricing all fixed
//! income securities and derivatives.
//!
//! # Financial Concept
//!
//! The discount factor DF(t) is the present value of $1 received at time t:
//! ```text
//! DF(t) = PV($1 at time t)
//!       = e^(-r(t) * t)
//!
//! where r(t) is the continuously compounded zero rate at maturity t
//! ```
//!
//! # Market Construction
//!
//! Discount curves are typically bootstrapped from liquid market instruments:
//! - **Money market**: Overnight rates (SOFR, €STR, SONIA)
//! - **Futures**: SOFR futures, Eurodollar futures
//! - **Swaps**: Fixed-float interest rate swaps (par rates)
//! - **Bonds**: Government bonds (when OIS not available)
//!
//! # Interpolation Methods
//!
//! The curve supports multiple interpolation schemes via [`crate::math::interp::InterpStyle`]:
//! - **Linear**: Simple, but may create arbitrage
//! - **LogLinear**: Piecewise-constant instantaneous forwards (linear in log DF)
//! - **MonotoneConvex**: Smooth, no-arbitrage (Hagan-West algorithm)
//! - **CubicHermite**: Shape-preserving cubic (requires monotone input for no-arb)
//! - **PiecewiseQuadraticForward**: Smooth forward curve (C²), commonly used for display
//!
//! # Use Cases
//!
//! - **Bond pricing**: Discount future coupons and principal
//! - **Swap valuation**: Mark-to-market fixed and floating legs
//! - **Option pricing**: Discount expected payoffs
//! - **Risk metrics**: DV01, duration, convexity calculation
//!
//! # Extrapolation Behavior and Limitations
//!
//! The curve supports two extrapolation policies via [`ExtrapolationPolicy`]:
//!
//! - **`FlatZero`** (conservative): Returns the discount factor at the boundary knot.
//!   Beyond the last knot, this implies zero forward rates. Use for risk management
//!   where you want to avoid assumptions about unobserved rates.
//!
//! - **`FlatForward`** (default): Extends the curve using the forward rate at the
//!   boundary. This is the market standard for production curves.
//!
//! ## Warning: Ultra-Long Tenor Extrapolation
//!
//! When extrapolating significantly beyond the last curve knot (e.g., pricing a 50Y
//! instrument from a 10Y curve), be aware of the following limitations:
//!
//! 1. **Model uncertainty**: Extrapolated forward rates are not market-implied.
//!    For tenors 2× beyond the last knot, consider the extrapolation unreliable.
//!
//! 2. **Risk sensitivity**: Greeks computed in extrapolated regions depend on
//!    the extrapolation assumption. Continuous zero-rate shocks still act at
//!    their specified maturities beyond the final source pillar.
//!
//! 3. **Regulatory considerations**: Basel III/IV and Solvency II have specific
//!    requirements for ultra-long rate extrapolation (Smith-Wilson, UFR methods).
//!    This implementation does not include regulatory extrapolation methods.
//!
//! **Best practice**: If you frequently price instruments beyond your curve's last
//! pillar, either:
//! - Extend the curve with appropriate long-dated instruments (e.g., 30Y, 50Y swaps)
//! - Use a regulatory-compliant extrapolation method for insurance/pension valuations
//! - Apply explicit haircuts or uncertainty bands to extrapolated values
//!
//! ## Example
//! ```rust
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//! # use finstack_quant_core::math::interp::InterpStyle;
//!
//! let curve = DiscountCurve::builder("USD-OIS")
//!     .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
//!     .knots([(0.0, 1.0), (5.0, 0.9)])
//!     .interp(InterpStyle::MonotoneConvex)
//!     .build()
//!     .expect("DiscountCurve builder should succeed");
//! assert!(curve.df(3.0) < 1.0);
//! ```
//!
//! # References
//!
//! - **Curve Construction and Bootstrapping**:
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Pearson. Chapters 4-7. `docs/REFERENCES.md#hull-options-futures`
//! - Andersen, L., & Piterbarg, V. (2010). *Interest Rate Modeling* (3 vols).
//!   Atlantic Financial Press. Volume 1, Chapters 2-3. `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//!
//! - **Interpolation Methods**:
//!   - Hagan, P. S., & West, G. (2006). "Interpolation Methods for Curve Construction."
//!   *Applied Mathematical Finance*, 13(2), 89-129.
//!   - Hagan, P. S., & West, G. (2008). "Methods for Constructing a Yield Curve."
//!   *Wilmott Magazine*, May 2008.
//!
//! - **Industry Standards**:
//!   - OpenGamma (2013). "Interest Rate Instruments and Market Conventions Guide."
//! - ISDA (2006). "2006 ISDA Definitions." Sections on discount factors and rates. `docs/REFERENCES.md#isda-2006-definitions`

mod builder;
mod curve;
mod evaluation;
mod traits;
mod transform;
mod validation;

pub use builder::DiscountCurveBuilder;
pub use validation::ValidationMode;

use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
use crate::{
    dates::{Date, DayCount},
    math::interp::types::Interp,
    types::CurveId,
};

/// Default minimum forward rate tenor in years (~30 seconds).
///
/// Very short tenors cause precision degradation in the formula (z2 - z1) / (t2 - t1)
/// due to catastrophic cancellation when z1*t1 ≈ z2*t2.
///
/// This constant can be overridden via [`DiscountCurveBuilder::min_forward_tenor`].
pub const DEFAULT_MIN_FORWARD_TENOR: f64 = 1e-6;

/// Piece-wise discount factor curve supporting several interpolation styles.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawDiscountCurve", into = "RawDiscountCurve")]
pub struct DiscountCurve {
    pub(crate) id: CurveId,
    pub(crate) base: Date,
    /// Day-count basis used to convert dates → time for discounting.
    pub(crate) day_count: DayCount,
    /// Knot times in **years**.
    pub(crate) knots: Box<[f64]>,
    /// Discount factors (unitless).
    pub(crate) dfs: Box<[f64]>,
    pub(crate) interp: Interp,
    /// Exact accumulated roll and continuous zero-rate shock evaluation.
    transform: Option<evaluation::CurveTransform>,
    /// Interpolation style (stored for serialization and bumping)
    pub(crate) style: InterpStyle,
    /// Extrapolation policy (stored for serialization and bumping)
    pub(crate) extrapolation: ExtrapolationPolicy,
    /// Minimum forward rate floor used during validation, if any.
    pub(crate) min_forward_rate: Option<f64>,
    /// Whether non-monotonic discount factors were explicitly allowed.
    pub(crate) allow_non_monotonic: bool,
    /// Minimum tenor for forward rate calculations (configurable)
    pub(crate) min_forward_tenor: f64,
    /// Exact typed recipe used to replay calibration after quote shocks.
    pub(crate) rate_calibration: Option<super::RateCalibrationRecipe>,
    /// Rate cut-off (business days) of the OIS compounding convention this
    /// curve was *calibrated* under, when bootstrapped with a
    /// `CompoundedWithRateCutoff` override.
    ///
    /// `None` = calibrated under a non-cut-off convention (registry default),
    /// or not calibrated from instruments at all (hand-built curve).
    ///
    /// Stored as a plain scalar (no dependency on the valuations-crate
    /// `FloatingLegCompounding` enum). Calibration **provenance only**: it is
    /// stamped on both the intermediate solver curves and the final curve as
    /// an audit trail of the bootstrap convention, and round-trips through
    /// serialization, but no pricing or re-bump path consults it — pricing
    /// takes the cut-off from the instrument's own `FloatingLegCompounding`.
    pub(crate) calibration_ois_cutoff_days: Option<i32>,
    /// Opaque FX policy stamp when bootstrap used cross-currency assumptions
    /// (XCCY basis, FX triangulation). Propagated to `ResultsMeta.fx_policy_applied`
    /// for instruments that depend on this curve.
    pub(crate) fx_policy: Option<String>,
}

/// Raw serializable state of DiscountCurve
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawDiscountCurve {
    /// Curve identifier
    pub id: String,
    /// Base date
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub base: Date,
    /// Day count convention for discount time basis
    pub day_count: DayCount,
    /// Time/value pairs used to construct the curve
    pub knot_points: Vec<(f64, f64)>,
    /// Interpolation style
    pub interp_style: InterpStyle,
    /// Extrapolation policy
    pub extrapolation: ExtrapolationPolicy,
    /// Minimum forward rate floor (if set)
    pub min_forward_rate: Option<f64>,
    /// Whether non-monotonic DFs are allowed (dangerous override)
    pub allow_non_monotonic: bool,
    /// Minimum tenor for forward rate calculations
    pub min_forward_tenor: f64,
    /// Exact typed calibration replay recipe.
    pub rate_calibration: Option<super::RateCalibrationRecipe>,
    /// OIS cut-off (business days) the curve was calibrated under, if any.
    pub calibration_ois_cutoff_days: Option<i32>,
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
            knot_points,
            interp_style: curve.style,
            extrapolation: curve.extrapolation,
            min_forward_rate: curve.min_forward_rate,
            allow_non_monotonic: curve.allow_non_monotonic,
            min_forward_tenor: curve.min_forward_tenor,
            rate_calibration: curve.rate_calibration,
            calibration_ois_cutoff_days: curve.calibration_ois_cutoff_days,
            fx_policy: curve.fx_policy,
            transform: curve.transform,
        }
    }
}

impl TryFrom<RawDiscountCurve> for DiscountCurve {
    type Error = crate::Error;

    fn try_from(state: RawDiscountCurve) -> crate::Result<Self> {
        let mut curve = DiscountCurve::builder(state.id)
            .base_date(state.base)
            .day_count(state.day_count)
            .knots(state.knot_points)
            .interp(state.interp_style)
            .extrapolation(state.extrapolation)
            .min_forward_tenor(state.min_forward_tenor)
            .rate_calibration_opt(state.rate_calibration)
            .calibration_ois_cutoff_days_opt(state.calibration_ois_cutoff_days)
            .fx_policy_opt(state.fx_policy)
            .validation(ValidationMode::Raw {
                allow_non_monotonic: state.allow_non_monotonic,
                forward_floor: state.min_forward_rate,
            })
            .build()?;
        if let Some(transform) = state.transform {
            curve.restore_transform(transform)?;
        }
        Ok(curve)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_curve_uses_continuous_rate_at_all_maturities() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");

        let curve = DiscountCurve::flat("USD-OIS", base, 0.04).expect("flat discount curve");

        assert_eq!(curve.len(), 2);
        assert_eq!(curve.interp_style(), InterpStyle::LogLinear);
        assert_eq!(curve.extrapolation(), ExtrapolationPolicy::FlatForward);
        for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
            assert!((curve.df(t) - (-0.04 * t).exp()).abs() < 1e-12);
        }
        assert!((curve.forward(2.0, 9.0).expect("flat forward") - 0.04).abs() < 1e-12);
    }

    #[test]
    fn flat_curve_supports_zero_and_negative_continuous_rates() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");

        for rate in [0.0, -0.01] {
            let curve = DiscountCurve::flat("EUR-OIS", base, rate).expect("flat discount curve");
            for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
                let expected = crate::math::Compounding::Continuous.df_from_rate(rate, t);
                assert!((curve.df(t) - expected).abs() < 1e-12);
            }
            assert!((curve.forward(2.0, 9.0).expect("flat forward") - rate).abs() < 1e-12);
        }
    }

    #[test]
    fn flat_curve_matches_manually_built_continuous_curve() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");
        let rate = 0.04;
        let one_year_df = crate::math::Compounding::Continuous.df_from_rate(rate, 1.0);
        let flat = DiscountCurve::flat("USD-OIS", base, rate).expect("flat discount curve");
        let manual = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.0, 1.0), (1.0, one_year_df)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .validation(ValidationMode::Raw {
                allow_non_monotonic: false,
                forward_floor: None,
            })
            .build()
            .expect("manual continuous curve");

        for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
            assert!((flat.df(t) - manual.df(t)).abs() < 1e-12);
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
            "extrapolation": "flat_forward",
            "min_forward_rate": null,
            "allow_non_monotonic": false,
            "min_forward_tenor": 1e-8,
            "rate_calibration": null,
            "calibration_ois_cutoff_days": null,
            "fx_policy": null
        });

        let curve: DiscountCurve =
            serde_json::from_value(canonical).expect("canonical serialized curve");
        let serialized = serde_json::to_value(curve).expect("serialize curve");
        let restored: DiscountCurve =
            serde_json::from_value(serialized.clone()).expect("round-trip curve");

        assert!(
            serialized["rate_calibration"].is_null(),
            "an absent rate calibration remains null"
        );
        assert!(restored.rate_calibration().is_none());
    }

    #[test]
    fn rebuild_with_knots_retains_permissive_validation_policy() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");
        let source = DiscountCurve::builder("USD-NEGATIVE")
            .base_date(base)
            .knots([(0.0, 1.0), (1.0, 1.01), (2.0, 0.99)])
            .min_forward_tenor(0.000_123)
            .validation(ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: Some(-0.02),
            })
            .build()
            .expect("permissive source curve");

        let rebuilt = source
            .rebuild_with_knots([(0.0, 1.0), (1.0, 1.011), (2.0, 0.991)])
            .expect("metadata-preserving rebuild");
        let serialized = serde_json::to_value(rebuilt).expect("serialize rebuilt curve");

        assert_eq!(serialized["allow_non_monotonic"], true);
        assert_eq!(serialized["min_forward_rate"], -0.02);
        assert_eq!(serialized["min_forward_tenor"], 0.000_123);
    }
}
