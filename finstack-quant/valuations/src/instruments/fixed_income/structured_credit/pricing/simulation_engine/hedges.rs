//! Period bucketing of hedge-swap flows for the simulation engine.
//!
//! The swap schedules are projected once per simulation run from the market
//! context (they do not depend on path randomness) and bucketed per payment
//! period: a net receipt joins interest proceeds, a net payment becomes a
//! `SwapCounterparty` fee recipient in the period waterfall.

use super::*;
use crate::instruments::fixed_income::structured_credit::types::{SwapNotional, SwapPriority};
use finstack_quant_cashflows::CashflowScheduleSource;
use std::collections::BTreeMap;

/// One hedge swap's projected net flows, keyed by payment date.
#[derive(Debug, Clone)]
pub(super) struct HedgeSchedule {
    /// Recipient id used for the swap's payments (`swap_{n}`).
    pub(super) id: String,
    /// Fee tier the payments rank in.
    pub(super) priority: SwapPriority,
    /// Notional the flows track.
    pub(super) notional: SwapNotional,
    /// The swap's contractual notional, the denominator of the rescaling.
    pub(super) contractual_notional: f64,
    /// Net flow per payment date from the deal's side (receipts positive).
    pub(super) flows: Vec<(Date, f64)>,
}

/// Project every hedge swap's net flows from the market context.
pub(super) fn hedge_schedules(
    instrument: &StructuredCredit,
    context: &MarketContext,
    as_of: Date,
) -> Result<Vec<HedgeSchedule>> {
    instrument
        .hedge_swaps
        .iter()
        .enumerate()
        .map(|(index, hedge)| {
            let schedule = hedge.swap.raw_cashflow_schedule(context, as_of)?;
            let mut by_date: BTreeMap<Date, f64> = BTreeMap::new();
            for flow in schedule.get_flows() {
                if matches!(flow.kind, CFKind::Notional) {
                    continue;
                }
                *by_date.entry(flow.date).or_insert(0.0) += flow.amount.amount();
            }
            Ok(HedgeSchedule {
                id: format!("swap_{}", index + 1),
                priority: hedge.priority,
                notional: hedge.notional.clone(),
                contractual_notional: hedge.swap.notional.amount(),
                flows: by_date.into_iter().collect(),
            })
        })
        .collect()
}

/// A period's hedge cash: receipts that join interest proceeds and payments
/// owed to the counterparties, each with its fee-tier priority.
pub(super) struct PeriodHedgeFlows {
    pub(super) receipts: Money,
    pub(super) payments: Vec<(String, Money, SwapPriority)>,
}

/// Bucket the swap flows dated in `(period_start, pay_date]`, rescaled to
/// the tracked notional at period start.
pub(super) fn period_hedge_flows(
    state: &SimulationState,
    period_start: Date,
    pay_date: Date,
) -> Result<PeriodHedgeFlows> {
    let mut receipts = Money::from((0_i64, state.base_currency));
    let mut payments = Vec::new();
    for schedule in &state.hedge_schedules {
        let tracked = match &schedule.notional {
            SwapNotional::Contractual => schedule.contractual_notional,
            SwapNotional::TranchePar(id) => state
                .tranche_balances
                .get(id.as_str())
                .map_or(0.0, Money::amount),
            SwapNotional::PoolPar => state.pool_outstanding.amount(),
        };
        let scale = if schedule.contractual_notional > 0.0 {
            (tracked / schedule.contractual_notional).max(0.0)
        } else {
            0.0
        };
        let net: f64 = schedule
            .flows
            .iter()
            .filter(|(date, _)| *date > period_start && *date <= pay_date)
            .map(|(_, amount)| amount)
            .sum::<f64>()
            * scale;
        if net > 0.0 {
            receipts = receipts.checked_add(Money::new(net, state.base_currency)?)?;
        } else if net < 0.0 {
            payments.push((
                schedule.id.clone(),
                Money::new(-net, state.base_currency)?,
                schedule.priority,
            ));
        }
    }
    Ok(PeriodHedgeFlows { receipts, payments })
}
