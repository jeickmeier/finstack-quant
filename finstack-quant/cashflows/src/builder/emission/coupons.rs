//! Coupon cashflow emission (fixed and floating).
//!
//! `emit_inflation_coupons` maps pre-indexed inflation coupon tuples into
//! `CashFlow` values. Index projection and ratio logic belong in the instrument
//! layer that supplies those tuples.

use crate::primitives::{CFKind, CashFlow};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, Tenor};
use finstack_quant_core::market_data::fixings::{fixing_series_id, require_fixing_value_exact};
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::InputError;
use rust_decimal::Decimal;
use tracing::{info, warn};

use crate::builder::rate_helpers::ResolvedFloatingRateFallback;
use crate::builder::specs::OvernightCompoundingMethod;

use super::super::compiler::{FixedSchedule, FloatSchedule};
use super::helpers::{add_pik_flow_if_nonzero, compute_reset_date};

/// Append pre-computed inflation-linked coupon cashflows.
///
/// This function does not project CPI/RPI/HICP fixings or calculate index
/// ratios. It preserves caller-computed indexed coupon amounts and tags them as
/// [`CFKind::InflationCoupon`] for downstream valuation/reporting.
///
/// Each tuple is `(payment_date, indexed_coupon_amount, accrual_factor, real_coupon_rate)`.
pub fn emit_inflation_coupons(
    ccy: Currency,
    coupons: &[(Date, f64, f64, f64)],
    out_flows: &mut Vec<CashFlow>,
) {
    for &(date, amount, accrual_factor, real_coupon_rate) in coupons {
        out_flows.push(CashFlow {
            date,
            reset_date: None,
            amount: Money::new(amount, ccy),
            kind: CFKind::InflationCoupon,
            accrual_factor,
            rate: Some(real_coupon_rate),
        });
    }
}

// Shared f64 ↔ Decimal conversion helpers live in the parent `emission` module
// so that `fees.rs` can use them too. Access via `super::`.
use super::{decimal_to_f64, f64_to_decimal};

/// Compute the index maturity date from a projection start date and index tenor.
///
/// For a term-rate index (e.g., 3M term SOFR / EURIBOR), the instrument
/// references a rate whose underlying deposit period runs from the accrual
/// start over the index tenor, so the forward is projected over
/// `[accrual_start, accrual_start + index_tenor]` — not from the fixing date
/// and not to the payment date. The fixing (reset) date remains flow
/// metadata only.
fn compute_index_maturity(
    reset_date: Date,
    index_tenor: Tenor,
) -> finstack_quant_core::Result<Date> {
    use finstack_quant_core::dates::TenorUnit;
    let maturity = match index_tenor.unit {
        TenorUnit::Months => {
            let months = i32::try_from(index_tenor.count).map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "index tenor months = {} exceeds i32::MAX",
                    index_tenor.count
                ))
            })?;
            reset_date.add_months(months)
        }
        TenorUnit::Days => reset_date + time::Duration::days(i64::from(index_tenor.count)),
        TenorUnit::Years => {
            let months = index_tenor.count.checked_mul(12).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "index tenor years = {} overflows month conversion",
                    index_tenor.count
                ))
            })?;
            let months = i32::try_from(months).map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "index tenor years = {} exceeds i32::MAX months",
                    index_tenor.count
                ))
            })?;
            reset_date.add_months(months)
        }
        TenorUnit::Weeks => {
            let days = index_tenor.count.checked_mul(7).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "index tenor weeks = {} overflows day conversion",
                    index_tenor.count
                ))
            })?;
            reset_date + time::Duration::days(i64::from(days))
        }
    };
    Ok(maturity)
}

/// Error for an overnight observation date strictly before the curve base
/// when no historical fixing series is available.
///
/// Historical fixings are supported via a [`ScalarTimeSeries`] stored in the
/// `MarketContext` under the canonical id `FIXING:{index_id}` (see
/// [`finstack_quant_core::market_data::fixings`]). This error is raised only when
/// no such series was provided; the emission layer routes it through the
/// spec's [`crate::builder::specs::FloatingRateFallback`] policy.
fn pre_base_observation_error(obs_date: Date, fwd: &ForwardCurve) -> finstack_quant_core::Error {
    finstack_quant_core::Error::Validation(format!(
        "overnight observation date {} is before the '{}' curve base date {}; the realized \
         historical fixings are missing — provide a MarketContext ScalarTimeSeries with id \
         '{}', supply a curve based on/before the observation date, or configure a \
         FloatingRateFallback",
        obs_date,
        fwd.id(),
        fwd.base_date(),
        fixing_series_id(fwd.id().as_str()),
    ))
}

/// Resolve the overnight rate for a single observation date, seamlessly mixing
/// realized historical fixings with curve-projected forwards.
///
/// Seasoned compounding windows (ARRC 2020 SOFR conventions; ISDA 2021
/// Supp. 70 §7.1(g)) contain observation dates on both sides of the valuation
/// date: realized fixings before it and projected forwards after it. Both
/// carry the same `(rate, days)` weighting in the compounding product.
///
/// - `obs_date < curve base`: realized fixing, resolved exactly from the
///   `FIXING:{index_id}` series. Weekend/holiday carry is represented by the
///   observation day weight, so a missing business-day publication is an
///   error rather than permission to reuse an arbitrarily old rate. Errors via
///   [`pre_base_observation_error`] when no series is provided; both errors
///   route through the spec's fallback policy upstream.
/// - `obs_date == curve base`: a published same-day fixing (exact-date match)
///   is preferred when present — today's fixing may or may not be published
///   yet; otherwise the rate is projected from `t = 0`.
/// - `obs_date > curve base`: the overnight forward is projected from the
///   curve over `[t, t + days/basis]`.
fn observed_overnight_rate(
    obs_date: Date,
    weight_days: u32,
    fwd: &ForwardCurve,
    fwd_dc_basis: f64,
    fixings: Option<&ScalarTimeSeries>,
    index_id: &str,
) -> finstack_quant_core::Result<f64> {
    let fwd_base = fwd.base_date();
    if obs_date < fwd_base {
        return match fixings {
            Some(series) => require_fixing_value_exact(Some(series), index_id, obs_date, fwd_base),
            None => Err(pre_base_observation_error(obs_date, fwd)),
        };
    }
    if obs_date == fwd_base {
        if let Some(fixing) = fixings.and_then(|series| series.value_on_exact(obs_date).ok()) {
            return Ok(fixing);
        }
    }
    let fwd_dc = fwd.day_count();
    let t = if obs_date == fwd_base {
        0.0
    } else {
        fwd_dc.year_fraction(
            fwd_base,
            obs_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?
    };
    // Use the average forward rate over the overnight tenor [t, t+days/basis]
    // rather than the instantaneous forward at t. For piecewise-constant
    // curves the two are identical, but for interpolated curves (linear,
    // cubic) `rate_period` gives the correct overnight forward average.
    let overnight_dt = f64::from(weight_days) / fwd_dc_basis;
    Ok(fwd.rate_period(t, t + overnight_dt))
}

fn rate_when_curve_missing(
    index_id: &str,
    reset_date: Date,
    spread_bp: f64,
    fallback: &ResolvedFloatingRateFallback,
    params: &crate::builder::rate_helpers::FloatingRateParams,
    context_suffix: &str,
) -> finstack_quant_core::Result<f64> {
    match fallback {
        ResolvedFloatingRateFallback::Error => {
            Err(finstack_quant_core::Error::Input(InputError::NotFound {
                id: format!(
                    "forward curve '{}' not found for reset date {}{}",
                    index_id, reset_date, context_suffix
                ),
            }))
        }
        ResolvedFloatingRateFallback::SpreadOnly => {
            warn!(
                reset_date = %reset_date,
                spread_bp = %spread_bp,
                "No forward curve resolved{context_suffix}, using fallback (spread-only) rate"
            );
            fallback
                .fallback_rate(params)
                .ok_or(finstack_quant_core::Error::Input(InputError::Invalid))
        }
        ResolvedFloatingRateFallback::FixedRate(index_rate) => {
            info!(
                reset_date = %reset_date,
                fixed_rate = %index_rate,
                "No forward curve resolved{context_suffix}, using fixed index rate"
            );
            fallback
                .fallback_rate(params)
                .ok_or(finstack_quant_core::Error::Input(InputError::Invalid))
        }
    }
}

fn rate_when_projection_fails(
    error: &finstack_quant_core::Error,
    reset_date: Date,
    index_maturity: Date,
    spread_bp: f64,
    fallback: &ResolvedFloatingRateFallback,
    params: &crate::builder::rate_helpers::FloatingRateParams,
) -> finstack_quant_core::Result<f64> {
    match fallback {
        ResolvedFloatingRateFallback::Error => Err(error.clone()),
        ResolvedFloatingRateFallback::SpreadOnly => {
            warn!(
                reset_date = %reset_date,
                index_maturity = %index_maturity,
                spread_bp = %spread_bp,
                error = %error,
                "Floating rate projection failed, using fallback (spread-only) rate"
            );
            fallback
                .fallback_rate(params)
                .ok_or(finstack_quant_core::Error::Input(InputError::Invalid))
        }
        ResolvedFloatingRateFallback::FixedRate(index_rate) => {
            info!(
                reset_date = %reset_date,
                fixed_rate = %index_rate,
                error = %error,
                "Floating rate projection failed, using fixed index rate"
            );
            fallback
                .fallback_rate(params)
                .ok_or(finstack_quant_core::Error::Input(InputError::Invalid))
        }
    }
}

/// Emit fixed coupon cashflows on a specific date.
///
/// Processes all fixed coupon schedules for the given date, computing coupon
/// amounts based on outstanding balances and splitting into cash/PIK according
/// to the coupon type. Cash and PIK flows are appended directly into the
/// provided `out_flows` buffer to avoid per-date allocations.
///
/// # Returns
///
/// `pik_to_add` — the total PIK coupon amount (across every fixed schedule
/// processed on date `d`) that the caller must capitalize into the outstanding
/// balance for subsequent periods. Cash flows are pushed into `out_flows` as a
/// side effect; the return value is exclusively the PIK leg.
pub(crate) fn emit_fixed_coupons_on(
    d: Date,
    fixed_schedules: &[FixedSchedule],
    outstanding_after: &finstack_quant_core::HashMap<Date, Decimal>,
    outstanding_fallback: Decimal,
    ccy: Currency,
    out_flows: &mut Vec<CashFlow>,
) -> finstack_quant_core::Result<f64> {
    let mut pik_to_add = 0.0;

    for schedule in fixed_schedules {
        let spec = &schedule.spec;
        let calendar = schedule.calendar;
        // Early exit: skip schedules where `d` is outside the date range.
        // This reduces iteration from O(N × M) to O(N + M) for multi-window instruments.
        if let (Some(&first), Some(&last)) = (schedule.dates.first(), schedule.dates.last()) {
            if d < first || d > last {
                continue;
            }
        }

        if let Some(period) = schedule.prev.get(&d).copied() {
            let accrual_start = period.accrual_start;
            let accrual_end = period.accrual_end;
            let is_stub = schedule.first_last.contains(&d);
            let is_termination_date = schedule
                .prev
                .values()
                .map(|period| period.accrual_end)
                .max()
                .is_some_and(|last| accrual_end == last);
            let base_out = *outstanding_after
                .get(&accrual_start)
                .unwrap_or(&outstanding_fallback);

            // ACT/ACT ICMA reference period: regular periods use the accrual
            // period itself (exact ISMA accrual). For stubs the adjacent
            // regular period is not available here, so core's frequency-based
            // quasi-coupon subdivision is used instead (coupon_period: None).
            let yf = spec.dc.year_fraction(
                accrual_start,
                accrual_end,
                finstack_quant_core::dates::DayCountContext {
                    calendar: Some(calendar),
                    frequency: Some(spec.freq),
                    bus_basis: None,
                    coupon_period: (!is_stub).then_some((accrual_start, accrual_end)),
                    end_is_termination_date: is_termination_date,
                },
            )?;

            let yf_dec = f64_to_decimal(yf)?;
            let coupon_total_dec = base_out * spec.rate * yf_dec;
            let coupon_total = decimal_to_f64(coupon_total_dec)?;

            let (cash_pct, pik_pct) = spec.coupon_type.split_parts()?;
            let cash_pct_f64 = decimal_to_f64(cash_pct)?;
            let pik_pct_f64 = decimal_to_f64(pik_pct)?;

            let cash_amt = coupon_total * cash_pct_f64;
            let pik_amt = coupon_total * pik_pct_f64;

            // Convert rate to f64 for CashFlow storage
            let rate_f64 = decimal_to_f64(spec.rate)?;

            // Gate on the cash split percentage (mirroring the floating path)
            // rather than the cash amount's sign, so negative-rate coupons
            // emit instead of being silently dropped.
            if cash_pct_f64 > 0.0 {
                let kind = if is_stub { CFKind::Stub } else { CFKind::Fixed };
                out_flows.push(CashFlow {
                    date: d,
                    reset_date: None,
                    amount: Money::new(cash_amt, ccy),
                    kind,
                    accrual_factor: yf,
                    rate: Some(rate_f64),
                });
            }

            pik_to_add += add_pik_flow_if_nonzero(out_flows, d, pik_amt, ccy, Some(rate_f64), yf)?;
        }
    }
    Ok(pik_to_add)
}

/// Compute the observation window `[obs_start, obs_end)` for overnight rate
/// sampling per ISDA 2021 Supp. 70 §7.1(g).
///
/// For `CompoundedWithObservationShift { shift_days }`, both window endpoints
/// are moved earlier by `shift_days` business days so the compounding product
/// uses pre-accrual rates AND their pre-accrual day weights (the so-called
/// "shift both" convention used by EUR €STR at 2 BD and GBP SONIA at 5 BD).
///
/// For every other method (`CompoundedInArrears`, `SimpleAverage`,
/// `CompoundedWithLookback`, `CompoundedWithLockout`) the observation window
/// coincides with the accrual window; method-specific rate-index shifting
/// (lookback) or end-of-period lockout remains a concern of
/// [`crate::builder::rate_helpers::compute_overnight_rate`]. The Lookback
/// variant currently applies its shift inside the accrual window (ARRC
/// 2020 SOFR at 2 BD); aligning it with the observation-window model is
/// tracked as a follow-up.
fn observation_window(
    method: &OvernightCompoundingMethod,
    accrual_start: Date,
    accrual_end: Date,
    calendar: &dyn finstack_quant_core::dates::HolidayCalendar,
) -> finstack_quant_core::Result<(Date, Date)> {
    if let OvernightCompoundingMethod::CompoundedWithObservationShift { shift_days } = method {
        if *shift_days > 0 {
            // Attempting shifts greater than i32::MAX is nonsensical and the
            // underlying u32 → i32 cast would wrap; guard defensively.
            let shift_i32: i32 = i32::try_from(*shift_days).map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "observation shift_days = {shift_days} exceeds i32::MAX"
                ))
            })?;
            let obs_start = accrual_start.add_business_days(-shift_i32, calendar)?;
            let obs_end = accrual_end.add_business_days(-shift_i32, calendar)?;
            return Ok((obs_start, obs_end));
        }
    }
    Ok((accrual_start, accrual_end))
}

/// Sample overnight rates with the ISDA 2021 / ARRC 2020 "Lookback" convention.
///
/// For each accrual-period business day `d`, the observed rate is sampled from
/// the forward curve `lookback_bd` business days **before** `d`, while the
/// per-day weight remains the accrual-period calendar-day weight tied to `d`.
/// Annualization uses the accrual-period day count.
///
/// This differs from `sample_overnight_rates` called on a shifted window:
/// that variant shifts BOTH rates and weights (Observation Shift). Lookback
/// shifts only the rate-observation index.
///
/// Walks the accrual business days and looks up each observation date
/// directly via
/// [`finstack_quant_core::dates::DateExt::add_business_days`] rather than
/// rewriting indices into a pre-sampled `accrual_start..accrual_end`
/// window, so rates from before `accrual_start` are sampled correctly
/// instead of clamping to `daily_rates[0]`.
///
/// Observation dates strictly before the curve base date are realized
/// historical fixings resolved from `fixings` (the `FIXING:{index_id}`
/// series, LOCF); see [`observed_overnight_rate`].
///
/// Reference: ARRC 2020 *Recommended Conventions* §2 "Lookback";
/// ISDA 2021 Supp. 70 §7.1(g)(ii).
struct OvernightSamplingInput<'a> {
    fwd: &'a ForwardCurve,
    overnight_basis: f64,
    calendar: &'a dyn finstack_quant_core::dates::HolidayCalendar,
    fixings: Option<&'a ScalarTimeSeries>,
    index_id: &'a str,
}

fn sample_overnight_rates_with_lookback(
    accrual_start: Date,
    accrual_end: Date,
    lookback_bd: u32,
    input: OvernightSamplingInput<'_>,
) -> finstack_quant_core::Result<(Vec<(f64, u32)>, u32)> {
    if lookback_bd == 0 {
        return sample_overnight_rates(
            accrual_start,
            accrual_end,
            input.fwd,
            input.overnight_basis,
            input.calendar,
            input.fixings,
            input.index_id,
        );
    }
    let lookback_i32: i32 = i32::try_from(lookback_bd).map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "lookback_days = {lookback_bd} exceeds i32::MAX"
        ))
    })?;

    let mut daily_rates: Vec<(f64, u32)> = Vec::new();
    let mut pre_first_fixing_days: u32 = 0;
    let mut current = accrual_start;

    while current < accrual_end {
        let next = current + time::Duration::days(1);
        let next_capped = if next > accrual_end {
            accrual_end
        } else {
            next
        };
        let days = (next_capped - current).whole_days().max(1) as u32;

        if current.is_business_day(input.calendar) {
            // ARRC 2020 §2: rate observation moves back `lookback_bd` business
            // days; accrual weight remains tied to `current`. Pre-base
            // observations resolve from historical fixings (or error through
            // the spec's fallback policy when no series is provided).
            let obs_date = current.add_business_days(-lookback_i32, input.calendar)?;
            if pre_first_fixing_days > 0 {
                // Leading non-business days use the preceding business-day
                // fixing instead of the following Monday fixing.
                let preceding_obs =
                    current.add_business_days(-(lookback_i32 + 1), input.calendar)?;
                let leading_rate = observed_overnight_rate(
                    preceding_obs,
                    pre_first_fixing_days,
                    input.fwd,
                    input.overnight_basis,
                    input.fixings,
                    input.index_id,
                )?;
                daily_rates.push((leading_rate, pre_first_fixing_days));
                pre_first_fixing_days = 0;
            }
            let rate = observed_overnight_rate(
                obs_date,
                days,
                input.fwd,
                input.overnight_basis,
                input.fixings,
                input.index_id,
            )?;
            daily_rates.push((rate, days));
        } else if daily_rates.is_empty() {
            pre_first_fixing_days += days;
        } else if let Some(last) = daily_rates.last_mut() {
            last.1 += days;
        }
        current = next_capped;
    }

    let total_days = (accrual_end - accrual_start).whole_days().max(1) as u32;
    Ok((daily_rates, total_days))
}

/// Sample daily overnight rates from a forward curve over a given observation window.
///
/// For each calendar day in `[window_start, window_end)`, assigns the overnight
/// rate fixing at the nearest preceding business day. Non-business days before the
/// first fixing accumulate into the first business day's weight; non-business days
/// after a fixing accumulate into the preceding fixing's weight.
///
/// Returns `(daily_rates, total_days)` where:
/// - `daily_rates` is a vec of `(rate, weight_days)` per fixing date.
/// - `total_days` is the total calendar days in the window (used as the denominator
///   for simple-average compounding methods).
///
/// # ISDA 2021 Reference
///
/// Per Section 7.1(g): the rate for each Reset Date accrues for the number of
/// calendar days from that Reset Date to the next Reset Date (or period end).
/// Callers pass the accrual window for in-arrears / lookback / lockout variants
/// and the **shifted** observation window for `CompoundedWithObservationShift`
/// (see [`observation_window`]).
///
/// Observation dates strictly before the curve base date are realized
/// historical fixings resolved from `fixings` (the `FIXING:{index_id}`
/// series, LOCF); see [`observed_overnight_rate`]. Realized fixings carry the
/// same `(rate, days)` weighting as projected forwards, so a partially
/// seasoned compounding window mixes both seamlessly.
fn sample_overnight_rates(
    accrual_start: Date,
    accrual_end: Date,
    fwd: &ForwardCurve,
    overnight_basis: f64,
    calendar: &dyn finstack_quant_core::dates::HolidayCalendar,
    fixings: Option<&ScalarTimeSeries>,
    index_id: &str,
) -> finstack_quant_core::Result<(Vec<(f64, u32)>, u32)> {
    let mut daily_rates: Vec<(f64, u32)> = Vec::new();
    let mut pre_first_fixing_days: u32 = 0;
    let mut current = accrual_start;

    while current < accrual_end {
        let next = current + time::Duration::days(1);
        let next_capped = if next > accrual_end {
            accrual_end
        } else {
            next
        };
        let days = (next_capped - current).whole_days().max(1) as u32;

        if current.is_business_day(calendar) {
            // Pre-base observations are realized historical fixings resolved
            // from the `FIXING:{index_id}` series; without a series the error routes through the spec's
            // fallback policy. T+0 prefers a published fixing, else projects
            // from t = 0; later dates project the overnight forward.
            if pre_first_fixing_days > 0 {
                let preceding = current.add_business_days(-1, calendar)?;
                let leading_rate = observed_overnight_rate(
                    preceding,
                    pre_first_fixing_days,
                    fwd,
                    overnight_basis,
                    fixings,
                    index_id,
                )?;
                daily_rates.push((leading_rate, pre_first_fixing_days));
                pre_first_fixing_days = 0;
            }
            let rate =
                observed_overnight_rate(current, days, fwd, overnight_basis, fixings, index_id)?;
            daily_rates.push((rate, days));
        } else if daily_rates.is_empty() {
            // Non-business day before the first fixing: accumulate to assign
            // to the first fixing's weight once we encounter it.
            //
            pre_first_fixing_days += days;
        } else if let Some(last) = daily_rates.last_mut() {
            // Non-business day after a fixing: add to the preceding fixing.
            last.1 += days;
        }
        current = next_capped;
    }

    let total_days = (accrual_end - accrual_start).whole_days().max(1) as u32;
    Ok((daily_rates, total_days))
}

/// Per-build market data resolved once for floating coupon emission.
///
/// Both slices are aligned index-for-index with the builder's float
/// schedules; entries are `None` when the `MarketContext` lacks the
/// corresponding curve or `FIXING:{index_id}` series.
#[derive(Clone, Copy)]
pub(crate) struct ResolvedFloatMarket<'a> {
    /// Forward curves, one per float schedule.
    pub(crate) curves: &'a [Option<std::sync::Arc<ForwardCurve>>],
    /// Historical fixing series (`FIXING:{index_id}`), one per float schedule.
    pub(crate) fixings: &'a [Option<ScalarTimeSeries>],
}

fn apply_daily_index_constraints_if_needed<'a>(
    daily_rates: &'a [(f64, u32)],
    params: &super::super::rate_helpers::FloatingRateParams,
    runtime_spec: &super::super::rate_helpers::ResolvedFloatingRateSpec,
) -> std::borrow::Cow<'a, [(f64, u32)]> {
    if runtime_spec.overnight_index_constraints
        != super::super::specs::OvernightIndexConstraintApplication::Daily
    {
        // No per-day cap/floor to apply: borrow the sampled rates instead of
        // copying the whole per-day vector (a SOFR coupon period samples one
        // entry per business day).
        return std::borrow::Cow::Borrowed(daily_rates);
    }

    std::borrow::Cow::Owned(
        daily_rates
            .iter()
            .map(|&(rate, days)| {
                let mut constrained = rate;
                if let Some(floor) = params.index_floor_bp {
                    constrained = constrained.max(floor * 1e-4);
                }
                if let Some(cap) = params.index_cap_bp {
                    constrained = constrained.min(cap * 1e-4);
                }
                (constrained, days)
            })
            .collect(),
    )
}

fn period_rate_params_for_overnight(
    params: &super::super::rate_helpers::FloatingRateParams,
    runtime_spec: &super::super::rate_helpers::ResolvedFloatingRateSpec,
) -> super::super::rate_helpers::FloatingRateParams {
    let mut final_params = params.clone();
    if runtime_spec.overnight_index_constraints
        == super::super::specs::OvernightIndexConstraintApplication::Daily
    {
        final_params.index_floor_bp = None;
        final_params.index_cap_bp = None;
    }
    final_params
}

/// Emit floating coupon cashflows on a specific date.
///
/// Processes all floating coupon schedules for the given date, looking up forward
/// rates from the optional market context and computing coupon amounts based on
/// `forward_rate * gearing + margin`. Splits into cash/PIK according to coupon type.
/// Cash and PIK flows are appended directly into the provided `out_flows` buffer.
///
/// Seasoned coupons (observation dates before the curve base) resolve realized
/// index fixings from `market.fixings` — the per-schedule `FIXING:{index_id}`
/// series aligned with `market.curves` (LOCF for overnight observations,
/// exact-date for term resets). Without a series, pre-base observations route
/// through the spec's [`crate::builder::specs::FloatingRateFallback`] policy.
///
/// # Returns
///
/// `pik_to_add` — the total PIK coupon amount (across every floating schedule
/// processed on date `d`) that the caller must capitalize into the outstanding
/// balance for subsequent periods. Cash flows are pushed into `out_flows` as a
/// side effect; the return value is exclusively the PIK leg.
pub(crate) fn emit_float_coupons_on(
    d: Date,
    float_schedules: &[FloatSchedule],
    outstanding_after: &finstack_quant_core::HashMap<Date, Decimal>,
    outstanding_fallback: Decimal,
    ccy: Currency,
    market: ResolvedFloatMarket<'_>,
    out_flows: &mut Vec<CashFlow>,
) -> finstack_quant_core::Result<f64> {
    let mut pik_to_add = 0.0;

    for ((schedule, resolved_curve), resolved_fixing) in float_schedules
        .iter()
        .zip(market.curves.iter())
        .zip(market.fixings.iter())
    {
        let spec = &schedule.spec;
        let calendar = schedule.calendar;
        // Early exit: skip schedules where `d` is outside the date range.
        // This reduces iteration from O(N × M) to O(N + M) for multi-window instruments.
        if let (Some(&first), Some(&last)) = (schedule.dates.first(), schedule.dates.last()) {
            if d < first || d > last {
                continue;
            }
        }

        if let Some(period) = schedule.prev.get(&d).copied() {
            let accrual_start = period.accrual_start;
            let accrual_end = period.accrual_end;
            let is_stub = schedule.first_last.contains(&d);
            let is_termination_date = schedule
                .prev
                .values()
                .map(|period| period.accrual_end)
                .max()
                .is_some_and(|last| accrual_end == last);
            let base_out = *outstanding_after
                .get(&accrual_start)
                .unwrap_or(&outstanding_fallback);

            // Accrual context uses the PAYMENT frequency (mirroring the fixed
            // path): for ACT/ACT ICMA the coupon period is the payment
            // period, not the reset cadence. Regular periods use the accrual
            // period itself as the ICMA reference period; stubs fall back to
            // core's frequency-based quasi-coupon subdivision.
            let yf = spec.rate_spec.dc.year_fraction(
                accrual_start,
                accrual_end,
                finstack_quant_core::dates::DayCountContext {
                    calendar: Some(calendar),
                    frequency: Some(spec.freq),
                    bus_basis: None,
                    coupon_period: (!is_stub).then_some((accrual_start, accrual_end)),
                    end_is_termination_date: is_termination_date,
                },
            )?;

            // Compute reset date (fixing date) from accrual start. The reset
            // date is flow metadata; term-rate projection runs over the
            // index's underlying deposit period from the accrual start.
            let reset_date = compute_reset_date(
                accrual_start,
                spec.rate_spec.reset_lag_days,
                spec.rate_spec.bdc,
                schedule.fixing_calendar,
            )?;

            // Underlying index tenor: explicit `index_tenor` when set,
            // falling back to the reset frequency. The forward is projected
            // over [accrual_start, accrual_start + index_tenor] — the period
            // the referenced deposit rate actually covers.
            let index_tenor = spec
                .rate_spec
                .index_tenor
                .unwrap_or(spec.rate_spec.reset_freq);
            let index_maturity = compute_index_maturity(accrual_start, index_tenor)?;

            let runtime_spec = &schedule.runtime_spec;
            let params = &runtime_spec.params;
            let spread_bp = params.spread_bp;

            // Compute total rate using centralized projection with floor/cap support.
            // When projection fails (curve error or missing curve), the fallback
            // policy on the spec controls behavior:
            //   Error      -> propagate immediately (strictest, default)
            //   SpreadOnly -> use spread as total rate (legacy)
            //   FixedRate(r) -> use r as the index component
            let total_rate = if let Some(ref method) = spec.rate_spec.overnight_compounding {
                // ── Overnight compounding path ──
                // Sample daily rates from the forward curve and compound them
                // according to the ISDA 2021 method, then apply floor/cap/gearing/spread.
                if let Some(fwd) = resolved_curve.as_deref() {
                    // Per-variant sampling so each ISDA 2021 convention gets
                    // rates from the correct window:
                    //
                    // - `CompoundedWithLookback`: rates sampled from
                    //   `lookback_days` business days before each accrual-
                    //   period business day; weights remain accrual-tied.
                    //   Annualization = accrual-period day count.
                    //   (ARRC 2020 §2; ISDA 2021 Supp. 70 §7.1(g)(ii).)
                    // - `CompoundedWithObservationShift`: the whole window
                    //   moves earlier by `shift_days` business days — both
                    //   rates AND weights come from the shifted window.
                    //   Annualization = shifted-window day count.
                    //   (ISDA 2021 Supp. 70 §7.1(g)(i).)
                    // - All other variants: sample on the accrual window.
                    //
                    // Sampling is done at the observation window here
                    // rather than post-hoc via index rewriting in
                    // `compute_overnight_rate` so that rates from before
                    // the accrual start are accessible (required for
                    // correct SOFR/SONIA compounded indices — ARRC 2020;
                    // BoE SONIA Compounded Index Guide).
                    // Overnight fixings are observed on the index's fixing
                    // calendar (resolved by the compiler from
                    // `fixing_calendar_id`, defaulting to the accrual
                    // calendar), not the accrual calendar.
                    let overnight_dc = spec
                        .rate_spec
                        .overnight_basis
                        .unwrap_or(finstack_quant_core::dates::DayCount::Act360);
                    let day_count_basis = match overnight_dc {
                        finstack_quant_core::dates::DayCount::Act360 => 360.0,
                        finstack_quant_core::dates::DayCount::Act365F => 365.0,
                        other => {
                            return Err(finstack_quant_core::Error::Validation(format!(
                                "overnight_basis must be Act360 or Act365F; got {other:?}"
                            )))
                        }
                    };
                    let fixing_calendar = schedule.fixing_calendar;
                    let index_id = spec.rate_spec.index_id.as_str();
                    let fixings = resolved_fixing.as_ref();
                    let sampled =
                        (|| match method {
                            OvernightCompoundingMethod::CompoundedWithLookback {
                                lookback_days,
                            } if *lookback_days > 0 => sample_overnight_rates_with_lookback(
                                accrual_start,
                                accrual_end,
                                *lookback_days,
                                OvernightSamplingInput {
                                    fwd,
                                    overnight_basis: day_count_basis,
                                    calendar: fixing_calendar,
                                    fixings,
                                    index_id,
                                },
                            ),
                            _ => {
                                let (obs_start, obs_end) = observation_window(
                                    method,
                                    accrual_start,
                                    accrual_end,
                                    fixing_calendar,
                                )?;
                                sample_overnight_rates(
                                    obs_start,
                                    obs_end,
                                    fwd,
                                    day_count_basis,
                                    fixing_calendar,
                                    fixings,
                                    index_id,
                                )
                            }
                        })();

                    match sampled {
                        Ok((daily_rates, total_days)) => {
                            // A non-empty accrual window that produces zero
                            // fixings means the index never fixes in the
                            // period; proceeding would silently treat the
                            // index as 0% and accrue spread-only.
                            if daily_rates.is_empty() && accrual_start < accrual_end {
                                return Err(finstack_quant_core::Error::Validation(format!(
                                    "overnight accrual period [{accrual_start}, {accrual_end}) \
                                     for index '{}' contains no business-day fixings on \
                                     calendar '{}'; check the business-day convention and \
                                     fixing calendar",
                                    spec.rate_spec.index_id,
                                    spec.rate_spec
                                        .fixing_calendar_id
                                        .as_deref()
                                        .unwrap_or(&spec.rate_spec.calendar_id),
                                )));
                            }

                            let daily_rates = apply_daily_index_constraints_if_needed(
                                &daily_rates,
                                params,
                                runtime_spec,
                            );
                            let compounded_index =
                                super::super::rate_helpers::compute_overnight_rate(
                                    *method,
                                    &daily_rates,
                                    total_days,
                                    day_count_basis,
                                );

                            let final_params =
                                period_rate_params_for_overnight(params, runtime_spec);
                            super::super::rate_helpers::calculate_floating_rate(
                                compounded_index,
                                &final_params,
                            )
                        }
                        // Sampling failures (e.g. pre-base observations on a
                        // seasoned coupon) route through the fallback policy,
                        // mirroring the term-rate path.
                        Err(error) => rate_when_projection_fails(
                            &error,
                            reset_date,
                            accrual_end,
                            spread_bp,
                            &runtime_spec.fallback,
                            params,
                        )?,
                    }
                } else {
                    rate_when_curve_missing(
                        spec.rate_spec.index_id.as_str(),
                        reset_date,
                        spread_bp,
                        &runtime_spec.fallback,
                        params,
                        " (overnight compounding)",
                    )?
                }
            } else if let Some(fwd) = resolved_curve.as_deref() {
                // ── Standard term rate projection path ──
                // Project the forward over the index's underlying deposit
                // period [accrual_start, accrual_start + index_tenor]; the
                // fixing date stays flow metadata only.
                //
                // Seasoned coupons (projection start strictly before the
                // curve base) resolve the realized index fixing from the
                // `FIXING:{index_id}` series using exact-date matching — term
                // resets fix on a specific published date, and reset dates
                // are business-day adjusted upstream. The fixing is the INDEX
                // rate only: gearing/spread/floors/caps apply on top exactly
                // as for projected rates. Without a series, the projection
                // errors and routes through the fallback policy.
                let projected = if reset_date < fwd.base_date()
                    || (reset_date == fwd.base_date() && resolved_fixing.is_some())
                {
                    params.validate().and_then(|()| {
                        require_fixing_value_exact(
                            resolved_fixing.as_ref(),
                            spec.rate_spec.index_id.as_str(),
                            reset_date,
                            fwd.base_date(),
                        )
                        .map(|index_rate| {
                            super::super::rate_helpers::calculate_floating_rate(index_rate, params)
                        })
                    })
                } else {
                    super::super::rate_helpers::project_floating_rate(
                        accrual_start,
                        index_maturity, // accrual_start + index tenor, not payment date
                        fwd,
                        params,
                    )
                };
                match projected {
                    Ok(rate) => rate,
                    Err(error) => rate_when_projection_fails(
                        &error,
                        reset_date,
                        index_maturity,
                        spread_bp,
                        &runtime_spec.fallback,
                        params,
                    )?,
                }
            } else {
                rate_when_curve_missing(
                    spec.rate_spec.index_id.as_str(),
                    reset_date,
                    spread_bp,
                    &runtime_spec.fallback,
                    params,
                    "",
                )?
            };

            // Convert f64 values to Decimal with proper error handling for NaN/Infinity.
            // This prevents silent masking of invalid values as zero.
            let total_rate_dec = f64_to_decimal(total_rate)?;
            let yf_dec = f64_to_decimal(yf)?;
            let coupon_total_dec = base_out * total_rate_dec * yf_dec;
            let coupon_total = decimal_to_f64(coupon_total_dec)?;

            let (cash_pct, pik_pct) = spec.coupon_type.split_parts()?;
            let cash_pct_f64 = decimal_to_f64(cash_pct)?;
            let pik_pct_f64 = decimal_to_f64(pik_pct)?;
            let cash_amt = coupon_total * cash_pct_f64;
            let pik_amt = coupon_total * pik_pct_f64;

            // Emit cash portion of floating coupon if any.
            // Note: PIK portion is emitted separately via add_pik_flow_if_nonzero.
            // For 100% PIK coupons, only the PIK flow is emitted, which is intentional
            // since the schedule structure (dates, accrual factors) is preserved in PIK flows.
            if cash_pct_f64 > 0.0 {
                out_flows.push(CashFlow {
                    date: d,
                    reset_date: Some(reset_date),
                    amount: Money::new(cash_amt, ccy),
                    kind: CFKind::FloatReset,
                    accrual_factor: yf,
                    rate: Some(total_rate),
                });
            }

            pik_to_add +=
                add_pik_flow_if_nonzero(out_flows, d, pik_amt, ccy, Some(total_rate), yf)?;
        }
    }
    Ok(pik_to_add)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::calendar::resolve_calendar_strict;
    use time::Month;

    #[test]
    fn emit_inflation_coupons_preserves_non_positive_amounts() {
        let mut flows = Vec::new();
        emit_inflation_coupons(
            Currency::USD,
            &[
                (
                    Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
                    0.0,
                    0.5,
                    0.02,
                ),
                (
                    Date::from_calendar_date(2025, Month::July, 1).expect("valid date"),
                    -12.5,
                    0.5,
                    0.02,
                ),
            ],
            &mut flows,
        );

        assert_eq!(flows.len(), 2);
        assert_eq!(flows[0].kind, CFKind::InflationCoupon);
        assert_eq!(flows[1].amount.amount(), -12.5);
    }

    #[test]
    fn sample_overnight_rates_propagates_day_count_errors() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::January, 3).expect("valid date");
        let curve = ForwardCurve::builder("TEST-ON", 1.0 / 360.0)
            .base_date(base)
            .day_count(finstack_quant_core::dates::DayCount::ActActIsma)
            .knots([(0.0, 0.05), (1.0, 0.05)])
            .build()
            .expect("valid forward curve");
        let calendar = resolve_calendar_strict("weekends_only").expect("calendar registered");

        let err = sample_overnight_rates(base, end, &curve, 360.0, calendar, None, "TEST-ON")
            .expect_err("Act/Act ISMA requires frequency context");

        assert!(
            err.to_string().contains("frequency") || err.to_string().contains("Invalid"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn sample_overnight_rates_with_lookback_propagates_day_count_errors() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let start = Date::from_calendar_date(2025, Month::January, 6).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::January, 7).expect("valid date");
        let curve = ForwardCurve::builder("TEST-ON", 1.0 / 360.0)
            .base_date(base)
            .day_count(finstack_quant_core::dates::DayCount::ActActIsma)
            .knots([(0.0, 0.05), (1.0, 0.05)])
            .build()
            .expect("valid forward curve");
        let calendar = resolve_calendar_strict("weekends_only").expect("calendar registered");

        let err = sample_overnight_rates_with_lookback(
            start,
            end,
            1,
            OvernightSamplingInput {
                fwd: &curve,
                overnight_basis: 360.0,
                calendar,
                fixings: None,
                index_id: "TEST-ON",
            },
        )
        .expect_err("Act/Act ISMA requires frequency context");

        assert!(
            err.to_string().contains("frequency") || err.to_string().contains("Invalid"),
            "unexpected error: {err}"
        );
    }
}
