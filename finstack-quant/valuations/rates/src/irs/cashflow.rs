//! Cashflow construction helpers for interest rate swaps.
//!
//! This module centralizes cashflow schedule generation for `InterestRateSwap`:
//! - Fixed-leg and floating-leg `CashFlowSchedule` builders
//! - Combined signed schedules with `CFKind` metadata used by `CashflowProvider`
//!
//! Pricing logic (discounting, forwards, PV) lives in `pricer.rs` and consumes
//! these schedules where appropriate.
//!
//! # Payment Dates
//!
//! Fixed and floating schedules emit payment dates in `CashFlow::date`,
//! including business-day adjustment and payment lag. The pricer discounts
//! those dates directly without applying another payment delay.

use finstack_quant_core::Result;
#[cfg(test)]
use finstack_quant_core::dates::BusinessDayConvention;
use finstack_quant_core::dates::{Date, DateExt, StubKind, TenorUnit};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::fixings::fixing_series_id;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;

use crate::irs::InterestRateSwap;
use finstack_quant_cashflows::builder::{
    CashFlowSchedule, CouponType, FloatingCouponSpec, FloatingLegCompounding, FloatingRateFallback,
    FloatingRateSpec, Notional, PrincipalExchange, RollRule, ScheduleParams, SchedulePeriod,
    build_periods, merge_cashflow_schedules,
};
use finstack_quant_valuations_core::instruments::pricing::overnight_conventions::builder_overnight_method;
use finstack_quant_valuations_core::instruments::{FloatLegSpec, PayReceive};

/// Coupon spec for one float leg. The caller chooses the calendar, the
/// accrual-date adjustment and the fallback; every other field follows the leg.
pub(crate) fn float_leg_coupon_spec(
    leg: &FloatLegSpec,
    calendar_id: &str,
    adjust_accrual_dates: bool,
    fallback: FloatingRateFallback,
) -> FloatingCouponSpec {
    FloatingCouponSpec {
        rate_spec: FloatingRateSpec {
            forward_curve_id: leg.forward_curve_id.clone(),
            spread_bp: leg.spread_bp,
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: leg.frequency,
            index_tenor: None,
            reset_lag_days: leg.reset_lag_days,
            fixing_calendar_id: leg.fixing_calendar_id.clone(),
            compounding: builder_overnight_method(leg.compounding),
            overnight_basis: None,
            fallback,
        },
        coupon_type: CouponType::Cash,
        schedule: ScheduleParams {
            frequency: leg.frequency,
            day_count: leg.day_count,
            business_day_convention: leg.business_day_convention,
            calendar_id: calendar_id.into(),
            stub: leg.stub,
            end_of_month: leg.end_of_month,
            payment_lag_days: leg.payment_lag_days,
            adjust_accrual_dates,
            roll_rule: RollRule::None,
        },
    }
}

/// Total backward shift (in business days) applied to overnight observation
/// dates.
///
/// `CompoundedInArrears` applies lookback semantics; ISDA 2021 observation
/// shift is the separate `CompoundedWithObservationShift` variant.
#[cfg(test)]
fn compounded_total_shift_days(compounding: FloatingLegCompounding) -> Result<i32> {
    match compounding {
        FloatingLegCompounding::CompoundedInArrears { lookback_days } => {
            Ok(-i32::try_from(lookback_days).unwrap_or(i32::MAX))
        }
        FloatingLegCompounding::CompoundedWithObservationShift { shift_days } => {
            Ok(-i32::try_from(shift_days).unwrap_or(i32::MAX))
        }
        FloatingLegCompounding::CompoundedWithRateCutoff { .. }
        | FloatingLegCompounding::Simple
        | FloatingLegCompounding::SimpleAverage => Ok(0),
    }
}

/// Whether the day-count-fraction weights follow the shifted observation dates
/// (ISDA 2021 observation shift) rather than the original accrual dates
/// (lookback).
#[cfg(test)]
fn uses_observation_shift_dcf(compounding: FloatingLegCompounding) -> bool {
    matches!(
        compounding,
        FloatingLegCompounding::CompoundedWithObservationShift { .. }
    )
}

fn is_irregular_fixed_period(
    period: &SchedulePeriod,
    fixed: &finstack_quant_valuations_core::instruments::FixedLegSpec,
    index: usize,
    period_count: usize,
) -> Result<bool> {
    let front = match fixed.stub {
        StubKind::ShortFront | StubKind::LongFront if index == 0 => true,
        StubKind::ShortBack | StubKind::LongBack if index + 1 == period_count => false,
        _ => return Ok(false),
    };
    // Only the declared stub-side edge can be irregular. Compare it with the
    // original schedule anchor, rather than stepping from a clamped February
    // date or accepting the opposite roll direction's grid.
    let periods = i64::try_from(period_count).map_err(|_| {
        finstack_quant_core::Error::Validation("fixed-leg period count exceeds i64".into())
    })?;
    let count = i64::from(fixed.frequency.count()) * periods * if front { -1 } else { 1 };
    let anchor = if front { fixed.end } else { fixed.start };
    let expected = match fixed.frequency.unit() {
        TenorUnit::Days => anchor.add_days(count).ok(),
        TenorUnit::Weeks => anchor.add_days(count * 7).ok(),
        TenorUnit::Months => i32::try_from(count)
            .ok()
            .and_then(|months| anchor.add_months(months).ok()),
        TenorUnit::Years => i32::try_from(count * 12)
            .ok()
            .and_then(|months| anchor.add_months(months).ok()),
    };
    let boundary = if front {
        period.unadjusted_start
    } else {
        period.unadjusted_end
    };
    // A nominal boundary beyond the supported date range cannot coincide with
    // the valid contractual boundary. EOM applies to generated roll dates,
    // while an exactly aligned raw contractual boundary remains valid too.
    Ok(!expected.is_some_and(|date| {
        date == boundary || (fixed.end_of_month && date.end_of_month() == boundary)
    }))
}

fn resolve_compounded_fixing_calendar(
    irs: &InterestRateSwap,
) -> Result<&'static dyn finstack_quant_core::dates::HolidayCalendar> {
    let float = irs.resolved_float_leg()?;
    let calendar_id = float
        .fixing_calendar_id
        .as_deref()
        .or(float.calendar_id.as_deref());
    finstack_quant_valuations_core::instruments::pricing::overnight::resolve_overnight_fixing_calendar(
        calendar_id,
        irs.notional.currency(),
        &format!("compounded RFR swap '{}'", irs.id),
    )
}

/// Project compounded overnight coupons with optional fixing metadata.
///
/// # Arguments
///
/// * `irs` - Swap supplying the floating-leg dates, calendar, notional and
///   arithmetic spread conventions.
/// * `disc` - Discount curve used as the overnight projection source when
///   `proj` is absent.
/// * `proj` - Explicit overnight forward curve, when the resolved market
///   supplies one; its source role is preserved even if derived from `disc`.
/// * `as_of` - Valuation date; earlier observations require exact historical
///   fixings and coupons paid on or before this date are excluded.
/// * `fixings` - Exact historical index observations in decimal rate units.
/// * `include_projected_fixings` - When true, retain every raw overnight
///   observation for export and fixing materialization. When false, omit that
///   metadata while preserving the same canonical daily coupon calculation.
///
/// # Errors
///
/// Propagates invalid schedules, calendars, projection inputs, missing exact
/// fixings and invalid monetary cashflows.
pub(crate) fn projected_compounded_float_leg_schedule(
    irs: &InterestRateSwap,
    disc: &DiscountCurve,
    proj: Option<&ForwardCurve>,
    as_of: Date,
    fixings: Option<&ScalarTimeSeries>,
    include_projected_fixings: bool,
) -> Result<CashFlowSchedule> {
    use finstack_quant_core::cashflow::{CFKind, CashFlow};

    let float = irs.resolved_float_leg()?;
    let periods = build_periods(float.schedule_params(
        float.calendar_or_weekends(),
        None,
        irs.adjust_float_accrual_dates,
    ))?;
    if periods.is_empty() {
        return finstack_quant_cashflows::schedule_from_classified_flows(
            Vec::new(),
            float.day_count,
            finstack_quant_cashflows::ScheduleBuildOpts {
                notional_hint: Some(irs.notional),
                ..Default::default()
            },
        );
    }

    let cal = resolve_compounded_fixing_calendar(irs)?;
    let projection_curve = match proj {
        Some(forward) => {
            finstack_quant_valuations_core::instruments::pricing::overnight::OvernightProjectionCurve::Forward(
                forward,
            )
        }
        None => {
            finstack_quant_valuations_core::instruments::pricing::overnight::OvernightProjectionCurve::Discount(
                disc,
            )
        }
    };

    let mut flows = Vec::with_capacity(periods.len());
    let mut projected_fixings = Vec::new();
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
        let (accrual_start, accrual_end) =
            finstack_quant_valuations_core::instruments::pricing::overnight::adjust_overnight_accrual_boundaries(
                period.accrual_start,
                period.accrual_end,
                float.business_day_convention,
                cal,
            )?;
        if accrual_end <= accrual_start {
            continue;
        }
        let projection =
            finstack_quant_valuations_core::instruments::pricing::overnight::project_overnight_coupon(
                finstack_quant_valuations_core::instruments::pricing::overnight::OvernightCouponProjectionInput {
                    curve: projection_curve,
                    fixings,
                    fixing_id: float.forward_curve_id.as_str(),
                    as_of,
                    accrual_start,
                    accrual_end,
                    day_count: float.day_count,
                    coupon_frequency: Some(float.frequency),
                    coupon_period: if float.day_count == finstack_quant_core::dates::DayCount::Act365L { (period.accrual_start, period.accrual_end) } else { (accrual_start, accrual_end) },
                    compounding: &float.compounding,
                    fixing_calendar: cal,
                    compounded_spread: 0.0,
                    // Preserve the daily-product rounding used by exported cashflows.
                    // Valuation omits only the separate projected-fixing metadata below.
                    need_observation_exposures: true,
                },
            )?;
        if include_projected_fixings {
            projected_fixings.extend(projection.observation_exposures.iter().map(|observation| {
                finstack_quant_cashflows::fixings::ProjectedFixing {
                    series_id: fixing_series_id(float.forward_curve_id.as_str()),
                    date: observation.observation_start,
                    value: Some(observation.projected_rate),
                }
            }));
        }
        let compound_factor = projection.compound_factor;
        let accrual_year_fraction = projection.accrual_year_fraction;

        let spread_bp = float.spread_bp.as_f64();
        let coupon_amount =
            finstack_quant_valuations_core::instruments::pricing::overnight::overnight_coupon_amount(
                irs.notional.amount(),
                &projection,
                spread_bp * finstack_quant_core::math::units::ONE_BASIS_POINT,
            );
        let all_in_rate = if accrual_year_fraction.abs() > f64::EPSILON {
            (compound_factor - 1.0) / accrual_year_fraction
                + spread_bp * finstack_quant_core::math::units::ONE_BASIS_POINT
        } else {
            spread_bp * finstack_quant_core::math::units::ONE_BASIS_POINT
        };
        flows.push(CashFlow::new(
            period.payment_date,
            None,
            Money::new(coupon_amount, irs.notional.currency())?,
            CFKind::FloatReset,
            accrual_year_fraction,
            Some(all_in_rate),
        ));
    }

    finstack_quant_cashflows::schedule_from_classified_flows(
        flows,
        float.day_count,
        finstack_quant_cashflows::ScheduleBuildOpts {
            notional_hint: Some(irs.notional),
            meta: finstack_quant_cashflows::builder::CashFlowMeta {
                projected_fixings,
                ..Default::default()
            },
        },
    )
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
pub(crate) fn fixed_leg_schedule(irs: &InterestRateSwap) -> Result<CashFlowSchedule> {
    use finstack_quant_core::cashflow::{CFKind, CashFlow};

    let fixed = irs.resolved_fixed_leg()?;
    let periods = build_periods(fixed.schedule_params(irs.adjust_fixed_accrual_dates))?;
    let period_count = periods.len();
    let rate = fixed.rate.as_f64();
    let flows = periods
        .into_iter()
        .enumerate()
        .map(|(index, period)| -> Result<CashFlow> {
            let kind = if is_irregular_fixed_period(&period, &fixed, index, period_count)? {
                CFKind::Stub
            } else {
                CFKind::Fixed
            };
            Ok(CashFlow::new(
                period.payment_date,
                None,
                Money::new(
                    irs.notional.amount() * rate * period.accrual_year_fraction,
                    irs.notional.currency(),
                )?,
                kind,
                period.accrual_year_fraction,
                Some(rate),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    finstack_quant_cashflows::schedule_from_classified_flows(
        flows,
        fixed.day_count,
        finstack_quant_cashflows::ScheduleBuildOpts {
            notional_hint: Some(irs.notional),
            ..Default::default()
        },
    )
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
/// * `as_of` - Optional valuation date for compounded-in-arrears legs; `None`
///   falls back to the projection (else discount) curve base date
/// * `include_projected_fixings` - Retain raw overnight observation metadata
///   for compounded coupons when true; false omits the metadata without changing
///   coupon economics. Term-rate and structure-only schedules are unchanged.
///
/// # Returns
///
/// A `CashFlowSchedule` containing all floating leg cashflows with `CFKind::FloatReset`
/// classifications.
///
/// # Errors
///
/// Propagates invalid schedules, missing market curves or historical fixings,
/// invalid projection inputs and invalid monetary cashflows.
pub(crate) fn float_leg_schedule_with_curves_as_of(
    irs: &InterestRateSwap,
    curves: Option<&MarketContext>,
    as_of: Option<Date>,
    include_projected_fixings: bool,
) -> Result<CashFlowSchedule> {
    let float = irs.resolved_float_leg()?;
    if matches!(
        float.compounding,
        FloatingLegCompounding::CompoundedInArrears { .. }
            | FloatingLegCompounding::CompoundedWithObservationShift { .. }
            | FloatingLegCompounding::CompoundedWithRateCutoff { .. }
    ) && let Some(market) = curves
    {
        let disc = market.get_discount(irs.fixed_leg.discount_curve_id.as_ref())?;
        let proj = if irs.is_single_curve_ois() {
            market.get_forward(float.forward_curve_id.as_str()).ok()
        } else {
            Some(market.get_forward(float.forward_curve_id.as_str())?)
        };
        let fixings = finstack_quant_core::market_data::fixings::get_fixing_series(
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
            include_projected_fixings,
        );
    }

    let mut float_b = CashFlowSchedule::builder();
    let fallback = if curves.is_some() {
        FloatingRateFallback::Error
    } else {
        FloatingRateFallback::SpreadOnly
    };
    let _ = float_b
        .principal(irs.notional, float.start, float.end)
        .principal_exchange(PrincipalExchange::None)
        .floating_cf(float_leg_coupon_spec(
            &float,
            float.calendar_id.as_deref().unwrap_or("weekends_only"),
            irs.adjust_float_accrual_dates,
            fallback,
        ));
    match (curves, as_of) {
        (Some(market), Some(date)) => float_b.build_as_of(market, date),
        _ => float_b.build(curves),
    }
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
    let fixed_sched = fixed_leg_schedule(irs)?;
    let float_sched = float_leg_schedule_with_curves_as_of(irs, curves, as_of, true)?;

    let (fixed_sign, floating_sign) = match irs.side {
        PayReceive::Receive => (1.0, -1.0),
        PayReceive::Pay => (-1.0, 1.0),
    };
    Ok(merge_cashflow_schedules(
        [
            fixed_sched.scale_amounts(fixed_sign)?,
            float_sched.scale_amounts(floating_sign)?,
        ],
        Notional::par(irs.notional.amount(), irs.notional.currency())?,
        irs.fixed_leg.day_count,
    ))
}

/// Fixed-leg annuity `Σ τᵢ · DF(as_of, payᵢ)` over the periods paid strictly
/// after `paid_after`, using curve-consistent relative discount factors and
/// Neumaier-compensated summation.
///
/// # Arguments
///
/// * `periods` - Fixed-leg accrual periods.
/// * `disc` - Discount curve providing `DF(as_of, payment_date)`.
/// * `as_of` - Valuation date the discount factors are relative to.
/// * `paid_after` - Only periods with `payment_date > paid_after` contribute
///   (the valuation date for a spot annuity, the exercise date for a forward one).
pub(crate) fn fixed_leg_annuity(
    periods: &[finstack_quant_cashflows::builder::SchedulePeriod],
    disc: &dyn finstack_quant_core::market_data::traits::Discounting,
    as_of: finstack_quant_core::dates::Date,
    paid_after: finstack_quant_core::dates::Date,
) -> Result<f64> {
    let mut annuity = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    for period in periods {
        if period.payment_date <= paid_after {
            continue;
        }
        let df = finstack_quant_core::market_data::traits::Discounting::df_between_dates(
            disc,
            as_of,
            period.payment_date,
        )?;
        annuity.add(period.accrual_year_fraction * df);
    }
    Ok(annuity.total())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::dates::{DayCount, DayCountContext, StubKind, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use time::macros::date;

    #[test]
    fn term_coupon_observation_cutoff_is_valuation_date_not_curve_base() {
        use finstack_quant_core::market_data::{
            scalars::ScalarTimeSeries, term_structures::ForwardCurve,
        };
        let mut irs = InterestRateSwap::example().expect("example");
        irs.notional = Money::from((10_000_000_i64, irs.notional.currency()));
        irs.float_leg.start = date!(2025 - 01 - 02);
        irs.float_leg.end = date!(2025 - 04 - 02);
        irs.float_leg.forward_curve_id = "TEST-TERM".into();
        irs.float_leg.frequency = Tenor::quarterly();
        irs.float_leg.compounding = FloatingLegCompounding::Simple;
        irs.float_leg.reset_lag_days = 0;
        irs.float_leg.spread_bp = Decimal::ZERO;
        irs.float_leg.day_count = DayCount::Act360;
        irs.float_leg.calendar_id = Some("weekends_only".into());
        irs.float_leg.fixing_calendar_id = Some("weekends_only".into());
        let curve = ForwardCurve::builder("TEST-TERM", 0.25)
            .base_date(date!(2025 - 01 - 01))
            .knots([(0.0, 0.04), (1.0, 0.04)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);
        let as_of = date!(2025 - 02 - 01);
        assert!(
            float_leg_schedule_with_curves_as_of(&irs, Some(&market), Some(as_of), true).is_err()
        );
        let market = market.insert_series(
            ScalarTimeSeries::new(
                "FIXING:TEST-TERM",
                vec![(date!(2025 - 01 - 02), 0.06)],
                None,
            )
            .expect("fixing"),
        );
        let schedule = float_leg_schedule_with_curves_as_of(&irs, Some(&market), Some(as_of), true)
            .expect("schedule");
        let coupon = schedule
            .get_flows()
            .iter()
            .find(|cf| cf.kind == CFKind::FloatReset)
            .expect("coupon");
        assert!(
            (coupon.amount.amount() - 150_000.0).abs() < 1e-6,
            "{}",
            coupon.amount
        );
    }

    #[test]
    fn ois_economics_mode_omits_only_projected_fixings() {
        use finstack_quant_cashflows::CashflowProvider;
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::math::interp::InterpStyle;
        use finstack_quant_valuations_core::instruments::Instrument;
        use serde_json::json;

        let as_of = date!(2025 - 01 - 02);
        let mut irs = InterestRateSwap::example().expect("example IRS");
        irs.id = "OIS-PROJECTION-DETAILS".into();
        irs.notional = Money::from((10_000_000_i64, Currency::USD));
        irs.fixed_leg.start = as_of;
        irs.fixed_leg.end = date!(2025 - 01 - 10);
        irs.fixed_leg.frequency = Tenor::annual();
        irs.fixed_leg.day_count = DayCount::Act360;
        irs.fixed_leg.business_day_convention = BusinessDayConvention::Following;
        irs.fixed_leg.calendar_id = Some("weekends_only".into());
        irs.fixed_leg.stub = StubKind::ShortFront;
        irs.fixed_leg.payment_lag_days = 2;
        irs.float_leg.start = irs.fixed_leg.start;
        irs.float_leg.end = irs.fixed_leg.end;
        irs.float_leg.frequency = Tenor::annual();
        irs.float_leg.day_count = DayCount::Act360;
        irs.float_leg.business_day_convention = BusinessDayConvention::Following;
        irs.float_leg.calendar_id = Some("weekends_only".into());
        irs.float_leg.fixing_calendar_id = Some("weekends_only".into());
        irs.float_leg.forward_curve_id = "USD-OIS".into();
        irs.float_leg.stub = StubKind::ShortFront;
        irs.float_leg.reset_lag_days = 0;
        irs.float_leg.payment_lag_days = 2;
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
                .interp(InterpStyle::Linear)
                .build()
                .expect("discount curve"),
        );

        for (label, compounding, raw_dates, exported_dates) in [
            (
                "eligible",
                FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
                vec![
                    date!(2025 - 01 - 02),
                    date!(2025 - 01 - 03),
                    date!(2025 - 01 - 06),
                    date!(2025 - 01 - 07),
                    date!(2025 - 01 - 08),
                    date!(2025 - 01 - 09),
                ],
                vec![
                    date!(2025 - 01 - 02),
                    date!(2025 - 01 - 03),
                    date!(2025 - 01 - 06),
                    date!(2025 - 01 - 07),
                    date!(2025 - 01 - 08),
                    date!(2025 - 01 - 09),
                ],
            ),
            (
                "cutoff",
                FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 2 },
                vec![
                    date!(2025 - 01 - 02),
                    date!(2025 - 01 - 03),
                    date!(2025 - 01 - 06),
                    date!(2025 - 01 - 07),
                    date!(2025 - 01 - 07),
                    date!(2025 - 01 - 07),
                ],
                vec![
                    date!(2025 - 01 - 02),
                    date!(2025 - 01 - 03),
                    date!(2025 - 01 - 06),
                    date!(2025 - 01 - 07),
                ],
            ),
        ] {
            irs.float_leg.compounding = compounding;
            irs.validate().expect("valid compact OIS");
            let before = irs.cashflow_schedule(&market, as_of).expect("full export");
            let economics =
                float_leg_schedule_with_curves_as_of(&irs, Some(&market), Some(as_of), false)
                    .expect("economics schedule");
            let full = float_leg_schedule_with_curves_as_of(&irs, Some(&market), Some(as_of), true)
                .expect("full floating schedule");
            assert!(economics.get_meta().projected_fixings.is_empty());
            assert_eq!(economics.get_flows().len(), 1);
            assert_eq!(full.get_flows().len(), 1);
            let actual = &economics.get_flows()[0];
            let expected = &full.get_flows()[0];
            assert_eq!(actual.date, date!(2025 - 01 - 14));
            assert_eq!(actual.date, expected.date);
            assert_eq!(actual.kind, CFKind::FloatReset);
            assert_eq!(actual.kind, expected.kind);
            assert_eq!(actual.amount.currency(), Currency::USD);
            assert_eq!(actual.amount.currency(), expected.amount.currency());
            assert_eq!(actual.accrual_factor, expected.accrual_factor);
            let actual_rate = actual.rate.expect("coupon rate");
            let expected_rate = expected.rate.expect("coupon rate");
            assert!((actual_rate - expected_rate).abs() <= 1e-12);
            assert!(
                ((1.0 + actual_rate * actual.accrual_factor)
                    - (1.0 + expected_rate * expected.accrual_factor))
                    .abs()
                    <= 1e-12
            );
            let amount_error = (actual.amount.amount() - expected.amount.amount()).abs();
            assert!(amount_error <= irs.notional.amount() * 1e-12);
            assert_eq!(
                full.get_meta()
                    .projected_fixings
                    .iter()
                    .map(|fixing| fixing.date)
                    .collect::<Vec<_>>(),
                raw_dates
            );
            assert!(full.get_meta().projected_fixings.iter().all(|fixing| {
                fixing.series_id == "FIXING:USD-OIS" && fixing.value.is_some_and(f64::is_finite)
            }));
            let _ = irs.value(&market, as_of).expect("economics-only value");
            let after = irs.cashflow_schedule(&market, as_of).expect("full export");
            assert_eq!(
                serde_json::to_value(&before).expect("serialize before export"),
                serde_json::to_value(&after).expect("serialize after export")
            );
            assert_eq!(
                after
                    .get_meta()
                    .projected_fixings
                    .iter()
                    .map(|fixing| fixing.date)
                    .collect::<Vec<_>>(),
                exported_dates
            );
            eprintln!(
                "FQ_OIS_DIAGNOSTIC {}",
                json!({
                    "case": format!("ois_p3/mode/{label}"),
                    "data": {
                        "fixture": {"instrument": irs, "market": market, "as_of": as_of.to_string()},
                        "economics_schedule": economics,
                        "full_floating_schedule": full,
                        "public_export": after,
                        "coupon_amount_error_usd": amount_error,
                        "factor_and_rate_bound": 1e-12,
                        "coupon_amount_bound_usd": irs.notional.amount() * 1e-12,
                    },
                })
            );
        }
    }

    #[test]
    fn irs_leg_schedules_do_not_emit_notional_flows() {
        let irs = InterestRateSwap::example().expect("example IRS");
        let fixed = fixed_leg_schedule(&irs).expect("fixed schedule");
        assert!(
            fixed
                .get_flows()
                .iter()
                .all(|cf| cf.kind == CFKind::Fixed || cf.kind == CFKind::Stub),
            "fixed_leg_schedule should be coupon-only"
        );

        let float =
            float_leg_schedule_with_curves_as_of(&irs, None, None, true).expect("float schedule");
        assert!(
            float
                .get_flows()
                .iter()
                .all(|cf| cf.kind == CFKind::FloatReset),
            "float_leg_schedule should be coupon-only"
        );
    }

    #[test]
    fn act365l_fixed_leg_classifies_actual_coupons_without_a_synthetic_period() {
        for (start, end, frequency, stub, denominator, kind) in [
            (
                date!(2023 - 03 - 01),
                date!(2024 - 03 - 01),
                Tenor::annual(),
                StubKind::ShortFront,
                366.0,
                CFKind::Fixed,
            ),
            (
                date!(2024 - 03 - 01),
                date!(2025 - 01 - 01),
                Tenor::annual(),
                StubKind::ShortFront,
                365.0,
                CFKind::Stub,
            ),
            (
                date!(2023 - 12 - 01),
                date!(2025 - 01 - 01),
                Tenor::annual(),
                StubKind::LongFront,
                365.0,
                CFKind::Stub,
            ),
            (
                date!(2023 - 12 - 01),
                date!(2024 - 03 - 01),
                Tenor::quarterly(),
                StubKind::ShortFront,
                366.0,
                CFKind::Fixed,
            ),
        ] {
            let mut irs = InterestRateSwap::example().expect("example IRS");
            irs.notional = Money::from((1_000_000_i64, irs.notional.currency()));
            irs.fixed_leg.start = start;
            irs.fixed_leg.end = end;
            irs.fixed_leg.frequency = frequency;
            irs.fixed_leg.stub = stub;
            irs.fixed_leg.day_count = DayCount::Act365L;
            irs.fixed_leg.business_day_convention = BusinessDayConvention::Unadjusted;
            irs.fixed_leg.calendar_id = Some("weekends_only".into());
            irs.fixed_leg.rate = Decimal::new(5, 2);
            let schedule = fixed_leg_schedule(&irs).expect("ACT/365L fixed schedule");
            let flows = schedule.get_flows();
            assert_eq!(flows.len(), 1);
            assert_eq!(flows[0].kind, kind);
            let expected_accrual = (end - start).whole_days() as f64 / denominator;
            assert!((flows[0].accrual_factor - expected_accrual).abs() < 1e-14);
            assert!((flows[0].amount.amount() - 50_000.0 * expected_accrual).abs() < 1e-8);
        }
    }

    #[test]
    fn fixed_leg_classifies_clamped_months_on_the_original_roll_grid() {
        for (start, end, stub, expected_kinds) in [
            (
                date!(2025 - 01 - 30),
                date!(2025 - 03 - 30),
                StubKind::ShortFront,
                vec![CFKind::Fixed, CFKind::Fixed],
            ),
            (
                date!(2025 - 02 - 28),
                date!(2025 - 03 - 30),
                StubKind::LongFront,
                vec![CFKind::Fixed],
            ),
            (
                date!(2025 - 02 - 28),
                date!(2025 - 03 - 30),
                StubKind::LongBack,
                vec![CFKind::Stub],
            ),
            (
                date!(2025 - 01 - 28),
                date!(2025 - 03 - 30),
                StubKind::LongBack,
                vec![CFKind::Fixed, CFKind::Stub],
            ),
            (
                date!(2025 - 01 - 29),
                date!(2025 - 03 - 30),
                StubKind::LongFront,
                vec![CFKind::Stub, CFKind::Fixed],
            ),
        ] {
            let mut irs = InterestRateSwap::example().expect("example IRS");
            irs.fixed_leg.start = start;
            irs.fixed_leg.end = end;
            irs.fixed_leg.frequency = Tenor::monthly();
            irs.fixed_leg.stub = stub;
            irs.fixed_leg.day_count = DayCount::Act365L;
            irs.fixed_leg.business_day_convention = BusinessDayConvention::Unadjusted;
            irs.fixed_leg.calendar_id = Some("weekends_only".into());
            irs.fixed_leg.end_of_month = false;
            let schedule = fixed_leg_schedule(&irs).expect("monthly fixed schedule");
            let kinds: Vec<_> = schedule.get_flows().iter().map(|cf| cf.kind).collect();
            assert_eq!(kinds, expected_kinds, "{start} to {end}, {stub:?}");
        }
    }

    #[test]
    fn compounded_irs_spread_uses_holiday_adjusted_accrual_fraction() {
        let as_of = date!(2024 - 12 - 02);
        let mut irs = InterestRateSwap::example().expect("example IRS");
        irs.notional = Money::from((1_000_000_i64, irs.notional.currency()));
        irs.float_leg.start = date!(2025 - 01 - 02);
        irs.float_leg.end = date!(2025 - 07 - 04);
        irs.float_leg.frequency = Tenor::semi_annual();
        irs.float_leg.day_count = DayCount::Act360;
        irs.float_leg.business_day_convention = BusinessDayConvention::ModifiedFollowing;
        irs.float_leg.calendar_id = Some("usny".into());
        irs.float_leg.forward_curve_id = "USD-SOFR".into();
        irs.float_leg.spread_bp = Decimal::from(100);
        irs.float_leg.compounding =
            FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .day_count(DayCount::Act365F)
                    .knots([(0.0, 1.0), (1.0, 0.97)])
                    .build()
                    .expect("discount"),
            )
            .insert(
                ForwardCurve::builder("USD-SOFR", 1.0 / 360.0)
                    .base_date(as_of)
                    .day_count(DayCount::Act360)
                    .knots([(0.0, 0.0), (1.0, 0.0)])
                    .build()
                    .expect("forward"),
            );

        let schedule = float_leg_schedule_with_curves_as_of(&irs, Some(&market), Some(as_of), true)
            .expect("compounded schedule");
        let total_accrual: f64 = schedule
            .get_flows()
            .iter()
            .map(|flow| flow.accrual_factor)
            .sum();
        let total_amount: f64 = schedule
            .get_flows()
            .iter()
            .map(|flow| flow.amount.amount())
            .sum();
        let expected_accrual = DayCount::Act360
            .year_fraction(
                date!(2025 - 01 - 02),
                date!(2025 - 07 - 07),
                DayCountContext::default(),
            )
            .expect("adjusted accrual");

        assert!((total_accrual - expected_accrual).abs() < 1.0e-15);
        assert!((total_amount - 1_000_000.0 * 0.01 * expected_accrual).abs() < 1.0e-9);
    }

    /// The in-module compounding loop must agree with the canonical-schedule
    /// builder: a lookback shifts observations only, while an observation
    /// shift moves the day-count-fraction weights with them.
    #[test]
    fn loop_shift_semantics_match_builder() {
        // Lookback: backward shift, DCF anchored to the accrual dates.
        let lookback_only = FloatingLegCompounding::CompoundedInArrears { lookback_days: 5 };
        assert_eq!(
            compounded_total_shift_days(lookback_only).expect("lookback-only is valid"),
            -5
        );
        assert!(!uses_observation_shift_dcf(lookback_only));

        // Observation shift: backward shift, DCF follows the observations.
        let shift_only = FloatingLegCompounding::CompoundedWithObservationShift { shift_days: 2 };
        assert_eq!(
            compounded_total_shift_days(shift_only).expect("shift-only is valid"),
            -2
        );
        assert!(uses_observation_shift_dcf(shift_only));
    }

    /// W-13: A single-curve OIS leg with a non-zero rate cut-off must NOT be
    /// priced via the plain compounded DF identity. The fast path freezes no
    /// rates, so on a steep curve it produces the same PV as a no-cut-off leg.
    /// After the fix, the rate cut-off is explicitly applied and the two legs
    /// must differ.
    #[test]
    fn single_curve_ois_rate_cutoff_differs_from_no_cutoff() {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use finstack_quant_valuations_core::instruments::{FixedLegSpec, FloatLegSpec};
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
                .notional(Money::from((10_000_000_i64, Currency::USD)))
                .side(PayReceive::Pay)
                .fixed_leg(FixedLegSpec {
                    discount_curve_id: disc_id.clone(),
                    rate: Decimal::ZERO,
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    business_day_convention: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: StubKind::ShortFront,
                    start,
                    end,

                    payment_lag_days: 0,
                    end_of_month: false,
                })
                .float_leg(FloatLegSpec {
                    discount_curve_id: disc_id.clone(),
                    forward_curve_id: disc_id.clone(), // single-curve OIS
                    spread_bp: Decimal::ZERO,
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    business_day_convention: BusinessDayConvention::ModifiedFollowing,
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

        let swap_no_cutoff = build_swap(
            "OIS-NO-CUTOFF",
            FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
        );
        // 5-day rate cut-off freezes the last 5 overnight rates of each period.
        let swap_cutoff = build_swap("OIS-CUTOFF-5D", FloatingLegCompounding::rate_cutoff(5));

        // Single-curve: only the discount curve is loaded (proj is None).
        let ctx = MarketContext::new().insert(disc);

        let sched_no_cutoff =
            float_leg_schedule_with_curves_as_of(&swap_no_cutoff, Some(&ctx), Some(as_of), true)
                .expect("no-cutoff schedule");
        let sched_cutoff =
            float_leg_schedule_with_curves_as_of(&swap_cutoff, Some(&ctx), Some(as_of), true)
                .expect("cutoff schedule");

        let pv_no_cutoff: f64 = sched_no_cutoff
            .get_flows()
            .iter()
            .map(|f| f.amount.amount())
            .sum();
        let pv_cutoff: f64 = sched_cutoff
            .get_flows()
            .iter()
            .map(|f| f.amount.amount())
            .sum();

        assert!(
            (pv_no_cutoff - pv_cutoff).abs() > 1.0,
            "single-curve OIS rate cut-off must change the floating coupon on a \
             steep curve: no_cutoff={pv_no_cutoff}, cutoff={pv_cutoff}"
        );
    }
}
