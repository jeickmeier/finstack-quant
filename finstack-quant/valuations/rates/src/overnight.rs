//! Rates-specific overnight helpers (arithmetic averaging and final fixing date).
//!
//! Compounded overnight coupon projection lives in
//! [`finstack_quant_valuations_core::instruments::pricing::overnight`]; arithmetic
//! daily averaging (used by Fed Funds / SOFR futures) and cap/floor final
//! fixing-date determination sit here alongside the rates instruments.

use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_core::Result;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext, HolidayCalendar};
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_valuations_core::instruments::pricing::overnight::{
    OvernightObservationExposure, OvernightProjectionCurve, interval_context,
};

/// Inputs for projecting an arithmetic-average overnight rate.
///
/// Used by futures on arithmetic daily overnight benchmarks such as 30-day
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
    /// Whether `accrual_end` is the instrument's contractual termination date;
    /// `None` when the caller has no schedule fact, which 30E/360 ISDA rejects.
    /// Only an interval ending on `accrual_end` inherits this status.
    pub end_is_termination_date: Option<bool>,
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

/// Last observation date that contractually determines an overnight coupon.
///
/// # Arguments
///
/// * `start` - Contractual accrual start date of the coupon period.
/// * `end` - Contractual accrual end date of the coupon period (`end > start`).
/// * `compounding` - Compounded overnight convention supplying lookback,
///   observation-shift, or rate-cutoff day counts.
/// * `calendar` - Fixing holiday calendar used to step business days.
///
/// # Errors
///
/// Returns `Error::Validation` when `end <= start`, when `compounding` is not
/// a compounded overnight method, or when shift/cutoff day counts overflow.
pub(crate) fn final_overnight_fixing_date(
    start: Date,
    end: Date,
    compounding: &FloatingLegCompounding,
    calendar: &dyn HolidayCalendar,
) -> Result<Date> {
    if end <= start {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Overnight identity has a non-positive observation window {start} -> {end}"
        )));
    }
    match compounding {
        FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days } if *cutoff_days > 0 => {
            let days = i32::try_from(*cutoff_days)
                .ok()
                .and_then(|d| d.checked_add(1))
                .ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "overnight rate cutoff {cutoff_days} overflows i32"
                    ))
                })?;
            end.add_business_days(-days, calendar)
        }
        FloatingLegCompounding::CompoundedInArrears { lookback_days }
        | FloatingLegCompounding::CompoundedWithObservationShift {
            shift_days: lookback_days,
        } => {
            let shift = i32::try_from(*lookback_days).map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "overnight compounding day count {lookback_days} exceeds i32::MAX"
                ))
            })?;
            end.add_business_days(-1, calendar)?
                .add_business_days(-shift, calendar)
        }
        FloatingLegCompounding::CompoundedWithRateCutoff { .. } => {
            end.add_business_days(-1, calendar)
        }
        FloatingLegCompounding::Simple | FloatingLegCompounding::SimpleAverage => {
            Err(finstack_quant_core::Error::Validation(
                "Overnight compounding requires a compounded overnight method".into(),
            ))
        }
    }
}

/// Project an arithmetic average of overnight fixings and its forward sensitivity.
///
/// The average is `sum(r_i * d_i) / sum(d_i)` on the supplied day-count basis.
/// Observations strictly before `as_of` must be present in the historical fixing
/// series. The same-day observation remains projected because valuation occurs at
/// the start of day, before publication.
///
/// # Arguments
///
/// * `input` - Curve/fixing sources, averaging window bounds, day-count convention,
///   and holiday calendar.
pub(crate) fn project_arithmetic_overnight_rate(
    input: OvernightArithmeticProjectionInput<'_>,
) -> Result<OvernightArithmeticProjection> {
    let day_count_context = DayCountContext {
        calendar: Some(input.fixing_calendar),
        coupon_period: Some((input.accrual_start, input.accrual_end)),
        end_is_termination_date: input.end_is_termination_date,
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
    let mut observation_date = if input.fixing_calendar.is_business_day(date)? {
        date
    } else {
        date.add_business_days(-1, input.fixing_calendar)?
    };
    while date < input.accrual_end {
        let next_observation_date = observation_date.add_business_days(1, input.fixing_calendar)?;
        let step_end = next_observation_date.min(input.accrual_end);
        let accrual = input.day_count.year_fraction(
            date,
            step_end,
            interval_context(day_count_context, step_end, input.accrual_end),
        )?;
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
                )?,
                0.0,
            )
        } else {
            let rate_accrual = if (date, step_end) == (observation_date, next_observation_date) {
                accrual
            } else {
                input.day_count.year_fraction(
                    observation_date,
                    next_observation_date,
                    interval_context(day_count_context, next_observation_date, input.accrual_end),
                )?
            };
            (
                input.curve.projected_rate(
                    observation_date,
                    next_observation_date,
                    rate_accrual,
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

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::calendar_by_id_strict;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use time::macros::date;

    #[test]
    fn final_overnight_fixing_date_validates_window_and_cutoff_overflow() {
        let usny = calendar_by_id_strict("usny").expect("usny calendar");
        let start = date!(2025 - 01 - 15);
        let err = final_overnight_fixing_date(
            start,
            start,
            &FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 2 },
            usny,
        )
        .expect_err("reversed/empty window must fail before cutoff branch");
        assert!(err.to_string().contains("non-positive observation window"));

        let overflow_err = final_overnight_fixing_date(
            start,
            date!(2025 - 04 - 15),
            &FloatingLegCompounding::CompoundedWithRateCutoff {
                cutoff_days: i32::MAX as u32,
            },
            usny,
        )
        .expect_err("i32::MAX cutoff_days + 1 must report overflow");
        assert!(overflow_err.to_string().contains("overflows"));
    }

    #[test]
    fn arithmetic_overnight_projection_matches_flat_curve_rate() {
        let usny = calendar_by_id_strict("usny").expect("usny calendar");
        let as_of = date!(2025 - 01 - 02);
        let start = date!(2025 - 01 - 02);
        let end = date!(2025 - 02 - 03);
        let fwd = ForwardCurve::flat("USD-SOFR-OIS", as_of, 0.045, DayCount::Act360).expect("fwd");
        let projected = project_arithmetic_overnight_rate(OvernightArithmeticProjectionInput {
            curve: OvernightProjectionCurve::Forward(&fwd),
            fixings: None,
            fixing_id: "USD-SOFR-OIS",
            as_of,
            accrual_start: start,
            accrual_end: end,
            end_is_termination_date: None,
            day_count: DayCount::Act360,
            fixing_calendar: usny,
        })
        .expect("arithmetic projection");
        assert!((projected.rate - 0.045).abs() < 1e-4);
        assert!((projected.parallel_forward_sensitivity - 1.0).abs() < 1e-12);
    }
}
