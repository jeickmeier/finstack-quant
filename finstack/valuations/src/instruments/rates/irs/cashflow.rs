//! Cashflow construction helpers for interest rate swaps.
//!
//! This module centralizes cashflow schedule generation for `InterestRateSwap`:
//! - Fixed-leg and floating-leg `CashFlowSchedule` builders
//! - Combined signed schedules with `CFKind` metadata used by `CashflowProvider`
//!
//! Pricing logic (discounting, forwards, PV) lives in `pricer.rs` and consumes
//! these schedules where appropriate.
//!
//! # Important: Accrual-End Dates vs Payment Dates
//!
//! Fixed leg schedules delegate to the cashflow builder, whose emitted
//! `CashFlow::date` already includes business-day adjustment and payment lag.
//! Compounded floating-leg schedules in this module emit accrual-end dates;
//! the pricer applies floating-leg payment lag when discounting those flows.

use finstack_core::dates::CalendarRegistry;
use finstack_core::dates::{BusinessDayConvention, Date, DateExt, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::ScalarTimeSeries;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::money::Money;
use finstack_core::Result;
use rust_decimal::Decimal;

use crate::cashflow::builder::{
    periods::{build_periods, BuildPeriodsParams, SchedulePeriod},
    CashFlowSchedule, FloatingCouponSpec, FloatingRateSpec, Notional,
};
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::rates::irs::{FloatingLegCompounding, InterestRateSwap, PayReceive};

fn default_rfr_calendar(currency: finstack_core::currency::Currency) -> Option<&'static str> {
    use finstack_core::currency::Currency;

    match currency {
        Currency::USD => Some("nyse"),
        Currency::EUR => Some("target"),
        Currency::GBP => Some("london"),
        Currency::JPY => Some("tokyo"),
        Currency::AUD => Some("sydney"),
        Currency::CAD => Some("toronto"),
        Currency::CHF => Some("zurich"),
        Currency::SEK => Some("stockholm"),
        Currency::NOK => Some("oslo"),
        Currency::DKK => Some("copenhagen"),
        Currency::NZD => Some("wellington"),
        _ => None,
    }
}

fn compounded_total_shift_days(compounding: FloatingLegCompounding) -> i32 {
    match compounding {
        FloatingLegCompounding::CompoundedInArrears {
            lookback_days,
            observation_shift,
        } => -lookback_days + observation_shift.unwrap_or(0),
        FloatingLegCompounding::CompoundedWithObservationShift { shift_days } => -shift_days,
        FloatingLegCompounding::CompoundedWithRateCutoff { .. } => 0,
        FloatingLegCompounding::Simple => 0,
    }
}

fn uses_observation_shift_dcf(compounding: FloatingLegCompounding) -> bool {
    matches!(
        compounding,
        FloatingLegCompounding::CompoundedWithObservationShift { .. }
    )
}

fn rate_cutoff_days(compounding: FloatingLegCompounding) -> Option<i32> {
    match compounding {
        FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days } if cutoff_days > 0 => {
            Some(cutoff_days)
        }
        _ => None,
    }
}

fn shift_business_or_weekdays(
    date: Date,
    days: i32,
    cal: Option<&dyn finstack_core::dates::HolidayCalendar>,
) -> Result<Date> {
    if let Some(cal) = cal {
        date.add_business_days(days, cal)
    } else {
        Ok(date.add_weekdays(days))
    }
}

fn is_irregular_fixed_period(
    period: &SchedulePeriod,
    fixed: &crate::instruments::common_impl::parameters::legs::FixedLegSpec,
    cal: &dyn finstack_core::dates::HolidayCalendar,
    adjust_accrual_dates: bool,
) -> Result<bool> {
    let mut expected_regular_end = fixed.frequency.add_to_date(
        period.accrual_start,
        None,
        BusinessDayConvention::Unadjusted,
    )?;
    if fixed.end_of_month {
        expected_regular_end = expected_regular_end.end_of_month();
    }
    if adjust_accrual_dates {
        expected_regular_end = finstack_core::dates::adjust(expected_regular_end, fixed.bdc, cal)?;
    }

    let dc_ctx = DayCountContext {
        calendar: Some(cal),
        frequency: Some(fixed.frequency),
        bus_basis: None,
        coupon_period: None,
    };
    let expected_regular_accrual =
        fixed
            .day_count
            .year_fraction(period.accrual_start, expected_regular_end, dc_ctx)?;

    let date_matches = period.accrual_end == expected_regular_end;
    let accrual_matches =
        (period.accrual_year_fraction - expected_regular_accrual).abs() <= 1.0e-10;

    Ok(!(date_matches || accrual_matches))
}

fn adjust_accrual_dates(irs: &InterestRateSwap) -> bool {
    matches!(
        irs.attributes.get_meta("schedule_adjust"),
        Some("acc_and_pay_dates")
    ) || matches!(
        irs.attributes.get_meta("adjust_accrual_dates"),
        Some("true")
    )
}

struct OvernightProjectionInputs<'a> {
    proj: Option<&'a ForwardCurve>,
    disc_fallback: Option<&'a DiscountCurve>,
    fixings: Option<&'a ScalarTimeSeries>,
    projection_base_date: Date,
    float: &'a crate::instruments::common_impl::parameters::legs::FloatLegSpec,
}

fn builder_overnight_method(
    compounding: FloatingLegCompounding,
) -> Result<Option<crate::cashflow::builder::OvernightCompoundingMethod>> {
    use crate::cashflow::builder::OvernightCompoundingMethod;

    Ok(match compounding {
        FloatingLegCompounding::Simple => None,
        FloatingLegCompounding::CompoundedInArrears {
            lookback_days,
            observation_shift,
        } => {
            if observation_shift.unwrap_or(0) == 0 {
                if lookback_days == 0 {
                    Some(OvernightCompoundingMethod::CompoundedInArrears)
                } else {
                    Some(OvernightCompoundingMethod::CompoundedWithLookback {
                        lookback_days: lookback_days as u32,
                    })
                }
            } else if lookback_days == 0 {
                Some(OvernightCompoundingMethod::CompoundedWithObservationShift {
                    shift_days: observation_shift.unwrap_or(0) as u32,
                })
            } else {
                // The canonical-schedule builder cannot model the hybrid
                // lookback + observation-shift convention. Silently degrading
                // to lookback-only would drop the observation shift and
                // diverge from the in-module compounding loop, which handles
                // the combined `total_shift` correctly. Reject explicitly
                // rather than mispricing.
                return Err(finstack_core::Error::Validation(format!(
                    "Compounded-in-arrears with both a lookback ({} days) and an \
                     observation shift ({} days) is not supported on the \
                     canonical-schedule pricing path. Use a lookback-only or \
                     observation-shift-only convention.",
                    lookback_days,
                    observation_shift.unwrap_or(0),
                )));
            }
        }
        FloatingLegCompounding::CompoundedWithObservationShift { shift_days } => {
            Some(OvernightCompoundingMethod::CompoundedWithObservationShift {
                shift_days: shift_days as u32,
            })
        }
        FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days } => {
            Some(OvernightCompoundingMethod::CompoundedWithLockout {
                lockout_days: cutoff_days as u32,
            })
        }
    })
}

fn resolve_compounded_fixing_calendar(
    irs: &InterestRateSwap,
) -> Result<Option<&'static dyn finstack_core::dates::HolidayCalendar>> {
    let float = irs.resolved_float_leg()?;
    let calendar_id = float
        .fixing_calendar_id
        .as_deref()
        .or(float.calendar_id.as_deref());

    if let Some(id) = calendar_id {
        return Ok(Some(
            CalendarRegistry::global().resolve_str(id).ok_or_else(|| {
                finstack_core::Error::Validation(format!(
                    "Fixing calendar '{}' not found in registry for compounded RFR swap '{}'. \
                     Load the calendar or remove fixing_calendar_id to use weekday stepping.",
                    id,
                    irs.id.as_str()
                ))
            })?,
        ));
    }

    Ok(default_rfr_calendar(irs.notional.currency())
        .and_then(|id| CalendarRegistry::global().resolve_str(id)))
}

fn projected_overnight_rate(
    obs_start: Date,
    obs_end: Date,
    dcf: f64,
    inputs: &OvernightProjectionInputs<'_>,
) -> Result<f64> {
    if obs_start < inputs.projection_base_date {
        return finstack_core::market_data::fixings::require_fixing_value(
            inputs.fixings,
            inputs.float.forward_curve_id.as_str(),
            obs_start,
            inputs.projection_base_date,
        );
    }

    if let Some(proj) = inputs.proj {
        let t0 = if obs_start <= proj.base_date() {
            0.0
        } else {
            proj.day_count().year_fraction(
                proj.base_date(),
                obs_start,
                DayCountContext::default(),
            )?
        };
        let t1 = if obs_end <= proj.base_date() {
            0.0
        } else {
            proj.day_count()
                .year_fraction(proj.base_date(), obs_end, DayCountContext::default())?
        };
        // Distinguish a genuine forward period from a zero-length one using an
        // *economic* tolerance, not raw machine epsilon. `f64::EPSILON`
        // (~2.2e-16 yr) is meaningless as a time threshold — any real period
        // exceeds it, and a near-degenerate sub-second period that should
        // collapse to a spot rate would not. `MIN_FORWARD_PERIOD_YEARS`
        // (~one second) is the smallest period worth projecting as a forward.
        const MIN_FORWARD_PERIOD_YEARS: f64 = 1.0 / (365.0 * 24.0 * 60.0 * 60.0);
        return Ok(if (t1 - t0).abs() > MIN_FORWARD_PERIOD_YEARS {
            proj.rate_period(t0, t1)
        } else {
            proj.rate(t0)
        });
    }

    if let Some(disc) = inputs.disc_fallback {
        let df_between = disc.df_between_dates(obs_start, obs_end)?;
        if !df_between.is_finite() || df_between <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "Invalid discount factor between observation dates ({} -> {}): df={:.3e}",
                obs_start, obs_end, df_between
            )));
        }
        const MIN_DCF_THRESHOLD: f64 = 1e-8;
        if dcf < MIN_DCF_THRESHOLD {
            return Err(finstack_core::Error::Validation(format!(
                "Day-count fraction {:.2e} is below minimum threshold ({:.0e}). \
                 This may indicate calendar misconfiguration causing same-day observations \
                 or invalid date ordering ({} -> {}).",
                dcf, MIN_DCF_THRESHOLD, obs_start, obs_end
            )));
        }
        let comp = 1.0 / df_between;
        return Ok((comp - 1.0) / dcf);
    }

    Err(finstack_core::Error::Input(
        finstack_core::InputError::NotFound {
            id: format!(
                "forward curve '{}' not found for reset date {} (overnight compounding)",
                inputs.float.forward_curve_id.as_str(),
                obs_start
            ),
        },
    ))
}

pub(crate) fn projected_compounded_float_leg_schedule(
    irs: &InterestRateSwap,
    disc: &DiscountCurve,
    proj: Option<&ForwardCurve>,
    as_of: Date,
    fixings: Option<&ScalarTimeSeries>,
) -> Result<CashFlowSchedule> {
    use finstack_core::cashflow::{CFKind, CashFlow};

    let float = irs.resolved_float_leg()?;
    let periods = build_periods(BuildPeriodsParams {
        start: float.start,
        end: float.end,
        frequency: float.frequency,
        stub: float.stub,
        bdc: float.bdc,
        calendar_id: float
            .calendar_id
            .as_deref()
            .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
        end_of_month: float.end_of_month,
        day_count: float.day_count,
        payment_lag_days: float.payment_lag_days,
        reset_lag_days: None,
        adjust_accrual_dates: adjust_accrual_dates(irs),
    })?;
    if periods.is_empty() {
        return Ok(crate::cashflow::traits::schedule_from_classified_flows(
            Vec::new(),
            float.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(irs.notional),
                ..Default::default()
            },
        ));
    }

    let cal = resolve_compounded_fixing_calendar(irs)?;
    let total_shift = compounded_total_shift_days(float.compounding.clone());
    let shift_dcf = uses_observation_shift_dcf(float.compounding.clone());
    let cutoff_days = rate_cutoff_days(float.compounding.clone());
    let disc_fallback = if proj.is_none() { Some(disc) } else { None };
    let projection = OvernightProjectionInputs {
        proj,
        disc_fallback,
        fixings,
        projection_base_date: proj
            .map(ForwardCurve::base_date)
            .unwrap_or_else(|| disc.base_date()),
        float: &float,
    };

    let mut flows = Vec::with_capacity(periods.len());
    for period in periods {
        if period.payment_date <= as_of {
            continue;
        }

        // Adjust accrual window to business days for RFR compounding. The schedule
        // builder intentionally preserves unadjusted roll dates (for bond-style
        // unadjusted-accrual conventions), but overnight-rate compounding requires a
        // business-day window — otherwise the inner loop can step onto weekends/holidays
        // and the observation-shift back-roll can collapse two adjacent steps onto the
        // same date (see `seek_business_day` semantics from a non-business day).
        let (accrual_start, accrual_end) = if let Some(cal) = cal {
            use finstack_core::dates::adjust;
            (
                adjust(period.accrual_start, float.bdc, cal)?,
                adjust(period.accrual_end, float.bdc, cal)?,
            )
        } else {
            (period.accrual_start, period.accrual_end)
        };
        if accrual_end <= accrual_start {
            continue;
        }
        // A single-curve OIS leg can be priced via the plain compounded DF
        // identity ∏(1 + r_i·dcf_i) = DF(start)/DF(end) for unseasoned future
        // periods — but only when that identity is self-consistent with the
        // convention the discount curve was *calibrated* under.
        //
        // W-13 narrowing: a rate cut-off freezes the last `cutoff_days`
        // overnight rates. Whether the `1/DF` fast path is exact then depends
        // entirely on the curve's own calibration convention:
        //
        //  * Curve calibrated WITHOUT the matching cut-off (registry-default
        //    `CompoundedInArrears`): the curve's forwards encode no lockout, so
        //    `1/DF` would price the leg as if there were no cut-off. The daily
        //    loop must explicitly apply the lockout — fast path DISABLED.
        //    (W-13's genuine bug; kept fixed.)
        //
        //  * Curve calibrated WITH the matching cut-off (e.g. Bloomberg SWPM
        //    SOFR, `ois_compounding: CompoundedWithRateCutoff{cutoff_days}`):
        //    the curve IS its own cut-off-consistent forward, so `1/DF` is
        //    exact and self-consistent — re-applying the lockout in a daily
        //    loop would fail to telescope back to the closed form and break
        //    par-rate self-consistency. Fast path RE-ENABLED.
        //
        // The curve's calibration convention is recovered from
        // `DiscountCurve::calibration_ois_cutoff_days`, which the bootstrap
        // stamps onto every solver and final curve. Because the bootstrap-
        // internal swaps reprice against curves carrying the same stamp, the
        // calibration and downstream pricing always agree on this branch.
        let cutoff_matches_calibration = match cutoff_days {
            None => true,
            Some(days) => disc
                .calibration_ois_cutoff_days()
                .is_some_and(|calibrated_days| calibrated_days == days),
        };
        let allow_fast_path = as_of <= accrual_start
            && total_shift == 0
            && cutoff_matches_calibration
            && proj.is_none_or(|p| disc.id() == p.id());
        let compound_factor = if allow_fast_path {
            1.0 / crate::instruments::common_impl::pricing::swap_legs::robust_relative_df(
                disc,
                accrual_start,
                accrual_end,
            )?
        } else {
            let cutoff = if let Some(days) = cutoff_days {
                let lockout_start = shift_business_or_weekdays(accrual_end, -days, cal)?;
                let lockout_ref_start = shift_business_or_weekdays(lockout_start, -1, cal)?;
                Some((lockout_start, lockout_ref_start, lockout_start))
            } else {
                None
            };
            let mut acc = 1.0;
            let mut d = accrual_start;
            while d < accrual_end {
                let next_d = if let Some(cal) = cal {
                    d.add_business_days(1, cal)?
                } else {
                    d.add_weekdays(1)
                };
                let step_end = if next_d > accrual_end {
                    accrual_end
                } else {
                    next_d
                };

                let mut obs_start = if total_shift == 0 {
                    d
                } else if let Some(cal) = cal {
                    d.add_business_days(total_shift, cal)?
                } else {
                    d.add_weekdays(total_shift)
                };
                let mut obs_end = if total_shift == 0 {
                    step_end
                } else if let Some(cal) = cal {
                    step_end.add_business_days(total_shift, cal)?
                } else {
                    step_end.add_weekdays(total_shift)
                };
                if let Some((lockout_start, lockout_ref_start, lockout_ref_end)) = cutoff {
                    if d >= lockout_start {
                        obs_start = lockout_ref_start;
                        obs_end = lockout_ref_end;
                    }
                }

                let (dcf_start, dcf_end) = if shift_dcf {
                    (obs_start, obs_end)
                } else {
                    (d, step_end)
                };
                let dcf = float.day_count.year_fraction(
                    dcf_start,
                    dcf_end,
                    DayCountContext::default(),
                )?;

                if obs_end <= obs_start {
                    return Err(finstack_core::Error::Validation(format!(
                        "Invalid observation period after applying shift: obs_start={}, obs_end={}, \
                         total_shift={} days. This may indicate lookback exceeds the daily step size \
                         or an invalid observation_shift configuration.",
                        obs_start, obs_end, total_shift
                    )));
                }

                let r = projected_overnight_rate(obs_start, obs_end, dcf, &projection)?;
                acc *= 1.0 + r * dcf;
                d = step_end;
            }
            acc
        };

        let spread_bp = decimal_to_f64(float.spread_bp, "float leg spread_bp")?;
        let interest = irs.notional.amount() * (compound_factor - 1.0);
        let spread_contrib = irs.notional.amount()
            * spread_bp
            * crate::constants::ONE_BASIS_POINT
            * period.accrual_year_fraction;
        let coupon_amount = interest + spread_contrib;
        let all_in_rate = if period.accrual_year_fraction.abs() > f64::EPSILON {
            (compound_factor - 1.0) / period.accrual_year_fraction
                + spread_bp * crate::constants::ONE_BASIS_POINT
        } else {
            spread_bp * crate::constants::ONE_BASIS_POINT
        };
        flows.push(CashFlow {
            date: period.payment_date,
            reset_date: None,
            amount: Money::new(coupon_amount, irs.notional.currency()),
            kind: CFKind::FloatReset,
            accrual_factor: period.accrual_year_fraction,
            rate: Some(all_in_rate),
        });
    }

    Ok(crate::cashflow::traits::schedule_from_classified_flows(
        flows,
        float.day_count,
        crate::cashflow::traits::ScheduleBuildOpts {
            notional_hint: Some(irs.notional),
            ..Default::default()
        },
    ))
}

/// Build an unsigned fixed-leg cashflow schedule for an IRS.
///
/// The resulting schedule has positive notionals and coupon amounts; caller is
/// responsible for applying `PayReceive` sign conventions.
///
/// # Arguments
///
/// * `irs` - The interest rate swap for which to build the schedule
///
/// # Returns
///
/// A `CashFlowSchedule` containing all fixed leg cashflows with `CFKind::Fixed`
/// or `CFKind::Stub` classifications. Amounts are unsigned (positive).
///
/// # Errors
///
/// Returns an error if the cashflow schedule cannot be built (e.g., invalid
/// date ranges or calendar lookups fail).
///
/// # Examples
///
/// ```ignore
/// use finstack_valuations::instruments::rates::irs::{InterestRateSwap, cashflow};
///
/// # fn example() -> finstack_core::Result<()> {
/// let irs = InterestRateSwap::example_standard()?;
/// let schedule = cashflow::fixed_leg_schedule(&irs)?;
///
/// // Schedule contains fixed coupon flows
/// assert!(!schedule.flows.is_empty());
/// # Ok(())
/// # }
/// ```
pub(crate) fn fixed_leg_schedule(irs: &InterestRateSwap) -> Result<CashFlowSchedule> {
    use finstack_core::cashflow::{CFKind, CashFlow};

    let fixed = irs.resolved_fixed_leg()?;
    let calendar_id = fixed
        .calendar_id
        .as_deref()
        .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID);
    let adjust_accrual_dates = adjust_accrual_dates(irs);
    let periods = build_periods(BuildPeriodsParams {
        start: fixed.start,
        end: fixed.end,
        frequency: fixed.frequency,
        stub: fixed.stub,
        bdc: fixed.bdc,
        calendar_id,
        end_of_month: fixed.end_of_month,
        day_count: fixed.day_count,
        payment_lag_days: fixed.payment_lag_days,
        reset_lag_days: None,
        adjust_accrual_dates,
    })?;
    let cal = crate::cashflow::builder::calendar::resolve_calendar_strict(calendar_id)?;
    let rate = decimal_to_f64(fixed.rate, "fixed leg rate")?;
    let flows = periods
        .into_iter()
        .map(|period| -> Result<CashFlow> {
            let kind = if is_irregular_fixed_period(&period, &fixed, cal, adjust_accrual_dates)? {
                CFKind::Stub
            } else {
                CFKind::Fixed
            };
            Ok(CashFlow {
                date: period.payment_date,
                reset_date: None,
                amount: Money::new(
                    irs.notional.amount() * rate * period.accrual_year_fraction,
                    irs.notional.currency(),
                ),
                kind,
                accrual_factor: period.accrual_year_fraction,
                rate: Some(rate),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(crate::cashflow::traits::schedule_from_classified_flows(
        flows,
        fixed.day_count,
        crate::cashflow::traits::ScheduleBuildOpts {
            notional_hint: Some(irs.notional),
            ..Default::default()
        },
    ))
}

/// Build an unsigned floating-leg cashflow schedule for an IRS with market curves.
///
/// When curves are provided, the schedule includes actual forward rate projections.
/// Without curves, only the spread contribution is used (useful for structure-only schedules).
///
/// # Arguments
///
/// * `irs` - The interest rate swap for which to build the schedule
/// * `curves` - Optional market context containing forward curves
///
/// # Returns
///
/// A `CashFlowSchedule` containing all floating leg cashflows with `CFKind::FloatReset`
/// classifications.
pub(crate) fn float_leg_schedule_with_curves(
    irs: &InterestRateSwap,
    curves: Option<&MarketContext>,
) -> Result<CashFlowSchedule> {
    float_leg_schedule_with_curves_as_of(irs, curves, None)
}

pub(crate) fn float_leg_schedule_with_curves_as_of(
    irs: &InterestRateSwap,
    curves: Option<&MarketContext>,
    as_of: Option<Date>,
) -> Result<CashFlowSchedule> {
    let float = irs.resolved_float_leg()?;
    if matches!(
        float.compounding,
        FloatingLegCompounding::CompoundedInArrears { .. }
            | FloatingLegCompounding::CompoundedWithObservationShift { .. }
            | FloatingLegCompounding::CompoundedWithRateCutoff { .. }
    ) {
        if let Some(market) = curves {
            let disc = market.get_discount(irs.fixed.discount_curve_id.as_ref())?;
            let proj = if irs.is_single_curve_ois() {
                market.get_forward(float.forward_curve_id.as_str()).ok()
            } else {
                Some(market.get_forward(float.forward_curve_id.as_str())?)
            };
            let fixings = finstack_core::market_data::fixings::get_fixing_series(
                market,
                float.forward_curve_id.as_str(),
            )
            .ok();
            let valuation_date = as_of.unwrap_or_else(|| {
                proj.as_ref()
                    .map(|c| c.base_date())
                    .unwrap_or_else(|| disc.base_date())
            });
            return projected_compounded_float_leg_schedule(
                irs,
                disc.as_ref(),
                proj.as_deref(),
                valuation_date,
                fixings,
            );
        }
    }

    let mut float_b = CashFlowSchedule::builder();
    let _ = float_b
        .principal(irs.notional, float.start, float.end)
        .floating_cf(FloatingCouponSpec {
            rate_spec: FloatingRateSpec {
                index_id: float.forward_curve_id.to_owned(),
                spread_bp: float.spread_bp,
                gearing: Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_cap_bp: None,
                all_in_floor_bp: None,
                index_cap_bp: None,
                reset_freq: float.frequency,
                reset_lag_days: float.reset_lag_days,
                dc: float.day_count,
                bdc: float.bdc,
                calendar_id: float
                    .calendar_id
                    .clone()
                    .unwrap_or_else(|| "weekends_only".to_string()),
                fixing_calendar_id: float.fixing_calendar_id.clone(),
                end_of_month: float.end_of_month,
                payment_lag_days: float.payment_lag_days,
                overnight_compounding: builder_overnight_method(float.compounding.clone())?,
                overnight_basis: None,
                fallback: if curves.is_some() {
                    crate::cashflow::builder::FloatingRateFallback::Error
                } else {
                    crate::cashflow::builder::FloatingRateFallback::SpreadOnly
                },
            },
            coupon_type: crate::cashflow::builder::CouponType::Cash,
            freq: float.frequency,
            stub: float.stub,
        });
    let mut sched = float_b.build_with_curves(curves)?;
    // IRS do not exchange notionals; return coupon-only schedule as documented.
    sched
        .flows
        .retain(|cf| cf.kind == crate::cashflow::primitives::CFKind::FloatReset);
    Ok(sched)
}

/// Build a full, signed cashflow schedule with `CFKind` metadata for an IRS with market curves.
///
/// When curves are provided, floating leg amounts include forward rate projections.
///
/// # Arguments
///
/// * `irs` - The interest rate swap for which to build the full schedule
/// * `curves` - Optional market context containing forward curves
#[cfg(test)]
pub(crate) fn full_signed_schedule_with_curves(
    irs: &InterestRateSwap,
    curves: Option<&MarketContext>,
) -> Result<CashFlowSchedule> {
    full_signed_schedule_with_curves_as_of(irs, curves, None)
}

pub(crate) fn full_signed_schedule_with_curves_as_of(
    irs: &InterestRateSwap,
    curves: Option<&MarketContext>,
    as_of: Option<Date>,
) -> Result<CashFlowSchedule> {
    use finstack_core::cashflow::{CFKind, CashFlow};

    let fixed_sched = fixed_leg_schedule(irs)?;
    let float_sched = match as_of {
        Some(as_of_date) => float_leg_schedule_with_curves_as_of(irs, curves, Some(as_of_date))?,
        None => float_leg_schedule_with_curves(irs, curves)?,
    };

    // Combine flows from both legs with proper CFKind classification
    let mut all_flows: Vec<CashFlow> =
        Vec::with_capacity(fixed_sched.flows.len() + float_sched.flows.len());

    // Add fixed leg flows
    for cf in fixed_sched.flows {
        if cf.kind == CFKind::Fixed || cf.kind == CFKind::Stub {
            let amt = match irs.side {
                PayReceive::ReceiveFixed => cf.amount,
                PayReceive::PayFixed => cf.amount * -1.0,
            };
            all_flows.push(CashFlow {
                date: cf.date,
                reset_date: cf.reset_date,
                amount: amt,
                kind: cf.kind, // Preserve precise CFKind
                accrual_factor: cf.accrual_factor,
                rate: cf.rate,
            });
        }
    }

    // Add floating leg flows
    for cf in float_sched.flows {
        if cf.kind == CFKind::FloatReset {
            let amt = match irs.side {
                PayReceive::ReceiveFixed => cf.amount * -1.0,
                PayReceive::PayFixed => cf.amount,
            };
            all_flows.push(CashFlow {
                date: cf.date,
                reset_date: cf.reset_date,
                amount: amt,
                kind: cf.kind, // Preserve precise CFKind
                accrual_factor: cf.accrual_factor,
                rate: cf.rate,
            });
        }
    }

    // Sort flows by date and CFKind priority
    all_flows.sort_by(|a, b| {
        use core::cmp::Ordering;
        match a.date.cmp(&b.date) {
            Ordering::Equal => {
                // Use kind ranking logic from cashflow builder
                let rank_a = match a.kind {
                    CFKind::Fixed | CFKind::Stub | CFKind::FloatReset => 0,
                    CFKind::Fee => 1,
                    CFKind::Amortization => 2,
                    CFKind::PIK => 3,
                    CFKind::Notional => 4,
                    _ => 5,
                };
                let rank_b = match b.kind {
                    CFKind::Fixed | CFKind::Stub | CFKind::FloatReset => 0,
                    CFKind::Fee => 1,
                    CFKind::Amortization => 2,
                    CFKind::PIK => 3,
                    CFKind::Notional => 4,
                    _ => 5,
                };
                rank_a.cmp(&rank_b)
            }
            other => other,
        }
    });

    if let Some(as_of) = as_of {
        all_flows.retain(|flow| flow.date > as_of);
    }

    // Create notional spec for swap (notional doesn't amortize)
    let notional = Notional::par(irs.notional.amount(), irs.notional.currency());

    Ok(CashFlowSchedule {
        flows: all_flows,
        notional,
        day_count: irs.fixed.day_count, // Use fixed leg day count as representative
        meta: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::api::engine;
    use crate::calibration::api::schema::CalibrationEnvelope;
    use finstack_core::cashflow::CFKind;
    use finstack_core::market_data::context::MarketContext;

    #[derive(serde::Deserialize)]
    struct GoldenFixtureEnvelope {
        inputs: GoldenFixtureInputs,
    }

    #[derive(serde::Deserialize)]
    struct GoldenFixtureInputs {
        valuation_date: String,
        instrument_json: serde_json::Value,
        market_envelope: CalibrationEnvelope,
        source_reference: GoldenSourceReference,
    }

    #[derive(serde::Deserialize)]
    struct GoldenSourceReference {
        bloomberg_reference: BloombergReference,
    }

    #[derive(serde::Deserialize)]
    struct BloombergReference {
        cashflows: Vec<BloombergCashflow>,
    }

    #[derive(serde::Deserialize)]
    struct BloombergCashflow {
        pay_date: String,
        payments_receive: f64,
        payments_pay: f64,
        net_payments: f64,
        discount: f64,
        pv: f64,
    }

    #[test]
    fn irs_leg_schedules_do_not_emit_notional_flows() {
        let irs = InterestRateSwap::example_standard().expect("example IRS");
        let fixed = fixed_leg_schedule(&irs).expect("fixed schedule");
        assert!(
            fixed
                .flows
                .iter()
                .all(|cf| cf.kind == CFKind::Fixed || cf.kind == CFKind::Stub),
            "fixed_leg_schedule should be coupon-only"
        );

        let float = float_leg_schedule_with_curves(&irs, None).expect("float schedule");
        assert!(
            float.flows.iter().all(|cf| cf.kind == CFKind::FloatReset),
            "float_leg_schedule should be coupon-only"
        );
    }

    #[test]
    fn rate_cutoff_maps_to_overnight_lockout() {
        let method = builder_overnight_method(FloatingLegCompounding::CompoundedWithRateCutoff {
            cutoff_days: 1,
        })
        .expect("rate cut-off is a supported canonical-schedule convention");

        assert_eq!(
            method,
            Some(
                crate::cashflow::builder::OvernightCompoundingMethod::CompoundedWithLockout {
                    lockout_days: 1
                }
            )
        );
    }

    /// W-14: The canonical-schedule path cannot model the hybrid
    /// lookback + observation-shift convention. It must error explicitly
    /// rather than silently degrading to lookback-only (which drops the
    /// observation shift and diverges from the in-module compounding loop).
    #[test]
    fn hybrid_lookback_and_observation_shift_errors() {
        let hybrid = FloatingLegCompounding::CompoundedInArrears {
            lookback_days: 2,
            observation_shift: Some(2),
        };
        let result = builder_overnight_method(hybrid);
        assert!(
            result.is_err(),
            "hybrid lookback + observation-shift must be rejected on the \
             canonical-schedule path, got {result:?}"
        );

        // Lookback-only and shift-only remain supported.
        assert!(
            builder_overnight_method(FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 2,
                observation_shift: None,
            })
            .is_ok()
        );
        assert!(
            builder_overnight_method(FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: Some(2),
            })
            .is_ok()
        );
    }

    #[test]
    fn fixed_leg_pv_uses_builder_payment_dates_once() {
        let fixture = load_bloomberg_fixture();
        let as_of = crate::pricer::parse_as_of_date(&fixture.inputs.valuation_date)
            .expect("fixture valuation date parses");
        let irs = load_fixture_irs(&fixture);
        let market = load_fixture_market(&fixture);
        let disc = market
            .get_discount(&irs.fixed.discount_curve_id)
            .expect("discount curve");
        let schedule = fixed_leg_schedule(&irs).expect("fixed schedule");
        let direct_pv: f64 = schedule
            .flows
            .iter()
            .map(|flow| {
                let df = crate::instruments::rates::irs::pricer::robust_relative_df(
                    &disc, as_of, flow.date,
                )
                .expect("discount factor");
                flow.amount.amount() * df
            })
            .sum();
        let priced_pv = irs.pv_fixed_leg(&disc, as_of).expect("fixed leg PV");

        assert!(
            (priced_pv - direct_pv).abs() < 1e-6,
            "fixed leg PV should discount builder-emitted payment dates exactly once: priced={priced_pv}, direct={direct_pv}"
        );
    }

    #[test]
    fn float_leg_pv_uses_schedule_payment_dates_once() {
        let fixture = load_bloomberg_fixture();
        let as_of = crate::pricer::parse_as_of_date(&fixture.inputs.valuation_date)
            .expect("fixture valuation date parses");
        let irs = load_fixture_irs(&fixture);
        let market = load_fixture_market(&fixture);
        let disc = market
            .get_discount(&irs.fixed.discount_curve_id)
            .expect("discount curve");
        let schedule = float_leg_schedule_with_curves_as_of(&irs, Some(&market), Some(as_of))
            .expect("float schedule");
        let direct_pv: f64 = schedule
            .flows
            .iter()
            .map(|flow| {
                let df = crate::instruments::rates::irs::pricer::robust_relative_df(
                    &disc, as_of, flow.date,
                )
                .expect("discount factor");
                flow.amount.amount() * df
            })
            .sum();
        let priced_pv = irs.pv_float_leg(&market, as_of).expect("float leg PV");

        assert!(
            (priced_pv - direct_pv).abs() < 1e-6,
            "float leg PV should discount schedule payment dates exactly once: priced={priced_pv}, direct={direct_pv}"
        );
    }

    #[test]
    fn write_bloomberg_schedule_diagnostic_csv() {
        let fixture = load_bloomberg_fixture();
        let as_of = crate::pricer::parse_as_of_date(&fixture.inputs.valuation_date)
            .expect("fixture valuation date parses");
        let irs = load_fixture_irs(&fixture);
        let market = load_fixture_market(&fixture);

        let fixed = fixed_leg_schedule(&irs).expect("fixed schedule");
        let float = float_leg_schedule_with_curves_as_of(&irs, Some(&market), Some(as_of))
            .expect("float schedule");
        let report_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/golden-reports/irs-schedule-diagnostics.csv");
        if let Some(parent) = report_path.parent() {
            std::fs::create_dir_all(parent).expect("create report dir");
        }

        let mut csv = String::from(
            "row,finstack_fixed_date,finstack_float_date,finstack_fixed_amount,finstack_float_amount,finstack_net_amount,bloomberg_pay_date,bloomberg_fixed_amount,bloomberg_float_amount,bloomberg_net_amount,bloomberg_discount,bloomberg_pv\n",
        );
        for (idx, bbg) in fixture
            .inputs
            .source_reference
            .bloomberg_reference
            .cashflows
            .iter()
            .enumerate()
        {
            let fixed_flow = &fixed.flows[idx];
            let float_flow = &float.flows[idx];
            csv.push_str(&format!(
                "{},{},{},{:.8},{:.8},{:.8},{},{:.8},{:.8},{:.8},{:.8},{:.8}\n",
                idx + 1,
                fixed_flow.date,
                float_flow.date,
                fixed_flow.amount.amount(),
                float_flow.amount.amount(),
                fixed_flow.amount.amount() - float_flow.amount.amount(),
                bbg.pay_date,
                coupon_only_fixed_amount(bbg),
                coupon_only_float_amount(bbg),
                bbg.net_payments,
                bbg.discount,
                bbg.pv,
            ));
        }

        std::fs::write(&report_path, csv).expect("write schedule diagnostic CSV");
        assert!(report_path.exists());
    }

    /// W-13: A single-curve OIS leg with a non-zero rate cut-off must NOT be
    /// priced via the plain compounded DF identity. The fast path freezes no
    /// rates, so on a steep curve it produces the same PV as a no-cut-off leg.
    /// After the fix, the rate cut-off is explicitly applied and the two legs
    /// must differ.
    #[test]
    fn single_curve_ois_rate_cutoff_differs_from_no_cutoff() {
        use crate::instruments::common_impl::parameters::legs::{FixedLegSpec, FloatLegSpec};
        use finstack_core::currency::Currency;
        use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
        use finstack_core::market_data::term_structures::DiscountCurve;
        use finstack_core::money::Money;
        use finstack_core::types::{CurveId, InstrumentId};
        use rust_decimal::Decimal;
        use time::Month;

        let date = |y: i32, m: u8, d: u8| {
            Date::from_calendar_date(y, Month::try_from(m).expect("month"), d).expect("date")
        };

        let as_of = date(2024, 1, 1);
        let start = date(2024, 1, 1);
        let end = date(2024, 4, 1);

        let disc_id = CurveId::new("USD-OIS");
        // Steep discount curve: short end ~2%, long end ~8% (steeply upward).
        let disc = DiscountCurve::builder(disc_id.clone())
            .base_date(as_of)
            .knots(vec![
                (0.0, 1.0),
                (0.10, (-0.02f64 * 0.10).exp()),
                (0.25, (-0.08f64 * 0.25).exp()),
                (1.0, (-0.08f64).exp()),
            ])
            .build()
            .expect("discount curve");

        let build_swap = |id: &str, compounding: FloatingLegCompounding| {
            InterestRateSwap::builder()
                .id(InstrumentId::new(id))
                .notional(Money::new(10_000_000.0, Currency::USD))
                .side(PayReceive::PayFixed)
                .fixed(FixedLegSpec {
                    discount_curve_id: disc_id.clone(),
                    rate: Decimal::ZERO,
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: StubKind::ShortFront,
                    start,
                    end,
                    par_method: None,
                    compounding_simple: true,
                    payment_lag_days: 0,
                    end_of_month: false,
                })
                .float(FloatLegSpec {
                    discount_curve_id: disc_id.clone(),
                    forward_curve_id: disc_id.clone(), // single-curve OIS
                    spread_bp: Decimal::ZERO,
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: StubKind::ShortFront,
                    reset_lag_days: 0,
                    fixing_calendar_id: None,
                    start,
                    end,
                    compounding,
                    payment_lag_days: 0,
                    end_of_month: false,
                })
                .build()
                .expect("swap")
        };

        let swap_no_cutoff = build_swap("OIS-NO-CUTOFF", FloatingLegCompounding::fedfunds());
        // 5-day rate cut-off freezes the last 5 overnight rates of each period.
        let swap_cutoff = build_swap("OIS-CUTOFF-5D", FloatingLegCompounding::rate_cutoff(5));

        // Single-curve: only the discount curve is loaded (proj is None).
        let ctx = MarketContext::new().insert(disc);

        let sched_no_cutoff =
            float_leg_schedule_with_curves_as_of(&swap_no_cutoff, Some(&ctx), Some(as_of))
                .expect("no-cutoff schedule");
        let sched_cutoff =
            float_leg_schedule_with_curves_as_of(&swap_cutoff, Some(&ctx), Some(as_of))
                .expect("cutoff schedule");

        let pv_no_cutoff: f64 = sched_no_cutoff
            .flows
            .iter()
            .map(|f| f.amount.amount())
            .sum();
        let pv_cutoff: f64 = sched_cutoff.flows.iter().map(|f| f.amount.amount()).sum();

        assert!(
            (pv_no_cutoff - pv_cutoff).abs() > 1.0,
            "single-curve OIS rate cut-off must change the floating coupon on a \
             steep curve: no_cutoff={pv_no_cutoff}, cutoff={pv_cutoff}"
        );
    }

    fn load_bloomberg_fixture() -> GoldenFixtureEnvelope {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/golden/data/pricing/irs/usd_sofr_5y_receive_fixed_swpm.json"
        )))
        .expect("fixture parses")
    }

    fn load_fixture_irs(fixture: &GoldenFixtureEnvelope) -> InterestRateSwap {
        crate::instruments::json_loader::InstrumentEnvelope::from_value(
            fixture.inputs.instrument_json.clone(),
        )
        .expect("fixture instrument loads")
        .as_any()
        .downcast_ref::<InterestRateSwap>()
        .expect("fixture instrument is IRS")
        .clone()
    }

    fn load_fixture_market(fixture: &GoldenFixtureEnvelope) -> MarketContext {
        let result = engine::execute_with_diagnostics(&fixture.inputs.market_envelope)
            .expect("fixture market envelope calibrates");
        MarketContext::try_from(result.result.final_market)
            .expect("fixture calibrated market rehydrates")
    }

    fn coupon_only_fixed_amount(cashflow: &BloombergCashflow) -> f64 {
        if cashflow.payments_receive.abs() > 1_000_000.0 {
            cashflow.payments_receive - 10_000_000.0
        } else {
            cashflow.payments_receive
        }
    }

    fn coupon_only_float_amount(cashflow: &BloombergCashflow) -> f64 {
        let payment = -cashflow.payments_pay;
        if payment.abs() > 1_000_000.0 {
            payment - 10_000_000.0
        } else {
            payment
        }
    }
}
