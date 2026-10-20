//! Coupon cashflow emission (fixed and floating).
//!
//! `emit_inflation_coupons` maps pre-indexed inflation coupon tuples into
//! `CashFlow` values. Index projection and ratio logic belong in the instrument
//! layer that supplies those tuples.

use crate::builder::FloatingLegCompounding;
use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowAccrual};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::fixings::{fixing_series_id, require_fixing_value_exact};
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::InputError;
use rust_decimal::Decimal;
use tracing::warn;

use crate::builder::overnight::overnight_day_count_basis;
use crate::builder::overnight::{OvernightObservationSchedule, OvernightRateConstraints};
use crate::builder::rate_helpers::{
    apply_bp_bounds, calculate_floating_rate, project_index_rate, project_overnight_rate,
};
use crate::builder::{
    CompiledFloatingCoupon, FloatingCouponEconomics, FloatingCouponPeriod, FloatingRateFallback,
    FloatingRateObservation, SchedulePeriod,
};

use crate::builder::compiler::{FixedSchedule, FloatSchedule};
use crate::builder::periods::compute_reset_date;
use finstack_quant_core::decimal::f64_to_decimal;

/// Add a PIK cashflow if the amount is strictly positive.
///
/// Returns the PIK amount for outstanding balance tracking (`0.0` when no
/// flow was added).
///
/// # Errors
///
/// Returns `Error::Validation` when `pik_amt` is negative. A negative PIK
/// coupon (negative all-in rate on a PIK/split leg) would have to *reduce*
/// the outstanding balance; that de-capitalization policy is not modeled, so
/// it fails loudly instead of being silently dropped. Configure an
/// `all_in_floor_bp` of zero (or pay cash) for negative-rate PIK structures.
fn add_pik_flow_if_nonzero(
    flows: &mut Vec<CashFlow>,
    date: Date,
    pik_amt: f64,
    ccy: Currency,
    rate: Option<f64>,
    accrual_factor: f64,
) -> finstack_quant_core::Result<f64> {
    if pik_amt < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "negative PIK coupon amount {pik_amt} on {date}: de-capitalizing PIK is not \
             supported; floor the all-in rate at zero or use a cash coupon"
        )));
    }
    if pik_amt > 0.0 {
        flows.push(CashFlow::new(
            date,
            None,
            Money::new(pik_amt, ccy)?,
            CFKind::Pik,
            accrual_factor,
            rate,
        ));
        Ok(pik_amt)
    } else {
        Ok(0.0)
    }
}

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

/// Error for an overnight observation date strictly before the curve base
/// when no historical fixing series is available.
///
/// Historical fixings are supported via a [`ScalarTimeSeries`] stored in the
/// `MarketContext` under the canonical id `FIXING:{forward_curve_id}` (see
/// [`finstack_quant_core::market_data::fixings`]). This error is raised only when
/// no such series was provided; the emission layer routes it through the
/// spec's [`crate::builder::FloatingRateFallback`] policy.
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
///   `FIXING:{forward_curve_id}` series. Weekend/holiday carry is represented by the
///   observation day weight, so a missing business-day publication is an
///   error rather than permission to reuse an arbitrarily old rate. Errors via
///   [`pre_base_observation_error`] when no series is provided. An incomplete
///   supplied series remains an error even when a fallback is configured.
/// - `obs_date == curve base`: a published same-day fixing (exact-date match)
///   is preferred when present — today's fixing may or may not be published
///   yet; otherwise the rate is projected from `t = 0`.
/// - `obs_date > curve base`: the overnight forward is projected from the
///   calendar observation interval, with both endpoints on the curve's clock.
fn observed_overnight_rate(
    obs_date: Date,
    day_count_basis: f64,
    fwd: &ForwardCurve,
    fixings: Option<&ScalarTimeSeries>,
    forward_curve_id: &str,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if obs_date < as_of {
        return match fixings {
            Some(series) => {
                require_fixing_value_exact(Some(series), forward_curve_id, obs_date, as_of)
            }
            None => Err(pre_base_observation_error(obs_date, fwd)),
        };
    }
    if obs_date == as_of {
        if let Some(fixing) = fixings.and_then(|series| series.value_on_exact(obs_date).ok()) {
            return Ok(fixing);
        }
    }
    project_overnight_rate(fwd, obs_date, day_count_basis)
}

/// Prevent a projection fallback from replacing incomplete supplied history.
///
/// This check runs only on a failed projection, keeping successful lookups on
/// their existing path. Missing curves and absent series can still use the
/// explicitly configured fallback; missing observations in a supplied series
/// cannot.
fn require_supplied_historical_fixings(
    observation: &FloatingRateObservation,
    observation_date: Option<Date>,
    curve: Option<&ForwardCurve>,
    fixings: Option<&ScalarTimeSeries>,
    forward_curve_id: &str,
) -> finstack_quant_core::Result<()> {
    let (Some(curve), Some(fixings)) = (curve, fixings) else {
        return Ok(());
    };
    let as_of = observation_date.unwrap_or(curve.base_date());
    let require_historical = |date| {
        if date < as_of {
            require_fixing_value_exact(Some(fixings), forward_curve_id, date, as_of)?;
        }
        Ok(())
    };
    match observation {
        FloatingRateObservation::Term { reset_date, .. } => require_historical(*reset_date),
        FloatingRateObservation::Overnight { schedule, .. } => schedule
            .observations()
            .iter()
            .try_for_each(|slice| require_historical(slice.observation_date)),
    }
}

/// Resolve a floating-rate fallback outcome shared by the curve-missing and
/// projection-failure call sites: propagate `error` when the policy demands
/// it, otherwise log and return the fallback index rate. `error` is built by the caller (a `NotFound` for a missing curve, or
/// the original projection error).
fn resolve_floating_rate_fallback(
    error: finstack_quant_core::Error,
    reset_date: Date,
    spread_bp: f64,
    fallback: &FloatingRateFallback,
) -> finstack_quant_core::Result<f64> {
    if !fallback.is_default() {
        warn!(
            reset_date = %reset_date,
            spread_bp = %spread_bp,
            ?fallback,
            error = %error,
            "Floating rate unavailable, using the configured fallback index rate"
        );
    }
    fallback.index_rate(error)
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
    outstanding_history: &[(Date, Decimal)],
    outstanding_fallback: Decimal,
    ccy: Currency,
    out_flows: &mut Vec<CashFlow>,
) -> finstack_quant_core::Result<f64> {
    let mut pik_to_add = 0.0;

    for schedule in fixed_schedules {
        let spec = &schedule.spec;
        let calendar = schedule.calendar;
        let first = schedule
            .periods
            .partition_point(|entry| entry.period.accrual_end < d);
        for entry in schedule.periods[first..]
            .iter()
            .take_while(|entry| entry.period.accrual_end == d)
        {
            let period = &entry.period;
            let d = period.payment_date;
            for (accrual_start, accrual_end, base_out) in super::balances::balance_segments(
                outstanding_history,
                period.accrual_start,
                period.accrual_end,
                outstanding_fallback,
            ) {
                let is_stub = entry.is_stub;
                let is_termination_date = schedule.terminal_accrual_end == Some(accrual_end);

                let day_count_ctx = crate::builder::periods::period_day_count_context(
                    period,
                    calendar,
                    spec.schedule.frequency,
                    spec.schedule.day_count,
                    spec.schedule.stub,
                    spec.schedule.end_of_month,
                    spec.schedule.roll_rule,
                    is_termination_date,
                );
                let coupon_period = day_count_ctx.coupon_period;
                let yf = crate::builder::periods::contractual_accrual(
                    spec.schedule.day_count,
                    (period.accrual_start, period.accrual_end),
                    accrual_start,
                    accrual_end,
                    day_count_ctx,
                )?;

                let yf_dec = f64_to_decimal(yf)?;
                let coupon_total_dec = base_out
                    .checked_mul(spec.rate)
                    .and_then(|amount| amount.checked_mul(yf_dec))
                    .ok_or_else(|| {
                        finstack_quant_core::Error::Validation(
                            "fixed coupon amount exceeds Decimal range".into(),
                        )
                    })?;
                let coupon_total = coupon_total_dec.as_f64();

                let (cash_fraction, pik_fraction) = spec.coupon_type.split_parts()?;
                let cash_fraction_f64 = cash_fraction.as_f64();
                let pik_fraction_f64 = pik_fraction.as_f64();

                let cash_amt = coupon_total * cash_fraction_f64;
                let pik_amt = coupon_total * pik_fraction_f64;

                let rate_f64 = spec.rate.as_f64();
                let accrual = CashFlowAccrual {
                    coupon_period,
                    end_is_termination_date: is_termination_date,
                    calendar_id: Some(spec.schedule.calendar_id.to_string()),
                    start: accrual_start,
                    end: accrual_end,
                    day_count: spec.schedule.day_count,
                    projected_index_rate: None,
                };

                // Gate on cash split, not amount sign, so negative-rate coupons emit.
                if cash_fraction_f64 > 0.0 {
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
                        .with_accrual(accrual.clone()),
                    );
                }

                let pik_added =
                    add_pik_flow_if_nonzero(out_flows, d, pik_amt, ccy, Some(rate_f64), yf)?;
                if pik_added > 0.0 {
                    if let Some(flow) = out_flows.last_mut() {
                        flow.principal_date = Some(period.accrual_end);
                        flow.accrual = Some(accrual);
                    }
                }
                pik_to_add += pik_added;
            }
        }
    }
    Ok(pik_to_add)
}

/// Per-build market data resolved once for floating coupon emission.
///
/// Both slices are aligned index-for-index with the builder's float
/// schedules; entries are `None` when the `MarketContext` lacks the
/// corresponding curve or `FIXING:{forward_curve_id}` series.
#[derive(Clone, Copy)]
pub(crate) struct ResolvedFloatMarket<'a> {
    /// Forward curves, one per float schedule.
    /// Explicit observation cutoff; absent for curve-base projection builds.
    pub(crate) observation_date: Option<Date>,
    pub(crate) curves: &'a [Option<std::sync::Arc<ForwardCurve>>],
    /// Historical fixing series (`FIXING:{forward_curve_id}`), one per float schedule.
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
/// index fixings from `market.fixings` — the per-schedule `FIXING:{forward_curve_id}`
/// series aligned with `market.curves` (exact-date observations for both
/// overnight and term resets). Without a series, pre-base observations route
/// through the spec's [`crate::builder::FloatingRateFallback`] policy.
///
/// # Returns
///
/// `pik_to_add` — the total PIK coupon amount (across every floating schedule
/// processed on date `d`) that the caller must capitalize into the outstanding
/// balance for subsequent periods. Cash flows are pushed into `out_flows` as a
/// side effect; the return value is exclusively the PIK leg.
pub(crate) struct FloatEmissionOutput<'a> {
    pub flows: &'a mut Vec<CashFlow>,
    pub projected_fixings: &'a mut Vec<crate::fixings::ProjectedFixing>,
}

#[derive(Clone, Copy)]
struct FloatingAccrualSegment {
    start: Date,
    end: Date,
    notional: Decimal,
    year_fraction: f64,
}

fn floating_accrual_context(
    schedule: &FloatSchedule,
    period: &SchedulePeriod,
    end: Date,
) -> finstack_quant_core::dates::DayCountContext<'static> {
    let params = &schedule.spec.schedule;
    crate::builder::periods::period_day_count_context(
        period,
        schedule.calendar,
        params.frequency,
        params.day_count,
        params.stub,
        params.end_of_month,
        params.roll_rule,
        schedule.terminal_accrual_end == Some(end),
    )
}

fn floating_year_fraction(
    schedule: &FloatSchedule,
    period: &SchedulePeriod,
    start: Date,
    end: Date,
) -> finstack_quant_core::Result<f64> {
    crate::builder::periods::contractual_accrual(
        schedule.spec.schedule.day_count,
        (period.accrual_start, period.accrual_end),
        start,
        end,
        floating_accrual_context(schedule, period, end),
    )
}

/// Project each segment's raw and constrained index rates with one period replay.
fn project_segment_indices(
    schedule: &FloatSchedule,
    period: &SchedulePeriod,
    segments: &[FloatingAccrualSegment],
    observation: &FloatingRateObservation,
    resolved: (Option<&ForwardCurve>, Option<&ScalarTimeSeries>),
    observation_date: Option<Date>,
    projected_fixings: &mut Vec<crate::fixings::ProjectedFixing>,
) -> finstack_quant_core::Result<Vec<(f64, f64)>> {
    let (curve, fixings) = resolved;
    let forward_curve_id = schedule.spec.rate_spec.forward_curve_id.as_str();
    let series_id = fixing_series_id(forward_curve_id);
    let curve = curve.ok_or_else(|| {
        finstack_quant_core::Error::Input(InputError::NotFound {
            id: format!(
                "forward curve '{forward_curve_id}' not found for coupon ending {}",
                period.accrual_end
            ),
        })
    })?;
    let as_of = observation_date.unwrap_or(curve.base_date());
    match observation {
        FloatingRateObservation::Term { reset_date, .. } => {
            let same_day_fixing_exists = *reset_date == as_of
                && fixings.is_some_and(|series| series.value_on_exact(*reset_date).is_ok());
            let index_rate = if *reset_date < as_of || same_day_fixing_exists {
                require_fixing_value_exact(fixings, forward_curve_id, *reset_date, as_of)?
            } else {
                project_index_rate(
                    *reset_date,
                    curve,
                    period.accrual_start,
                    period.accrual_end,
                    floating_year_fraction(
                        schedule,
                        period,
                        period.accrual_start,
                        period.accrual_end,
                    )?,
                )?
            };
            projected_fixings.push(crate::fixings::ProjectedFixing {
                series_id,
                date: *reset_date,
                value: Some(index_rate),
            });
            Ok(vec![(index_rate, index_rate); segments.len()])
        }
        FloatingRateObservation::Overnight {
            schedule: observations,
            day_count_basis,
            constraints,
        } => {
            let mut accumulator = observations.accumulator(*day_count_basis, *constraints)?;
            let mut raw_fixings = finstack_quant_core::HashMap::default();
            for slice in observations.observations() {
                if raw_fixings.contains_key(&slice.observation_date) {
                    continue;
                }
                // Carry weights do not change a published one-day index fixing.
                let rate = observed_overnight_rate(
                    slice.observation_date,
                    *day_count_basis,
                    curve,
                    fixings,
                    forward_curve_id,
                    as_of,
                )?;
                raw_fixings.insert(slice.observation_date, rate);
                projected_fixings.push(crate::fixings::ProjectedFixing {
                    series_id: series_id.clone(),
                    date: slice.observation_date,
                    value: Some(rate),
                });
            }
            let mut rates = Vec::with_capacity(segments.len());
            let mut previous_constrained = 0.0;
            let mut previous_projected = 0.0;
            for segment in segments {
                let replay = observations.advance(&mut accumulator, segment.end, |slice| {
                    raw_fixings
                        .get(&slice.observation_date)
                        .copied()
                        .ok_or_else(|| {
                            finstack_quant_core::Error::Validation(
                                "compiled overnight observation is missing its raw fixing".into(),
                            )
                        })
                })?;
                let elapsed =
                    floating_year_fraction(schedule, period, period.accrual_start, segment.end)?;
                let constrained = replay.constrained_rate * elapsed;
                let projected = replay.projected_rate * elapsed;
                rates.push(if segment.year_fraction == 0.0 {
                    (0.0, 0.0)
                } else {
                    (
                        (constrained - previous_constrained) / segment.year_fraction,
                        (projected - previous_projected) / segment.year_fraction,
                    )
                });
                previous_constrained = constrained;
                previous_projected = projected;
            }

            let final_index_rate = accumulator.result()?.constrained_rate;
            let params = &schedule.rate_params;
            let mut affine_params = params.clone();
            affine_params.index_floor_bp = None;
            affine_params.index_cap_bp = None;
            affine_params.all_in_floor_bp = None;
            affine_params.all_in_cap_bp = None;
            let mut final_params = params.clone();
            if constraints.application == crate::builder::OvernightIndexConstraintApplication::Daily
            {
                // Daily index bounds have already been applied before compounding.
                final_params.index_floor_bp = None;
                final_params.index_cap_bp = None;
            }
            // Period bounds apply once to the completed coupon. Allocate their
            // final rate adjustment over contractual accrual time, preserving
            // the original daily compounded increments as principal changes.
            let adjustment = (calculate_floating_rate(final_index_rate, &final_params)
                - calculate_floating_rate(final_index_rate, &affine_params))
                / params.gearing;
            for (constrained, _) in &mut rates {
                *constrained += adjustment;
            }
            Ok(rates)
        }
    }
}

pub(crate) fn emit_float_coupons_on(
    d: Date,
    float_schedules: &[FloatSchedule],
    outstanding_history: &[(Date, Decimal)],
    outstanding_fallback: Decimal,
    ccy: Currency,
    market: ResolvedFloatMarket<'_>,
    output: FloatEmissionOutput<'_>,
) -> finstack_quant_core::Result<f64> {
    let FloatEmissionOutput {
        flows: out_flows,
        projected_fixings,
    } = output;
    let mut pik_to_add = 0.0;
    for ((schedule, resolved_curve), resolved_fixing) in float_schedules
        .iter()
        .zip(market.curves)
        .zip(market.fixings)
    {
        let spec = &schedule.spec;
        let first = schedule
            .periods
            .partition_point(|entry| entry.period.accrual_end < d);
        for entry in schedule.periods[first..]
            .iter()
            .take_while(|entry| entry.period.accrual_end == d)
        {
            let period = &entry.period;
            let segments = super::balances::balance_segments(
                outstanding_history,
                period.accrual_start,
                period.accrual_end,
                outstanding_fallback,
            )
            .into_iter()
            .map(|(start, end, notional)| {
                let year_fraction = floating_year_fraction(schedule, period, start, end)?;
                if !year_fraction.is_finite() || year_fraction < 0.0 {
                    return Err(finstack_quant_core::Error::Validation(
                        "floating coupon segment accrual factor must be non-negative and finite"
                            .into(),
                    ));
                }
                Ok(FloatingAccrualSegment {
                    start,
                    end,
                    notional,
                    year_fraction,
                })
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;
            let reset_date = compute_reset_date(
                period.accrual_start,
                spec.rate_spec.reset_lag_days,
                spec.schedule.business_day_convention,
                schedule.fixing_calendar,
            )?;
            let params = &schedule.rate_params;
            let observation = if let Some(method) = spec
                .rate_spec
                .compounding
                .filter(FloatingLegCompounding::is_overnight)
            {
                let basis = spec
                    .rate_spec
                    .overnight_basis
                    .unwrap_or(spec.schedule.day_count);
                let day_count_basis = overnight_day_count_basis(basis)?;
                let observations = OvernightObservationSchedule::compile(
                    period.accrual_start,
                    period.accrual_end,
                    method,
                    schedule.fixing_calendar,
                )?;
                let mut constraints = OvernightRateConstraints {
                    application: spec.rate_spec.overnight_index_constraints,
                    index_floor_bp: params.index_floor_bp,
                    index_cap_bp: params.index_cap_bp,
                };
                if constraints.application
                    == crate::builder::OvernightIndexConstraintApplication::Period
                {
                    // Preserve unbounded prefixes; final period bounds are applied once below.
                    constraints.index_floor_bp = None;
                    constraints.index_cap_bp = None;
                }
                FloatingRateObservation::Overnight {
                    schedule: observations,
                    day_count_basis,
                    constraints,
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
            let series_id = fixing_series_id(spec.rate_spec.forward_curve_id.as_str());
            match &observation {
                FloatingRateObservation::Term { reset_date, .. } => {
                    projected_fixings.push(crate::fixings::ProjectedFixing {
                        series_id,
                        date: *reset_date,
                        value: None,
                    })
                }
                FloatingRateObservation::Overnight { schedule, .. } => {
                    projected_fixings.extend(schedule.observations().iter().map(|slice| {
                        crate::fixings::ProjectedFixing {
                            series_id: series_id.clone(),
                            date: slice.observation_date,
                            value: None,
                        }
                    }))
                }
            }
            let mut settlement_params = params.clone();
            let indices = match project_segment_indices(
                schedule,
                period,
                &segments,
                &observation,
                (resolved_curve.as_deref(), resolved_fixing.as_ref()),
                market.observation_date,
                projected_fixings,
            ) {
                Ok(indices) => {
                    if matches!(observation, FloatingRateObservation::Overnight { .. }) {
                        settlement_params.index_floor_bp = None;
                        settlement_params.index_cap_bp = None;
                        settlement_params.all_in_floor_bp = None;
                        settlement_params.all_in_cap_bp = None;
                    }
                    indices
                }
                Err(error) => {
                    require_supplied_historical_fixings(
                        &observation,
                        market.observation_date,
                        resolved_curve.as_deref(),
                        resolved_fixing.as_ref(),
                        spec.rate_spec.forward_curve_id.as_str(),
                    )?;
                    let index_rate = resolve_floating_rate_fallback(
                        error,
                        reset_date,
                        params.spread_bp,
                        &spec.rate_spec.fallback,
                    )?;
                    let constrained_index =
                        apply_bp_bounds(index_rate, params.index_floor_bp, params.index_cap_bp);
                    vec![(constrained_index, index_rate); segments.len()]
                }
            };
            let (cash_fraction, pik_fraction) = spec.coupon_type.split_parts()?;
            let cash_fraction = cash_fraction.as_f64();
            let compiled = CompiledFloatingCoupon::compile(
                FloatingCouponPeriod {
                    accrual_start: period.accrual_start,
                    accrual_end: period.accrual_end,
                    payment_date: period.payment_date,
                    day_count: spec.schedule.day_count,
                    day_count_context: floating_accrual_context(
                        schedule,
                        period,
                        period.accrual_end,
                    ),
                    accrual_factor: floating_year_fraction(
                        schedule,
                        period,
                        period.accrual_start,
                        period.accrual_end,
                    )?,
                },
                FloatingCouponEconomics {
                    cash_fraction,
                    pik_fraction: pik_fraction.as_f64(),
                    rate_params: settlement_params,
                },
                observation,
            )?;
            let settlements = segments
                .iter()
                .zip(indices)
                .map(|(segment, (index_rate, projected_index_rate))| {
                    compiled.settle_index_rate(
                        segment.notional.as_f64(),
                        index_rate,
                        projected_index_rate,
                        segment.year_fraction,
                    )
                })
                .collect::<finstack_quant_core::Result<Vec<_>>>()?;
            let period_pik: f64 = settlements
                .iter()
                .map(|settlement| settlement.pik_amount)
                .sum();
            if !period_pik.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "PIK coupon on {} must capitalize a finite net amount, got {period_pik}",
                    period.accrual_end,
                )));
            }
            if period_pik < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "negative PIK coupon net amount {period_pik} on {}: de-capitalizing PIK is not \
                     supported; floor the all-in rate at zero or use a cash coupon",
                    period.accrual_end,
                )));
            }
            for (segment, settlement) in segments.iter().zip(settlements) {
                let accrual = CashFlowAccrual {
                    coupon_period: floating_accrual_context(schedule, period, segment.end)
                        .coupon_period,
                    end_is_termination_date: schedule.terminal_accrual_end == Some(segment.end),
                    calendar_id: Some(spec.schedule.calendar_id.to_string()),
                    start: segment.start,
                    end: segment.end,
                    day_count: spec.schedule.day_count,
                    projected_index_rate: Some(settlement.projected_index_rate),
                };
                if cash_fraction > 0.0 {
                    out_flows.push(
                        CashFlow::new(
                            period.payment_date,
                            Some(reset_date),
                            Money::new(settlement.cash_amount, ccy)?,
                            CFKind::FloatReset,
                            segment.year_fraction,
                            Some(settlement.all_in_rate),
                        )
                        .with_accrual(accrual.clone()),
                    );
                }
                if settlement.pik_amount != 0.0 {
                    // Signed rows allocate one non-negative contractual PIK
                    // coupon. Every row capitalizes on the same final boundary.
                    out_flows.push(
                        CashFlow::new(
                            period.payment_date,
                            None,
                            Money::new(settlement.pik_amount, ccy)?,
                            CFKind::Pik,
                            segment.year_fraction,
                            Some(settlement.all_in_rate),
                        )
                        .with_principal_date(period.accrual_end)
                        .with_accrual(accrual),
                    );
                }
            }
            pik_to_add += period_pik;
        }
    }
    Ok(pik_to_add)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::FloatingLegCompounding;
    use finstack_quant_core::dates::{calendar_by_id_strict, DayCount};
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
    fn overnight_projection_converts_curve_basis_but_preserves_observed_fixings() {
        let past = Date::from_calendar_date(2025, Month::January, 3).expect("past fixing");
        let base = Date::from_calendar_date(2025, Month::January, 6).expect("curve base");
        let future = Date::from_calendar_date(2025, Month::January, 7).expect("future fixing");
        let curve = ForwardCurve::builder("TEST-ON", 1.0 / 365.0)
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 0.04), (1.0, 0.04)])
            .build()
            .expect("curve");
        let fixings =
            ScalarTimeSeries::new("FIXING:TEST-ON", vec![(past, 0.041), (base, 0.042)], None)
                .expect("observed ACT/360 fixings");
        for (date, expected) in [(past, 0.041), (base, 0.042)] {
            let actual = observed_overnight_rate(
                date,
                360.0,
                &curve,
                Some(&fixings),
                "TEST-ON",
                curve.base_date(),
            )
            .expect("observed fixing");
            assert!((actual - expected).abs() < 1e-14);
        }
        let projected = observed_overnight_rate(
            future,
            360.0,
            &curve,
            Some(&fixings),
            "TEST-ON",
            curve.base_date(),
        )
        .expect("projected ACT/360 fixing");
        assert!((projected - 0.04 * 360.0 / 365.0).abs() < 1e-14);
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
        let calendar = calendar_by_id_strict("weekends_only").expect("calendar registered");

        let observations = OvernightObservationSchedule::compile(
            base,
            end,
            FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
            calendar,
        )
        .expect("observation schedule");
        let err = observations
            .replay(end, 360.0, OvernightRateConstraints::default(), |slice| {
                observed_overnight_rate(
                    slice.observation_date,
                    360.0,
                    &curve,
                    None,
                    "TEST-ON",
                    curve.base_date(),
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
        let calendar = calendar_by_id_strict("weekends_only").expect("calendar registered");

        let observations = OvernightObservationSchedule::compile(
            start,
            end,
            FloatingLegCompounding::CompoundedInArrears { lookback_days: 1 },
            calendar,
        )
        .expect("observation schedule");
        let err = observations
            .replay(end, 360.0, OvernightRateConstraints::default(), |slice| {
                observed_overnight_rate(
                    slice.observation_date,
                    360.0,
                    &curve,
                    None,
                    "TEST-ON",
                    curve.base_date(),
                )
            })
            .expect_err("Act/Act ISMA requires frequency context");

        assert!(
            err.to_string().contains("frequency") || err.to_string().contains("Invalid"),
            "unexpected error: {err}"
        );
    }
}
