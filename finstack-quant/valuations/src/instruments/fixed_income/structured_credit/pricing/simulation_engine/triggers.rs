//! Tranche coverage state and executable period consequences.
//!
//! Two kinds of coverage test coexist:
//!
//! * the waterfall's own [`CoverageTestSpec`] tiers, stateless positions
//!   evaluated by the executor every period; and
//! * per-tranche [`CoverageTrigger`]s (`Tranche::oc_trigger` /
//!   `Tranche::ic_trigger`), which carry breach/cure memory and a
//!   [`TriggerConsequence`]. A `DivertCashFlow` consequence is realized by
//!   inserting a coverage-test tier after the tranche's interest tier for the
//!   period, so it diverts positionally like every other test.
//!
//! Both feed the CLO reinvestment rule: while any coverage test fails,
//! principal proceeds are not reinvested but repay the notes.

use super::*;
use crate::instruments::fixed_income::structured_credit::pricing::coverage_tests::TestContext;
use crate::instruments::fixed_income::structured_credit::pricing::waterfall::{
    coverage_inputs, evaluate_coverage_tests, senior_fee_accrual, special_serviced_balance,
    SeniorFeeInputs,
};
use crate::instruments::fixed_income::structured_credit::types::{
    CoverageTestAction, CoverageTestSpec, CoverageTrigger, PaymentCalculation, PaymentType,
    TriggerConsequence,
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
    /// Suspend collateral purchases this period (a `StopReinvestment` or
    /// `AccelerateAmortization` consequence, or any failing coverage test).
    pub(super) stop_reinvestment: bool,
    pub(super) accelerate: bool,
    trap: bool,
    /// Coverage tests to place after their tranche's interest tier for this
    /// period (breached `DivertCashFlow` triggers).
    diversions: Vec<CoverageTestSpec>,
}

impl TriggerActions {
    pub(super) fn apply(&self, waterfall: &mut Waterfall) {
        for test in &self.diversions {
            waterfall.insert_coverage_test(test.clone());
        }
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

/// Period cash the coverage tests are evaluated against.
#[derive(Clone, Copy)]
pub(super) struct TriggerCashInputs {
    /// Interest proceeds available this period (pool interest plus carry).
    pub(super) interest: Money,
    /// Principal proceeds available this period (collections, released
    /// recoveries and carry).
    pub(super) principal: Money,
    /// Recovery value of defaulted collateral not yet received as cash,
    /// counted in the OC numerator.
    pub(super) defaulted_collateral_value: Money,
}

/// Advance the per-tranche breach/cure states and evaluate the waterfall's
/// own coverage tests for this period's reinvestment rule.
pub(super) fn advance(
    state: &mut SimulationState,
    waterfall: &Waterfall,
    period: SimulationPeriod,
    period_start: Date,
    cash: TriggerCashInputs,
    market: &MarketContext,
) -> Result<TriggerActions> {
    let TriggerCashInputs {
        interest,
        principal,
        defaulted_collateral_value,
    } = cash;

    // Tranche-level triggers, at their trigger or cure level depending on
    // the saved breach state.
    let mut specs: Vec<CoverageTestSpec> = Vec::with_capacity(state.tranche_triggers.len());
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
        let mut spec = if saved.oc {
            CoverageTestSpec::oc(saved.tranche_id.clone(), level)
        } else {
            CoverageTestSpec::ic(saved.tranche_id.clone(), level)
        };
        spec.id = format!("{}_trigger", spec.id);
        specs.push(spec);
    }
    let waterfall_specs: Vec<CoverageTestSpec> = waterfall.coverage_tests().cloned().collect();
    if specs.is_empty() && waterfall_specs.is_empty() {
        return Ok(TriggerActions::default());
    }

    let tranche_index = state
        .tranches
        .tranches
        .iter()
        .enumerate()
        .map(|(i, tranche)| (tranche.id.as_str(), i))
        .collect();
    let pool_balance = Money::new(state.pool_state.balances.iter().sum(), state.base_currency)?;
    let unresolved_npl = state.unresolved_npl();
    let special_serviced_balance = special_serviced_balance(
        &state.pool,
        Some(&state.pool_state.balances),
        Some(&state.pool_state.special_serviced),
        state.base_currency,
    )?;
    let fees = senior_fee_accrual(
        waterfall,
        state.tranches,
        &tranche_index,
        SeniorFeeInputs {
            available: interest,
            tranche_balances: Some(&state.tranche_balances),
            deferred_interest: Some(&state.deferred_interest),
            pool_balance,
            special_serviced_balance,
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
    let all_specs: Vec<&CoverageTestSpec> = specs.iter().chain(waterfall_specs.iter()).collect();
    let (claim_caps, coverage_rules) = coverage_inputs(waterfall);
    let results = evaluate_coverage_tests(
        &all_specs,
        &TestContext {
            pool: &state.pool,
            tranches: state.tranches,
            as_of: period.payment,
            valuation_date: period.valuation,
            period_start: Some(period_start),
            cash_balance: principal,
            interest_collections: interest,
            rules: coverage_rules,
            market: Some(market),
            tranche_balances: Some(&state.tranche_balances),
            payable_principal_tranche_ids: Some(&payable),
            asset_balances: Some(&state.pool_state.balances),
            live_collateral: Some(state.live_collateral(&unresolved_npl)),
            current_pool_balance: Some(pool_balance),
            senior_fees: fees,
            restricted_cash: state.principal_funding_account,
            defaulted_collateral_value,
            interest_claim_caps: &claim_caps,
            floating_rate_shift: state.floating_rate_shift,
            deferred_interest: Some(&state.deferred_interest),
        },
    )?;

    let mut actions = TriggerActions::default();
    for (saved, spec) in state.tranche_triggers.iter_mut().zip(specs.iter()) {
        let result = results
            .iter()
            .find(|result| result.test_id == spec.id)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "missing trigger result {}",
                    spec.id
                ))
            })?;
        if saved.trigger.update(result.current_ratio, period.payment) {
            match saved.trigger.consequence {
                TriggerConsequence::DivertCashFlow => actions.diversions.push(spec.clone()),
                TriggerConsequence::TrapExcessSpread => actions.trap = true,
                TriggerConsequence::AccelerateAmortization => {
                    actions.accelerate = true;
                    actions.stop_reinvestment = true;
                }
                TriggerConsequence::StopReinvestment => actions.stop_reinvestment = true,
            }
        }
    }
    // CLO reinvestment requires the coverage tests to pass: while a test whose
    // cure pays down the notes fails, principal proceeds repay the notes
    // instead of buying collateral. A failing `Reinvest` test keeps the window
    // open: its cure is recycled to rebuild par.
    if results.iter().any(|result| {
        !result.is_passing
            && all_specs
                .iter()
                .find(|spec| spec.id == result.test_id)
                .is_none_or(|spec| spec.action == CoverageTestAction::PayDownSenior)
    }) {
        actions.stop_reinvestment = true;
    }
    Ok(actions)
}
