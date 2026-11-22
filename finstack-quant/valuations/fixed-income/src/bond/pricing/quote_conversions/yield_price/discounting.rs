//! Yield discount-factor formulas and yield-to-maturity cashflow discounting.

use super::super::types::YieldCompounding;
use super::conventions::{flow_times, street_flow_times};
use crate::bond::Bond;
use finstack_quant_core::dates::Date;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;

/// Discount factor from yield.
///
/// Computes the discount factor for a given yield, time, and compounding convention.
///
/// # Arguments
///
/// * `ytm` - Yield to maturity as decimal (e.g., 0.05 for 5%)
/// * `t` - Time in years from valuation date to cashflow date
/// * `comp` - Compounding convention (see [`YieldCompounding`])
/// * `bond_frequency` - Bond's coupon frequency (used for `Street` and `Moosmuller`)
///
/// # Compounding Formulas
///
/// | Convention | Formula |
/// |------------|---------|
/// | Simple | `1 / (1 + y * t)` |
/// | Annual | `(1 + y)^(-t)` |
/// | Periodic(m) | `(1 + y/m)^(-m*t)` |
/// | Continuous | `exp(-y*t)` |
/// | Street | `(1 + y/f)^(-f*t)` where f = frequency |
/// | Moosmuller | `1/(1 + y*w) * (1 + y/f)^(-(k-1))` (schedule-aware in pricing) |
///
/// # Errors
///
/// Returns `Err` if the bond frequency is invalid (zero periods).
///
/// # Negative Yields
///
/// Negative yields are supported for all compounding conventions. However:
/// - **Extreme negative yields** (< -50%) will log a warning as they often indicate
///   data or input errors.
/// - For periodic/annual compounding, yields more negative than `-m` (where `m` is
///   compounding frequency) would make `(1 + y/m)` negative, leading to `NaN` from
///   `powf`. Such cases return `Err`.
/// - Discount factors > 1.0 are mathematically valid for negative rates but unusual
///   in practice.
#[inline]
pub fn df_from_yield(
    ytm: f64,
    t: f64,
    comp: YieldCompounding,
    bond_frequency: finstack_quant_core::dates::Tenor,
) -> finstack_quant_core::Result<f64> {
    if t <= 0.0 {
        return Ok(1.0);
    }

    // Warn on extreme negative yields which often indicate data errors
    if ytm < -0.5 {
        tracing::warn!(
            ytm = ytm,
            "Extreme negative yield detected (< -50%). This may indicate a data error."
        );
    }

    Ok(match comp {
        YieldCompounding::Rate(rate_comp) => {
            let df = rate_comp.df_from_rate(ytm, t)?;
            if !df.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{rate_comp} compounding base is non-positive for ytm={ytm}, t={t}"
                )));
            }
            df
        }
        YieldCompounding::Street => {
            let m = bond_frequency.payments_per_year()?.max(1.0);
            let base = 1.0 + ytm / m;
            if base <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Street compounding base (1 + y/m) = {} is non-positive for ytm={}, m={}",
                    base, ytm, m
                )));
            }
            base.powf(-m * t)
        }
        YieldCompounding::Moosmuller => {
            // Time-based Moosmüller: the fractional period at the start is `w`
            // and the remaining whole coupon periods are `k-1`. On a period
            // boundary (`t` is a multiple of `1/m`) this collapses to Street:
            // `w = 1/m` and `DF = (1 + y/m)^(-m*t)`.
            let m = bond_frequency.payments_per_year()?.max(1.0);
            let n_full = (t * m).floor();
            let mut w = t - n_full / m;
            if w <= 1e-12 {
                w = 1.0 / m;
            }
            // Whole coupon periods after the first are `round((t - w) * m)`.
            let k_minus_1 = if t <= w + 1e-12 {
                0.0
            } else {
                ((t - w) * m).round().max(0.0)
            };
            moosmuller_df(ytm, m, w, k_minus_1)?
        }
    })
}

/// Price from yield using explicit day count and frequency (no `Bond` borrow required).
///
/// For the [`YieldCompounding::Moosmuller`] convention the first (potentially
/// irregular) coupon period is flagged from the **cashflow schedule** — the
/// year-fraction to the first post-`as_of` flow — rather than inferred from time.
///
/// For ACT/ACT (ICMA) day counts, a reference coupon period inferred from the
/// cashflow schedule (via the internal `icma_reference_period` helper) is supplied to the
/// day-count context so mid-coupon settlement dates and stub spans resolve on
/// the quasi-coupon grid instead of erroring.
/// ACT/365L requires contractual coupon boundaries that raw dated flows do
/// not carry; use [`price_from_ytm`] with a bond supplying its schedule instead.
///
/// # Arguments
///
/// * `day_count` - Bond coupon day-count convention used to measure settlement-
///   to-cashflow time.
/// * `frequency` - Contractual coupon frequency, including ACT/ACT reference-period
///   context and periodic compounding frequency.
/// * `flows` - Dated signed bond cashflows in payment-date order; flows on or
///   before `as_of` are excluded.
/// * `as_of` - Yield settlement/valuation date from which cashflows discount.
/// * `ytm` - Annual yield to maturity as a decimal under `comp`.
/// * `comp` - Yield compounding convention used to turn `ytm` into discount
///   factors.
#[inline]
pub fn price_from_ytm_compounded_params(
    day_count: finstack_quant_core::dates::DayCount,
    frequency: finstack_quant_core::dates::Tenor,
    flows: &[(Date, Money)],
    as_of: Date,
    ytm: f64,
    comp: YieldCompounding,
) -> finstack_quant_core::Result<f64> {
    let timed = flow_times(day_count, frequency, flows, as_of)?;
    price_from_ytm_timed(&timed, frequency, ytm, comp)
}

/// Discount already measured bond cashflows under the selected yield convention.
///
/// # Arguments
///
/// * `flows` - Payment dates, elapsed year fractions, and signed currency amounts.
/// * `frequency` - Contractual coupon tenor used by Street and Moosmüller yields.
/// * `ytm` - Annual yield expressed as a decimal under `comp`.
/// * `comp` - Convention converting elapsed time and yield into discount factors.
pub(crate) fn price_from_ytm_timed(
    flows: &[(Date, f64, Money)],
    frequency: finstack_quant_core::dates::Tenor,
    ytm: f64,
    comp: YieldCompounding,
) -> finstack_quant_core::Result<f64> {
    // Schedule-aware first-period length for Moosmüller: the year-fraction
    // from `as_of` to the first cashflow strictly after `as_of`.
    let schedule_first_period = if matches!(comp, YieldCompounding::Moosmuller) {
        flows.iter().map(|&(_, yf, _)| yf).find(|&yf| yf > 0.0)
    } else {
        None
    };

    let mut pv = NeumaierAccumulator::new();
    let mut moosmuller_k: u32 = 0;
    let mut moosmuller_date: Option<Date> = None;
    for &(date, t, amount) in flows {
        if t > 0.0 {
            let df = match (comp, schedule_first_period) {
                (YieldCompounding::Moosmuller, Some(w)) => {
                    // Coupon + redemption on the same payment date share `k`.
                    if moosmuller_date != Some(date) {
                        moosmuller_k = moosmuller_k.saturating_add(1);
                        moosmuller_date = Some(date);
                    }
                    let m = frequency.payments_per_year()?.max(1.0);
                    moosmuller_df(ytm, m, w, f64::from(moosmuller_k.saturating_sub(1)))?
                }
                _ => df_from_yield(ytm, t, comp, frequency)?,
            };
            pv.add(amount.amount() * df)?;
        }
    }
    pv.get_total()
}

/// Moosmüller discount factor of the coupon `k_minus_1` whole periods after
/// the first (simple-interest) period of length `w`.
///
/// ```text
/// DF_1 = 1 / (1 + y * w)
/// DF_k = 1 / (1 + y * w) * (1 + y / f)^{1-k}   for k ≥ 2
/// ```
///
/// `w` is the year fraction from settlement to the next coupon and `f` is `m`.
fn moosmuller_df(ytm: f64, m: f64, w: f64, k_minus_1: f64) -> finstack_quant_core::Result<f64> {
    let simple_denom = 1.0 + ytm * w;
    if simple_denom <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Moosmüller simple-interest denom (1 + y*w) = {simple_denom} is non-positive for ytm={ytm}, w={w}"
        )));
    }
    let df_simple = 1.0 / simple_denom;
    if k_minus_1 <= 0.0 {
        return Ok(df_simple);
    }
    let periodic_base = 1.0 + ytm / m;
    if periodic_base <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Moosmüller periodic base (1 + y/m) = {periodic_base} is non-positive for ytm={ytm}, m={m}"
        )));
    }
    Ok(df_simple * periodic_base.powf(-k_minus_1))
}

/// Price from ytm (using Street convention).
///
/// # Arguments
///
/// * `bond` - Bond supplying coupon day count and frequency conventions.
/// * `flows` - Dated signed bond cashflows to discount; flows on or before
///   `as_of` are excluded.
/// * `as_of` - Yield settlement/valuation date from which cashflows discount.
/// * `ytm` - Annual Street-compounded yield to maturity as a decimal.
pub fn price_from_ytm(
    bond: &Bond,
    flows: &[(Date, Money)],
    as_of: Date,
    ytm: f64,
) -> finstack_quant_core::Result<f64> {
    let (timed, compounding) = street_flow_times(bond, flows, as_of)?;
    price_from_ytm_timed(&timed, bond.cashflow_spec.frequency(), ytm, compounding)
}
