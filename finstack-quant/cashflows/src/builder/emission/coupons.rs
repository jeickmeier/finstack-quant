//! Coupon cashflow emission (fixed and floating).
//!
//! `emit_inflation_coupons` maps pre-indexed inflation coupon tuples into
//! `CashFlow` values. Index projection and ratio logic belong in the instrument
//! layer that supplies those tuples.

use crate::primitives::{CFKind, CashFlow};
use finstack_quant_core::cashflow::CashFlowAccrual;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::fixings::{fixing_series_id, require_fixing_value_exact};
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::InputError;
use rust_decimal::Decimal;
use tracing::{info, warn};

use crate::builder::overnight::{OvernightObservationSchedule, OvernightRateConstraints};
use crate::builder::rate_helpers::ResolvedFloatingRateFallback;
use crate::builder::{
    CompiledFloatingCoupon, FloatingCouponEconomics, FloatingCouponPeriod, FloatingRateObservation,
};

use super::super::compiler::{FixedSchedule, FloatSchedule};
use super::helpers::{add_pik_flow_if_nonzero, compute_reset_date};

/// Append pre-computed inflation-linked coupon cashflows.
///
/// This function does not project CPI/RPI/HICP fixings or calculate index
/// ratios. It preserves caller-computed indexed coupon amounts and tags them as
/// [`CFKind::InflationCoupon`] for downstream valuation/reporting.
///
/// Each tuple is `(payment_date, indexed_coupon_amount, accrual_factor, real_coupon_rate)`.
///
/// # Arguments
///
/// * `ccy` - Currency assigned to every emitted inflation-coupon cashflow.
/// * `coupons` - Precomputed payment tuples of date, indexed amount, accrual
///   factor in years, and annual real coupon rate as a decimal.
/// * `out_flows` - Mutable schedule flow list to which one inflation-coupon
///   row is appended for each supplied tuple.
pub fn emit_inflation_coupons(
    ccy: Currency,
    coupons: &[(Date, f64, f64, f64)],
    out_flows: &mut Vec<CashFlow>,
) -> finstack_quant_core::Result<()> {
    for &(date, amount, accrual_factor, real_coupon_rate) in coupons {
        out_flows.push(CashFlow::new(
            date,
            None,
            Money::new(amount, ccy)?,
            CFKind::InflationCoupon,
            accrual_factor,
            Some(real_coupon_rate),
        ));
    }
    Ok(())
}

// Shared f64 ↔ Decimal conversion helpers live in the parent `emission` module
// so that `fees.rs` can use them too. Access via `super::`.
use super::{decimal_to_f64, f64_to_decimal};

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
    fwd_day_count_basis: f64,
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
    let fwd_day_count = fwd.day_count();
    let t = if obs_date == fwd_base {
        0.0
    } else {
        fwd_day_count.year_fraction(
            fwd_base,
            obs_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?
    };
    // Use the average forward rate over the overnight tenor [t, t+days/basis]
    // rather than the instantaneous forward at t. For piecewise-constant
    // curves the two are identical, but for interpolated curves (linear,
    // cubic) `rate_period` gives the correct overnight forward average.
    let overnight_dt = f64::from(weight_days) / fwd_day_count_basis;
    Ok(fwd.rate_period(t, t + overnight_dt))
}

/// Resolve a floating-rate fallback outcome shared by the curve-missing and
/// projection-failure call sites: propagate `error` when the policy demands
/// it, otherwise log and return the fallback (spread-only or fixed-rate) all-in
/// rate. `error` is built by the caller (a `NotFound` for a missing curve, or
/// the original projection error).
fn resolve_floating_rate_fallback(
    error: finstack_quant_core::Error,
    reset_date: Date,
    spread_bp: f64,
    fallback: &ResolvedFloatingRateFallback,
    params: &crate::builder::rate_helpers::FloatingRateParams,
) -> finstack_quant_core::Result<(f64, Option<f64>)> {
    match fallback {
        ResolvedFloatingRateFallback::Error => Err(error),
        ResolvedFloatingRateFallback::SpreadOnly => {
            warn!(
                reset_date = %reset_date,
                spread_bp = %spread_bp,
                error = %error,
                "Floating rate unavailable, using fallback (spread-only) rate"
            );
            fallback
                .fallback_rate(params)
                .map(|rate| (rate, fallback.fallback_index_rate()))
                .ok_or(finstack_quant_core::Error::Input(InputError::Invalid))
        }
        ResolvedFloatingRateFallback::FixedRate(index_rate) => {
            info!(
                reset_date = %reset_date,
                fixed_rate = %index_rate,
                error = %error,
                "Floating rate unavailable, using fixed index rate"
            );
            fallback
                .fallback_rate(params)
                .map(|rate| (rate, fallback.fallback_index_rate()))
                .ok_or(finstack_quant_core::Error::Input(InputError::Invalid))
        }
    }
}

fn rate_when_curve_missing(
    index_id: &str,
    reset_date: Date,
    spread_bp: f64,
    fallback: &ResolvedFloatingRateFallback,
    params: &crate::builder::rate_helpers::FloatingRateParams,
    context_suffix: &str,
) -> finstack_quant_core::Result<(f64, Option<f64>)> {
    let error = finstack_quant_core::Error::Input(InputError::NotFound {
        id: format!(
            "forward curve '{}' not found for reset date {}{}",
            index_id, reset_date, context_suffix
        ),
    });
    resolve_floating_rate_fallback(error, reset_date, spread_bp, fallback, params)
}

fn rate_when_projection_fails(
    error: &finstack_quant_core::Error,
    reset_date: Date,
    spread_bp: f64,
    fallback: &ResolvedFloatingRateFallback,
    params: &crate::builder::rate_helpers::FloatingRateParams,
) -> finstack_quant_core::Result<(f64, Option<f64>)> {
    resolve_floating_rate_fallback(error.clone(), reset_date, spread_bp, fallback, params)
}

fn fallback_index_rate(
    outcome: finstack_quant_core::Result<(f64, Option<f64>)>,
) -> finstack_quant_core::Result<f64> {
    outcome?.1.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "floating-rate fallback did not provide a finite index rate".to_string(),
        )
    })
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
            let is_termination_date = schedule.terminal_accrual_end == Some(accrual_end);
            let base_out = *outstanding_after
                .get(&accrual_start)
                .unwrap_or(&outstanding_fallback);

            // ACT/ACT ICMA: regular periods use the unadjusted scheduled
            // span; stubs use the adjacent regular coupon as the reference.
            let yf = spec.schedule.day_count.year_fraction(
                accrual_start,
                accrual_end,
                finstack_quant_core::dates::DayCountContext {
                    calendar: Some(calendar),
                    frequency: Some(spec.schedule.frequency),
                    bus_basis: None,
                    coupon_period: crate::builder::date_generation::icma_coupon_period(
                        period.unadjusted_start,
                        period.unadjusted_end,
                        spec.schedule.frequency,
                    ),
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

            let rate_f64 = decimal_to_f64(spec.rate)?;

            // Gate on the cash split percentage (mirroring the floating path)
            // rather than the cash amount's sign, so negative-rate coupons
            // emit instead of being silently dropped.
            if cash_pct_f64 > 0.0 {
                let kind = if is_stub { CFKind::Stub } else { CFKind::Fixed };
                out_flows.push(
                    CashFlow::new(
                        d,
                        None,
                        Money::new(cash_amt, ccy)?,
                        kind,
                        yf,
                        Some(rate_f64),
                    )
                    .with_accrual(CashFlowAccrual {
                        start: accrual_start,
                        end: accrual_end,
                        day_count: spec.schedule.day_count,
                        projected_index_rate: None,
                    }),
                );
            }

            let pik_added =
                add_pik_flow_if_nonzero(out_flows, d, pik_amt, ccy, Some(rate_f64), yf)?;
            if pik_added > 0.0 {
                if let Some(flow) = out_flows.last_mut() {
                    flow.accrual = Some(CashFlowAccrual {
                        start: accrual_start,
                        end: accrual_end,
                        day_count: spec.schedule.day_count,
                        projected_index_rate: None,
                    });
                }
            }
            pik_to_add += pik_added;
        }
    }
    Ok(pik_to_add)
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
            let is_termination_date = schedule.terminal_accrual_end == Some(accrual_end);
            let base_out = *outstanding_after
                .get(&accrual_start)
                .unwrap_or(&outstanding_fallback);

            // Accrual context uses the PAYMENT frequency (mirroring the fixed
            // path): for ACT/ACT ICMA the coupon period is the payment
            // period, not the reset cadence. Regular periods use the
            // unadjusted scheduled span; stubs use the adjacent regular coupon.
            let yf = spec.schedule.day_count.year_fraction(
                accrual_start,
                accrual_end,
                finstack_quant_core::dates::DayCountContext {
                    calendar: Some(calendar),
                    frequency: Some(spec.schedule.frequency),
                    bus_basis: None,
                    coupon_period: crate::builder::date_generation::icma_coupon_period(
                        period.unadjusted_start,
                        period.unadjusted_end,
                        spec.schedule.frequency,
                    ),
                    end_is_termination_date: is_termination_date,
                },
            )?;

            // Compute the term-index fixing date from accrual start and the
            // configured reset lag.
            let reset_date = compute_reset_date(
                accrual_start,
                spec.rate_spec.reset_lag_days,
                spec.schedule.business_day_convention,
                schedule.fixing_calendar,
            )?;

            let runtime_spec = &schedule.runtime_spec;
            let params = &runtime_spec.params;
            let spread_bp = params.spread_bp;
            let (cash_pct, pik_pct) = spec.coupon_type.split_parts()?;
            let cash_pct_f64 = decimal_to_f64(cash_pct)?;
            let pik_pct_f64 = decimal_to_f64(pik_pct)?;
            let base_out_f64 = decimal_to_f64(base_out)?;

            let observation = if let Some(method) = spec.rate_spec.overnight_compounding {
                let overnight_basis = spec
                    .rate_spec
                    .overnight_basis
                    .unwrap_or(spec.schedule.day_count);
                let day_count_basis = match overnight_basis {
                    finstack_quant_core::dates::DayCount::Act360 => 360.0,
                    finstack_quant_core::dates::DayCount::Act365F => 365.0,
                    other => {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "overnight compounding requires Act360 or Act365F; got {other:?}"
                        )))
                    }
                };
                let observations = OvernightObservationSchedule::compile(
                    accrual_start,
                    accrual_end,
                    method,
                    schedule.fixing_calendar,
                )?;
                if observations.observations().is_empty() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "overnight accrual period [{accrual_start}, {accrual_end}) for index '{}' contains no business-day fixings",
                        spec.rate_spec.index_id
                    )));
                }
                FloatingRateObservation::Overnight {
                    schedule: observations,
                    day_count_basis,
                    constraints: OvernightRateConstraints {
                        application: runtime_spec.overnight_index_constraints,
                        index_floor_bp: params.index_floor_bp,
                        index_cap_bp: params.index_cap_bp,
                    },
                }
            } else {
                FloatingRateObservation::Term {
                    reset_date,
                    tenor_years: resolved_curve.as_deref().map_or_else(
                        || {
                            spec.rate_spec
                                .index_tenor
                                .unwrap_or(spec.rate_spec.reset_frequency)
                                .to_years()
                        },
                        ForwardCurve::tenor,
                    ),
                }
            };
            let mut settlement_params = params.clone();
            if matches!(&observation, FloatingRateObservation::Overnight { .. }) {
                settlement_params.index_floor_bp = None;
                settlement_params.index_cap_bp = None;
            }
            let compiled = CompiledFloatingCoupon::compile(
                FloatingCouponPeriod {
                    accrual_start,
                    accrual_end,
                    payment_date: d,
                    day_count: spec.schedule.day_count,
                    accrual_factor: yf,
                },
                FloatingCouponEconomics {
                    cash_fraction: cash_pct_f64,
                    pik_fraction: pik_pct_f64,
                    rate_params: settlement_params,
                },
                observation,
            )?;

            let settlement = match compiled.observation() {
                FloatingRateObservation::Overnight {
                    day_count_basis, ..
                } => {
                    let fallback = |error: &finstack_quant_core::Error| {
                        fallback_index_rate(rate_when_projection_fails(
                            error,
                            reset_date,
                            spread_bp,
                            &runtime_spec.fallback,
                            params,
                        ))
                    };
                    if let Some(fwd) = resolved_curve.as_deref() {
                        let fixings = resolved_fixing.as_ref();
                        let index_id = spec.rate_spec.index_id.as_str();
                        let mut state = compiled.replay_state();
                        compiled.capture_notional(&mut state, base_out_f64)?;
                        match compiled.advance_overnight(&mut state, accrual_end, |slice| {
                            observed_overnight_rate(
                                slice.observation_date,
                                slice.rate_tenor_days,
                                fwd,
                                *day_count_basis,
                                fixings,
                                index_id,
                            )
                        }) {
                            Ok(_) => compiled.settle(&state)?,
                            Err(error) => {
                                let index_rate = fallback(&error)?;
                                compiled.settle_index_rate(base_out_f64, index_rate, index_rate)?
                            }
                        }
                    } else {
                        let index_rate = fallback_index_rate(rate_when_curve_missing(
                            spec.rate_spec.index_id.as_str(),
                            reset_date,
                            spread_bp,
                            &runtime_spec.fallback,
                            params,
                            " (overnight compounding)",
                        ))?;
                        compiled.settle_index_rate(base_out_f64, index_rate, index_rate)?
                    }
                }
                FloatingRateObservation::Term { .. } => {
                    let index_rate = if let Some(fwd) = resolved_curve.as_deref() {
                        let same_day_fixing_exists = reset_date == fwd.base_date()
                            && resolved_fixing
                                .as_ref()
                                .is_some_and(|series| series.value_on_exact(reset_date).is_ok());
                        let projected = if reset_date < fwd.base_date() || same_day_fixing_exists {
                            require_fixing_value_exact(
                                resolved_fixing.as_ref(),
                                spec.rate_spec.index_id.as_str(),
                                reset_date,
                                fwd.base_date(),
                            )
                        } else {
                            super::super::rate_helpers::project_index_rate(reset_date, fwd)
                        };
                        match projected {
                            Ok(rate) => rate,
                            Err(error) => fallback_index_rate(rate_when_projection_fails(
                                &error,
                                reset_date,
                                spread_bp,
                                &runtime_spec.fallback,
                                params,
                            ))?,
                        }
                    } else {
                        fallback_index_rate(rate_when_curve_missing(
                            spec.rate_spec.index_id.as_str(),
                            reset_date,
                            spread_bp,
                            &runtime_spec.fallback,
                            params,
                            "",
                        ))?
                    };
                    let mut state = compiled.replay_state();
                    compiled.observe_term(&mut state, index_rate)?;
                    compiled.capture_notional(&mut state, base_out_f64)?;
                    compiled.settle(&state)?
                }
            };
            let total_rate = settlement.all_in_rate;
            let projected_index_rate = Some(settlement.projected_index_rate);
            let cash_amt = settlement.cash_amount;
            let pik_amt = settlement.pik_amount;

            // Emit cash portion of floating coupon if any.
            // Note: PIK portion is emitted separately via add_pik_flow_if_nonzero.
            // For 100% PIK coupons, only the PIK flow is emitted, which is intentional
            // since the schedule structure (dates, accrual factors) is preserved in PIK flows.
            if cash_pct_f64 > 0.0 {
                out_flows.push(
                    CashFlow::new(
                        d,
                        Some(reset_date),
                        Money::new(cash_amt, ccy)?,
                        CFKind::FloatReset,
                        yf,
                        Some(total_rate),
                    )
                    .with_accrual(CashFlowAccrual {
                        start: accrual_start,
                        end: accrual_end,
                        day_count: spec.schedule.day_count,
                        projected_index_rate,
                    }),
                );
            }

            let pik_added =
                add_pik_flow_if_nonzero(out_flows, d, pik_amt, ccy, Some(total_rate), yf)?;
            if pik_added > 0.0 {
                if let Some(flow) = out_flows.last_mut() {
                    flow.accrual = Some(CashFlowAccrual {
                        start: accrual_start,
                        end: accrual_end,
                        day_count: spec.schedule.day_count,
                        projected_index_rate,
                    });
                }
            }
            pik_to_add += pik_added;
        }
    }
    Ok(pik_to_add)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::calendar::resolve_calendar_strict;
    use crate::builder::OvernightCompoundingMethod;
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
        )
        .expect("valid inflation coupons fixture");

        assert_eq!(flows.len(), 2);
        assert_eq!(flows[0].kind, CFKind::InflationCoupon);
        assert_eq!(flows[1].amount.amount(), -12.5);
    }

    #[test]
    fn overnight_replay_propagates_day_count_errors() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::January, 3).expect("valid date");
        let curve = ForwardCurve::builder("TEST-ON", 1.0 / 360.0)
            .base_date(base)
            .day_count(finstack_quant_core::dates::DayCount::ActActIsma)
            .knots([(0.0, 0.05), (1.0, 0.05)])
            .build()
            .expect("valid forward curve");
        let calendar = resolve_calendar_strict("weekends_only").expect("calendar registered");

        let observations = OvernightObservationSchedule::compile(
            base,
            end,
            OvernightCompoundingMethod::CompoundedInArrears,
            calendar,
        )
        .expect("observation schedule");
        let err = observations
            .replay(end, 360.0, OvernightRateConstraints::default(), |slice| {
                observed_overnight_rate(
                    slice.observation_date,
                    slice.rate_tenor_days,
                    &curve,
                    360.0,
                    None,
                    "TEST-ON",
                )
            })
            .expect_err("Act/Act ISMA requires frequency context");

        assert!(
            err.to_string().contains("frequency") || err.to_string().contains("Invalid"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn overnight_lookback_replay_propagates_day_count_errors() {
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

        let observations = OvernightObservationSchedule::compile(
            start,
            end,
            OvernightCompoundingMethod::CompoundedWithLookback { lookback_days: 1 },
            calendar,
        )
        .expect("observation schedule");
        let err = observations
            .replay(end, 360.0, OvernightRateConstraints::default(), |slice| {
                observed_overnight_rate(
                    slice.observation_date,
                    slice.rate_tenor_days,
                    &curve,
                    360.0,
                    None,
                    "TEST-ON",
                )
            })
            .expect_err("Act/Act ISMA requires frequency context");

        assert!(
            err.to_string().contains("frequency") || err.to_string().contains("Invalid"),
            "unexpected error: {err}"
        );
    }
}
