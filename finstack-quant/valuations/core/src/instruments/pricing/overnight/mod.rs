//! Projection engine for compounded overnight-RFR coupons of swap-style instruments.
//!
//! IRS, basis, cross-currency, futures, cap/floor, TRS, revolver and risk
//! pathways use this module so lookback, observation-shift, cutoff, fixing,
//! and sensitivity semantics stay identical among them. The cashflow builder
//! (bonds, term loans, structured coupons, bond LSMC) compounds through its
//! own engine, `finstack_quant_cashflows::builder` (`overnight` and
//! `floating_replay`), which carries a non-business accrual start as a
//! leading slice where the IRS path snaps endpoints to business days. Mirror
//! convention changes in both.

use crate::instruments::pricing::default_rate_calendar_id;
use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, DayCountContext, HolidayCalendar, Tenor,
    adjust, calendar_by_id_strict,
};
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::market_data::traits::Discounting;

/// Curve source used to project future overnight fixings.
#[derive(Clone, Copy)]
pub enum OvernightProjectionCurve<'a> {
    /// Explicit overnight forward curve.
    Forward(&'a ForwardCurve),
    /// Single-curve OIS fallback projected from discount factors.
    Discount(&'a DiscountCurve),
}

impl OvernightProjectionCurve<'_> {
    /// Project an annualized simple overnight rate over `[obs_start, obs_end)`
    /// using a precomputed accrual year fraction.
    ///
    /// # Arguments
    ///
    /// * `obs_start` - Start date of the overnight observation interval.
    /// * `obs_end` - Exclusive end date of the observation interval (`obs_end > obs_start`).
    /// * `rate_accrual` - Positive, finite day-count fraction matching the
    ///   projection curve's annualization basis for `[obs_start, obs_end)`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `rate_accrual` is not positive/finite,
    /// `obs_end <= obs_start`, or `obs_start` precedes a forward curve's base
    /// date (in which case a historical fixing must be supplied).
    pub fn projected_rate(&self, obs_start: Date, obs_end: Date, rate_accrual: f64) -> Result<f64> {
        if !rate_accrual.is_finite() || rate_accrual <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Overnight projection requires positive curve time and index accrual for \
                 {obs_start} -> {obs_end}"
            )));
        }
        if obs_end <= obs_start {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Overnight projection requires an ordered observation window for \
                 {obs_start} -> {obs_end}"
            )));
        }
        match *self {
            Self::Forward(forward) => {
                if obs_start < forward.base_date() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Overnight projection for {} starts before its curve base date {}; \
                         supply the historical fixing for {obs_start}",
                        forward.id(),
                        forward.base_date()
                    )));
                }
                forward.rate_between_dates(obs_start, obs_end, rate_accrual)
            }
            Self::Discount(discount) => {
                Ok((1.0 / discount.df_between_dates(obs_start, obs_end)? - 1.0) / rate_accrual)
            }
        }
    }
}

/// Inputs for one compounded overnight coupon projection.
pub struct OvernightCouponProjectionInput<'a> {
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
    /// Whether `accrual_end` is the instrument's contractual termination date;
    /// `None` when the caller has no schedule fact, which 30E/360 ISDA rejects.
    /// Only an interval ending on `accrual_end` inherits this status.
    pub end_is_termination_date: Option<bool>,
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
pub struct OvernightObservationExposure {
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
pub struct OvernightCouponProjection {
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

impl OvernightCouponProjection {
    /// Iterator of raw daily/observation [`ProjectedFixing`] records for `index_id`.
    ///
    /// Computes the canonical `FIXING:{index_id}` series key once and yields one
    /// fixing entry per element of [`Self::observation_exposures`].
    ///
    /// # Arguments
    ///
    /// * `index_id` - Contractual overnight index or forward-curve identifier.
    pub fn projected_fixings<'a>(
        &'a self,
        index_id: &str,
    ) -> impl Iterator<Item = finstack_quant_cashflows::fixings::ProjectedFixing> + 'a {
        let series_id = finstack_quant_core::market_data::fixings::fixing_series_id(index_id);
        self.observation_exposures.iter().map(move |observation| {
            finstack_quant_cashflows::fixings::ProjectedFixing {
                series_id: series_id.clone(),
                date: observation.observation_start,
                value: Some(observation.projected_rate),
            }
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
    ///   responsibility; this method returns an unsigned economic amount.
    /// * `spread` - Arithmetic spread in decimal rate units (not basis points).
    pub fn coupon_amount(&self, notional: f64, spread: f64) -> f64 {
        notional * (self.compound_factor - 1.0) + notional * spread * self.accrual_year_fraction
    }
}

/// Resolve the explicit or currency-standard fixing calendar for an RFR coupon.
pub fn resolve_overnight_fixing_calendar(
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
    calendar_by_id_strict(id)
}

/// Adjust an overnight coupon's contractual accrual boundaries before daily compounding.
///
/// The schedule builder may preserve unadjusted roll dates, while overnight observations
/// must start and end on business days. IRS and cap/floor projection share this helper.
pub fn adjust_overnight_accrual_boundaries(
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
            let tag = finstack_quant_core::wire::serde_tag(compounding)?;
            Err(finstack_quant_core::Error::Validation(format!(
                "Overnight coupon projection requires a compounded convention, not {tag}"
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

fn cutoff_days(compounding: &FloatingLegCompounding) -> Result<Option<i32>> {
    match compounding {
        FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days } if *cutoff_days > 0 => {
            let days = i32_days(*cutoff_days)?;
            days.checked_add(1).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "overnight rate cutoff {cutoff_days} overflows i32"
                ))
            })?;
            Ok(Some(days))
        }
        _ => Ok(None),
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
            let dcf = input.day_count.year_fraction(
                date,
                step_end,
                interval_context(day_count_context, step_end, input.accrual_end),
            )?;
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

/// The day-count context for one interval of a coupon accruing to
/// `accrual_end`: only an interval ending there inherits the coupon's
/// Narrow `end_is_termination_date` to the final interval of `[_, accrual_end)`.
///
/// Only an interval ending on `accrual_end` keeps the coupon's contractual
/// termination status; every other interval end is ordinary.
///
/// # Arguments
///
/// * `context` - Base coupon day-count context.
/// * `interval_end` - Exclusive end date of the current observation sub-interval.
/// * `accrual_end` - Exclusive end date of the full coupon accrual period.
pub fn interval_context<'a>(
    context: DayCountContext<'a>,
    interval_end: Date,
    accrual_end: Date,
) -> DayCountContext<'a> {
    DayCountContext {
        end_is_termination_date: context
            .end_is_termination_date
            .map(|terminal| terminal && interval_end == accrual_end),
        ..context
    }
}

/// Project one compounded overnight coupon and its parallel-forward sensitivity.
///
/// The sensitivity differentiates the complete product, not a simple endpoint
/// forward. Realized fixing factors have zero forward sensitivity; every future
/// factor contributes through the product rule.
pub fn project_overnight_coupon(
    input: OvernightCouponProjectionInput<'_>,
) -> Result<OvernightCouponProjection> {
    let day_count_context = DayCountContext {
        calendar: Some(input.fixing_calendar),
        frequency: input.coupon_frequency,
        coupon_period: Some(input.coupon_period),
        end_is_termination_date: input.end_is_termination_date,
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
    let cutoff = if let Some(days) = cutoff_days(input.compounding)? {
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
    let mut observation_factors = input.need_observation_exposures.then(Vec::new);
    let mut date = input.accrual_start;
    while date < input.accrual_end {
        let step_end = date
            .add_business_days(1, input.fixing_calendar)?
            .min(input.accrual_end);
        let mut obs_start = date.add_business_days(-shift_days, input.fixing_calendar)?;
        let mut obs_end = step_end.add_business_days(-shift_days, input.fixing_calendar)?;
        if let Some((lockout_start, reference_start, reference_end)) = cutoff
            && date >= lockout_start
        {
            obs_start = reference_start;
            obs_end = reference_end;
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
        let dcf = input.day_count.year_fraction(
            dcf_start,
            dcf_end,
            interval_context(day_count_context, dcf_end, input.accrual_end),
        )?;
        if !dcf.is_finite() || dcf <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Overnight observation has non-positive day-count fraction for \
                 {dcf_start} -> {dcf_end}"
            )));
        }
        let rate_accrual_year_fraction = if (dcf_start, dcf_end) == (obs_start, obs_end) {
            dcf
        } else {
            let yf = input.day_count.year_fraction(
                obs_start,
                obs_end,
                interval_context(day_count_context, obs_end, input.accrual_end),
            )?;
            if !yf.is_finite() || yf <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Overnight rate interval has non-positive day-count fraction for \
                     {obs_start} -> {obs_end}"
                )));
            }
            yf
        };

        // Valuation is at start of day, before the same-day fixing is published.
        let (rate, rate_sensitivity) = if obs_start < input.as_of {
            (
                finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                    input.fixings,
                    input.fixing_id,
                    obs_start,
                )?,
                0.0,
            )
        } else {
            (
                input
                    .curve
                    .projected_rate(obs_start, obs_end, rate_accrual_year_fraction)?,
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
        if let Some(ref mut factors) = observation_factors {
            factors.push((
                obs_start,
                obs_end,
                rate,
                rate_accrual_year_fraction,
                dcf,
                factor,
                rate_sensitivity,
            ));
        }
        fixing_date = Some(fixing_date.map_or(obs_start, |current: Date| current.max(obs_start)));
        date = step_end;
    }

    let observation_exposures = if let Some(observation_factors) = observation_factors {
        let mut prefixes = Vec::with_capacity(observation_factors.len() + 1);
        prefixes.push(1.0);
        for observation in &observation_factors {
            prefixes.push(prefixes.last().copied().unwrap_or(1.0) * observation.5);
        }
        let mut suffixes = vec![1.0; observation_factors.len() + 1];
        for index in (0..observation_factors.len()).rev() {
            suffixes[index] = suffixes[index + 1] * observation_factors[index].5;
        }
        observation_factors
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
            .collect()
    } else {
        Vec::new()
    };

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
mod tests;
