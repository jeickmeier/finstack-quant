use super::annuity::periods_per_year;
use super::types::YieldCompounding;
use super::ExitCandidate;
use crate::cashflow::accrual::AccrualIndex;
use crate::cashflow::builder::CashFlowSchedule;
use crate::cashflow::primitives::CFKind;
use crate::instruments::fixed_income::bond::pricing::engine::tree::BondValuator;
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::fixed_income::bond::pricing::ytm_solver::{solve_ytm, YtmPricingSpec};
use crate::instruments::fixed_income::bond::Bond;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;
use rust_decimal::prelude::ToPrimitive;

/// Discount factor from yield.
///
/// Computes the discount factor for a given yield, time, and compounding convention.
///
/// # Arguments
///
/// * `ytm` - Yield to maturity as decimal (e.g., 0.05 for 5%)
/// * `t` - Time in years from valuation date to cashflow date
/// * `comp` - Compounding convention (see [`YieldCompounding`])
/// * `bond_frequency` - Bond's coupon frequency (used for `Street` and `TreasuryActual`)
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
/// | TreasuryActual | Simple for t < 1/f, then periodic |
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
        YieldCompounding::Simple => {
            let denom = 1.0 + ytm * t;
            // Check for non-positive denominator which would give invalid discount factor
            if denom <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Simple interest denominator (1 + y*t) = {} is non-positive for ytm={}, t={}",
                    denom, ytm, t
                )));
            }
            1.0 / denom
        }
        YieldCompounding::Annual => {
            let base = 1.0 + ytm;
            if base <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Annual compounding base (1 + y) = {} is non-positive for ytm={}",
                    base, ytm
                )));
            }
            base.powf(-t)
        }
        YieldCompounding::Periodic(m) => {
            let m = m as f64;
            let base = 1.0 + ytm / m;
            if base <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Periodic compounding base (1 + y/m) = {} is non-positive for ytm={}, m={}",
                    base, ytm, m
                )));
            }
            base.powf(-m * t)
        }
        YieldCompounding::Continuous => (-ytm * t).exp(),
        YieldCompounding::Street => {
            let m = periods_per_year(bond_frequency)?.max(1.0);
            let base = 1.0 + ytm / m;
            if base <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Street compounding base (1 + y/m) = {} is non-positive for ytm={}, m={}",
                    base, ytm, m
                )));
            }
            base.powf(-m * t)
        }
        YieldCompounding::TreasuryActual => {
            // ISDA/Treasury actual convention:
            // - Use simple interest for the first (potentially irregular) period
            // - Use periodic compounding for subsequent full periods
            //
            // LIMITATION: Stub period detection is TIME-BASED, not SCHEDULE-AWARE.
            // We identify the first period as t < 1/frequency (i.e., less than
            // one full coupon period). This is a reasonable approximation that
            // captures the essence of the convention for standard bonds.
            //
            // For bonds with irregular first coupons that don't align with the
            // standard frequency (e.g., a long-first stub spanning 8 months on
            // a semi-annual bond), this heuristic may misclassify the stub.
            // For exact ISDA compliance with non-standard structures, consider
            // passing actual stub information from the cashflow schedule.
            let m = periods_per_year(bond_frequency)?.max(1.0);
            let period_length = 1.0 / m;

            // Validate periodic compounding base for extreme negative yields
            let periodic_base = 1.0 + ytm / m;
            if periodic_base <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "TreasuryActual periodic base (1 + y/m) = {} is non-positive for ytm={}, m={}",
                    periodic_base, ytm, m
                )));
            }

            if t <= period_length {
                // First (potentially stub) period: simple interest
                let denom = 1.0 + ytm * t;
                if denom <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "TreasuryActual simple interest denom (1 + y*t) = {} is non-positive for ytm={}, t={}",
                        denom, ytm, t
                    )));
                }
                1.0 / denom
            } else {
                // For subsequent periods, we need to compound:
                // - Simple interest for the first period portion
                // - Periodic compounding for the remaining full periods
                //
                // Total time t = stub_time + n_full_periods / m
                // where stub_time <= period_length
                //
                // DF = DF_stub * DF_periodic
                //    = 1/(1 + y*stub) * (1 + y/m)^(-n_full_periods)
                let n_full_periods = (t * m).floor();
                let stub_time = t - n_full_periods / m;

                if stub_time > 1e-10 {
                    // Has a stub period
                    let stub_denom = 1.0 + ytm * stub_time;
                    if stub_denom <= 0.0 {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "TreasuryActual stub denom (1 + y*stub) = {} is non-positive for ytm={}, stub_time={}",
                            stub_denom, ytm, stub_time
                        )));
                    }
                    let df_stub = 1.0 / stub_denom;
                    let df_periodic = periodic_base.powf(-n_full_periods);
                    df_stub * df_periodic
                } else {
                    // No stub, pure periodic
                    periodic_base.powf(-m * t)
                }
            }
        }
        YieldCompounding::Moosmuller => {
            // Time-based Moosmüller: the fractional period at the start is `w`
            // and the remaining whole coupon periods are `k-1`. On a period
            // boundary (`t` is a multiple of `1/m`) this collapses to Street:
            // `w = 1/m` and `DF = (1 + y/m)^(-m*t)`.
            let m = periods_per_year(bond_frequency)?.max(1.0);
            let n_full = (t * m).floor();
            let mut w = t - n_full / m;
            if w <= 1e-12 {
                w = 1.0 / m;
            }
            df_moosmuller_with_first_period(ytm, t, m, w)?
        }
    })
}

/// Schedule-aware U.S. Treasury actual discount factor.
///
/// Treasury Appendix B decomposes the first payment horizon into an initial
/// fractional quasi-coupon and zero or more full coupon periods:
///
/// ```text
/// DF(t) = 1 / (1 + y * w) * (1 + y / m)^(-k)
/// ```
///
/// where `w < 1/m` is the initial fractional period and `k` is the number of
/// full periods through `t`. This distinction is material for a long first
/// coupon: the full regular part compounds periodically rather than being
/// folded into the simple-interest fraction.
///
/// Source: 31 CFR Part 356, Appendix B, section II.
pub(super) fn df_treasury_actual_with_first_period(
    ytm: f64,
    t: f64,
    m: f64,
    first_period_len: f64,
) -> finstack_quant_core::Result<f64> {
    if !m.is_finite() || m <= 0.0 || !first_period_len.is_finite() || first_period_len <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "TreasuryActual requires positive finite frequency and first-period length; \
             got m={m}, first_period_len={first_period_len}"
        )));
    }

    // Split a long first period into its initial fractional quasi-coupon and
    // complete regular periods. The epsilon keeps an exact regular boundary
    // from becoming a nearly-full simple period under floating-point noise.
    let complete_first_periods = (first_period_len * m + 1e-12).floor().max(0.0);
    let mut fractional_period = first_period_len - complete_first_periods / m;
    if fractional_period.abs() < 1e-12 {
        fractional_period = 0.0;
    }
    if fractional_period < 0.0 || fractional_period >= 1.0 / m + 1e-12 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "TreasuryActual could not decompose first-period length {first_period_len} \
             at frequency {m}"
        )));
    }

    let simple_denom = 1.0 + ytm * fractional_period.min(t);
    if simple_denom <= 0.0 || !simple_denom.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "TreasuryActual simple-interest denominator is invalid: {simple_denom}"
        )));
    }
    let df_simple = 1.0 / simple_denom;

    let periodic_periods = (m * (t - fractional_period).max(0.0)).round();
    if periodic_periods == 0.0 {
        return Ok(df_simple);
    }
    let periodic_base = 1.0 + ytm / m;
    if periodic_base <= 0.0 || !periodic_base.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "TreasuryActual periodic base (1 + y/m) is invalid: {periodic_base}"
        )));
    }
    Ok(df_simple * periodic_base.powf(-periodic_periods))
}

/// Moosmüller discount factor with a schedule-flagged first-period length `w`.
///
/// ```text
/// DF_1 = 1 / (1 + y * w)
/// DF_k = 1 / (1 + y * w) * (1 + y / f)^{1-k}   for k ≥ 2
/// ```
///
/// `w` is the year fraction from settlement to the next coupon and `f` is `m`.
/// Subsequent coupon counts are inferred as `round((t - w) * m)`.
pub(super) fn df_moosmuller_with_first_period(
    ytm: f64,
    t: f64,
    m: f64,
    first_period_len: f64,
) -> finstack_quant_core::Result<f64> {
    let w = first_period_len;
    let simple_denom = 1.0 + ytm * w;
    if simple_denom <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Moosmüller simple-interest denom (1 + y*w) = {simple_denom} is non-positive for ytm={ytm}, w={w}"
        )));
    }
    let df_simple = 1.0 / simple_denom;
    if t <= first_period_len + 1e-12 {
        return Ok(df_simple);
    }

    let periodic_base = 1.0 + ytm / m;
    if periodic_base <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Moosmüller periodic base (1 + y/m) = {periodic_base} is non-positive for ytm={ytm}, m={m}"
        )));
    }
    let k_minus_1 = ((t - first_period_len) * m).round().max(0.0);
    Ok(df_simple * periodic_base.powf(-k_minus_1))
}

/// Infer an ACT/ACT (ICMA) reference coupon period from the cashflow schedule.
///
/// Returns the quasi-coupon period `(first_flow − frequency, first_flow)`
/// surrounding the settlement date, where `first_flow` is the first cashflow
/// date strictly after `as_of`. Supplying this period through
/// [`DayCountContext::coupon_period`] lets ISMA year fractions resolve for
/// spans that are not whole multiples of the coupon frequency (mid-coupon
/// settlement, stubs), which the frequency-only path rejects; the core
/// reference-period traversal then walks the quasi-coupon grid anchored on
/// this period for every flow date.
///
/// # Arguments
///
/// * `day_count` - Bond coupon day-count convention; only
///   [`finstack_quant_core::dates::DayCount::ActActIsma`] yields a period.
/// * `frequency` - Contractual coupon frequency defining the quasi-coupon
///   grid; week/day frequencies are unsupported by ICMA and yield `None`.
/// * `dates` - Coupon payment dates in ascending order; the first date
///   strictly after `as_of` anchors the grid.
/// * `as_of` - Settlement/valuation date the reference period must surround.
///
/// Returns `None` for non-ICMA day counts, week/day frequencies, or when no
/// flow falls strictly after `as_of` (callers then keep the frequency-only
/// context and its existing error behavior).
pub(crate) fn icma_reference_period(
    day_count: finstack_quant_core::dates::DayCount,
    frequency: finstack_quant_core::dates::Tenor,
    dates: impl IntoIterator<Item = Date>,
    as_of: Date,
) -> Option<(Date, Date)> {
    use finstack_quant_core::dates::{DateExt, DayCount, TenorUnit};
    if day_count != DayCount::ActActIsma {
        return None;
    }
    let months = match frequency.unit() {
        TenorUnit::Months => frequency.count() as i32,
        TenorUnit::Years => (frequency.count() as i32).checked_mul(12)?,
        TenorUnit::Weeks | TenorUnit::Days => return None,
    };
    if months <= 0 {
        return None;
    }
    let next = dates.into_iter().find(|d| *d > as_of)?;
    let mut prev = next.add_months(-months);
    // Preserve the end-of-month roll so the quasi-coupon grid matches an
    // EOM schedule (mirrors the traversal's own EOM handling).
    if next == next.end_of_month() {
        prev = prev.end_of_month();
    }
    (prev < next).then_some((prev, next))
}

/// Price from yield using explicit day count and frequency (no `Bond` borrow required).
///
/// For the [`YieldCompounding::TreasuryActual`] convention the first (potentially
/// irregular) coupon period is flagged from the **cashflow schedule** — the
/// year-fraction to the first post-`as_of` flow — rather than inferred from time.
/// This keeps the YTM↔price conversion correct for new issues with long first
/// coupons, where the time-based `t <= 1/m` heuristic in [`df_from_yield`] would
/// misapply simple interest to the wrong horizon.
///
/// For ACT/ACT (ICMA) day counts, a reference coupon period inferred from the
/// cashflow schedule (via the internal `icma_reference_period` helper) is supplied to the
/// day-count context so mid-coupon settlement dates and stub spans resolve on
/// the quasi-coupon grid instead of erroring.
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
    // ACT/ACT (ICMA) requires the coupon frequency in the day-count context;
    // the default context hard-errors for that convention. The schedule-derived
    // reference coupon period additionally lets ISMA resolve mid-coupon
    // settlement and stub spans that are not whole coupon multiples.
    let dc_ctx = DayCountContext {
        frequency: Some(frequency),
        coupon_period: icma_reference_period(
            day_count,
            frequency,
            flows.iter().map(|(d, _)| *d),
            as_of,
        ),
        ..DayCountContext::default()
    };

    // Schedule-aware first-period length for TreasuryActual / Moosmüller:
    // the year-fraction from `as_of` to the first cashflow strictly after `as_of`.
    let schedule_first_period = if matches!(
        comp,
        YieldCompounding::TreasuryActual | YieldCompounding::Moosmuller
    ) {
        let mut first: Option<f64> = None;
        for &(date, _) in flows {
            if date <= as_of {
                continue;
            }
            let yf = day_count.year_fraction(as_of, date, dc_ctx)?;
            if yf > 0.0 {
                first = Some(yf);
                break;
            }
        }
        first
    } else {
        None
    };

    let mut pv = NeumaierAccumulator::new();
    let mut moosmuller_k: u32 = 0;
    let mut moosmuller_date: Option<Date> = None;
    for &(date, amount) in flows {
        if date <= as_of {
            continue;
        }
        let t = day_count.year_fraction(as_of, date, dc_ctx)?;
        if t > 0.0 {
            let df = match (comp, schedule_first_period) {
                (YieldCompounding::TreasuryActual, Some(first_period_len)) => {
                    let m = periods_per_year(frequency)?.max(1.0);
                    df_treasury_actual_with_first_period(ytm, t, m, first_period_len)?
                }
                (YieldCompounding::Moosmuller, Some(w)) => {
                    // Coupon + redemption on the same payment date share `k`.
                    if moosmuller_date != Some(date) {
                        moosmuller_k = moosmuller_k.saturating_add(1);
                        moosmuller_date = Some(date);
                    }
                    let m = periods_per_year(frequency)?.max(1.0);
                    moosmuller_df_for_coupon(ytm, m, w, moosmuller_k)?
                }
                _ => df_from_yield(ytm, t, comp, frequency)?,
            };
            pv.add(amount.amount() * df);
        }
    }
    Ok(pv.total())
}

fn moosmuller_df_for_coupon(ytm: f64, m: f64, w: f64, k: u32) -> finstack_quant_core::Result<f64> {
    let simple_denom = 1.0 + ytm * w;
    if simple_denom <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Moosmüller simple-interest denom (1 + y*w) = {simple_denom} is non-positive for ytm={ytm}, w={w}"
        )));
    }
    let df_simple = 1.0 / simple_denom;
    if k <= 1 {
        return Ok(df_simple);
    }
    let periodic_base = 1.0 + ytm / m;
    if periodic_base <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Moosmüller periodic base (1 + y/m) = {periodic_base} is non-positive for ytm={ytm}, m={m}"
        )));
    }
    Ok(df_simple * periodic_base.powf(-f64::from(k.saturating_sub(1))))
}

/// Price from ytm compounded.
///
/// # Arguments
///
/// * `bond` - Bond supplying coupon day count and frequency conventions.
/// * `flows` - Dated signed bond cashflows to discount; flows on or before
///   `as_of` are excluded.
/// * `as_of` - Yield settlement/valuation date from which cashflows discount.
/// * `ytm` - Annual yield to maturity as a decimal under `comp`.
/// * `comp` - Yield compounding convention used to turn `ytm` into discount
///   factors.
pub fn price_from_ytm_compounded(
    bond: &Bond,
    flows: &[(Date, Money)],
    as_of: Date,
    ytm: f64,
    comp: YieldCompounding,
) -> finstack_quant_core::Result<f64> {
    price_from_ytm_compounded_params(
        bond.cashflow_spec.day_count(),
        bond.cashflow_spec.frequency(),
        flows,
        as_of,
        ytm,
        comp,
    )
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
    price_from_ytm_compounded(bond, flows, as_of, ytm, YieldCompounding::Street)
}

/// Dirty price in currency from a Japanese simple yield (単利).
///
/// Closed form for a **bullet fixed-rate** bond, using ACT/365F remaining life
/// and the contractual annual coupon rate:
///
/// ```text
/// P = 100 * (1 + C * n) / (1 + y * n)
/// dirty = P / 100 * notional
/// ```
///
/// This is not a discount-factor convention and must not be used as
/// [`df_from_yield`] compounding.
///
/// # Arguments
///
/// * `bond` - Bullet fixed-rate bond supplying the annual coupon rate,
///   notional, and contractual maturity used for remaining life `n`.
/// * `quote_date` - Settlement/quote date from which ACT/365F remaining life
///   is measured to `bond.maturity`.
/// * `simple_yield` - Tokyo simple yield as a decimal (e.g. `0.02` for 2%).
pub(crate) fn price_from_japanese_simple_yield(
    bond: &Bond,
    quote_date: Date,
    simple_yield: f64,
) -> finstack_quant_core::Result<f64> {
    let crate::instruments::fixed_income::bond::CashflowSpec::Fixed(spec) = &bond.cashflow_spec
    else {
        return Err(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::Invalid,
        ));
    };
    let coupon = spec.rate.to_f64().unwrap_or(0.0);
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
    let denom = 1.0 + simple_yield * n;
    if denom <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Japanese simple yield denominator (1 + y*n) = {denom} is non-positive for y={simple_yield}, n={n}"
        )));
    }
    let dirty_pct = 100.0 * (1.0 + coupon * n) / denom;
    if dirty_pct <= 0.0 {
        return Err(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::Invalid,
        ));
    }
    Ok(dirty_pct / 100.0 * bond.notional.amount())
}

/// Compute outstanding principal at a given date from the cashflow schedule.
///
/// This is used by YTW and other return calculations to determine the
/// redemption amount for callable/putable bonds after every seasoned balance
/// event, including amortization, PIK, later draws, and repayments.
pub(crate) fn outstanding_principal_at_date(
    schedule: &CashFlowSchedule,
    target_date: Date,
) -> finstack_quant_core::Result<f64> {
    let path = schedule.outstanding_by_date()?;
    let index = path.partition_point(|(date, _)| *date <= target_date);
    Ok(if index == 0 {
        schedule.get_notional().initial.amount()
    } else {
        path[index - 1].1.amount()
    }
    .max(0.0))
}

/// Resolve the principal struck by an exercise right and any contractual
/// redemption that the exercise proceeds replace on the same date.
///
/// Scheduled amortization and PIK are applied before exercise. At contractual
/// maturity, however, the positive `Notional` flow is the hold alternative,
/// so the strike is quoted on the balance immediately before that redemption
/// and the normal redemption must be removed from the candidate cashflow path.
pub(crate) fn exercise_principal_and_replaced_redemption(
    schedule: &CashFlowSchedule,
    exercise_date: Date,
    maturity: Date,
) -> finstack_quant_core::Result<(f64, f64)> {
    let after_events = outstanding_principal_at_date(schedule, exercise_date)?;
    let (principal_redeemed, replaced_redemption) = if exercise_date == maturity {
        schedule
            .get_flows()
            .iter()
            .filter(|flow| {
                flow.get_balance_date() == exercise_date
                    && flow.kind == CFKind::Notional
                    && flow.amount.amount() > 0.0
            })
            .fold((0.0, 0.0), |(principal, same_day_cash), flow| {
                (
                    principal + flow.amount.amount(),
                    same_day_cash
                        + if flow.date == exercise_date {
                            flow.amount.amount()
                        } else {
                            0.0
                        },
                )
            })
    } else {
        (0.0, 0.0)
    };
    Ok((
        (after_events + principal_redeemed).max(0.0),
        replaced_redemption,
    ))
}

/// Resolve the dirty exercise-date cash amount for one workout candidate.
///
/// Issuer calls retain the deterministic make-whole term and use the same
/// reference-curve valuation as the tree pricer. Put candidates have no
/// make-whole term. The returned amount is net of any contractual maturity
/// redemption already present in the truncated flow path.
pub(crate) fn exercise_redemption_amount(
    bond: &Bond,
    curves: &MarketContext,
    flows: &[(Date, Money)],
    schedule: &CashFlowSchedule,
    accrual_index: &AccrualIndex,
    candidate: &ExitCandidate,
) -> finstack_quant_core::Result<f64> {
    let (outstanding, replaced_redemption) =
        exercise_principal_and_replaced_redemption(schedule, candidate.date, bond.maturity)?;
    let accrued = accrual_index.accrued_at(candidate.date)?;
    let floor_price = outstanding * candidate.price_pct_of_par / 100.0;
    let clean_redemption = if let Some(spec) = &candidate.make_whole {
        let reference_curve = curves.get_discount(&spec.reference_curve_id)?;
        BondValuator::make_whole_call_price(
            spec,
            reference_curve.as_ref(),
            candidate.date,
            flows,
            floor_price,
            accrued,
        )?
    } else {
        floor_price
    };
    Ok(clean_redemption + accrued - replaced_redemption)
}

/// Enumerate call/put exit candidates for yield-to-worst analysis.
///
/// For each call or put window `[start_date, end_date]` in `bond.call_put`,
/// this function produces one `ExitCandidate` per admissible exercise date:
///
/// 1. Seed with the exact contractual `start_date` and `end_date`.
/// 2. Extend with any flow dates that fall within `[start_date, end_date]`.
/// 3. Sort and de-duplicate the resulting dates.
/// 4. Retain only dates in `[as_of, bond.maturity]`.
///
/// Returns an empty `Vec` when the bond has no `call_put` schedule.
///
/// # Arguments
///
/// * `bond`  – The bond whose `call_put` schedule is enumerated.
/// * `flows` – Holder-view cashflows used to align candidates to payment dates.
/// * `as_of` – Earliest admissible exercise date (valuation/quote date).
pub(crate) fn enumerate_exit_paths(
    bond: &Bond,
    flows: &[(Date, Money)],
    as_of: Date,
) -> Vec<ExitCandidate> {
    let Some(cp) = &bond.call_put else {
        return Vec::new();
    };

    let mut call_candidates: Vec<ExitCandidate> = Vec::new();
    let mut put_candidates: Vec<ExitCandidate> = Vec::new();

    let push_period_candidates = |candidates: &mut Vec<ExitCandidate>,
                                  option: &crate::instruments::fixed_income::bond::CallPut,
                                  retain_make_whole: bool| {
        let mut exercise_dates = vec![option.start_date, option.end_date];
        exercise_dates.extend(
            flows
                .iter()
                .map(|(date, _)| *date)
                .filter(|date| *date >= option.start_date && *date <= option.end_date),
        );
        exercise_dates.sort_unstable();
        exercise_dates.dedup();

        for exercise_date in exercise_dates {
            if exercise_date >= as_of && exercise_date <= bond.maturity {
                candidates.push(ExitCandidate {
                    date: exercise_date,
                    price_pct_of_par: option.price_pct_of_par,
                    make_whole: if retain_make_whole {
                        option.make_whole.clone()
                    } else {
                        None
                    },
                });
            }
        }
    };

    for c in &cp.calls {
        push_period_candidates(&mut call_candidates, c, true);
    }
    for p in &cp.puts {
        push_period_candidates(&mut put_candidates, p, false);
    }

    // Adjacent call windows can share boundary dates while carrying different
    // make-whole terms or reference curves. Retain every contractual call
    // candidate so each consumer can evaluate the realized redemption and
    // select the issuer-cheapest path. For puts, the holder exercises the
    // highest fixed strike, so same-date candidates can be collapsed here.
    call_candidates.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| left.price_pct_of_par.total_cmp(&right.price_pct_of_par))
    });
    put_candidates.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| right.price_pct_of_par.total_cmp(&left.price_pct_of_par))
    });
    put_candidates.dedup_by_key(|candidate| candidate.date);
    call_candidates.extend(put_candidates);

    call_candidates
}

/// Build every exercise and rolled-maturity cashflow path from one quote-date
/// entitlement set.
fn workout_cashflow_paths(
    bond: &Bond,
    curves: &MarketContext,
    flows: &[(Date, Money)],
    quote_date: Date,
    schedule: &CashFlowSchedule,
) -> finstack_quant_core::Result<Vec<Vec<(Date, Money)>>> {
    let has_quote_date_exercise = bond.call_put.as_ref().is_some_and(|option_schedule| {
        option_schedule
            .calls
            .iter()
            .chain(&option_schedule.puts)
            .any(|option| option.start_date <= quote_date && quote_date <= option.end_date)
    });
    let inclusive_flows;
    let path_flows = if has_quote_date_exercise {
        // Contractual holder cash on a live exercise date is paid before the
        // exercise decision. Rebuild from the materialized schedule so a
        // no-lag quote does not lose a same-day coupon at the usual strict
        // settlement boundary.
        inclusive_flows =
            bond.pricing_dated_cashflows_from_schedule_inclusive(schedule, quote_date, quote_date)?;
        inclusive_flows.as_slice()
    } else {
        flows
    };
    let mut candidates = enumerate_exit_paths(bond, path_flows, quote_date);

    // Contractual maturity can precede its business-day-adjusted final payment.
    // The held path must retain that rolled cash rather than truncating it at
    // the unadjusted maturity date.
    let maturity_candidate = path_flows
        .iter()
        .map(|(date, _)| *date)
        .max()
        .map_or(bond.maturity, |last| last.max(bond.maturity));
    candidates.push(ExitCandidate {
        date: maturity_candidate,
        price_pct_of_par: 0.0,
        make_whole: None,
    });

    let accrual_index = AccrualIndex::build(schedule, &bond.accrual_config())?;
    let mut paths = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let mut path: Vec<(Date, Money)> = path_flows
            .iter()
            .copied()
            .filter(|(date, _)| {
                (*date > quote_date || (has_quote_date_exercise && *date == quote_date))
                    && *date <= candidate.date
            })
            .collect();
        let redemption = if candidate.price_pct_of_par > 0.0 {
            exercise_redemption_amount(
                bond,
                curves,
                path_flows,
                schedule,
                &accrual_index,
                &candidate,
            )?
        } else {
            0.0
        };
        path.push((
            candidate.date,
            Money::new(redemption, bond.notional.currency())?,
        ));
        paths.push(path);
    }

    Ok(paths)
}

/// Price a workout path while retaining holder cash paid on the exercise date.
fn price_workout_path_from_yield(
    bond: &Bond,
    path: &[(Date, Money)],
    quote_date: Date,
    ytw: f64,
) -> finstack_quant_core::Result<f64> {
    let immediate_cash = path
        .iter()
        .filter(|(date, _)| *date == quote_date)
        .map(|(_, amount)| amount.amount())
        .sum::<f64>();
    Ok(immediate_cash + price_from_ytm(bond, path, quote_date, ytw)?)
}

/// Solve one workout path after separating non-discountable quote-date cash.
fn solve_workout_path_yield(
    bond: &Bond,
    path: &[(Date, Money)],
    quote_date: Date,
    dirty_price_target: Money,
) -> finstack_quant_core::Result<f64> {
    let immediate_cash = path
        .iter()
        .filter(|(date, _)| *date == quote_date)
        .map(|(_, amount)| amount.amount())
        .sum::<f64>();
    let future_flows: Vec<_> = path
        .iter()
        .copied()
        .filter(|(date, _)| *date > quote_date)
        .collect();
    let target = dirty_price_target.amount();
    let residual_target = target - immediate_cash;
    let tolerance = 1e-12 * target.abs().max(immediate_cash.abs()).max(1.0);

    if future_flows.is_empty() {
        return Ok(if immediate_cash < target - tolerance {
            // A lower immediate payoff is the limiting worst path as the
            // holding-period yield tends to negative infinity.
            f64::NEG_INFINITY
        } else {
            // Immediate cash at or above the target does not lower the minimum
            // finite yield supplied by another workout path.
            f64::INFINITY
        });
    }
    if residual_target <= tolerance {
        return Ok(f64::INFINITY);
    }

    let coupon_rate = match &bond.cashflow_spec {
        crate::instruments::fixed_income::bond::CashflowSpec::Fixed(spec) => {
            spec.rate.to_f64().unwrap_or(0.0)
        }
        _ => 0.0,
    };
    solve_ytm(
        &future_flows,
        quote_date,
        Money::new(residual_target, dirty_price_target.currency())?,
        YtmPricingSpec {
            day_count: bond.cashflow_spec.day_count(),
            notional: bond.notional,
            coupon_rate,
            compounding: YieldCompounding::Street,
            frequency: bond.cashflow_spec.frequency(),
        },
    )
}

/// Solve yield-to-worst over all call/put/maturity candidates for a given flow set.
///
/// Returns the worst (minimum) yield and the corresponding truncated cashflow path.
///
/// # Call/Put Redemption Convention
///
/// Call/put redemption prices are dirty street redemption amounts:
/// `outstanding_principal × (price_pct_of_par / 100) + accrued_interest(exercise_date)`,
/// where `outstanding_principal` is the remaining principal at the exercise date after
/// any amortization. This correctly handles amortizing callable bonds and is consistent
/// with the tree-based OAS pricing.
///
/// # Arguments
///
/// * `bond` - The bond to calculate YTW for
/// * `curves` - Market context supplying any make-whole reference curves.
/// * `flows` - Holder-view cashflows (coupons + principal)
/// * `as_of` - Valuation/quote date
/// * `dirty_price_target` - Target dirty price to match
/// * `schedule` - Full cashflow schedule used for canonical outstanding
///   principal and same-day maturity-redemption replacement.
pub(crate) fn solve_ytw_from_flows(
    bond: &Bond,
    curves: &MarketContext,
    flows: &[(Date, Money)],
    as_of: Date,
    dirty_price_target: Money,
    schedule: &CashFlowSchedule,
) -> finstack_quant_core::Result<(f64, Vec<(Date, Money)>)> {
    let mut best_yield = f64::INFINITY;
    let mut best_flows: Vec<(Date, Money)> = Vec::new();
    for path in workout_cashflow_paths(bond, curves, flows, as_of, schedule)? {
        let y = solve_workout_path_yield(bond, &path, as_of, dirty_price_target)?;
        if y < best_yield {
            best_yield = y;
            best_flows = path;
        }
    }

    Ok((best_yield, best_flows))
}

/// Convert a Street yield-to-worst quote into settlement dirty price.
///
/// Every admissible call, put, and rolled-maturity path is priced at `ytw`.
/// Because each path yield decreases monotonically as dirty price increases,
/// the inverse of `YTW(P) = min_i yield_i(P)` is the minimum path price at the
/// target yield. Return-floor protection is first lowered into its effective
/// call schedule, and make-whole calls use their deterministic reference-curve
/// redemption.
///
/// # Arguments
///
/// * `bond` - Callable or puttable bond whose cashflow schedule and exercise
///   candidates define the yield-to-worst paths.
/// * `curves` - Market context supplying schedule inputs and any make-whole
///   reference curves.
/// * `as_of` - Valuation or trade date; settlement and coupon entitlement are
///   derived from the bond's quote convention.
/// * `ytw` - Annual Street-compounded yield to worst as a decimal.
///
/// # Returns
///
/// Settlement-date dirty price in the bond's notional currency.
///
/// # Errors
///
/// Returns an error when `ytw` is non-finite, required curves or schedules are
/// unavailable, return-floor lowering fails, or a path cannot be priced under
/// the bond's Street yield convention.
pub fn price_from_ytw(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    ytw: f64,
) -> finstack_quant_core::Result<f64> {
    if !ytw.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "yield to worst must be finite, got {ytw}"
        )));
    }

    let effective_bond = bond.effective_for_pricing(curves, as_of)?;
    let quote_ctx = QuoteDateContext::new(&effective_bond, curves, as_of)?;
    let flows = quote_ctx.entitled_flows(&effective_bond, curves, as_of)?;
    let schedule = effective_bond.full_cashflow_schedule(curves)?;
    let paths = workout_cashflow_paths(
        &effective_bond,
        curves,
        &flows,
        quote_ctx.quote_date,
        &schedule,
    )?;

    let mut best_price: Option<f64> = None;
    for path in paths {
        let price =
            price_workout_path_from_yield(&effective_bond, &path, quote_ctx.quote_date, ytw)?;
        best_price = Some(best_price.map_or(price, |current| current.min(price)));
    }
    best_price.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "yield-to-worst inversion produced no workout paths".to_string(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::{CashFlowMeta, Notional};
    use crate::cashflow::primitives::CashFlow;
    use crate::instruments::fixed_income::bond::{
        CallPut, CallPutSchedule, MakeWholeSpec, ProtectionWindow, ReturnFloorSpec,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::Rate;
    use time::macros::date;

    #[test]
    fn exit_windows_keep_exact_contractual_boundaries() {
        let mut bond = Bond::example().expect("example bond");
        let start = date!(2027 - 01 - 10);
        let flow_date = date!(2027 - 01 - 15);
        let end = date!(2027 - 01 - 20);
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: start,
                end_date: end,
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        let flows = vec![(flow_date, Money::from((5_i64, Currency::USD)))];

        let dates: Vec<_> = enumerate_exit_paths(&bond, &flows, date!(2026 - 01 - 01))
            .into_iter()
            .map(|candidate| candidate.date)
            .collect();
        assert_eq!(dates, vec![start, flow_date, end]);
    }

    #[test]
    fn same_date_calls_retain_distinct_make_whole_terms_while_puts_collapse() {
        let mut bond = Bond::example().expect("example bond");
        let exercise_date = date!(2027 - 01 - 15);
        bond.call_put = Some(CallPutSchedule {
            calls: vec![
                CallPut {
                    start_date: date!(2027 - 01 - 10),
                    end_date: exercise_date,
                    price_pct_of_par: 100.0,
                    make_whole: Some(MakeWholeSpec {
                        reference_curve_id: "REF-A".into(),
                        spread_bp: 25.0,
                    }),
                },
                CallPut {
                    start_date: exercise_date,
                    end_date: date!(2027 - 01 - 20),
                    price_pct_of_par: 99.0,
                    make_whole: Some(MakeWholeSpec {
                        reference_curve_id: "REF-B".into(),
                        spread_bp: 50.0,
                    }),
                },
            ],
            puts: vec![
                CallPut {
                    start_date: exercise_date,
                    end_date: exercise_date,
                    price_pct_of_par: 98.0,
                    make_whole: None,
                },
                CallPut {
                    start_date: exercise_date,
                    end_date: exercise_date,
                    price_pct_of_par: 101.0,
                    make_whole: None,
                },
            ],
        });

        let candidates = enumerate_exit_paths(&bond, &[], date!(2026 - 01 - 01));
        let calls: Vec<_> = candidates
            .iter()
            .filter(|candidate| candidate.date == exercise_date)
            .filter_map(|candidate| candidate.make_whole.as_ref())
            .collect();
        let puts: Vec<_> = candidates
            .iter()
            .filter(|candidate| candidate.date == exercise_date && candidate.make_whole.is_none())
            .collect();

        assert_eq!(calls.len(), 2);
        assert!(calls
            .iter()
            .any(|spec| spec.reference_curve_id.as_str() == "REF-A"));
        assert!(calls
            .iter()
            .any(|spec| spec.reference_curve_id.as_str() == "REF-B"));
        assert_eq!(puts.len(), 1);
        assert_eq!(puts[0].price_pct_of_par, 101.0);
    }

    #[test]
    fn make_whole_redemption_uses_reference_value_above_fixed_floor() {
        let as_of = date!(2025 - 01 - 01);
        let exercise_date = date!(2026 - 01 - 01);
        let mut bond = Bond::fixed(
            "MW-WORKOUT",
            Money::from((100_i64, Currency::USD)),
            Rate::from_decimal(0.10).expect("valid rate fixture"),
            as_of,
            date!(2027 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.settlement_convention = None;
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: exercise_date,
                end_date: exercise_date,
                price_pct_of_par: 50.0,
                make_whole: Some(MakeWholeSpec {
                    reference_curve_id: "USD-REF".into(),
                    spread_bp: 0.0,
                }),
            }],
            puts: Vec::new(),
        });
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (2.0, 0.90)])
                    .build()
                    .expect("discount curve"),
            )
            .insert(
                DiscountCurve::builder("USD-REF")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (2.0, 1.0)])
                    .build()
                    .expect("reference curve"),
            );
        let flows = bond
            .pricing_dated_cashflows(&market, as_of)
            .expect("pricing flows");
        let schedule = bond
            .full_cashflow_schedule(&market)
            .expect("cashflow schedule");
        let accrual_index =
            AccrualIndex::build(&schedule, &bond.accrual_config()).expect("accrual index");
        let candidate = enumerate_exit_paths(&bond, &flows, as_of)
            .into_iter()
            .find(|candidate| candidate.date == exercise_date)
            .expect("make-whole candidate");

        let redemption = exercise_redemption_amount(
            &bond,
            &market,
            &flows,
            &schedule,
            &accrual_index,
            &candidate,
        )
        .expect("make-whole redemption");
        let remaining_reference_value: f64 = flows
            .iter()
            .filter(|(date, _)| *date > exercise_date)
            .map(|(_, amount)| amount.amount())
            .sum();

        assert!((redemption - remaining_reference_value).abs() < 1e-10);
        assert!(redemption > 100.0);
    }

    #[test]
    fn quoted_workout_uses_make_whole_redemption_when_selecting_ytw_path() {
        let as_of = date!(2025 - 01 - 15);
        let exercise_date = date!(2025 - 12 - 15);
        let mut make_whole_bond = Bond::fixed(
            "MW-QUOTED-WORKOUT",
            Money::from((100_i64, Currency::USD)),
            Rate::from_decimal(0.10).expect("valid rate fixture"),
            as_of,
            date!(2027 - 01 - 15),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        make_whole_bond.settlement_convention = None;
        make_whole_bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(100.0);
        make_whole_bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: exercise_date,
                end_date: exercise_date,
                price_pct_of_par: 50.0,
                make_whole: Some(MakeWholeSpec {
                    reference_curve_id: "USD-REF".into(),
                    spread_bp: 0.0,
                }),
            }],
            puts: Vec::new(),
        });
        let mut fixed_strike_bond = make_whole_bond.clone();
        fixed_strike_bond
            .call_put
            .as_mut()
            .expect("call schedule")
            .calls[0]
            .make_whole = None;
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (2.0, 0.90)])
                    .build()
                    .expect("discount curve"),
            )
            .insert(
                DiscountCurve::builder("USD-REF")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (2.0, 1.0)])
                    .build()
                    .expect("reference curve"),
            );

        let make_whole_flows = make_whole_bond
            .pricing_dated_cashflows(&market, as_of)
            .expect("pricing flows");
        let (_, make_whole_path, _) =
            crate::instruments::fixed_income::bond::metrics::quoted_workout_path(
                &make_whole_bond,
                &market,
                as_of,
                &make_whole_flows,
            )
            .expect("make-whole workout")
            .expect("callable workout path");
        let fixed_flows = fixed_strike_bond
            .pricing_dated_cashflows(&market, as_of)
            .expect("pricing flows");
        let (_, fixed_path, _) =
            crate::instruments::fixed_income::bond::metrics::quoted_workout_path(
                &fixed_strike_bond,
                &market,
                as_of,
                &fixed_flows,
            )
            .expect("fixed-strike workout")
            .expect("callable workout path");

        let make_whole_exit = make_whole_path
            .iter()
            .map(|(date, _)| *date)
            .max()
            .expect("make-whole exit date");
        let fixed_exit = fixed_path
            .iter()
            .map(|(date, _)| *date)
            .max()
            .expect("fixed-strike exit date");
        assert!(make_whole_exit > exercise_date);
        assert_eq!(fixed_exit, exercise_date);

        let target_ytw = 0.10;
        let maturity_price = price_from_ytm(&make_whole_bond, &make_whole_flows, as_of, target_ytw)
            .expect("maturity price");
        let make_whole_price = price_from_ytw(&make_whole_bond, &market, as_of, target_ytw)
            .expect("make-whole YTW price");
        let fixed_price = price_from_ytw(&fixed_strike_bond, &market, as_of, target_ytw)
            .expect("fixed-strike YTW price");
        assert!((make_whole_price - maturity_price).abs() < 1e-10);
        assert!(fixed_price + 25.0 < make_whole_price);
    }

    #[test]
    fn price_from_ytw_matches_ytm_for_bullet_with_rolled_final_payment() {
        let as_of = date!(2025 - 01 - 03);
        let mut bond = Bond::fixed(
            "YTW-ROLLED-MATURITY",
            Money::from((100_i64, Currency::USD)),
            Rate::from_decimal(0.05).expect("valid rate fixture"),
            as_of,
            date!(2026 - 01 - 03),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.settlement_convention = None;
        let market = MarketContext::new();
        let quote_ctx = QuoteDateContext::new(&bond, &market, as_of).expect("quote context");
        let flows = quote_ctx
            .entitled_flows(&bond, &market, as_of)
            .expect("entitled flows");
        assert!(flows.iter().any(|(date, _)| *date > bond.maturity));

        let target_yield = 0.04;
        let expected =
            price_from_ytm(&bond, &flows, quote_ctx.quote_date, target_yield).expect("YTM price");
        let actual =
            price_from_ytw(&bond, &market, as_of, target_yield).expect("YTW inverse price");

        assert!((actual - expected).abs() < 1e-12);
    }

    #[test]
    fn same_day_coupon_precedes_exercise_in_ytw_price_and_round_trip() {
        let issue = date!(2024 - 01 - 15);
        let as_of = date!(2025 - 01 - 15);
        let mut bond = Bond::fixed(
            "YTW-SAME-DAY-EXERCISE",
            Money::from((100_i64, Currency::USD)),
            Rate::from_decimal(0.10).expect("valid rate fixture"),
            issue,
            date!(2026 - 01 - 15),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.settlement_convention = None;
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: as_of,
                end_date: as_of,
                price_pct_of_par: 101.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        let market = MarketContext::new();
        let schedule = bond
            .full_cashflow_schedule(&market)
            .expect("cashflow schedule");
        let inclusive_flows = bond
            .pricing_dated_cashflows_from_schedule_inclusive(&schedule, as_of, as_of)
            .expect("inclusive exercise flows");
        let same_day_coupon: f64 = inclusive_flows
            .iter()
            .filter(|(date, _)| *date == as_of)
            .map(|(_, amount)| amount.amount())
            .sum();
        assert!((same_day_coupon - 5.0).abs() < 1e-10);

        let target_ytw = 0.10;
        let maturity_price = same_day_coupon
            + price_from_ytm(&bond, &inclusive_flows, as_of, target_ytw)
                .expect("maturity path price");
        let immediate_call_cash = same_day_coupon + 101.0;
        assert!(immediate_call_cash > maturity_price);

        let inverted =
            price_from_ytw(&bond, &market, as_of, target_ytw).expect("same-day YTW price");
        assert!((inverted - maturity_price).abs() < 1e-10);

        let strict_flows = bond
            .pricing_dated_cashflows(&market, as_of)
            .expect("strict quote flows");
        let (solved, selected_path) = solve_ytw_from_flows(
            &bond,
            &market,
            &strict_flows,
            as_of,
            Money::new(inverted, Currency::USD).expect("valid money fixture"),
            &schedule,
        )
        .expect("same-day YTW solve");
        assert!((solved - target_ytw).abs() < 1e-10);
        assert!(selected_path
            .iter()
            .any(|(date, amount)| *date == as_of && amount.amount() > 0.0));
    }

    #[test]
    fn price_from_ytw_lowers_return_floor_only_bond_into_workout_paths() {
        let as_of = date!(2025 - 01 - 15);
        let exercise_date = date!(2025 - 12 - 15);
        let mut bullet = Bond::fixed(
            "YTW-RETURN-FLOOR",
            Money::from((100_i64, Currency::USD)),
            Rate::from_decimal(0.10).expect("valid rate fixture"),
            as_of,
            date!(2030 - 01 - 15),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bullet.settlement_convention = None;
        let floored = bullet
            .clone()
            .with_return_floor(
                ReturnFloorSpec::moic(0.50).window(ProtectionWindow::Between {
                    start: exercise_date,
                    end: exercise_date.next_day().expect("following date"),
                }),
            );
        assert!(floored.call_put.is_none());

        let market = MarketContext::new();
        let target_ytw = 0.0;
        let bullet_price =
            price_from_ytw(&bullet, &market, as_of, target_ytw).expect("bullet YTW price");
        let floored_price =
            price_from_ytw(&floored, &market, as_of, target_ytw).expect("floor YTW price");

        let effective = floored
            .effective_for_pricing(&market, as_of)
            .expect("lowered floor");
        assert!(effective.return_floor.is_none());
        assert!(effective
            .call_put
            .as_ref()
            .is_some_and(CallPutSchedule::has_options));
        let quote_ctx = QuoteDateContext::new(&effective, &market, as_of).expect("quote context");
        let flows = quote_ctx
            .entitled_flows(&effective, &market, as_of)
            .expect("entitled flows");
        let schedule = effective
            .full_cashflow_schedule(&market)
            .expect("cashflow schedule");
        let paths =
            workout_cashflow_paths(&effective, &market, &flows, quote_ctx.quote_date, &schedule)
                .expect("lowered workout paths");
        assert!(paths.iter().any(|path| {
            path.iter()
                .map(|(date, _)| *date)
                .max()
                .is_some_and(|date| date <= exercise_date.next_day().expect("following date"))
        }));
        let manual_min = paths
            .iter()
            .map(|path| {
                price_workout_path_from_yield(&effective, path, quote_ctx.quote_date, target_ytw)
                    .expect("path price")
            })
            .fold(f64::INFINITY, f64::min);

        assert!((floored_price - manual_min).abs() < 1e-12);
        assert!(floored_price + 25.0 < bullet_price);
    }

    #[test]
    fn terminal_exercise_replaces_contractual_redemption() {
        let issue = date!(2025 - 01 - 01);
        let maturity = date!(2026 - 01 - 01);
        let schedule = CashFlowSchedule::from_parts(
            vec![
                CashFlow::new(
                    issue,
                    None,
                    Money::from((-100_i64, Currency::USD)),
                    CFKind::Notional,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    maturity,
                    None,
                    Money::from((5_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    maturity,
                    None,
                    Money::from((100_i64, Currency::USD)),
                    CFKind::Notional,
                    0.0,
                    None,
                ),
            ],
            Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
            DayCount::Act365F,
            CashFlowMeta {
                issue_date: Some(issue),
                maturity_date: Some(maturity),
                ..CashFlowMeta::default()
            },
        );

        let (principal, replaced) =
            exercise_principal_and_replaced_redemption(&schedule, maturity, maturity)
                .expect("terminal principal decomposition");
        assert_eq!(principal, 100.0);
        assert_eq!(replaced, 100.0);
    }
}
