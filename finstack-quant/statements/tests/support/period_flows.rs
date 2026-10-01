//! Test-only driver that rolls [`calculate_period_flows`] across a period grid
//! for a set of instruments, mirroring the evaluator's capital-structure runtime
//! (opening balance = prior closing balance, no PIK toggle, no residual schedule).
#![allow(dead_code)]

use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_valuations::instruments::Instrument;

#[derive(Clone)]
pub(crate) struct ScheduleInstrument {
    pub(crate) schedule: CashFlowSchedule,
    id: finstack_quant_core::types::InstrumentId,
    attributes: finstack_quant_core::types::Attributes,
}

impl ScheduleInstrument {
    pub(crate) fn new(schedule: CashFlowSchedule) -> Self {
        Self {
            schedule,
            id: "SCHEDULE".into(),
            attributes: Default::default(),
        }
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for ScheduleInstrument {
    fn raw_cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        Ok(self.schedule.clone())
    }
}

impl Instrument for ScheduleInstrument {
    fn id(&self) -> &str {
        self.id.as_str()
    }
    fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
        finstack_quant_valuations::pricer::InstrumentType::Bond
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn attributes(&self) -> &finstack_quant_core::types::Attributes {
        &self.attributes
    }
    fn attributes_mut(&mut self) -> &mut finstack_quant_core::types::Attributes {
        &mut self.attributes
    }
    fn clone_box(&self) -> Box<dyn finstack_quant_valuations::instruments::Instrument> {
        Box::new(self.clone())
    }

    fn base_value(
        &self,
        _market: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        Err(finstack_quant_core::Error::Validation(
            "test schedule does not price".into(),
        ))
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
    {
        Ok(Default::default())
    }
}
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
    instruments: &IndexMap<String, Arc<dyn Instrument>>,
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
