//! The period's waterfall: the base waterfall with this period's rules
//! (accumulation lockout, shifting interest or step-down, the AFC cap,
//! target OC, net-WAC carryover, the reserve target, reinvestment holds,
//! tranche triggers, redemption) and hedge payments applied.

use super::*;
use crate::instruments::fixed_income::structured_credit::pricing::resolve;
use crate::instruments::fixed_income::structured_credit::types::SwapPriority;
use std::borrow::Cow;

/// Period state deciding which rules apply.
pub(super) struct PeriodRuleFlags<'a> {
    /// Controlled accumulation holds investor principal this period.
    pub(super) is_accumulating: bool,
    /// The deal revolves this period.
    pub(super) is_reinvestment_active: bool,
    /// Live available-funds cap rate (decimal).
    pub(super) live_afc_cap: f64,
    /// This period's reserve target, when a reserve rule is configured.
    pub(super) reserve_target: Option<f64>,
    /// Actions of the tranche triggers evaluated this period.
    pub(super) trigger_actions: &'a super::super::triggers::TriggerActions,
}

/// Period flows the rules read.
pub(super) struct PeriodRuleFlows<'a> {
    /// Scheduled pool principal.
    pub(super) scheduled_principal: Money,
    /// Pool prepayments.
    pub(super) prepayment: Money,
    /// Recoveries released this period.
    pub(super) released_recoveries: Money,
    /// Hedge payments ranked as fees this period.
    pub(super) hedge_payments: &'a [(String, Money, SwapPriority)],
}

/// Resolve the period's waterfall, copying the base waterfall once on the
/// first rule that edits it (borrowed when no rule applies this period).
///
/// # Arguments
///
/// * `state` - Simulation state (note balances, pool balances, deal age).
/// * `inputs` - The period's deal, base waterfall and dates.
/// * `flags` - Which period rules are in effect.
/// * `flows` - Period flows the shifting-interest split reads.
///
/// # Errors
///
/// Returns an error when the reserve target cannot be expressed as money.
pub(super) fn resolve_period_waterfall<'w>(
    state: &SimulationState,
    inputs: &PeriodInputs<'w>,
    flags: &PeriodRuleFlags<'_>,
    flows: &PeriodRuleFlows<'_>,
) -> Result<Cow<'w, Waterfall>> {
    let rules = inputs.instrument.waterfall_rules.as_ref();
    let pay_date = inputs.period.payment;
    // The period's waterfall: the base waterfall, copied once by the first
    // rule that edits it (no copy when no rule applies this period).
    //
    // Controlled accumulation locks out investor principal (held flat) and takes
    // precedence; otherwise shifting interest and step-down govern principal
    // allocation, with shifting interest winning when both are configured.
    // Step-down switches principal to pro-rata once the deal has seasoned past
    // the step-down date with every trigger passing.
    let mut period_waterfall = std::borrow::Cow::Borrowed(inputs.waterfall);
    if flags.is_accumulating {
        resolve::apply_accumulation_lockout(period_waterfall.to_mut(), &state.tranche_balances);
    } else if let Some(si) = rules.and_then(|r| r.shifting_interest.as_ref()) {
        let months_from_closing = state.closing_date.months_until(pay_date);
        // Senior's pro-rata share (by current balance) governs scheduled
        // principal; the schedule lock-out governs unscheduled principal.
        let senior_bal = state
            .tranche_balances
            .get(si.senior_id.as_str())
            .map_or(0.0, |m| m.amount());
        let total_debt: f64 = state
            .tranches
            .tranches
            .iter()
            .filter(|t| t.seniority != TrancheSeniority::Equity)
            .map(|t| {
                state
                    .tranche_balances
                    .get(t.id.as_str())
                    .map_or(0.0, |m| m.amount())
            })
            .sum();
        let senior_prorata_share = if total_debt > 0.0 {
            senior_bal / total_debt
        } else {
            0.0
        };
        // Unscheduled (prepayment + recovery) fraction of distributable principal.
        let scheduled = flows.scheduled_principal.amount().max(0.0);
        let unscheduled =
            flows.prepayment.amount().max(0.0) + flows.released_recoveries.amount().max(0.0);
        let unscheduled_fraction = if scheduled + unscheduled > 0.0 {
            unscheduled / (scheduled + unscheduled)
        } else {
            1.0
        };
        let metrics = step_down_metrics(state);
        resolve::apply_shifting_interest(
            period_waterfall.to_mut(),
            si,
            months_from_closing,
            senior_prorata_share,
            unscheduled_fraction,
            &state.tranche_balances,
            &metrics,
        );
    } else {
        let metrics = step_down_metrics(state);
        if resolve::step_down_in_effect(rules, pay_date, &metrics) {
            resolve::apply_step_down(period_waterfall.to_mut(), &state.tranche_balances);
        }
    }

    // Layer the available-funds cap onto the per-period waterfall using the live
    // cap rate, so the cash *routed* to capped tranches' interest matches the
    // interest *recorded* in Step 5 (both keyed on the live AFC cap).
    let afc = rules.and_then(|r| r.afc.as_ref());
    let afc_carryover = afc.is_some_and(|afc| afc.carryover);
    if let Some(afc) = afc {
        resolve::apply_afc_cap(period_waterfall.to_mut(), afc, flags.live_afc_cap);
    }

    // Targeted OC amortization caps the notes' principal at the amount that
    // holds overcollateralization at the target on the post-collection pool.
    if let Some(spec) = rules.and_then(|r| r.target_oc.as_ref()) {
        if !flags.is_accumulating {
            resolve::apply_target_oc(
                period_waterfall.to_mut(),
                spec,
                state.pool_state.balances.iter().sum::<f64>()
                    + state.principal_funding_account.amount(),
                state.original_pool_balance.amount(),
                &state.tranche_balances,
            );
        }
    }

    // Net-WAC carryover: each capped tranche's recipient asks for the
    // balance brought into the period.
    if afc_carryover {
        resolve::apply_net_wac_carryover(period_waterfall.to_mut(), &state.carryover_balance);
    }

    if let Some(target) = flags.reserve_target {
        resolve::apply_reserve_target(
            period_waterfall.to_mut(),
            Money::new(target, state.base_currency)?,
        );
    }

    if flags.is_reinvestment_active {
        let resolved = period_waterfall.to_mut();
        let amortizing: &[String] = state
            .pool
            .reinvestment_period
            .as_ref()
            .map_or(&[], |period| period.amortizing_tranches.as_slice());
        for tier in &mut resolved.tiers {
            if tier.payment_type != PaymentType::Principal {
                continue;
            }
            // Principal that cannot be placed remains in the capital account
            // during revolving periods instead of leaking to equity, so the
            // manager's principal-tier incentive share has nothing to take.
            tier.recipients.retain(|recipient| {
                recipient.recipient_type != RecipientType::Equity
                    && !matches!(
                        recipient.calculation,
                        PaymentCalculation::IncentiveFee { .. }
                    )
            });
            // Every note not listed as amortizing is held flat: its regular
            // principal target is its current balance for this period.
            for recipient in &mut tier.recipients {
                if let PaymentCalculation::TranchePrincipal {
                    tranche_id,
                    target_balance,
                    ..
                } = &mut recipient.calculation
                {
                    if !amortizing.iter().any(|id| id == tranche_id) {
                        *target_balance = state.tranche_balances.get(tranche_id).copied();
                    }
                }
            }
        }
    }

    if !state.tranche_triggers.is_empty() {
        flags.trigger_actions.apply(period_waterfall.to_mut());
    }

    // On a redemption date the equity residual is withheld: it carries as
    // undistributed interest into the redemption proceeds, which the
    // orchestration distributes after the notes are redeemed.
    if inputs.period.redemption {
        period_waterfall
            .to_mut()
            .tiers
            .retain(|tier| tier.payment_type != PaymentType::Residual);
    }

    // Hedge payments rank last: their junior-fee position is placed after
    // the tiers the rules above insert or remove.
    if !flows.hedge_payments.is_empty() {
        insert_hedge_payments(period_waterfall.to_mut(), flows.hedge_payments);
    }
    Ok(period_waterfall)
}

/// Rank this period's hedge payments in `waterfall` as fixed fees to the swap
/// counterparty at each payment's configured priority.
pub(super) fn insert_hedge_payments(
    waterfall: &mut crate::instruments::fixed_income::structured_credit::types::Waterfall,
    payments: &[(String, Money, SwapPriority)],
) {
    for (id, amount, priority) in payments {
        waterfall.insert_hedge_payment(
            crate::instruments::fixed_income::structured_credit::types::Recipient::fixed_fee(
                id.clone(),
                "SwapCounterparty",
                *amount,
            ),
            *priority,
        );
    }
}
