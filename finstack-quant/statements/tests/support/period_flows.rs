//! Test-only driver that rolls [`calculate_period_flows`] across a period grid
//! for a set of instruments, mirroring the evaluator's capital-structure runtime
//! (opening balance = prior closing balance, no PIK toggle, no residual schedule).
#![allow(dead_code)]

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::dates::{Date, Period};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_statements::capital_structure::{
    calculate_period_flows, CapitalStructureCashflows, CashflowBreakdown,
};
use finstack_quant_statements::Result;
use indexmap::IndexMap;
use std::sync::Arc;

/// Aggregate per-instrument period flows into a [`CapitalStructureCashflows`].
///
/// Totals are only populated when every instrument shares one currency (the
/// runtime applies FX policy for mixed portfolios; these tests do not).
pub fn aggregate_period_flows(
    instruments: &IndexMap<String, Arc<dyn CashflowProvider + Send + Sync>>,
    periods: &[Period],
    market_ctx: &MarketContext,
    as_of: Date,
) -> Result<CapitalStructureCashflows> {
    let mut result = CapitalStructureCashflows::new();
    let mut currencies = Vec::new();

    for (instrument_id, instrument) in instruments {
        let schedule = instrument.cashflow_schedule(market_ctx, as_of)?;
        let currency = schedule.get_notional().initial.currency();
        if !currencies.contains(&currency) {
            currencies.push(currency);
        }
        let mut opening = Money::new(0.0, currency).expect("valid money fixture");
        for period in periods {
            let (breakdown, closing, _net_new_funding, _warnings) = calculate_period_flows(
                instrument.as_ref(),
                period,
                opening,
                Money::new(0.0, currency).expect("valid money fixture"),
                market_ctx,
                as_of,
                None,
            )?;
            let by_ccy = result.totals_by_currency.entry(currency).or_default();
            let total = by_ccy
                .entry(period.id)
                .or_insert_with(|| CashflowBreakdown::with_currency(currency));
            total.interest_expense_cash += breakdown.interest_expense_cash;
            total.interest_expense_pik += breakdown.interest_expense_pik;
            total.principal_payment += breakdown.principal_payment;
            total.debt_balance += breakdown.debt_balance;
            total.fees += breakdown.fees;
            total.accrued_interest += breakdown.accrued_interest;
            result
                .by_instrument
                .entry(instrument_id.clone())
                .or_default()
                .insert(period.id, breakdown);
            opening = closing;
        }
    }

    if let [currency] = currencies.as_slice() {
        result.reporting_currency = Some(*currency);
        result.totals = result
            .totals_by_currency
            .get(currency)
            .cloned()
            .unwrap_or_default();
    }
    Ok(result)
}
