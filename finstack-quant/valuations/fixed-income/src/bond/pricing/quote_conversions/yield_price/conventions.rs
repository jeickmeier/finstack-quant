//! Day-count timing conventions (ICMA, Street, ACT/365L) and Japanese simple yield.

use super::super::types::YieldCompounding;
use crate::bond::Bond;
use finstack_quant_cashflows::builder::{icma_reference_period, icma_year_fraction};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;
use rust_decimal::prelude::ToPrimitive;

/// Actual payment date, yield time in years and native-currency payment.
pub(crate) type TimedCashflows = Vec<(Date, f64, Money)>;

/// Measure raw dated cashflows using the context their dates can establish.
///
/// # Arguments
///
/// * `day_count` - Convention for elapsed yield time; ACT/365L requires unavailable coupon metadata.
/// * `frequency` - Contractual coupon tenor for ICMA reference-period inference.
/// * `flows` - Dated signed cashflows in payment order.
/// * `as_of` - Yield settlement date; earlier or equal payments receive zero time.
pub(crate) fn flow_times(
    day_count: finstack_quant_core::dates::DayCount,
    frequency: finstack_quant_core::dates::Tenor,
    flows: &[(Date, Money)],
    as_of: Date,
) -> finstack_quant_core::Result<TimedCashflows> {
    let ref_period = icma_reference_period(
        day_count,
        frequency,
        flows.iter().map(|(date, _)| *date),
        as_of,
    );
    flows
        .iter()
        .map(|&(date, amount)| {
            let time = if date <= as_of {
                0.0
            } else {
                icma_year_fraction(day_count, Some(frequency), ref_period, as_of, date)?
            };
            Ok((date, time, amount))
        })
        .collect()
}

/// Measure ACT/365L bond flows on contractual coupon dates for quoted yield.
///
/// Operational payment adjustment or lag does not extend the coupon clock.
/// Actual payment dates remain on each flow for cashflow entitlement and curve
/// discounting. Custom flows sharing one payment date must identify one
/// contractual coupon end; aggregated distinct coupon ends are ambiguous.
///
/// # Arguments
///
/// * `bond` - Bond supplying coupon convention, tenor, and actual contractual periods.
/// * `flows` - Dated signed cashflows in payment order.
/// * `as_of` - Yield settlement date; earlier or equal payments receive zero time.
///
/// # References
///
/// - [OpenGamma Bond Pricing, section 7](https://quant.opengamma.io/Bond-Pricing-OpenGamma.pdf):
///   quoted yield uses contractual coupon timing independently of exact payment dates.
pub(crate) fn bond_flow_times(
    bond: &Bond,
    flows: &[(Date, Money)],
    as_of: Date,
) -> finstack_quant_core::Result<TimedCashflows> {
    use finstack_quant_core::dates::DayCount;

    let day_count = bond.cashflow_spec.day_count();
    let frequency = bond.cashflow_spec.frequency();
    match day_count {
        DayCount::Act365L => {
            let coupons = crate::bond::pricing::time_basis::bond_coupon_dates(bond)?;
            act365l_flow_times(&coupons, flows, as_of)
        }
        DayCount::ActActIsma => icma_bond_flow_times(bond, flows, as_of),
        _ => flow_times(day_count, frequency, flows, as_of),
    }
}

fn icma_bond_flow_times(
    bond: &Bond,
    flows: &[(Date, Money)],
    as_of: Date,
) -> finstack_quant_core::Result<TimedCashflows> {
    use finstack_quant_cashflows::builder::{BuildPeriodsParams, build_periods, period_accrual};

    if bond.custom_cashflows.is_some() {
        return Err(finstack_quant_core::Error::Validation(
            "ICMA quoted yield requires a qualified contractual coupon grid".into(),
        ));
    }
    let mut params = BuildPeriodsParams::from_schedule(
        bond.cashflow_spec.schedule(),
        bond.issue_date,
        bond.maturity,
        None,
    );
    let mut periods = build_periods(params)?;
    params.adjust_accrual_dates = false;
    for period in &mut periods {
        period.accrual_start = period.unadjusted_start;
        period.accrual_end = period.unadjusted_end;
    }
    flows
        .iter()
        .map(|&(date, amount)| {
            if date <= as_of {
                return Ok((date, 0.0, amount));
            }
            let mut matching_periods = periods.iter().filter(|period| period.payment_date == date);
            let contractual_date = matching_periods
                .next()
                .map_or(date, |period| period.unadjusted_end);
            if matching_periods.any(|period| period.unadjusted_end != contractual_date)
                || contractual_date <= as_of
                || contractual_date > bond.maturity
                || as_of < bond.issue_date
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ICMA quoted yield cannot resolve a contractual time for cash on {date}"
                )));
            }
            let mut years = NeumaierAccumulator::new();
            for period in &periods {
                let start = as_of.max(period.unadjusted_start);
                let end = contractual_date.min(period.unadjusted_end);
                if start < end {
                    years.add(period_accrual(period, start, end, &params)?)?;
                }
            }
            Ok((date, years.get_total()?, amount))
        })
        .collect()
}

/// Prepare Street yield cash on the regular fixed-coupon bullet grid.
///
/// # Arguments
///
/// * `bond` - Bond supplying the contractual coupon intervals and payment dates.
/// * `flows` - Entitled native-currency cash by actual payment date.
/// * `as_of` - Buyer settlement date used for the broken coupon fraction.
pub(crate) fn street_flow_times(
    bond: &Bond,
    flows: &[(Date, Money)],
    as_of: Date,
) -> finstack_quant_core::Result<(TimedCashflows, YieldCompounding)> {
    use finstack_quant_cashflows::builder::{
        BuildPeriodsParams, CouponType, RollRule, build_periods, period_accrual,
    };
    use finstack_quant_core::dates::{DateExt, DayCount};

    let other_profile = || {
        Ok((
            bond_flow_times(bond, flows, as_of)?,
            YieldCompounding::Street,
        ))
    };
    let crate::bond::CashflowSpec::Fixed(spec) = &bond.cashflow_spec else {
        return other_profile();
    };
    let schedule = &spec.schedule;
    if spec.coupon_type != CouponType::Cash
        || !matches!(
            schedule.day_count,
            DayCount::Thirty360 | DayCount::ActActIsma
        )
        || !matches!(schedule.roll_rule, RollRule::None)
        || bond.custom_cashflows.is_some()
        || as_of < bond.issue_date
        || bond.maturity <= as_of
    {
        return other_profile();
    }
    let Some(months) = schedule
        .frequency
        .months()
        .and_then(|months| i32::try_from(months).ok())
    else {
        return other_profile();
    };
    if months == 0 || 12 % months != 0 {
        return other_profile();
    }

    let mut params =
        BuildPeriodsParams::from_schedule(schedule, bond.issue_date, bond.maturity, None);
    let periods = build_periods(params)?;
    if bond.has_exercise_rights()
        && !periods
            .last()
            .is_some_and(|terminal| flows.iter().any(|(date, _)| *date == terminal.payment_date))
    {
        // An exercise path has a different contractual endpoint.
        return other_profile();
    }
    for period in &periods {
        let regular_start = period.unadjusted_end.add_months(-months)?;
        let regular_start = if schedule.end_of_month {
            regular_start.end_of_month()
        } else {
            regular_start
        };
        if period.unadjusted_start != regular_start {
            return other_profile();
        }
    }
    let first = periods.partition_point(|period| period.unadjusted_end <= as_of);
    let mut broken = *periods.get(first).ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Street yield requires a remaining contractual coupon interval".into(),
        )
    })?;
    params.adjust_accrual_dates = false;
    broken.accrual_start = broken.unadjusted_start;
    broken.accrual_end = broken.unadjusted_end;
    let full = period_accrual(&broken, broken.accrual_start, broken.accrual_end, &params)?;
    let remaining = period_accrual(&broken, as_of, broken.accrual_end, &params)?;
    if !(full.is_finite() && remaining.is_finite() && remaining > 0.0 && remaining <= full) {
        return Err(finstack_quant_core::Error::Validation(
            "Street yield requires a positive broken coupon fraction on the contractual grid"
                .into(),
        ));
    }
    let fraction = remaining / full;
    let frequency = f64::from(12 / months);
    let timed = flows
        .iter()
        .map(|&(date, amount)| {
            if date <= as_of {
                return Ok((date, 0.0, amount));
            }
            let mut slots = periods
                .iter()
                .enumerate()
                .filter(|(_, period)| period.payment_date == date);
            let slot = slots.next().map(|(slot, _)| slot);
            match slot {
                Some(slot) if slot >= first && slots.next().is_none() => {
                    Ok((date, (fraction + (slot - first) as f64) / frequency, amount))
                }
                _ => Err(finstack_quant_core::Error::Validation(format!(
                    "Street yield cannot resolve the contractual coupon slot for cash on {date}"
                ))),
            }
        })
        .collect::<finstack_quant_core::Result<Vec<_>>>()?;
    let compounding = if first + 1 == periods.len() {
        YieldCompounding::Rate(finstack_quant_core::math::Compounding::Simple)
    } else {
        YieldCompounding::Street
    };
    Ok((timed, compounding))
}

/// Measure ACT/365L quoted flows using explicit contractual coupon metadata.
///
/// Payment adjustment does not extend the contractual yield clock. Multiple
/// distinct coupon ends on one aggregated payment date cannot identify that
/// clock uniquely and are rejected.
///
/// # Arguments
///
/// * `coupons` - Payment dates and full accrual boundaries as `(payment, start, end)`.
/// * `flows` - Dated signed coupon and principal amounts in payment order.
/// * `as_of` - Settlement date; earlier or equal payments receive zero time.
pub(crate) fn act365l_flow_times(
    coupons: &[(Date, Date, Date)],
    flows: &[(Date, Money)],
    as_of: Date,
) -> finstack_quant_core::Result<TimedCashflows> {
    use crate::bond::pricing::time_basis::act365l_year_fraction;
    let mut periods: Vec<_> = coupons
        .iter()
        .map(|&(_, start, end)| (start, end))
        .collect();
    periods.sort_unstable();
    periods.dedup();
    flows
        .iter()
        .map(|&(date, amount)| {
            let time = if date <= as_of {
                0.0
            } else {
                let mut contractual_end = None;
                for &(_, _, end) in coupons.iter().filter(|&&(payment, _, _)| payment == date) {
                    if contractual_end.is_some_and(|previous| previous != end) {
                        return Err(finstack_quant_core::Error::Validation(
                            "ACT/365L quoted yield cannot time aggregated coupons with different \
                             contractual ends on one payment date"
                                .into(),
                        ));
                    }
                    contractual_end = Some(end);
                }
                // Workout dates inside a coupon retain the actual coupon's
                // denominator; ordinary coupon and redemption payments use
                // their contractual coupon end even when payment is delayed.
                let end = contractual_end.unwrap_or(date);
                if end <= as_of {
                    return Err(finstack_quant_core::Error::Validation(
                        "ACT/365L quoted yield requires settlement before the contractual end \
                         of an unpaid coupon"
                            .into(),
                    ));
                }
                act365l_year_fraction(&periods, as_of, end)?
            };
            Ok((date, time, amount))
        })
        .collect()
}

/// JSDA inputs of a Japanese simple yield (単利): annual coupon rate and
/// ACT/365F remaining life from `quote_date` to maturity.
///
/// Only bullet fixed-rate bonds carry a Tokyo simple yield; other coupon
/// shapes are rejected, as is a non-positive remaining life.
fn japanese_simple_yield_terms(
    bond: &Bond,
    quote_date: Date,
) -> finstack_quant_core::Result<(f64, f64)> {
    let crate::bond::CashflowSpec::Fixed(spec) = &bond.cashflow_spec else {
        return Err(finstack_quant_core::Error::Validation(
            "Japanese simple yield requires a fixed-coupon bond".to_string(),
        ));
    };
    let coupon = spec.rate.to_f64().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "bond '{}' coupon rate {} is not representable as f64",
            bond.id, spec.rate
        ))
    })?;
    let n = finstack_quant_core::dates::DayCount::Act365F.year_fraction(
        quote_date,
        bond.maturity,
        DayCountContext::default(),
    )?;
    if n <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "Japanese simple yield requires positive ACT/365F remaining life".to_string(),
        ));
    }
    Ok((coupon, n))
}

/// Japanese simple yield (単利) from a clean price, JSDA convention.
///
/// ```text
/// y = [C + (100 − P) / N] / P
/// ```
///
/// with `C` the annual coupon (% of par), `P` the CLEAN price (% of par) and
/// `N` the ACT/365F remaining life in years from `quote_date`. Accrued
/// interest plays no part in the formula.
///
/// # Arguments
///
/// * `bond` - Bullet fixed-rate bond supplying the coupon and maturity.
/// * `quote_date` - Settlement/quote date from which `N` is measured.
/// * `clean_price_pct` - Clean price as a percentage of par (e.g. `98.5`);
///   must be positive.
pub(crate) fn japanese_simple_yield(
    bond: &Bond,
    quote_date: Date,
    clean_price_pct: f64,
) -> finstack_quant_core::Result<f64> {
    let (coupon, n) = japanese_simple_yield_terms(bond, quote_date)?;
    if !clean_price_pct.is_finite() || clean_price_pct <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Japanese simple yield requires a positive clean price (got {clean_price_pct})"
        )));
    }
    Ok((100.0 * coupon + (100.0 - clean_price_pct) / n) / clean_price_pct)
}

/// Clean price (% of par) from a Japanese simple yield (単利), the exact
/// inverse of [`japanese_simple_yield`]:
///
/// ```text
/// P = 100 · (1 + C·N) / (1 + y·N)
/// ```
///
/// This is not a discount-factor convention and must not be used as
/// [`super::df_from_yield`] compounding.
///
/// # Arguments
///
/// * `bond` - Bullet fixed-rate bond supplying the coupon and maturity.
/// * `quote_date` - Settlement/quote date from which ACT/365F remaining life
///   `N` is measured to `bond.maturity`.
/// * `simple_yield` - Tokyo simple yield as a decimal (e.g. `0.02` for 2%).
pub(crate) fn clean_price_from_japanese_simple_yield(
    bond: &Bond,
    quote_date: Date,
    simple_yield: f64,
) -> finstack_quant_core::Result<f64> {
    let (coupon, n) = japanese_simple_yield_terms(bond, quote_date)?;
    let denom = 1.0 + simple_yield * n;
    if denom <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Japanese simple yield denominator (1 + y*n) = {denom} is non-positive for y={simple_yield}, n={n}"
        )));
    }
    let clean_pct = 100.0 * (1.0 + coupon * n) / denom;
    if clean_pct <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "Japanese simple yield implies a non-positive clean price".to_string(),
        ));
    }
    Ok(clean_pct)
}
