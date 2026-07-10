//! MtM-resetting cross-currency swap PV path.
//!
//! Implements the cashflow stream under the CIP no-FX-vol approximation. The constant leg
//! behaves like a vanilla fixed-notional XCCY leg (initial exchange, periodic coupons on
//! `N_C`, final exchange). The resetting leg's notional is re-marked at each accrual-period
//! start using `N_j^R = N_C / X_j^FRA` where `X_j^FRA = X_0 * P_R(T_j) / P_C(T_j)`, with
//! coupons accruing on the new notional and a rebalancing cashflow paid on the resetting
//! leg only to fund the notional change.
//!
//! The constant leg has **no** rebalancing cashflow — this matches standard MtM-XCCY market
//! convention (QuantLib's `MtMCrossCurrencyBasisSwap` is structured the same way). Under
//! CIP no-FX-vol, the constant-currency half of the FX swap that funds the rebalancing is
//! PV-fair from today's perspective, so emitting it explicitly would double-count.
//!
//! The whole PV reduces to a Neumaier-accumulated sum of reporting-currency-converted
//! discounted cashflows, requiring no additional FX surface beyond what
//! `pv_leg_in_reporting_ccy` already needs for fixed-notional XCCY.
//!
//! See `docs/superpowers/specs/2026-05-10-xccy-mtm-reset-design.md` for the spec.

use crate::cashflow::builder::periods::{build_periods, BuildPeriodsParams};
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::pricing::swap_legs::robust_relative_df;
use crate::instruments::common_impl::pricing::time::rate_period_on_dates;
use crate::instruments::rates::xccy_swap::types::{ResettingSide, XccySwap};
use finstack_quant_core::dates::Date;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

/// Build the shared accrual schedule for both legs of an MtM-resetting XCCY swap.
///
/// `XccySwap::validate` aligns the two leg schedules, so both call sites (PV and
/// cashflow-schedule builders) compute the same periods. This helper centralises
/// the field-by-field mapping from a leg into `BuildPeriodsParams` so the two
/// paths can never drift (e.g. one forgetting to disable `adjust_accrual_dates`).
fn build_xccy_mtm_periods(
    leg: &crate::instruments::rates::xccy_swap::XccySwapLeg,
) -> Result<Vec<crate::cashflow::builder::periods::SchedulePeriod>> {
    let cal_id = match leg.calendar_id.as_deref() {
        Some(id) if crate::cashflow::builder::calendar::resolve_calendar_strict(id).is_ok() => id,
        _ if leg.allow_calendar_fallback => crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID,
        _ => {
            return Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap MtM leg {} requires a resolvable calendar_id",
                leg.currency
            )))
        }
    };
    build_periods(BuildPeriodsParams {
        start: leg.start,
        end: leg.end,
        frequency: leg.frequency,
        stub: leg.stub,
        bdc: leg.bdc,
        calendar_id: cal_id,
        end_of_month: false,
        day_count: leg.day_count,
        payment_lag_days: leg.payment_lag_days,
        reset_lag_days: leg.reset_lag_days,
        adjust_accrual_dates: false,
    })
}

fn require_positive_finite(value: f64, id: &str, label: &str) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "XccySwap '{id}' requires a positive finite {label}, got {value}"
        )));
    }
    Ok(())
}

/// Canonical fixing-series ID for an MtM notional reset quoted as
/// `resetting_currency -> constant_currency`.
pub(super) fn mtm_fx_fixing_series_id(
    resetting_currency: finstack_quant_core::currency::Currency,
    constant_currency: finstack_quant_core::currency::Currency,
) -> String {
    finstack_quant_core::market_data::fixings::fixing_series_id(&format!(
        "FX-{resetting_currency}-{constant_currency}"
    ))
}

fn require_historical_fx_reset(
    context: &finstack_quant_core::market_data::context::MarketContext,
    resetting_currency: finstack_quant_core::currency::Currency,
    constant_currency: finstack_quant_core::currency::Currency,
    reset_date: Date,
    as_of: Date,
) -> Result<f64> {
    let series_id = mtm_fx_fixing_series_id(resetting_currency, constant_currency);
    let series = context.get_series(&series_id).ok();
    finstack_quant_core::market_data::fixings::require_fixing_value_exact(
        series,
        &format!("FX-{resetting_currency}-{constant_currency}"),
        reset_date,
        as_of,
    )
}

/// Compute the PV of an MtM-resetting XCCY swap in reporting currency.
///
/// Dispatched from `XccySwap::base_value` when `notional_exchange` is `MtmResetting`.
/// Assumes the swap has been validated (schedules aligned, legs in distinct currencies,
/// FX matrix reachable).
pub(crate) fn pv_mtm_reset(
    swap: &XccySwap,
    resetting_side: ResettingSide,
    context: &finstack_quant_core::market_data::context::MarketContext,
    as_of: Date,
) -> Result<Money> {
    let (constant_leg, resetting_leg) = swap.partition_legs(resetting_side)?;

    let disc_c = context.get_discount(&constant_leg.discount_curve_id)?;
    let disc_r = context.get_discount(&resetting_leg.discount_curve_id)?;
    let fwd_c = context.get_forward(&constant_leg.forward_curve_id)?;
    let fwd_r = context.get_forward(&resetting_leg.forward_curve_id)?;

    let fx = context.fx().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "XccySwap '{}': MtM-reset PV requires an FxMatrix in the MarketContext",
            swap.id
        ))
    })?;

    let n_c = constant_leg.notional.amount();
    let reporting_ccy = swap.reporting_currency;

    // FX rate (resetting -> constant) at the valuation date. The forward FX at any
    // curve-time T is derived as `spot_x_at_as_of * P_R(T) / P_C(T)` via CIP; the
    // spec's `X_0` is this value at the swap's start date, NOT necessarily spot.
    let spot_x_at_as_of = fx
        .rate(FxQuery::new(
            resetting_leg.currency,
            constant_leg.currency,
            as_of,
        ))?
        .rate;

    // Build the shared schedule (aligned per `XccySwap::validate`).
    let periods = build_xccy_mtm_periods(constant_leg)?;

    if periods.is_empty() {
        return Ok(Money::new(0.0, reporting_ccy));
    }

    let mut pv = NeumaierAccumulator::new();

    // Every amount passed here is already discounted on its own currency
    // curve, so convert the resulting PV at valuation-date spot. Forward FX
    // belongs to the equivalent route where the undiscounted cashflow is
    // converted first and then discounted on the reporting-currency curve.
    let convert = |amount: f64, from_ccy: finstack_quant_core::currency::Currency| -> Result<f64> {
        if from_ccy == reporting_ccy {
            return Ok(amount);
        }
        let rate = fx.rate(FxQuery::new(from_ccy, reporting_ccy, as_of))?.rate;
        Ok(amount * rate)
    };

    // Compute the per-period notional at T_start (the swap's start date). This is the
    // resetting-leg principal amount exchanged at initial exchange AND seeded into the
    // per-period loop. Distinct from `n_c / spot_x_at_as_of` for forward-starting swaps.
    // Uses relative DFs from `as_of` so the CIP forward FX is anchored at the same time
    // as `spot_x_at_as_of`.
    let n_r_initial = if constant_leg.start < as_of {
        let historical_fx = require_historical_fx_reset(
            context,
            resetting_leg.currency,
            constant_leg.currency,
            constant_leg.start,
            as_of,
        )?;
        require_positive_finite(
            historical_fx,
            swap.id.as_str(),
            "historical initial FX reset",
        )?;
        n_c / historical_fx
    } else {
        compute_resetting_notional(
            n_c,
            spot_x_at_as_of,
            as_of,
            constant_leg.start,
            disc_c.as_ref(),
            disc_r.as_ref(),
            &swap.id,
        )?
    };

    let fixing_id_c = finstack_quant_core::market_data::fixings::fixing_series_id(
        constant_leg.forward_curve_id.as_str(),
    );
    let fixing_id_r = finstack_quant_core::market_data::fixings::fixing_series_id(
        resetting_leg.forward_curve_id.as_str(),
    );
    let fixings_c = context.get_series(&fixing_id_c).ok();
    let fixings_r = context.get_series(&fixing_id_r).ok();

    // Initial principal exchange at start. We use `initial_principal_sign` exactly as the
    // existing fixed-notional path does (`pv_leg_in_reporting_ccy`): a `Receive` leg's
    // initial sign is -1, which yields a negative-PV cashflow (the leg "pays out" notional
    // at start). The resetting-leg notional at start is `N_0^R = N_C / X_0`.
    if constant_leg.start > as_of {
        let df_c0 = robust_relative_df(disc_c.as_ref(), as_of, constant_leg.start)?;
        let df_r0 = robust_relative_df(disc_r.as_ref(), as_of, resetting_leg.start)?;

        let cf_c = constant_leg.side.initial_principal_sign() * n_c * df_c0;
        pv.add(convert(cf_c, constant_leg.currency)?);

        let cf_r = resetting_leg.side.initial_principal_sign() * n_r_initial * df_r0;
        pv.add(convert(cf_r, resetting_leg.currency)?);
    }

    // Per-period loop. For each accrual period [T_j, T_{j+1}]:
    //   - Constant leg accrues a coupon on its fixed notional `N_C`.
    //   - Resetting leg accrues a coupon on `N_j^R = N_C / X_j^FRA`, the notional captured at
    //     the START of the period (i.e. at accrual_start = T_j).
    //   - At each interior reset T_j (j = 1..n-1), the resetting leg emits a rebalancing
    //     cashflow of `(N_j^R - N_{j-1}^R)` in its own currency. There is NO corresponding
    //     constant-leg rebalancing: under CIP-no-vol the FX swap that funds the notional
    //     change is PV-fair from today's perspective, and the constant leg's net
    //     contribution is implicit in its unchanged principal-and-coupon schedule.
    //     (Cross-check: QuantLib's MtM-XCCY example
    //     https://www.implementingquantlib.com/2023/09/cross-currency-swaps.html
    //     emits rebalancing only on the resetting leg.)
    let mut n_r_prev = n_r_initial;
    for (j, period) in periods.iter().enumerate() {
        // Notional captured at the start of THIS period (T_j) = N_j^R. Also returns
        // P_R(as_of, accrual_start) so the rebalancing block below can reuse it
        // without a second curve lookup.
        // `df_r_at_period_start` is `Some` only for interior resets (j ≥ 1),
        // where the rebalancing block below consumes it. For the initial
        // period (j = 0) there is no rebalancing, so it is `None` — an honest
        // "absent" rather than a `NaN` sentinel that could silently propagate.
        let (n_r_j, df_r_at_period_start): (f64, Option<f64>) = if j == 0 {
            (n_r_initial, None)
        } else if period.accrual_start < as_of {
            let historical_fx = require_historical_fx_reset(
                context,
                resetting_leg.currency,
                constant_leg.currency,
                period.accrual_start,
                as_of,
            )?;
            require_positive_finite(historical_fx, swap.id.as_str(), "historical FX reset")?;
            (n_c / historical_fx, None)
        } else {
            let (n, df) = compute_resetting_notional_and_df_r(
                n_c,
                spot_x_at_as_of,
                as_of,
                period.accrual_start,
                disc_c.as_ref(),
                disc_r.as_ref(),
                &swap.id,
            )?;
            (n, Some(df))
        };

        if period.payment_date <= as_of {
            n_r_prev = n_r_j;
            continue;
        }

        let df_c_pay = robust_relative_df(disc_c.as_ref(), as_of, period.payment_date)?;
        let df_c_pay =
            require_positive_df(df_c_pay, &swap.id, "constant-leg", period.payment_date)?;
        let df_r_pay = robust_relative_df(disc_r.as_ref(), as_of, period.payment_date)?;
        let df_r_pay =
            require_positive_df(df_r_pay, &swap.id, "resetting-leg", period.payment_date)?;

        // 1. Constant-leg floating coupon (notional N_C).
        // Project the index forward over the accrual interval
        // [accrual_start, accrual_end] — the index tenor. The reset_date is the
        // observation/fixing date, not the start of the projection window; using
        // it (when a fixing lag places reset_date < accrual_start) projects a
        // longer-than-index window and overstates the coupon on a steep curve.
        let fixing_date_c = period.reset_date.unwrap_or(period.accrual_start);
        let rate_c = if fixing_date_c < as_of {
            finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                fixings_c,
                constant_leg.forward_curve_id.as_str(),
                fixing_date_c,
                as_of,
            )?
        } else {
            rate_period_on_dates(fwd_c.as_ref(), period.accrual_start, period.accrual_end)?
        };
        let coupon_c = constant_leg.side.coupon_sign()
            * n_c
            * rate_c
            * period.accrual_year_fraction
            * df_c_pay;
        pv.add(convert(coupon_c, constant_leg.currency)?);

        // 2. Resetting-leg floating coupon on N_j^R (notional captured at this period's start,
        //    NOT n_r_prev which is the prior period's notional). Includes the basis spread.
        let fixing_date_r = period.reset_date.unwrap_or(period.accrual_start);
        let rate_r = if fixing_date_r < as_of {
            finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                fixings_r,
                resetting_leg.forward_curve_id.as_str(),
                fixing_date_r,
                as_of,
            )?
        } else {
            rate_period_on_dates(fwd_r.as_ref(), period.accrual_start, period.accrual_end)?
        };
        let spread_decimal =
            decimal_to_f64(resetting_leg.spread_bp, "XccySwap resetting leg spread_bp")? / 10_000.0;
        let coupon_r = resetting_leg.side.coupon_sign()
            * n_r_j
            * (rate_r + spread_decimal)
            * period.accrual_year_fraction
            * df_r_pay;
        pv.add(convert(coupon_r, resetting_leg.currency)?);

        // 3. Rebalancing on the resetting leg only, at the START of this period (T_j).
        //    Skip the very first period — no rebalancing before initial exchange.
        //    Also skip when `accrual_start <= as_of` (the reset already happened); the
        //    outer gate only checks `payment_date > as_of`, so for swaps with a positive
        //    `payment_lag_days` a past reset on a not-yet-settled coupon could otherwise
        //    fire and produce a spurious past-dated PV contribution.
        //    The resetting leg ends its old notional (`N_{j-1}^R`) and starts a fresh one
        //    (`N_j^R`). Net cashflow uses `initial_principal_sign` on the delta, which gives
        //    the correct sign for both Pay/Receive resetting sides. The constant leg has no
        //    corresponding rebalancing cashflow (see comment above).
        if j > 0 && period.accrual_start > as_of {
            // Reuse the resetting-leg DF already computed inside
            // compute_resetting_notional_and_df_r above. require_positive_df has
            // already vetted it for finiteness/positivity. For j ≥ 1 the option
            // is always `Some`; the explicit error guards against a future
            // refactor breaking that invariant (no NaN can leak through).
            let df_r_reset = df_r_at_period_start.ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "xccy MtM swap {}: missing resetting-leg discount factor for \
                     interior reset at {}",
                    swap.id, period.accrual_start
                ))
            })?;
            let delta_n_r = n_r_j - n_r_prev;
            let rebal_r = resetting_leg.side.initial_principal_sign() * delta_n_r * df_r_reset;
            pv.add(convert(rebal_r, resetting_leg.currency)?);
        }

        n_r_prev = n_r_j;
    }

    // Final principal exchanges are settled once the leg end date has passed.
    if constant_leg.end > as_of {
        let df_c_end = robust_relative_df(disc_c.as_ref(), as_of, constant_leg.end)?;
        let df_c_end = require_positive_df(df_c_end, &swap.id, "constant-leg", constant_leg.end)?;
        let df_r_end = robust_relative_df(disc_r.as_ref(), as_of, resetting_leg.end)?;
        let df_r_end = require_positive_df(df_r_end, &swap.id, "resetting-leg", resetting_leg.end)?;

        let cf_c_final = constant_leg.side.final_principal_sign() * n_c * df_c_end;
        pv.add(convert(cf_c_final, constant_leg.currency)?);

        let cf_r_final = resetting_leg.side.final_principal_sign() * n_r_prev * df_r_end;
        pv.add(convert(cf_r_final, resetting_leg.currency)?);
    }

    Ok(Money::new(pv.total(), reporting_ccy))
}

/// Enumerate the resetting leg's cashflow stream for `cashflow_schedule`.
///
/// Mirrors the per-period notional logic in [`pv_mtm_reset`] but emits each cashflow as a
/// [`CashFlow`] record in the resetting leg's native currency (no FX conversion, no
/// discounting — `cashflow_schedule` is the pre-PV reporting view). The constant leg's
/// cashflows are unchanged from the fixed-notional case and are built by the caller via
/// `leg_coupon_schedule` + `leg_principal_schedule`.
///
/// The emitted flows are, in order:
/// 1. Initial principal exchange at `T_0` with amount `sign * N_0^R` (kind `Notional`).
/// 2. For each future coupon period `[T_j, T_{j+1}]`: a floating coupon at the payment
///    date with amount `sign * N_j^R * (R + s) * τ` (kind `FloatReset`). The notional
///    `N_j^R` captures the per-period mark.
/// 3. For each interior reset `T_j` (j ≥ 1): a rebalancing flow with amount
///    `sign * (N_j^R - N_{j-1}^R)` (kind `Notional`). The constant leg has no
///    corresponding rebalancing — under CIP no-vol the constant-currency half of the
///    funding FX swap is PV-fair, so we don't double-count it.
/// 4. Final principal exchange at `T_n` with amount `sign * N_n^R` (kind `Notional`).
pub(crate) fn mtm_resetting_leg_schedule(
    swap: &XccySwap,
    resetting_side: ResettingSide,
    context: &finstack_quant_core::market_data::context::MarketContext,
    as_of: Date,
) -> Result<crate::cashflow::builder::CashFlowSchedule> {
    use crate::cashflow::builder::{CashFlowMeta, CashFlowSchedule, Notional};
    use crate::cashflow::primitives::{CFKind, CashFlow};
    use crate::instruments::common_impl::numeric::decimal_to_f64;
    use finstack_quant_core::money::fx::FxQuery;
    use finstack_quant_core::money::Money;

    let (constant_leg, resetting_leg) = swap.partition_legs(resetting_side)?;

    let disc_c = context.get_discount(&constant_leg.discount_curve_id)?;
    let disc_r = context.get_discount(&resetting_leg.discount_curve_id)?;
    let fwd_r = context.get_forward(&resetting_leg.forward_curve_id)?;

    let fx = context.fx().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "XccySwap '{}': MtM-reset cashflow_schedule requires an FxMatrix in the MarketContext",
            swap.id
        ))
    })?;

    let n_c = constant_leg.notional.amount();
    let spot_x_at_as_of = fx
        .rate(FxQuery::new(
            resetting_leg.currency,
            constant_leg.currency,
            as_of,
        ))?
        .rate;

    let periods = build_xccy_mtm_periods(constant_leg)?;

    let mut flows: Vec<CashFlow> = Vec::with_capacity(periods.len() * 2 + 2);

    // Per-period notional at T_start — also drives the initial principal cashflow.
    let n_r_initial = if constant_leg.start < as_of {
        let historical_fx = require_historical_fx_reset(
            context,
            resetting_leg.currency,
            constant_leg.currency,
            constant_leg.start,
            as_of,
        )?;
        require_positive_finite(
            historical_fx,
            swap.id.as_str(),
            "historical initial FX reset",
        )?;
        n_c / historical_fx
    } else {
        compute_resetting_notional(
            n_c,
            spot_x_at_as_of,
            as_of,
            constant_leg.start,
            disc_c.as_ref(),
            disc_r.as_ref(),
            &swap.id,
        )?
    };

    // Initial principal exchange.
    let cf_initial_amount = resetting_leg.side.initial_principal_sign() * n_r_initial;
    flows.push(CashFlow {
        date: resetting_leg.start,
        reset_date: None,
        amount: Money::new(cf_initial_amount, resetting_leg.currency),
        kind: CFKind::Notional,
        accrual_factor: 0.0,
        rate: None,
    });

    let spread_decimal =
        decimal_to_f64(resetting_leg.spread_bp, "XccySwap resetting leg spread_bp")? / 10_000.0;

    let mut n_r_prev = n_r_initial;
    for (j, period) in periods.iter().enumerate() {
        let n_r_j = if j == 0 {
            n_r_initial
        } else if period.accrual_start < as_of {
            let historical_fx = require_historical_fx_reset(
                context,
                resetting_leg.currency,
                constant_leg.currency,
                period.accrual_start,
                as_of,
            )?;
            require_positive_finite(historical_fx, swap.id.as_str(), "historical FX reset")?;
            n_c / historical_fx
        } else {
            compute_resetting_notional(
                n_c,
                spot_x_at_as_of,
                as_of,
                period.accrual_start,
                disc_c.as_ref(),
                disc_r.as_ref(),
                &swap.id,
            )?
        };

        // Coupon at payment date on the period-start notional N_j^R.
        if period.payment_date > as_of {
            let rate_r = crate::instruments::common_impl::pricing::time::rate_period_on_dates(
                fwd_r.as_ref(),
                // Project over the accrual interval (index tenor), not from the
                // observation date — see the coupon-pricing note above.
                period.accrual_start,
                period.accrual_end,
            )?;
            let coupon_amount = resetting_leg.side.coupon_sign()
                * n_r_j
                * (rate_r + spread_decimal)
                * period.accrual_year_fraction;
            flows.push(CashFlow {
                date: period.payment_date,
                reset_date: period.reset_date,
                amount: Money::new(coupon_amount, resetting_leg.currency),
                kind: CFKind::FloatReset,
                accrual_factor: period.accrual_year_fraction,
                rate: Some(rate_r + spread_decimal),
            });
        }

        // Rebalancing at the START of this period (j ≥ 1 only).
        if j > 0 && period.accrual_start > as_of {
            let delta_n_r = n_r_j - n_r_prev;
            let rebal_amount = resetting_leg.side.initial_principal_sign() * delta_n_r;
            if rebal_amount != 0.0 {
                flows.push(CashFlow {
                    date: period.accrual_start,
                    reset_date: None,
                    amount: Money::new(rebal_amount, resetting_leg.currency),
                    kind: CFKind::Notional,
                    accrual_factor: 0.0,
                    rate: None,
                });
            }
        }

        n_r_prev = n_r_j;
    }

    // Final principal exchange uses the LAST period's notional N_n^R.
    let cf_final_amount = resetting_leg.side.final_principal_sign() * n_r_prev;
    flows.push(CashFlow {
        date: resetting_leg.end,
        reset_date: None,
        amount: Money::new(cf_final_amount, resetting_leg.currency),
        kind: CFKind::Notional,
        accrual_factor: 0.0,
        rate: None,
    });

    flows.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(CashFlowSchedule {
        flows,
        notional: Notional::par(resetting_leg.notional.amount(), resetting_leg.currency),
        day_count: resetting_leg.day_count,
        meta: CashFlowMeta::default(),
    })
}

/// Per-period resetting-leg notional under CIP no-FX-vol: `N_C / X_t^FRA`.
///
/// Uses *relative* discount factors from `as_of` (via `robust_relative_df`) so the CIP
/// forward FX `X_t^FRA = spot_x_at_as_of · P_R(as_of, t) / P_C(as_of, t)` is consistent
/// with the spot rate observed at `as_of`. Using absolute DFs from each curve's base
/// date would only agree when `as_of == curve.base_date` — i.e., the same day the
/// curves were calibrated — and would silently bias every intraday revaluation.
///
/// Returns `(notional, p_r)` so the caller can reuse the resetting-leg DF at `date`
/// for the rebalancing cashflow without a second curve lookup.
fn compute_resetting_notional_and_df_r(
    n_constant: f64,
    spot_x_at_as_of: f64,
    as_of: Date,
    date: Date,
    disc_c: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    disc_r: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    swap_id: &finstack_quant_core::types::InstrumentId,
) -> Result<(f64, f64)> {
    let p_c = robust_relative_df(disc_c, as_of, date)?;
    let p_c = require_positive_df(p_c, swap_id, "constant-leg", date)?;
    let p_r = robust_relative_df(disc_r, as_of, date)?;
    let p_r = require_positive_df(p_r, swap_id, "resetting-leg", date)?;
    let x_t = spot_x_at_as_of * (p_r / p_c);
    if !x_t.is_finite() || x_t <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "XccySwap '{swap_id}': non-positive forward FX at date {date}"
        )));
    }
    Ok((n_constant / x_t, p_r))
}

/// Thin wrapper retained for call sites that only need the notional (initial principal
/// exchange, final exchange, schedule-builder pre-loop).
fn compute_resetting_notional(
    n_constant: f64,
    spot_x_at_as_of: f64,
    as_of: Date,
    date: Date,
    disc_c: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    disc_r: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    swap_id: &finstack_quant_core::types::InstrumentId,
) -> Result<f64> {
    let (n_r, _) = compute_resetting_notional_and_df_r(
        n_constant,
        spot_x_at_as_of,
        as_of,
        date,
        disc_c,
        disc_r,
        swap_id,
    )?;
    Ok(n_r)
}

/// Guard that a discount factor is finite and strictly positive; returns the value on success.
fn require_positive_df(
    df: f64,
    swap_id: &finstack_quant_core::types::InstrumentId,
    curve_role: &str,
    date: Date,
) -> Result<f64> {
    if !df.is_finite() || df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "XccySwap '{swap_id}': non-positive/non-finite {curve_role} DF at {date}: {df}"
        )));
    }
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_fx_reset_requires_exact_fixing_series_observation() {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
        use time::macros::date;

        let reset_date = date!(2025 - 01 - 02);
        let as_of = date!(2025 - 01 - 03);
        let series = ScalarTimeSeries::new(
            mtm_fx_fixing_series_id(Currency::EUR, Currency::USD),
            vec![(reset_date, 1.08)],
            None,
        )
        .expect("valid FX fixing series");
        let market = MarketContext::new().insert_series(series);

        let rate =
            require_historical_fx_reset(&market, Currency::EUR, Currency::USD, reset_date, as_of)
                .expect("exact reset fixing");
        assert_eq!(rate, 1.08);
        assert!(require_historical_fx_reset(
            &market,
            Currency::EUR,
            Currency::USD,
            date!(2025 - 01 - 01),
            as_of,
        )
        .is_err());
    }

    #[test]
    fn compute_resetting_notional_matches_formula() {
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use time::Month;

        let base = Date::from_calendar_date(2025, Month::January, 2).expect("date");
        // Flat 2% USD discount, flat 1% EUR discount, Act/365F.
        let disc_c = DiscountCurve::builder(CurveId::new("USD-OIS"))
            .base_date(base)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, (-0.02_f64 * 5.0).exp())])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .build()
            .expect("build USD curve");
        let disc_r = DiscountCurve::builder(CurveId::new("EUR-OIS"))
            .base_date(base)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, (-0.01_f64 * 5.0).exp())])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .build()
            .expect("build EUR curve");

        let spot = 1.10_f64; // USD per EUR
        let n_c = 10_000_000.0;
        // Use a date 2.5 years from base (approximately, using Act/365F).
        // 2.5 * 365.25 ≈ 913 days — round to 912 for even arithmetic.
        let date = Date::from_calendar_date(2027, Month::July, 2).expect("date");
        let swap_id = InstrumentId::new("TEST-XCCY-SWAP");

        // Reference values computed via df_on_date_curve (each curve uses its own axis).
        // When `as_of == curve.base_date`, robust_relative_df reduces to df_on_date_curve.
        let p_c = disc_c.df_on_date_curve(date).expect("p_c");
        let p_r = disc_r.df_on_date_curve(date).expect("p_r");
        let expected = n_c / (spot * p_r / p_c);

        let actual = compute_resetting_notional(n_c, spot, base, date, &disc_c, &disc_r, &swap_id)
            .expect("formula ok");
        assert!(
            (actual - expected).abs() < 1e-6,
            "got {actual}, expected {expected}"
        );
    }

    /// Regression: the resetting notional must use the CIP forward
    /// `F = S · P_R / P_C` (low-yield currency trades at a forward premium),
    /// verified against a hand-computed value rather than the implementation's
    /// own DF lookups.
    ///
    /// EUR@1% (resetting), USD@2% (constant), S = 1.10 USD per EUR:
    /// F(1y) = 1.10 · e^{-0.01} / e^{-0.02} = 1.10 · e^{0.01} ≈ 1.111055,
    /// so N_R(1y) = N_C / F(1y) ≈ 9,000,455 EUR for N_C = 10mm USD.
    #[test]
    fn resetting_notional_matches_hand_computed_cip_forward() {
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use time::Month;

        let base = Date::from_calendar_date(2025, Month::January, 2).expect("date");
        // Pin an explicit knot at t = 1.0 so the 1y DF is exactly e^{-r}
        // regardless of the linear-in-DF interpolation between knots.
        let disc_c = DiscountCurve::builder(CurveId::new("USD-OIS"))
            .base_date(base)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.02_f64).exp()),
                (5.0, (-0.02_f64 * 5.0).exp()),
            ])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .build()
            .expect("build USD curve");
        let disc_r = DiscountCurve::builder(CurveId::new("EUR-OIS"))
            .base_date(base)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.01_f64).exp()),
                (5.0, (-0.01_f64 * 5.0).exp()),
            ])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .build()
            .expect("build EUR curve");

        let spot = 1.10_f64; // USD per EUR
        let n_c = 10_000_000.0; // USD
                                // Exactly 365 days from base under Act/365F -> t = 1.0.
        let date = Date::from_calendar_date(2026, Month::January, 2).expect("date");
        let swap_id = InstrumentId::new("TEST-XCCY-SWAP");

        let forward = spot * (0.01_f64).exp(); // S · e^{(r_C − r_R)·1y}
        assert!(
            forward > spot,
            "low-yield EUR must be at a forward premium vs USD"
        );
        let expected = n_c / forward;

        let actual = compute_resetting_notional(n_c, spot, base, date, &disc_c, &disc_r, &swap_id)
            .expect("formula ok");
        assert!(
            (actual - expected).abs() < 1e-6,
            "got {actual}, expected hand-computed {expected}"
        );
    }
}
