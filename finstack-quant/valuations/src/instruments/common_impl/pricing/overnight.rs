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
    /// Actual full coupon boundaries enclosing the accrual window; ACT/365L
    /// preserves this period's denominator when projecting a coupon slice.
    pub coupon_period: (Date, Date),
    /// Shared overnight compounding convention.
    pub compounding: &'a FloatingLegCompounding,
    /// Resolved fixing calendar.
    pub fixing_calendar: &'a dyn HolidayCalendar,
    /// Daily spread included inside each factor, in decimal rate units.
    pub compounded_spread: f64,
    /// When true, emit per-observation product-rule exposures.
    ///
    /// Cap/floor and listed-rate futures need the daily loadings. IRS, XCCY,
    /// TRS, basis, and revolver PV can leave this false so fully future
    /// discount-curve coupons use the closed-form DF identity.
    pub need_observation_exposures: bool,
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

/// Inputs for an arithmetic-average overnight-rate projection.
///
/// This is the settlement convention used by contracts such as one-month
/// SOFR and Federal Funds futures. Calendar-day weights are produced by
/// advancing between fixing-calendar business days, so a Friday observation
/// naturally carries through the weekend.
pub(crate) struct OvernightArithmeticProjectionInput<'a> {
    /// Projection source for future observations.
    pub curve: OvernightProjectionCurve<'a>,
    /// Historical fixing series for observations before `as_of`.
    pub fixings: Option<&'a ScalarTimeSeries>,
    /// Fixing/index identifier used in missing-fixing errors.
    pub fixing_id: &'a str,
    /// Valuation date separating realized and projected observations.
    pub as_of: Date,
    /// Contractual averaging-period start.
    pub accrual_start: Date,
    /// Contractual averaging-period end.
    pub accrual_end: Date,
    /// Day-count basis used for the published overnight rate.
    pub day_count: DayCount,
    /// Resolved fixing calendar.
    pub fixing_calendar: &'a dyn HolidayCalendar,
}

/// Projected economics of an arithmetic-average overnight rate.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OvernightArithmeticProjection {
    /// Calendar-day-weighted annualized rate in decimal units.
    pub rate: f64,
    /// Derivative of `rate` to a parallel bump of every projected observation.
    pub parallel_forward_sensitivity: f64,
    /// Last distinct overnight observation date determining the average.
    pub fixing_date: Date,
    /// Per-observation derivatives; realized observations carry zero derivative.
    pub observation_exposures: Vec<OvernightObservationExposure>,
}

/// Standard rates calendar for a currency when a contract does not supply one.
pub(crate) fn default_rate_calendar_id(currency: Currency) -> Option<&'static str> {
    match currency {
        Currency::USD => Some("usny"),
        Currency::EUR => Some("target2"),
        Currency::GBP => Some("gblo"),
        Currency::JPY => Some("jpto"),
        Currency::AUD => Some("auce"),
        Currency::CAD => Some("cato"),
        Currency::CHF => Some("chzh"),
        _ => None,
    }
}

/// Resolve the explicit or currency-standard fixing calendar for an RFR coupon.
pub(crate) fn resolve_overnight_fixing_calendar(
    calendar_id: Option<&str>,
    currency: Currency,
    instrument_label: &str,
) -> Result<&'static dyn HolidayCalendar> {
    let default_id = default_rate_calendar_id(currency);
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
    business_day_convention: BusinessDayConvention,
    calendar: &dyn HolidayCalendar,
) -> Result<(Date, Date)> {
    Ok((
        adjust(accrual_start, business_day_convention, calendar)?,
        adjust(accrual_end, business_day_convention, calendar)?,
    ))
}

fn shifted_observation_days(compounding: &FloatingLegCompounding) -> Result<(i32, bool)> {
    match compounding {
        FloatingLegCompounding::Simple | FloatingLegCompounding::SimpleAverage => {
            Err(finstack_quant_core::Error::Validation(format!(
                "Overnight coupon projection requires a compounded convention, not {compounding}"
            )))
        }
        FloatingLegCompounding::CompoundedInArrears { lookback_days } => {
            Ok((i32_days(*lookback_days)?, false))
        }
        FloatingLegCompounding::CompoundedWithObservationShift { shift_days } => {
            Ok((i32_days(*shift_days)?, true))
        }
        FloatingLegCompounding::CompoundedWithRateCutoff { .. } => Ok((0, false)),
    }
}

/// Convert a business-day count to the signed day arithmetic used here.
fn i32_days(days: u32) -> Result<i32> {
    i32::try_from(days).map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "overnight compounding day count {days} exceeds i32::MAX"
        ))
    })
}

fn cutoff_days(compounding: &FloatingLegCompounding) -> Option<i32> {
    match compounding {
        FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days } if *cutoff_days > 0 => {
            i32::try_from(*cutoff_days).ok()
        }
        _ => None,
    }
}

fn discount_identity_eligible(input: &OvernightCouponProjectionInput<'_>) -> bool {
    !input.need_observation_exposures
        && input.compounded_spread == 0.0
        && matches!(input.curve, OvernightProjectionCurve::Discount(_))
        && matches!(
            input.compounding,
            FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }
        )
}

fn last_overnight_observation(
    start: Date,
    end: Date,
    calendar: &dyn HolidayCalendar,
) -> Result<Date> {
    if end <= start {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Overnight identity has a non-positive observation window {start} -> {end}"
        )));
    }
    end.add_business_days(-1, calendar)
}
/// Last observation date that contractually determines an overnight coupon.
pub(crate) fn final_overnight_fixing_date(
    start: Date,
    end: Date,
    compounding: &FloatingLegCompounding,
    calendar: &dyn HolidayCalendar,
) -> Result<Date> {
    let (shift_days, _) = shifted_observation_days(compounding)?;
    if let Some(days) = cutoff_days(compounding) {
        return end.add_business_days(-(days + 1), calendar);
    }
    last_overnight_observation(start, end, calendar)?.add_business_days(-shift_days, calendar)
}

fn projection_from_compound_factor(
    compound_factor: f64,
    accrual_year_fraction: f64,
    fixing_date: Date,
) -> Result<OvernightCouponProjection> {
    if !compound_factor.is_finite() || compound_factor <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Overnight compound product must remain finite and positive, got {compound_factor}"
        )));
    }
    let rate = (compound_factor - 1.0) / accrual_year_fraction;
    if !rate.is_finite() {
        return Err(finstack_quant_core::Error::Validation(
            "Overnight coupon rate and sensitivities must remain finite".into(),
        ));
    }
    Ok(OvernightCouponProjection {
        rate,
        accrual_year_fraction,
        compound_factor,
        parallel_forward_sensitivity: 0.0,
        parallel_forward_second_sensitivity: 0.0,
        fixing_date,
        observation_exposures: Vec::new(),
    })
}

fn discount_compound_factor(discount: &DiscountCurve, start: Date, end: Date) -> Result<f64> {
    let df = discount.df_between_dates(start, end)?;
    if !df.is_finite() || df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Overnight discount identity requires a positive DF ratio for {start} -> {end}, got {df}"
        )));
    }
    Ok(1.0 / df)
}

fn project_overnight_coupon_discount_identity(
    input: OvernightCouponProjectionInput<'_>,
    day_count_context: DayCountContext<'_>,
    accrual_year_fraction: f64,
) -> Result<OvernightCouponProjection> {
    let OvernightProjectionCurve::Discount(discount) = input.curve else {
        return Err(finstack_quant_core::Error::Internal(
            "discount identity selected without a discount curve".into(),
        ));
    };

    if input.accrual_start >= input.as_of {
        let compound_factor =
            discount_compound_factor(discount, input.accrual_start, input.accrual_end)?;
        let fixing_date = last_overnight_observation(
            input.accrual_start,
            input.accrual_end,
            input.fixing_calendar,
        )?;
        return projection_from_compound_factor(
            compound_factor,
            accrual_year_fraction,
            fixing_date,
        );
    }

    let mut compound_factor = 1.0_f64;
    let mut fixing_date = None;
    let mut date = input.accrual_start;
    while date < input.accrual_end {
        let step_end = date
            .add_business_days(1, input.fixing_calendar)?
            .min(input.accrual_end);
        if date < input.as_of {
            let dcf = input
                .day_count
                .year_fraction(date, step_end, day_count_context)?;
            if !dcf.is_finite() || dcf <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Overnight observation has non-positive day-count fraction for \
                     {date} -> {step_end}"
                )));
            }
            let rate = finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                input.fixings,
                input.fixing_id,
                date,
                input.as_of,
            )?;
            if !rate.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Overnight observation rate must be finite for {date} -> {step_end}, got {rate}"
                )));
            }
            let factor = 1.0 + rate * dcf;
            if !factor.is_finite() || factor <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Overnight compounding factor must be finite and positive for \
                     {date} -> {step_end}, got {factor}"
                )));
            }
            compound_factor *= factor;
            fixing_date = Some(fixing_date.map_or(date, |current: Date| current.max(date)));
            date = step_end;
            continue;
        }
        compound_factor *= discount_compound_factor(discount, date, input.accrual_end)?;
        let last = last_overnight_observation(date, input.accrual_end, input.fixing_calendar)?;
        fixing_date = Some(fixing_date.map_or(last, |current| current.max(last)));
        date = input.accrual_end;
    }

    projection_from_compound_factor(
        compound_factor,
        accrual_year_fraction,
        fixing_date.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "Overnight coupon projection produced no observation periods".into(),
            )
        })?,
    )
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
            if obs_start < forward.base_date() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Overnight projection for {} starts before its curve base date {}; supply the historical fixing for {obs_start}",
                    forward.id(), forward.base_date()
                )));
            }
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
            let rate_accrual = day_count.year_fraction(obs_start, obs_end, day_count_context)?;
            if !rate_accrual.is_finite() || rate_accrual <= 0.0 || t1 <= t0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Overnight projection requires positive curve time and index accrual for {obs_start} -> {obs_end}"
                )));
            }
            // ForwardCurve rates are annualized on its own clock. Preserve the
            // observation's growth when quoting it on the overnight index basis.
            Ok(forward.rate_period(t0, t1) * (t1 - t0) / rate_accrual)
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

/// Project an arithmetic average of overnight fixings and its forward sensitivity.
///
/// The average is `sum(r_i * d_i) / sum(d_i)` on the supplied day-count basis.
/// Observations strictly before `as_of` must be present in the historical fixing
/// series. The same-day observation remains projected because valuation occurs at
/// the start of day, before publication.
pub(crate) fn project_arithmetic_overnight_rate(
    input: OvernightArithmeticProjectionInput<'_>,
) -> Result<OvernightArithmeticProjection> {
    let day_count_context = DayCountContext {
        calendar: Some(input.fixing_calendar),
        coupon_period: Some((input.accrual_start, input.accrual_end)),
        ..DayCountContext::default()
    };
    let total_accrual =
        input
            .day_count
            .year_fraction(input.accrual_start, input.accrual_end, day_count_context)?;
    if input.accrual_end <= input.accrual_start
        || !total_accrual.is_finite()
        || total_accrual <= 0.0
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Invalid arithmetic overnight averaging period {} -> {} with year fraction {}",
            input.accrual_start, input.accrual_end, total_accrual
        )));
    }

    let mut weighted_sum = 0.0;
    let mut parallel_forward_sensitivity = 0.0;
    let mut observation_exposures = Vec::new();
    let mut fixing_date = None;
    let mut date = input.accrual_start;
    let mut observation_date = if input.fixing_calendar.is_business_day(date) {
        date
    } else {
        date.add_business_days(-1, input.fixing_calendar)?
    };
    while date < input.accrual_end {
        let next_observation_date = observation_date.add_business_days(1, input.fixing_calendar)?;
        let step_end = next_observation_date.min(input.accrual_end);
        let accrual = input
            .day_count
            .year_fraction(date, step_end, day_count_context)?;
        if !accrual.is_finite() || accrual <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Arithmetic overnight observation has non-positive accrual for {date} -> \
                 {step_end}"
            )));
        }
        let (rate, stochastic_exposure) = if observation_date < input.as_of {
            (
                finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                    input.fixings,
                    input.fixing_id,
                    observation_date,
                    input.as_of,
                )?,
                0.0,
            )
        } else {
            (
                projected_rate(
                    input.curve,
                    observation_date,
                    next_observation_date,
                    input.day_count,
                    day_count_context,
                )?,
                1.0,
            )
        };
        if !rate.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Arithmetic overnight observation rate must be finite for {date} -> \
                 {step_end}, got {rate}"
            )));
        }
        let weight = accrual / total_accrual;
        weighted_sum += rate * weight;
        let derivative = weight * stochastic_exposure;
        parallel_forward_sensitivity += derivative;
        observation_exposures.push(OvernightObservationExposure {
            observation_start: observation_date,
            observation_end: step_end,
            projected_rate: rate,
            rate_accrual_year_fraction: accrual,
            factor_accrual_year_fraction: accrual,
            coupon_forward_derivative: derivative,
        });
        fixing_date = Some(fixing_date.map_or(observation_date, |current: Date| {
            current.max(observation_date)
        }));
        date = step_end;
        observation_date = next_observation_date;
    }

    if !weighted_sum.is_finite() || !parallel_forward_sensitivity.is_finite() {
        return Err(finstack_quant_core::Error::Validation(
            "Arithmetic overnight rate and sensitivity must remain finite".into(),
        ));
    }
    Ok(OvernightArithmeticProjection {
        rate: weighted_sum,
        parallel_forward_sensitivity,
        fixing_date: fixing_date.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "Arithmetic overnight projection produced no observations".into(),
            )
        })?,
        observation_exposures,
    })
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
        coupon_period: Some(input.coupon_period),
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

    if discount_identity_eligible(&input) {
        return project_overnight_coupon_discount_identity(
            input,
            day_count_context,
            accrual_year_fraction,
        );
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

/// Cash coupon from a compounded overnight projection plus an arithmetic spread.
///
/// Index interest is `N × (compound_factor − 1)`. The spread accrues on the
/// projector's holiday-adjusted year fraction, not the unadjusted schedule
/// fraction. IRS, XCCY, TRS, and basis-swap overnight legs must use this
/// helper so coupon amounts cannot drift apart.
///
/// # Arguments
///
/// * `notional` - Leg notional in currency units. Sign is the caller's
///   responsibility; this helper returns an unsigned economic amount.
/// * `projection` - Compounded overnight coupon already projected on the
///   adjusted accrual window.
/// * `spread` - Arithmetic spread in decimal rate units (not basis points).
pub(crate) fn overnight_coupon_amount(
    notional: f64,
    projection: &OvernightCouponProjection,
    spread: f64,
) -> f64 {
    notional * (projection.compound_factor - 1.0)
        + notional * spread * projection.accrual_year_fraction
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use time::macros::date;

    #[test]
    fn overnight_projection_preserves_index_rates_across_curve_clocks() {
        let start = date!(2025 - 01 - 02);
        let end = date!(2025 - 04 - 02);
        let calendar = finstack_quant_core::dates::calendar_by_id("usny").expect("USD calendar");
        let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
        let mut factors = Vec::new();
        for (curve_day_count, curve_rate) in [
            (DayCount::Act360, 0.04),
            (DayCount::Act365F, 0.04 * 365.0 / 360.0),
        ] {
            let forward = ForwardCurve::builder("USD-SOFR", 1.0 / 360.0)
                .base_date(start)
                .day_count(curve_day_count)
                .knots([(0.0, curve_rate), (1.0, curve_rate)])
                .build()
                .expect("forward curve");
            let project = |curve: &ForwardCurve| {
                project_overnight_coupon(OvernightCouponProjectionInput {
                    curve: OvernightProjectionCurve::Forward(curve),
                    fixings: None,
                    fixing_id: "USD-SOFR",
                    as_of: start,
                    accrual_start: start,
                    accrual_end: end,
                    day_count: DayCount::Act360,
                    coupon_frequency: None,
                    coupon_period: (start, end),
                    compounding: &compounding,
                    fixing_calendar: calendar,
                    compounded_spread: 0.0,
                    need_observation_exposures: true,
                })
                .expect("overnight projection")
            };
            let projection = project(&forward);
            for observation in &projection.observation_exposures {
                assert!((observation.projected_rate - 0.04).abs() < 1e-13);
            }
            // Sensitivities are to the contractual index rate. Convert the raw
            // curve bump so both clocks represent the same 0.01 bp index bump.
            let curve_bump_bp = 0.01 * curve_rate / 0.04;
            let up = forward.with_parallel_bump(curve_bump_bp).expect("up curve");
            let down = forward
                .with_parallel_bump(-curve_bump_bp)
                .expect("down curve");
            let finite_difference = (project(&up).rate - project(&down).rate) / 2e-6;
            assert!((projection.parallel_forward_sensitivity - finite_difference).abs() < 1e-8);
            factors.push(projection.compound_factor);
        }
        assert!((factors[0] - factors[1]).abs() < 1e-13);
    }

    #[test]
    fn overnight_coupon_amount_uses_compound_factor_and_projection_yf() {
        let projection = OvernightCouponProjection {
            rate: 0.04,
            accrual_year_fraction: 0.25,
            compound_factor: 1.01,
            parallel_forward_sensitivity: 0.0,
            parallel_forward_second_sensitivity: 0.0,
            fixing_date: date!(2025 - 04 - 02),
            observation_exposures: Vec::new(),
        };
        let notional = 1_000_000.0;
        let amount = overnight_coupon_amount(notional, &projection, 0.001);
        let expected = notional * (1.01 - 1.0) + notional * 0.001 * 0.25;
        assert!(
            (amount - expected).abs() < 1e-12,
            "overnight coupon amount {amount} != N*(CF-1) + N*spread*yf {expected}"
        );
    }

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
        let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
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
            coupon_period: (accrual_start, accrual_end),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
            need_observation_exposures: false,
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
        assert!(
            projection.observation_exposures.is_empty(),
            "discount identity path should not emit daily observation exposures"
        );
    }

    fn walked_last_overnight_observation(
        start: Date,
        end: Date,
        calendar: &dyn HolidayCalendar,
    ) -> Date {
        let mut date = start;
        let mut last = start;
        while date < end {
            last = date;
            date = date
                .add_business_days(1, calendar)
                .expect("business day")
                .min(end);
        }
        last
    }

    fn assert_discount_identity_fixing_date(
        base_date: Date,
        discount: &DiscountCurve,
        calendar: &dyn HolidayCalendar,
        accrual_start: Date,
        accrual_end: Date,
    ) {
        let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
        let project = |need_exposures: bool| {
            project_overnight_coupon(OvernightCouponProjectionInput {
                curve: OvernightProjectionCurve::Discount(discount),
                fixings: None,
                fixing_id: "USD-SOFR",
                as_of: base_date,
                accrual_start,
                accrual_end,
                day_count: DayCount::Act360,
                coupon_frequency: None,
                coupon_period: (accrual_start, accrual_end),
                compounding: &compounding,
                fixing_calendar: calendar,
                compounded_spread: 0.0,
                need_observation_exposures: need_exposures,
            })
            .expect("coupon projection")
        };
        let identity = project(false);
        let walked = project(true);
        let last_observation = last_overnight_observation(accrual_start, accrual_end, calendar)
            .expect("last overnight observation");
        let business_day_before_end = accrual_end
            .add_business_days(-1, calendar)
            .expect("business day before end");
        let walked_last = walked_last_overnight_observation(accrual_start, accrual_end, calendar);

        assert!(
            (identity.compound_factor - walked.compound_factor).abs() < 1.0e-12,
            "identity CF {} vs walked CF {}",
            identity.compound_factor,
            walked.compound_factor
        );
        assert!(
            (identity.rate - walked.rate).abs() < 1.0e-12,
            "identity rate {} vs walked rate {}",
            identity.rate,
            walked.rate
        );
        assert_eq!(identity.fixing_date, walked.fixing_date);
        assert_eq!(identity.fixing_date, last_observation);
        assert_eq!(identity.fixing_date, business_day_before_end);
        assert_eq!(identity.fixing_date, walked_last);
        assert!(identity.observation_exposures.is_empty());
        assert!(!walked.observation_exposures.is_empty());
    }

    #[test]
    fn discount_identity_matches_daily_walk_for_future_coupon() {
        let base_date = date!(2024 - 12 - 02);
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.90)])
            .build()
            .expect("discount curve");
        let calendar =
            resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
                .expect("USNY calendar");

        assert_discount_identity_fixing_date(
            base_date,
            &discount,
            calendar,
            date!(2025 - 01 - 02),
            date!(2026 - 01 - 02),
        );
        // Accrual ends on Saturday so the last observation must be the prior Friday.
        assert_discount_identity_fixing_date(
            base_date,
            &discount,
            calendar,
            date!(2025 - 01 - 03),
            date!(2025 - 01 - 11),
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
                coupon_period: (date!(2025 - 01 - 02), date!(2025 - 04 - 02)),
                compounding: &compounding,
                fixing_calendar: calendar,
                compounded_spread: 0.0,
                need_observation_exposures: true,
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
        let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
        let projection = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&forward),
            fixings: Some(&fixings),
            fixing_id: "USD-SOFR-OIS",
            as_of,
            accrual_start,
            accrual_end,
            day_count: DayCount::Act360,
            coupon_frequency: None,
            coupon_period: (accrual_start, accrual_end),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
            need_observation_exposures: true,
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
        let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };

        let result = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&forward),
            fixings: None,
            fixing_id: "BAD-OVERNIGHT",
            as_of,
            accrual_start: as_of,
            accrual_end: date!(2025 - 01 - 03),
            day_count: DayCount::Act360,
            coupon_frequency: None,
            coupon_period: (as_of, date!(2025 - 01 - 03)),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
            need_observation_exposures: false,
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
        let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };

        let result = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&forward),
            fixings: None,
            fixing_id: "OVERFLOW-OVERNIGHT",
            as_of,
            accrual_start: as_of,
            accrual_end: date!(2025 - 01 - 07),
            day_count: DayCount::Act360,
            coupon_frequency: None,
            coupon_period: (as_of, date!(2025 - 01 - 07)),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
            need_observation_exposures: false,
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
        let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };

        let projection = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&forward),
            fixings: None,
            fixing_id: "BRL-OVERNIGHT",
            as_of,
            accrual_start: as_of,
            accrual_end: date!(2025 - 01 - 10),
            day_count: DayCount::Bus252,
            coupon_frequency: None,
            coupon_period: (as_of, date!(2025 - 01 - 10)),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
            need_observation_exposures: false,
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
        let as_of = date!(2025 - 01 - 01);
        let forward = ForwardCurve::builder("ICMA-OVERNIGHT", 1.0 / 365.0)
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 0.03), (1.0, 0.03)])
            .build()
            .expect("forward curve");
        let calendar =
            resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
                .expect("calendar");
        let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };

        let projection = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&forward),
            fixings: None,
            fixing_id: "ICMA-OVERNIGHT",
            as_of,
            accrual_start: as_of,
            accrual_end: date!(2025 - 07 - 01),
            day_count: DayCount::ActActIsma,
            coupon_frequency: Some(Tenor::semi_annual()),
            coupon_period: (as_of, date!(2025 - 07 - 01)),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
            need_observation_exposures: false,
        })
        .expect("Act/Act ISMA projection");

        assert_eq!(projection.accrual_year_fraction, 0.5);
    }
}
