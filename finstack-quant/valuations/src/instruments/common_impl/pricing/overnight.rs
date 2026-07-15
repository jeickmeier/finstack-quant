//! Shared projection engine for compounded overnight-RFR coupons.
//!
//! IRS, cap/floor, and risk pathways use this module so lookback,
//! observation-shift, cutoff, fixing, and sensitivity semantics stay identical.

use crate::instruments::rates::irs::FloatingLegCompounding;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    adjust, calendar_by_id, BusinessDayConvention, Date, DateExt, DayCount, DayCountContext,
    HolidayCalendar, Tenor,
};
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::Result;

/// Curve source used to project future overnight fixings.
#[derive(Clone, Copy)]
pub(crate) enum OvernightProjectionCurve<'a> {
    /// Explicit overnight forward curve.
    Forward(&'a ForwardCurve),
    /// Single-curve OIS fallback projected from discount factors.
    Discount(&'a DiscountCurve),
}

/// Inputs for one compounded overnight coupon projection.
pub(crate) struct OvernightCouponProjectionInput<'a> {
    /// Projection source for future observations.
    pub curve: OvernightProjectionCurve<'a>,
    /// Historical fixing series for observations before `as_of`.
    pub fixings: Option<&'a ScalarTimeSeries>,
    /// Fixing/index identifier used in missing-fixing errors.
    pub fixing_id: &'a str,
    /// Valuation date separating realized and projected observations.
    pub as_of: Date,
    /// Contractual accrual start.
    pub accrual_start: Date,
    /// Contractual accrual end.
    pub accrual_end: Date,
    /// Day-count basis for each overnight observation.
    pub day_count: DayCount,
    /// Coupon frequency required by context-sensitive day-count conventions.
    pub coupon_frequency: Option<Tenor>,
    /// Shared overnight compounding convention.
    pub compounding: &'a FloatingLegCompounding,
    /// Resolved fixing calendar.
    pub fixing_calendar: &'a dyn HolidayCalendar,
    /// Daily spread included inside each factor, in decimal rate units.
    pub compounded_spread: f64,
}

/// First-order stochastic exposure of one overnight observation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OvernightObservationExposure {
    /// Date on which the overnight rate is observed and fixed.
    pub observation_start: Date,
    /// End of the overnight rate interval.
    pub observation_end: Date,
    /// Projected or realized overnight rate for the interval.
    pub projected_rate: f64,
    /// Day-count fraction used to quote the overnight interval rate.
    pub rate_accrual_year_fraction: f64,
    /// Day-count fraction multiplying this rate in the compounded coupon factor.
    ///
    /// This differs from the quoted-rate accrual under lookback without observation shift.
    pub factor_accrual_year_fraction: f64,
    /// Product-rule derivative of the annualized coupon rate to this interval rate.
    ///
    /// Historical observations carry zero derivative.
    pub coupon_forward_derivative: f64,
}

/// Projected economics of one compounded overnight coupon.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OvernightCouponProjection {
    /// Equivalent simple annualized coupon rate.
    pub rate: f64,
    /// Coupon accrual fraction recomputed from the adjusted accrual boundaries.
    pub accrual_year_fraction: f64,
    /// Full compounded factor `∏(1 + (rᵢ+s)dᵢ)`.
    pub compound_factor: f64,
    /// Derivative of `rate` to a parallel bump of every projected overnight rate.
    pub parallel_forward_sensitivity: f64,
    /// Second derivative of `rate` to the same parallel overnight-rate bump.
    pub parallel_forward_second_sensitivity: f64,
    /// Last distinct overnight observation date determining the coupon.
    pub fixing_date: Date,
    /// Per-observation derivatives used by date-specific stochastic models.
    pub observation_exposures: Vec<OvernightObservationExposure>,
}

/// Resolve the explicit or currency-standard fixing calendar for an RFR coupon.
pub(crate) fn resolve_overnight_fixing_calendar(
    calendar_id: Option<&str>,
    currency: Currency,
    instrument_label: &str,
) -> Result<&'static dyn HolidayCalendar> {
    let default_id = match currency {
        Currency::USD => Some("usny"),
        Currency::EUR => Some("target2"),
        Currency::GBP => Some("gblo"),
        Currency::JPY => Some("jpto"),
        Currency::AUD => Some("auce"),
        Currency::CAD => Some("cato"),
        Currency::CHF => Some("chzh"),
        _ => None,
    };
    let id = calendar_id.or(default_id).ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "{instrument_label} requires an explicit overnight fixing calendar for {currency}"
        ))
    })?;
    calendar_by_id(id).ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "Overnight fixing calendar '{id}' is not registered for {instrument_label}"
        ))
    })
}

/// Adjust an overnight coupon's contractual accrual boundaries before daily compounding.
///
/// The schedule builder may preserve unadjusted roll dates, while overnight observations
/// must start and end on business days. IRS and cap/floor projection share this helper.
pub(crate) fn adjust_overnight_accrual_boundaries(
    accrual_start: Date,
    accrual_end: Date,
    bdc: BusinessDayConvention,
    calendar: &dyn HolidayCalendar,
) -> Result<(Date, Date)> {
    Ok((
        adjust(accrual_start, bdc, calendar)?,
        adjust(accrual_end, bdc, calendar)?,
    ))
}

fn shifted_observation_days(compounding: &FloatingLegCompounding) -> Result<(i32, bool)> {
    match compounding {
        FloatingLegCompounding::Simple => Err(finstack_quant_core::Error::Validation(
            "Overnight coupon projection requires a compounded convention, not Simple".into(),
        )),
        FloatingLegCompounding::CompoundedInArrears {
            lookback_days,
            observation_shift,
        } => {
            let observation_shift = observation_shift.unwrap_or(0);
            if *lookback_days != 0 && observation_shift != 0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Overnight coupon cannot combine a {lookback_days}-day lookback with a \
                     {observation_shift}-day observation shift"
                )));
            }
            if observation_shift != 0 {
                Ok((observation_shift, true))
            } else {
                Ok((*lookback_days, false))
            }
        }
        FloatingLegCompounding::CompoundedWithObservationShift { shift_days } => {
            Ok((*shift_days, true))
        }
        FloatingLegCompounding::CompoundedWithRateCutoff { .. } => Ok((0, false)),
    }
}

fn cutoff_days(compounding: &FloatingLegCompounding) -> Option<i32> {
    match compounding {
        FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days } if *cutoff_days > 0 => {
            Some(*cutoff_days)
        }
        _ => None,
    }
}

fn projected_rate(
    curve: OvernightProjectionCurve<'_>,
    obs_start: Date,
    obs_end: Date,
    day_count: DayCount,
    day_count_context: DayCountContext<'_>,
) -> Result<f64> {
    match curve {
        OvernightProjectionCurve::Forward(forward) => {
            let t0 = if obs_start <= forward.base_date() {
                0.0
            } else {
                forward.day_count().year_fraction(
                    forward.base_date(),
                    obs_start,
                    day_count_context,
                )?
            };
            let t1 = if obs_end <= forward.base_date() {
                0.0
            } else {
                forward.day_count().year_fraction(
                    forward.base_date(),
                    obs_end,
                    day_count_context,
                )?
            };
            Ok(if t1 > t0 {
                forward.rate_period(t0, t1)
            } else {
                forward.rate(t0)
            })
        }
        OvernightProjectionCurve::Discount(discount) => {
            let dcf = day_count.year_fraction(obs_start, obs_end, day_count_context)?;
            if dcf <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Overnight projection has non-positive observation accrual for \
                     {obs_start} -> {obs_end}"
                )));
            }
            Ok((1.0 / discount.df_between_dates(obs_start, obs_end)? - 1.0) / dcf)
        }
    }
}

/// Project one compounded overnight coupon and its parallel-forward sensitivity.
///
/// The sensitivity differentiates the complete product, not a simple endpoint
/// forward. Realized fixing factors have zero forward sensitivity; every future
/// factor contributes through the product rule.
pub(crate) fn project_overnight_coupon(
    input: OvernightCouponProjectionInput<'_>,
) -> Result<OvernightCouponProjection> {
    let day_count_context = DayCountContext {
        calendar: Some(input.fixing_calendar),
        frequency: input.coupon_frequency,
        ..DayCountContext::default()
    };
    let accrual_year_fraction =
        input
            .day_count
            .year_fraction(input.accrual_start, input.accrual_end, day_count_context)?;
    if input.accrual_end <= input.accrual_start
        || !accrual_year_fraction.is_finite()
        || accrual_year_fraction <= 0.0
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Invalid overnight coupon accrual {} -> {} with year fraction {}",
            input.accrual_start, input.accrual_end, accrual_year_fraction
        )));
    }
    if !input.compounded_spread.is_finite() {
        return Err(finstack_quant_core::Error::Validation(
            "Compounded overnight spread must be finite".into(),
        ));
    }

    let (shift_days, shift_dcf) = shifted_observation_days(input.compounding)?;
    let cutoff = if let Some(days) = cutoff_days(input.compounding) {
        let lockout_start = input
            .accrual_end
            .add_business_days(-days, input.fixing_calendar)?;
        let reference_start = lockout_start.add_business_days(-1, input.fixing_calendar)?;
        Some((lockout_start, reference_start, lockout_start))
    } else {
        None
    };

    let mut compound_factor = 1.0_f64;
    let mut factor_derivative = 0.0_f64;
    let mut factor_second_derivative = 0.0_f64;
    let mut fixing_date = None;
    let mut observation_factors = Vec::new();
    let mut date = input.accrual_start;
    while date < input.accrual_end {
        let step_end = date
            .add_business_days(1, input.fixing_calendar)?
            .min(input.accrual_end);
        let mut obs_start = date.add_business_days(-shift_days, input.fixing_calendar)?;
        let mut obs_end = step_end.add_business_days(-shift_days, input.fixing_calendar)?;
        if let Some((lockout_start, reference_start, reference_end)) = cutoff {
            if date >= lockout_start {
                obs_start = reference_start;
                obs_end = reference_end;
            }
        }
        if obs_end <= obs_start {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Overnight observation period is not positive after adjustment: \
                 {obs_start} -> {obs_end}"
            )));
        }
        let (dcf_start, dcf_end) = if shift_dcf {
            (obs_start, obs_end)
        } else {
            (date, step_end)
        };
        let dcf = input
            .day_count
            .year_fraction(dcf_start, dcf_end, day_count_context)?;
        if !dcf.is_finite() || dcf <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Overnight observation has non-positive day-count fraction for \
                 {dcf_start} -> {dcf_end}"
            )));
        }
        let rate_accrual_year_fraction =
            input
                .day_count
                .year_fraction(obs_start, obs_end, day_count_context)?;
        if !rate_accrual_year_fraction.is_finite() || rate_accrual_year_fraction <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Overnight rate interval has non-positive day-count fraction for \
                 {obs_start} -> {obs_end}"
            )));
        }

        // Valuation is at start of day, before the same-day fixing is published.
        let (rate, rate_sensitivity) = if obs_start < input.as_of {
            (
                finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                    input.fixings,
                    input.fixing_id,
                    obs_start,
                    input.as_of,
                )?,
                0.0,
            )
        } else {
            (
                projected_rate(
                    input.curve,
                    obs_start,
                    obs_end,
                    input.day_count,
                    day_count_context,
                )?,
                1.0,
            )
        };
        if !rate.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Overnight observation rate must be finite for {obs_start} -> {obs_end}, got \
                 {rate}"
            )));
        }
        let factor = 1.0 + (rate + input.compounded_spread) * dcf;
        if !factor.is_finite() || factor <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Overnight compounding factor must be finite and positive for \
                 {obs_start} -> {obs_end}, got {factor}"
            )));
        }
        factor_second_derivative =
            factor_second_derivative * factor + 2.0 * factor_derivative * dcf * rate_sensitivity;
        factor_derivative = factor_derivative * factor + compound_factor * dcf * rate_sensitivity;
        if !factor_derivative.is_finite() || !factor_second_derivative.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "Overnight compound sensitivities must remain finite".into(),
            ));
        }
        compound_factor *= factor;
        if !compound_factor.is_finite() || compound_factor <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Overnight compound product must remain finite and positive, got \
                 {compound_factor}"
            )));
        }
        observation_factors.push((
            obs_start,
            obs_end,
            rate,
            rate_accrual_year_fraction,
            dcf,
            factor,
            rate_sensitivity,
        ));
        fixing_date = Some(fixing_date.map_or(obs_start, |current: Date| current.max(obs_start)));
        date = step_end;
    }

    let mut prefixes = Vec::with_capacity(observation_factors.len() + 1);
    prefixes.push(1.0);
    for observation in &observation_factors {
        prefixes.push(prefixes.last().copied().unwrap_or(1.0) * observation.5);
    }
    let mut suffixes = vec![1.0; observation_factors.len() + 1];
    for index in (0..observation_factors.len()).rev() {
        suffixes[index] = suffixes[index + 1] * observation_factors[index].5;
    }
    let observation_exposures = observation_factors
        .iter()
        .enumerate()
        .map(
            |(
                index,
                &(
                    observation_start,
                    observation_end,
                    projected_rate,
                    rate_accrual_year_fraction,
                    factor_dcf,
                    _,
                    rate_sensitivity,
                ),
            )| OvernightObservationExposure {
                observation_start,
                observation_end,
                projected_rate,
                rate_accrual_year_fraction,
                factor_accrual_year_fraction: factor_dcf,
                coupon_forward_derivative: factor_dcf
                    * prefixes[index]
                    * suffixes[index + 1]
                    * rate_sensitivity
                    / accrual_year_fraction,
            },
        )
        .collect();

    let rate = (compound_factor - 1.0) / accrual_year_fraction;
    let parallel_forward_sensitivity = factor_derivative / accrual_year_fraction;
    let parallel_forward_second_sensitivity = factor_second_derivative / accrual_year_fraction;
    if !rate.is_finite()
        || !parallel_forward_sensitivity.is_finite()
        || !parallel_forward_second_sensitivity.is_finite()
    {
        return Err(finstack_quant_core::Error::Validation(
            "Overnight coupon rate and sensitivities must remain finite".into(),
        ));
    }

    Ok(OvernightCouponProjection {
        rate,
        accrual_year_fraction,
        compound_factor,
        parallel_forward_sensitivity,
        parallel_forward_second_sensitivity,
        fixing_date: fixing_date.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "Overnight coupon projection produced no observation periods".into(),
            )
        })?,
        observation_exposures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use time::macros::date;

    #[test]
    fn discount_projection_uses_coupon_day_count_and_telescopes() {
        let base_date = date!(2024 - 12 - 02);
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("discount curve");
        let calendar =
            resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
                .expect("USNY calendar");
        let compounding = FloatingLegCompounding::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        };
        let accrual_start = date!(2025 - 01 - 02);
        let accrual_end = date!(2025 - 04 - 02);
        let expected_accrual_year_fraction = DayCount::Act360
            .year_fraction(accrual_start, accrual_end, DayCountContext::default())
            .expect("accrual fraction");

        let projection = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Discount(&discount),
            fixings: None,
            fixing_id: "USD-SOFR",
            as_of: base_date,
            accrual_start,
            accrual_end,
            day_count: DayCount::Act360,
            coupon_frequency: None,
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
        })
        .expect("coupon projection");
        let expected = 1.0
            / discount
                .df_between_dates(accrual_start, accrual_end)
                .expect("relative discount factor");

        assert!(
            (projection.compound_factor - expected).abs() < 1.0e-12,
            "daily discount projection should telescope: {} vs {}",
            projection.compound_factor,
            expected
        );
        let expected_rate = (projection.compound_factor - 1.0) / expected_accrual_year_fraction;
        assert!(
            (projection.rate - expected_rate).abs() < 1.0e-12,
            "projector must normalize from adjusted dates: {} vs {}",
            projection.rate,
            expected_rate
        );
    }

    #[test]
    fn compounded_coupon_parallel_sensitivity_matches_finite_difference() {
        let base_date = date!(2024 - 12 - 02);
        let forward = ForwardCurve::builder("USD-SOFR-OIS", 1.0 / 360.0)
            .base_date(base_date)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.03), (0.2, 0.04), (0.5, 0.055), (1.0, 0.06)])
            .build()
            .expect("forward curve");
        let up = forward.with_parallel_bump(0.01).expect("up bump");
        let down = forward.with_parallel_bump(-0.01).expect("down bump");
        let calendar =
            resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test caplet")
                .expect("USNY calendar");
        let compounding = FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 };
        let project = |curve: &ForwardCurve| {
            project_overnight_coupon(OvernightCouponProjectionInput {
                curve: OvernightProjectionCurve::Forward(curve),
                fixings: None,
                fixing_id: "USD-SOFR-OIS",
                as_of: base_date,
                accrual_start: date!(2025 - 01 - 02),
                accrual_end: date!(2025 - 04 - 02),
                day_count: DayCount::Act360,
                coupon_frequency: None,
                compounding: &compounding,
                fixing_calendar: calendar,
                compounded_spread: 0.0,
            })
            .expect("coupon projection")
        };

        let base = project(&forward);
        let finite_difference = (project(&up).rate - project(&down).rate) / (2.0e-6);
        assert!(
            (base.parallel_forward_sensitivity - finite_difference).abs() < 1.0e-8,
            "analytic product sensitivity {} should match finite difference {}",
            base.parallel_forward_sensitivity,
            finite_difference
        );
        let exposure_sum: f64 = base
            .observation_exposures
            .iter()
            .map(|exposure| exposure.coupon_forward_derivative)
            .sum();
        assert!(
            (exposure_sum - finite_difference).abs() < 1.0e-8,
            "date-specific product derivatives {exposure_sum} should sum to the parallel \
             finite difference {finite_difference}"
        );

        let second_up = project(&forward.with_parallel_bump(1.0).expect("second up bump"));
        let second_down = project(&forward.with_parallel_bump(-1.0).expect("second down bump"));
        let second_finite_difference =
            (second_up.rate - 2.0 * base.rate + second_down.rate) / 1.0e-8;
        assert!(
            (base.parallel_forward_second_sensitivity - second_finite_difference).abs() < 1.0e-6,
            "analytic product second sensitivity {} should match finite difference {}",
            base.parallel_forward_second_sensitivity,
            second_finite_difference
        );
    }

    #[test]
    fn historical_observation_has_zero_stochastic_exposure() {
        let accrual_start = date!(2025 - 01 - 02);
        let as_of = date!(2025 - 01 - 03);
        let accrual_end = date!(2025 - 01 - 07);
        let forward = ForwardCurve::builder("USD-SOFR-OIS", 1.0 / 360.0)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.04), (1.0, 0.04)])
            .build()
            .expect("forward curve");
        let fixings =
            ScalarTimeSeries::new("FIXING:USD-SOFR-OIS", vec![(accrual_start, 0.035)], None)
                .expect("fixings");
        let calendar =
            resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
                .expect("USNY calendar");
        let compounding = FloatingLegCompounding::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        };
        let projection = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&forward),
            fixings: Some(&fixings),
            fixing_id: "USD-SOFR-OIS",
            as_of,
            accrual_start,
            accrual_end,
            day_count: DayCount::Act360,
            coupon_frequency: None,
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
        })
        .expect("projection");

        assert_eq!(
            projection.observation_exposures[0].coupon_forward_derivative,
            0.0
        );
        assert!(
            projection.observation_exposures[1].coupon_forward_derivative > 0.0,
            "same-day unpublished observation should retain stochastic exposure"
        );
    }

    #[test]
    fn rejects_non_positive_compounding_factor() {
        let as_of = date!(2025 - 01 - 02);
        let forward = ForwardCurve::builder("BAD-OVERNIGHT", 1.0 / 360.0)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, -400.0), (1.0, -400.0)])
            .build()
            .expect("finite pathological forward curve");
        let calendar =
            resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
                .expect("USNY calendar");
        let compounding = FloatingLegCompounding::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        };

        let result = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&forward),
            fixings: None,
            fixing_id: "BAD-OVERNIGHT",
            as_of,
            accrual_start: as_of,
            accrual_end: date!(2025 - 01 - 03),
            day_count: DayCount::Act360,
            coupon_frequency: None,
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
        });

        assert!(
            result.is_err(),
            "non-positive daily compounding factor must fail closed"
        );
    }

    #[test]
    fn rejects_non_finite_projected_rate_or_compound_product() {
        let as_of = date!(2025 - 01 - 02);
        let forward = ForwardCurve::builder("OVERFLOW-OVERNIGHT", 1.0 / 360.0)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, f64::MAX), (1.0, f64::MAX)])
            .build()
            .expect("finite pathological forward curve");
        let calendar =
            resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
                .expect("USNY calendar");
        let compounding = FloatingLegCompounding::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        };

        let result = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&forward),
            fixings: None,
            fixing_id: "OVERFLOW-OVERNIGHT",
            as_of,
            accrual_start: as_of,
            accrual_end: date!(2025 - 01 - 07),
            day_count: DayCount::Act360,
            coupon_frequency: None,
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
        });

        assert!(
            result.is_err(),
            "non-finite rate-derived factors or products must fail closed"
        );
    }

    #[test]
    fn discount_projection_rejects_non_positive_or_non_finite_relative_df() {
        let base_date = date!(2025 - 01 - 02);
        let discount = DiscountCurve::builder("UNDERFLOW-DISCOUNT")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, f64::MIN_POSITIVE)])
            .build()
            .expect("positive finite input discount factors");

        let result = projected_rate(
            OvernightProjectionCurve::Discount(&discount),
            base_date,
            date!(2027 - 01 - 04),
            DayCount::Act360,
            DayCountContext::default(),
        );

        assert!(
            result.is_err(),
            "underflowed relative discount factors must fail closed"
        );
    }

    #[test]
    fn projector_uses_fixing_calendar_for_business_252_accrual() {
        let as_of = date!(2025 - 01 - 02);
        let forward = ForwardCurve::builder("BRL-OVERNIGHT", 1.0 / 252.0)
            .base_date(as_of)
            .day_count(DayCount::Bus252)
            .knots([(0.0, 0.10), (1.0, 0.10)])
            .build()
            .expect("forward curve");
        let calendar =
            resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
                .expect("calendar");
        let compounding = FloatingLegCompounding::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        };

        let projection = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&forward),
            fixings: None,
            fixing_id: "BRL-OVERNIGHT",
            as_of,
            accrual_start: as_of,
            accrual_end: date!(2025 - 01 - 10),
            day_count: DayCount::Bus252,
            coupon_frequency: None,
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
        })
        .expect("business/252 projection");
        let expected = DayCount::Bus252
            .year_fraction(
                as_of,
                date!(2025 - 01 - 10),
                DayCountContext {
                    calendar: Some(calendar),
                    ..DayCountContext::default()
                },
            )
            .expect("business/252 accrual");

        assert_eq!(projection.accrual_year_fraction, expected);
    }

    #[test]
    fn projector_uses_coupon_frequency_for_act_act_isma_accrual() {
        let as_of = date!(2025 - 01 - 02);
        let forward = ForwardCurve::builder("ICMA-OVERNIGHT", 1.0 / 365.0)
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 0.03), (1.0, 0.03)])
            .build()
            .expect("forward curve");
        let calendar =
            resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
                .expect("calendar");
        let compounding = FloatingLegCompounding::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        };

        let projection = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&forward),
            fixings: None,
            fixing_id: "ICMA-OVERNIGHT",
            as_of,
            accrual_start: as_of,
            accrual_end: date!(2025 - 07 - 02),
            day_count: DayCount::ActActIsma,
            coupon_frequency: Some(Tenor::semi_annual()),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
        })
        .expect("Act/Act ISMA projection");

        assert_eq!(projection.accrual_year_fraction, 0.5);
    }
}
