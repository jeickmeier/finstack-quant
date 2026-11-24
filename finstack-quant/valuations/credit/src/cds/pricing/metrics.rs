//! Premium schedules, coupon accruals, par spread, risky annuity and PV01, and
//! full NPV for single-name CDS pricing.

use super::engine::{
    AodInputs, CouponPeriod, act360_inclusive_year_fraction, add_premium_period, calendar_for,
    clean_accrued_amount, pv_protection_leg, pv_protection_leg_raw, signed_npv, spread_decimal,
    upfront_pv,
};
use crate::CdsDateRegime;
use crate::cds::CreditDefaultSwap;
use crate::constants::numerical;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Schedule, StubKind, Tenor};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::units::BASIS_POINTS_PER_UNIT;
use finstack_quant_core::{Error, Result};

/// Generate contractual accrual and payment dates from the premium terms.
fn premium_schedule(cds: &CreditDefaultSwap) -> Result<Schedule> {
    let builder = match cds.premium_leg.date_regime {
        CdsDateRegime::Standard => {
            if cds.premium_leg.frequency != Tenor::quarterly() {
                return Err(Error::Validation(
                    "premium.date_regime = standard requires quarterly frequency; set \
                     premium.date_regime = bespoke for bespoke premium terms"
                        .into(),
                ));
            }
            // The standard regime runs quarterly on CDS roll dates from the
            // roll on or before the premium start, with a short final period
            // for an off-grid maturity; it owns that stub (record
            // CALIBRATION-FRAMEWORK-007, O7).
            let anchor = if finstack_quant_core::dates::is_cds_date(cds.premium_leg.start) {
                cds.premium_leg.start
            } else {
                finstack_quant_core::dates::prev_cds_date(cds.premium_leg.start)?
            };
            Schedule::builder(anchor, cds.premium_leg.end)?
                .frequency(Tenor::quarterly())
                .stub_rule(StubKind::ShortBack)
        }
        CdsDateRegime::Bespoke => Schedule::builder(cds.premium_leg.start, cds.premium_leg.end)?
            .frequency(cds.premium_leg.frequency)
            .stub_rule(cds.premium_leg.stub),
    };
    let mut builder = builder.grid(finstack_quant_core::dates::GridMode::Natural);
    builder = match cds.premium_leg.calendar_id.as_deref() {
        Some(calendar_id) => {
            builder.adjust_with(cds.premium_leg.business_day_convention, calendar_id)
        }
        None => builder.unadjusted(),
    };
    let schedule = builder.build()?;
    let mut dates = schedule.get_dates().to_vec();
    dates[0] = cds.premium_leg.start;
    Schedule::try_new(
        dates,
        schedule.get_payment_dates().to_vec(),
        schedule.get_fixing_dates().to_vec(),
    )
}

pub(crate) fn generate_isda_schedule(cds: &CreditDefaultSwap) -> Result<Vec<Date>> {
    let schedule = premium_schedule(cds)?;
    let mut dates = vec![cds.premium_leg.start];
    dates.extend(schedule.get_payment_dates());
    Ok(dates)
}

pub(crate) fn coupon_periods(cds: &CreditDefaultSwap) -> Result<Vec<CouponPeriod>> {
    if cds.premium_leg.start >= cds.premium_leg.end {
        return Ok(Vec::new());
    }
    let schedule = premium_schedule(cds)?;
    let adjusted = cds.uses_adjusted_premium_accrual_dates();
    Ok(schedule
        .get_dates()
        .windows(2)
        .enumerate()
        .map(|(idx, dates)| {
            let payment_date = schedule.get_payment_dates()[idx];
            CouponPeriod {
                accrual_start: if adjusted && idx > 0 {
                    schedule.get_payment_dates()[idx - 1]
                } else {
                    dates[0]
                },
                accrual_end: if adjusted
                    && (idx + 1 < schedule.get_payment_dates().len()
                        || cds.valuation_convention.act360_includes_last_day())
                {
                    payment_date
                } else {
                    dates[1]
                },
                payment_date,
                is_final: idx + 1 == schedule.get_payment_dates().len(),
            }
        })
        .collect())
}

pub(crate) fn coupon_accrual(cds: &CreditDefaultSwap, period: &CouponPeriod) -> Result<f64> {
    if period.accrual_end > period.accrual_start
        && cds.premium_leg.day_count == DayCount::Act360
        && (cds.valuation_convention.act360_includes_last_day()
            || (cds.uses_adjusted_premium_accrual_dates() && period.is_final))
    {
        return Ok(act360_inclusive_year_fraction(
            period.accrual_start,
            period.accrual_end,
        ));
    }

    cds.premium_leg.day_count.year_fraction(
        period.accrual_start,
        period.accrual_end,
        DayCountContext::default(),
    )
}

pub(crate) fn premium_cashflow_accruals(cds: &CreditDefaultSwap) -> Result<Vec<(Date, f64)>> {
    coupon_periods(cds)?
        .into_iter()
        .map(|period| Ok((period.payment_date, coupon_accrual(cds, &period)?)))
        .collect()
}

/// Premium accrued up to the default date, using the jump-to-default
/// convention's exclusive cutoff and rolled payment dates.
pub(crate) fn default_accrued_fraction(cds: &CreditDefaultSwap, as_of: Date) -> Result<f64> {
    if as_of <= cds.premium_leg.start || as_of >= cds.premium_leg.end {
        return Ok(0.0);
    }
    let schedule = generate_isda_schedule(cds)?;
    let last_coupon = schedule
        .into_iter()
        .take_while(|&coupon_date| coupon_date <= as_of)
        .last()
        .unwrap_or(cds.premium_leg.start);
    cds.premium_leg
        .day_count
        .year_fraction(last_coupon, as_of, DayCountContext::default())
}

pub(crate) fn clean_quote_accrual_fraction(cds: &CreditDefaultSwap, as_of: Date) -> Result<f64> {
    if as_of <= cds.premium_leg.start || as_of >= cds.premium_leg.end {
        return Ok(0.0);
    }
    let periods = coupon_periods(cds)?;
    let Some(period) = periods
        .iter()
        .find(|period| period.accrual_start <= as_of && as_of < period.accrual_end)
    else {
        return Ok(0.0);
    };
    if period.accrual_start == as_of {
        return Ok(0.0);
    }
    let close = as_of.next_day().ok_or_else(|| {
        Error::Validation("CDS quote valuation close is outside the date range".into())
    })?;
    cds.premium_leg
        .day_count
        .year_fraction(period.accrual_start, close, DayCountContext::default())
}

fn clean_par_spread_denominator(
    cds: &CreditDefaultSwap,
    disc: &DiscountCurve,
    surv: &HazardCurve,
    as_of: Date,
) -> Result<f64> {
    let full_annuity = premium_leg_annuity(cds, disc, surv, as_of, 1.0, true)?;
    let accrued_per_unit_spread = clean_quote_accrual_fraction(cds, as_of)?;
    Ok(full_annuity - accrued_per_unit_spread)
}

/// Per-unit-notional, per-unit-spread denominator used by [`par_spread`].
#[must_use = "denominator computation is pure"]
pub(crate) fn par_spread_denominator(
    cds: &CreditDefaultSwap,
    disc: &DiscountCurve,
    surv: &HazardCurve,
    as_of: Date,
) -> Result<f64> {
    if !cds.uses_full_premium_par_spread_denominator() && cds.uses_clean_price() {
        clean_par_spread_denominator(cds, disc, surv, as_of)
    } else {
        premium_leg_annuity(cds, disc, surv, as_of, 1.0, true)
    }
}

/// Calculate par spread (bp) that sets NPV to zero.
#[must_use = "par spread calculation is pure computation"]
pub(crate) fn par_spread(
    cds: &CreditDefaultSwap,
    disc: &DiscountCurve,
    surv: &HazardCurve,
    as_of: Date,
) -> Result<f64> {
    let protection_pv = pv_protection_leg(cds, disc, surv, as_of)?;
    let denom = par_spread_denominator(cds, disc, surv, as_of)?;

    // Guard `denom < tolerance` (not `denom.abs() < tolerance`): a seasoned
    // clean denominator can go negative near a coupon date.
    if denom < numerical::RATE_COMPARISON_TOLERANCE {
        return Err(Error::Validation(
            "Par spread denominator is non-positive or too small (risky \
             annuity ≈ 0, or clean denominator went negative for a seasoned \
             CDS). This may indicate zero survival probability, an expired \
             CDS, or accrued exceeding the remaining risky annuity."
                .to_string(),
        ));
    }

    Ok(protection_pv.amount() / (denom * cds.notional.amount()) * BASIS_POINTS_PER_UNIT)
}

/// Risky annuity: survival-weighted duration of premium payments (without AoD).
#[must_use = "risky annuity calculation is pure computation"]
pub(crate) fn risky_annuity(
    cds: &CreditDefaultSwap,
    disc: &DiscountCurve,
    surv: &HazardCurve,
    as_of: Date,
) -> Result<f64> {
    premium_leg_annuity(cds, disc, surv, as_of, 1.0, false)
}

/// Premium leg PV per unit notional at a running `spread` (decimal),
/// `Σ spread · τ · DF · SP` over unsettled coupons, plus the
/// accrual-on-default integral when `include_aod` is set.
pub(crate) fn premium_leg_annuity(
    cds: &CreditDefaultSwap,
    disc: &DiscountCurve,
    surv: &HazardCurve,
    as_of: Date,
    spread: f64,
    include_aod: bool,
) -> Result<f64> {
    let periods = coupon_periods(cds)?;
    let calendar = if include_aod {
        calendar_for(cds)?
    } else {
        None
    };

    let mut annuity = 0.0;
    for period in periods {
        if !crate::cds::cash_is_eligible(
            period.payment_date,
            as_of,
            cds.include_valuation_date_cashflows,
        ) {
            continue;
        }
        let accrual = coupon_accrual(cds, &period)?;
        let df = disc.df_between_dates(as_of, period.payment_date)?;
        add_premium_period(
            &mut annuity,
            AodInputs {
                cds,
                spread,
                accrual_start_date: period.accrual_start,
                start_date: period.accrual_start.max(as_of),
                end_date: period.accrual_end,
                settlement_delay: cds.protection_leg.settlement_delay,
                calendar,
                as_of,
                disc,
                surv,
                grid_cache: None,
            },
            accrual,
            df,
            include_aod,
        )?;
    }
    Ok(annuity)
}

/// Risky PV01: PV of 1bp of running premium on the notional, on the
/// instrument's valuation convention (`annuity × Notional / 10000`).
#[must_use = "risky PV01 calculation is pure computation"]
pub(crate) fn risky_pv01(
    cds: &CreditDefaultSwap,
    disc: &DiscountCurve,
    surv: &HazardCurve,
    as_of: Date,
) -> Result<f64> {
    let annuity = if cds.uses_clean_price() {
        clean_par_spread_denominator(cds, disc, surv, as_of)?
    } else {
        risky_annuity(cds, disc, surv, as_of)?
    };
    Ok(annuity * cds.notional.amount() / BASIS_POINTS_PER_UNIT)
}

/// Canonical CDS instrument NPV from the holder's side, including dated
/// upfront cash at its discount factor and Bloomberg CDSW clean accrued add-back.
pub(crate) fn npv_full(
    cds: &CreditDefaultSwap,
    disc: &DiscountCurve,
    surv: &HazardCurve,
    as_of: Date,
) -> Result<f64> {
    let protection_pv = pv_protection_leg_raw(cds, disc, surv, as_of)?;
    let spread = spread_decimal(cds)?;
    let premium_pv =
        cds.notional.amount() * premium_leg_annuity(cds, disc, surv, as_of, spread, true)?;
    let upfront_pv = upfront_pv(cds, disc, as_of)?;
    let clean_accrued = clean_accrued_amount(cds, spread, as_of)?;
    Ok(signed_npv(
        cds,
        protection_pv,
        premium_pv,
        upfront_pv,
        clean_accrued,
    ))
}
