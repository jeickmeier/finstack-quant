//! Single-period execution for schedule-driven instrument collateral.

use super::super::exercise::RateView;
use super::super::pool_flows::PoolFlows;
use super::super::reserve::{fund_collateral_draws, replenish_reserve_from_repayments};
use super::super::state::SimulationState;
use super::schedule::PreparedInstrumentSchedules;
use super::types::{CollateralKind, NameState, PeriodBucket};
use finstack_quant_core::Result;
use finstack_quant_core::dates::{Date, DateExt, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Period default probability and recovery of one instrument from its hazard
/// curve: the probability of defaulting in `(prev_date, pay_date]`
/// conditional on surviving to `prev_date`.
///
/// # Arguments
///
/// * `context` - Market context holding the hazard curve.
/// * `curve_id` - Hazard curve identifier.
/// * `prev_date` - Period start (exclusive).
/// * `pay_date` - Period payment date (inclusive).
/// * `recovery_override` - Instrument recovery that replaces the curve's.
pub(in super::super) fn hazard_period_default(
    context: &MarketContext,
    curve_id: &str,
    prev_date: Date,
    pay_date: Date,
    recovery_override: Option<f64>,
) -> Result<(f64, f64)> {
    let hazard = context.get_hazard(curve_id)?;
    let start = prev_date.max(hazard.base_date());
    let sp = hazard.survival_at_dates(&[start, pay_date.max(start)])?;
    let pd = if sp[0] > 0.0 {
        (1.0 - sp[1] / sp[0]).clamp(0.0, 1.0)
    } else {
        1.0
    };
    Ok((pd, recovery_override.unwrap_or(hazard.recovery_rate())))
}

/// Per-name inputs for one legal period.
#[derive(Debug, Clone, Copy)]
pub(in super::super) struct NamePeriod {
    /// Period default probability (a realized default is `1.0`).
    pub(in super::super) pd: f64,
    /// Recovery fraction applied to this period's default.
    pub(in super::super) recovery_rate: f64,
    /// End-of-period utilization for revolvers with simulated draws; `None`
    /// keeps the bucketed draw schedule.
    pub(in super::super) utilization: Option<f64>,
    /// Interest added on the counterfactual path where earlier draws accrue
    /// at their fair spread instead of the contractual margin (signed, in
    /// pool currency, before the default haircut).
    pub(in super::super) interest_addon: f64,
}

/// One legal period's inputs shared by every instrument.
pub(in super::super) struct PeriodModel<'a> {
    /// Index of the legal period being simulated.
    pub(in super::super) k: usize,
    /// Period start (exclusive).
    pub(in super::super) prev_date: Date,
    /// Period payment date (inclusive).
    pub(in super::super) pay_date: Date,
    /// Behavioural prepayment rate for the period, applied to bonds and loans.
    pub(in super::super) period_smm: f64,
    /// Refinancing-rate view for the incentive exercise rule.
    pub(in super::super) rate_view: RateView<'a>,
    /// Per-name inputs, in pool order.
    pub(in super::super) names: &'a [NamePeriod],
}

/// A funded draw's destination.
enum DrawUse {
    /// Advanced to the borrower: the instrument's balance rises.
    Balance,
    /// Drawn at default and written off, recovering at the given rate.
    LossAtDefault { recovery_rate: f64 },
}

struct DrawRequest {
    index: usize,
    amount: f64,
    destination: DrawUse,
}

/// Simulate one legal period of instrument collateral.
///
/// Applies, per instrument, the period default and recovery, the coupon and
/// fees (re-projected on the rate shift and, for simulated revolvers, on the
/// simulated balance), scheduled or simulated principal, behavioural
/// prepayment and exercise; then funds the period's draws from the reserve
/// and principal collections and replenishes the reserve from revolver
/// repayments.
///
/// # Arguments
///
/// * `prepared` - Shared bucketed schedules.
/// * `names` - Per-name running states of this run, in pool order.
/// * `state` - Simulation state whose pool rows and reserve are updated.
/// * `model` - This period's dates and per-name inputs.
pub(in super::super) fn run_period(
    prepared: &PreparedInstrumentSchedules,
    names: &mut [NameState],
    state: &mut SimulationState<'_>,
    model: PeriodModel<'_>,
) -> Result<PoolFlows> {
    let ccy = state.currency;
    let as_of = prepared.as_of;
    let schedules = &prepared.schedules;
    let k = model.k;
    let pay_date = model.pay_date;
    let prev_date = model.prev_date;

    if schedules.len() != state.pool_state.len()
        || names.len() != schedules.len()
        || model.names.len() != schedules.len()
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "instrument schedules ({}) do not align with pool rows ({})",
            schedules.len(),
            state.pool_state.len()
        )));
    }

    // The materialized rows carry closing-date balances; the schedules know
    // the outstanding at the valuation date.
    if k == 0 {
        for (i, schedule) in schedules.iter().enumerate() {
            if !state.pool_state.defaulted[i] {
                state.pool_state.balances[i] = schedule.opening_balance;
            }
        }
        state.pool_outstanding =
            Money::from_model_value(state.pool_state.balances.iter().sum(), ccy)?;
    }

    let empty = PeriodBucket::default();
    let shift = state.floating_rate_shift;
    let mut flows = PoolFlows::zero(ccy);
    let mut total_interest = 0.0;
    let mut total_scheduled = 0.0;
    let mut total_prepay = 0.0;
    let mut total_default = 0.0;
    let mut total_recovery = 0.0;
    let mut total_premium = 0.0;
    let mut requests: Vec<DrawRequest> = Vec::new();
    let mut revolver_repayments = 0.0;

    for name in names.iter_mut() {
        name.period_draw = 0.0;
    }
    for (i, schedule) in schedules.iter().enumerate() {
        let name = &mut names[i];
        if name.retired || state.pool_state.defaulted[i] {
            continue;
        }
        let input = model.names[i];
        let bucket = schedule.periods.get(k).unwrap_or(&empty);
        let balance = state.pool_state.balances[i].max(0.0);
        let simulated = schedule.revolver.as_ref().zip(input.utilization);
        if balance <= 0.0 && bucket.draw <= 0.0 && bucket.pik <= 0.0 && simulated.is_none() {
            continue;
        }

        let pd = input.pd.clamp(0.0, 1.0);
        let recovery_rate = input.recovery_rate.clamp(0.0, 1.0);
        let scale_bop = name.scale;
        // Interest and fees on the pre-default position, haircut for the
        // defaulting fraction accruing half the period on average.
        let haircut = 1.0 - 0.5 * pd;
        let params = schedule.floating_params.as_ref();
        let carry = match simulated {
            Some((terms, _)) => {
                let commitment = terms.commitment_at(k) * scale_bop;
                let interest = if bucket.interest.is_empty() {
                    // The expected schedule carried no balance this period;
                    // accrue the simulated balance at the nearest recorded rate.
                    let accrual = terms.day_count.year_fraction(
                        prev_date.max(as_of),
                        pay_date,
                        DayCountContext::default(),
                    )?;
                    balance * accrual * schedule.fallback_rate(k)
                } else {
                    bucket.interest_on_balance(balance, shift, as_of, params)
                };
                interest + bucket.fees_on_balance(balance, commitment, scale_bop)
            }
            None => (bucket.interest_with_shift(shift, as_of, params) + bucket.fees()) * scale_bop,
        };
        total_interest += (carry + input.interest_addon) * haircut;

        // Loan-equivalent exposure: the defaulting fraction draws part of its
        // undrawn commitment, funded through the reserve and lost at once.
        if let Some(terms) = &schedule.revolver
            && (terms.leq > 0.0 || terms.lc_exposure_at(k) > 0.0)
            && pd > 0.0
        {
            let undrawn = (terms.commitment_at(k) * scale_bop
                - terms.lc_outstanding_at(k) * scale_bop
                - balance)
                .max(0.0);
            let leq_draw = (undrawn * terms.leq + terms.lc_exposure_at(k) * scale_bop) * pd;
            if leq_draw > 0.0 {
                requests.push(DrawRequest {
                    index: i,
                    amount: leq_draw,
                    destination: DrawUse::LossAtDefault { recovery_rate },
                });
            }
        }

        let default_amt = balance * pd;
        total_default += default_amt;
        total_recovery += default_amt * recovery_rate;
        if pd >= 1.0 - 1e-10 {
            state.pool_state.defaulted[i] = true;
            state.pool_state.balances[i] = 0.0;
            name.retired = true;
            name.scale = 0.0;
            continue;
        }

        let scale_after = scale_bop * (1.0 - pd);
        let after_default = balance - default_amt;
        let (repayment, draw_req, pik) = match simulated {
            Some((terms, utilization)) => {
                let target = utilization.clamp(0.0, 1.0) * terms.commitment_at(k) * scale_after;
                (
                    (after_default - target).max(0.0),
                    (target - after_default).max(0.0),
                    0.0,
                )
            }
            None => (
                (bucket.repayment * scale_after).min(after_default),
                bucket.draw * scale_after,
                bucket.pik * scale_after,
            ),
        };
        let mut remaining = (after_default - repayment).max(0.0);

        // Prepayment on the survivor after scheduled principal; revolvers
        // have no behavioural prepayment, utilization is their model.
        let prepay = if schedule.kind == CollateralKind::Revolver {
            0.0
        } else {
            remaining * model.period_smm
        };
        remaining -= prepay;

        // Call or put exercise on whatever remains.
        let mut redeemed = 0.0;
        if remaining > 0.0
            && let Some(redemption) =
                schedule
                    .exercise
                    .evaluate(k, prev_date, pay_date, model.rate_view)?
        {
            let par = remaining;
            let cash = par * redemption.price_pct.min(100.0) / 100.0;
            let premium = par * (redemption.price_pct - 100.0).max(0.0) / 100.0;
            total_premium += premium;
            let discount_loss = (par - cash).max(0.0);
            if discount_loss > 0.0 {
                total_default += discount_loss;
                let immediate_date =
                    pay_date.add_months(-i32::try_from(state.recovery_lag_months).unwrap_or(0))?;
                flows.workout_claims.push((
                    immediate_date,
                    Money::from((0_i64, ccy)),
                    Money::from_model_value(discount_loss, ccy)?,
                ));
            }
            redeemed = cash;
            remaining = 0.0;
            name.retired = true;
        }
        // A simulated revolver retires at its maturity period.
        if simulated.is_some() && pay_date >= schedule.maturity {
            name.retired = true;
        }

        total_scheduled += repayment;
        total_prepay += prepay + redeemed;
        if schedule.kind == CollateralKind::Revolver {
            revolver_repayments += repayment;
        }

        name.scale = if name.retired {
            0.0
        } else if schedule.kind == CollateralKind::Revolver {
            scale_after
        } else {
            scale_after * (1.0 - model.period_smm)
        };
        state.pool_state.balances[i] = remaining + pik;
        if draw_req > 0.0 && !name.retired {
            requests.push(DrawRequest {
                index: i,
                amount: draw_req,
                destination: DrawUse::Balance,
            });
        }
    }

    // Fund this period's draws: reserve first, then principal collections.
    let requested_total: f64 = requests.iter().map(|r| r.amount).sum();
    let collections = total_scheduled + total_prepay;
    let funding = fund_collateral_draws(
        state,
        Money::from_model_value(requested_total, ccy)?,
        Money::from_model_value(collections, ccy)?,
    )?;
    if funding.unfunded.amount() > 0.0 {
        tracing::warn!(
            target: "finstack_quant_valuations::structured_credit",
            pay_date = %pay_date,
            unfunded = funding.unfunded.amount(),
            "collateral draws exceeded the reserve and principal collections; the \
             shortfall is capped and recorded as unfunded"
        );
    }
    let funded = funding.from_reserve.amount() + funding.from_principal.amount();
    let ratio = if requested_total > 0.0 {
        funded / requested_total
    } else {
        0.0
    };
    for request in requests {
        let amount = request.amount * ratio;
        match request.destination {
            DrawUse::Balance => {
                state.pool_state.balances[request.index] += amount;
                names[request.index].period_draw += amount;
            }
            DrawUse::LossAtDefault { recovery_rate } => {
                total_default += amount;
                total_recovery += amount * recovery_rate;
            }
        }
    }

    // Revolver repayments replenish the reserve toward its target, bounded
    // by the principal cash still available after draw funding.
    let room = (collections - funding.from_principal.amount()).max(0.0);
    let replenished = replenish_reserve_from_repayments(
        state,
        Money::from_model_value(revolver_repayments.min(room), ccy)?,
    )?;

    flows.interest = Money::from_model_value(total_interest, ccy)?;
    flows.scheduled_principal = Money::from_model_value(total_scheduled, ccy)?;
    flows.prepayment = Money::from_model_value(total_prepay, ccy)?;
    flows.default = Money::from_model_value(total_default, ccy)?;
    flows.recovery = Money::from_model_value(total_recovery, ccy)?;
    flows.call_premium = Money::from_model_value(total_premium, ccy)?;
    flows.draw_from_principal = funding.from_principal;
    flows.reserve_replenished = replenished;
    Ok(flows)
}
