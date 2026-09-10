//! Tranche coverage state and executable period consequences.

use super::*;
use crate::instruments::fixed_income::structured_credit::pricing::waterfall::{
    evaluate_coverage_tests, senior_fee_accrual, SeniorFeeInputs,
};
use crate::instruments::fixed_income::structured_credit::types::{
    waterfall::CoverageTrigger as WaterfallTrigger, CoverageTrigger, PaymentCalculation,
    PaymentType, TriggerConsequence,
};

pub(super) struct TrancheTriggerState {
    tranche_id: String,
    oc: bool,
    pub(super) trigger: CoverageTrigger,
}

pub(super) fn initial_states(tranches: &TrancheStructure) -> Vec<TrancheTriggerState> {
    tranches
        .tranches
        .iter()
        .flat_map(|tranche| {
            [
                (true, tranche.oc_trigger.clone()),
                (false, tranche.ic_trigger.clone()),
            ]
            .into_iter()
            .filter_map(|(oc, trigger)| {
                trigger.map(|trigger| TrancheTriggerState {
                    tranche_id: tranche.id.to_string(),
                    oc,
                    trigger,
                })
            })
        })
        .collect()
}

#[derive(Default)]
pub(super) struct TriggerActions {
    pub(super) stop_reinvestment: bool,
    pub(super) accelerate: bool,
    trap: bool,
    diversions: Vec<WaterfallTrigger>,
}

impl TriggerActions {
    pub(super) fn apply(&self, waterfall: &mut Waterfall) {
        waterfall.coverage_triggers.extend(self.diversions.clone());
        if self.accelerate {
            let mut principal = waterfall
                .tiers
                .iter()
                .filter(|tier| tier.payment_type == PaymentType::Principal)
                .min_by_key(|tier| tier.priority)
                .map_or_else(Vec::new, |tier| {
                    tier.recipients
                        .iter()
                        .filter(|recipient| {
                            matches!(recipient.recipient_type, RecipientType::Tranche(_))
                        })
                        .cloned()
                        .collect::<Vec<_>>()
                });
            for recipient in &mut principal {
                if let PaymentCalculation::TranchePrincipal { target_balance, .. } =
                    &mut recipient.calculation
                {
                    *target_balance = None;
                }
            }
            for tier in &mut waterfall.tiers {
                for recipient in &mut tier.recipients {
                    if let PaymentCalculation::TranchePrincipal { target_balance, .. } =
                        &mut recipient.calculation
                    {
                        *target_balance = None;
                    }
                }
                if tier.payment_type == PaymentType::Residual {
                    tier.recipients.clone_from(&principal);
                    tier.divertible = false;
                }
            }
        } else if self.trap {
            // The executor retains these interest proceeds in its interest
            // account. They become distributable when the trigger cures.
            waterfall
                .tiers
                .retain(|tier| tier.payment_type != PaymentType::Residual);
        }
    }
}

pub(super) fn advance(
    state: &mut SimulationState,
    waterfall: &Waterfall,
    period: SimulationPeriod,
    period_start: Date,
    interest: Money,
    principal: Money,
    market: &MarketContext,
) -> Result<TriggerActions> {
    if state.tranche_triggers.is_empty() {
        return Ok(TriggerActions::default());
    }
    let mut tests = waterfall.clone();
    tests.coverage_triggers.clear();
    for saved in &state.tranche_triggers {
        let trigger = &saved.trigger;
        if !trigger.trigger_level.is_finite()
            || trigger.trigger_level <= 0.0
            || trigger
                .cure_level
                .is_some_and(|level| !level.is_finite() || level < trigger.trigger_level)
            || trigger
                .breach_date
                .is_some_and(|date| date > period.payment)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "invalid coverage trigger for {}",
                saved.tranche_id
            )));
        }
        let level = if trigger.breach_date.is_some() {
            trigger.cure_level.unwrap_or(trigger.trigger_level)
        } else {
            trigger.trigger_level
        };
        tests.coverage_triggers.push(WaterfallTrigger {
            tranche_id: saved.tranche_id.clone(),
            oc_trigger: saved.oc.then_some(level),
            ic_trigger: (!saved.oc).then_some(level),
        });
    }
    let tranche_index = state
        .tranches
        .tranches
        .iter()
        .enumerate()
        .map(|(i, tranche)| (tranche.id.as_str(), i))
        .collect();
    let pool_balance = Money::new(state.pool_state.balances.iter().sum(), state.base_currency)?;
    let fees = senior_fee_accrual(
        &tests,
        state.tranches,
        &tranche_index,
        SeniorFeeInputs {
            available: interest,
            tranche_balances: Some(&state.tranche_balances),
            deferred_interest: Some(&state.deferred_interest),
            pool_balance,
            period_start,
            payment_date: period.payment,
            valuation_date: period.valuation,
            market,
            reserve_balance: state.reserve_balance,
            floating_rate_shift: state.floating_rate_shift,
        },
    )?;
    let payable: Vec<_> = waterfall
        .tiers
        .iter()
        .filter(|tier| tier.payment_type == PaymentType::Principal)
        .min_by_key(|tier| tier.priority)
        .into_iter()
        .flat_map(|tier| &tier.recipients)
        .filter_map(|recipient| match &recipient.recipient_type {
            RecipientType::Tranche(id) => Some(id.as_str()),
            _ => None,
        })
        .collect();
    let results = evaluate_coverage_tests(
        &tests,
        state.tranches,
        state.pool,
        period.payment,
        period_start,
        period.valuation,
        principal,
        interest,
        pool_balance,
        market,
        Some(&state.tranche_balances),
        Some(&state.pool_state.balances),
        &payable,
        fees,
        state.principal_funding_account,
        state.floating_rate_shift,
        Some(&state.deferred_interest),
    )?;
    let mut actions = TriggerActions::default();
    for (saved, test) in state
        .tranche_triggers
        .iter_mut()
        .zip(&tests.coverage_triggers)
    {
        let id = format!(
            "{}_{}",
            if saved.oc { "OC" } else { "IC" },
            saved.tranche_id
        );
        let result = results
            .iter()
            .find(|result| result.test_id == id)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!("missing trigger result {id}"))
            })?;
        if saved.trigger.update(result.current_ratio, period.payment) {
            match saved.trigger.consequence {
                TriggerConsequence::DivertCashFlow => actions.diversions.push(test.clone()),
                TriggerConsequence::TrapExcessSpread => actions.trap = true,
                TriggerConsequence::AccelerateAmortization => {
                    actions.accelerate = true;
                    actions.stop_reinvestment = true;
                }
                TriggerConsequence::StopReinvestment => actions.stop_reinvestment = true,
            }
        }
    }
    Ok(actions)
}
