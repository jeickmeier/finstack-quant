//! Levered real estate equity pricer implementation.

use super::levered::LeveredRealEstateEquity;
use crate::instruments::Instrument;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Error;
use std::collections::BTreeMap;

pub(crate) fn compute_pv(
    inst: &LeveredRealEstateEquity,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    validate_currency(inst)?;
    let asset_pv = inst.asset.value(market, as_of)?;
    Money::new(
        asset_pv.amount() - financing_pv(inst, market, as_of)?,
        inst.currency,
    )
}

/// Total PV of the financing instruments, in the equity currency.
pub(crate) fn financing_pv(
    inst: &LeveredRealEstateEquity,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    let mut total = 0.0;
    for financing in &inst.financing {
        let pv = financing.as_instrument().value(market, as_of)?;
        if pv.currency() != inst.currency {
            return Err(Error::Validation("financing PV currency mismatch".into()));
        }
        total += pv.amount();
    }
    Ok(total)
}

pub(crate) fn validate_currency(inst: &LeveredRealEstateEquity) -> finstack_quant_core::Result<()> {
    if inst.asset.currency != inst.currency {
        return Err(Error::Validation(
            "asset currency must match levered equity currency".into(),
        ));
    }
    Ok(())
}

pub(crate) fn asset_sale_proceeds_at(
    inst: &LeveredRealEstateEquity,
    as_of: Date,
    exit: Date,
) -> finstack_quant_core::Result<f64> {
    let Some((_d, proceeds)) = super::pricer::sale_proceeds_at(&inst.asset, as_of, exit)? else {
        return Err(Error::Validation(
            "sale_price or terminal_cap_rate is required to compute sale proceeds".into(),
        ));
    };
    Ok(proceeds)
}

pub(crate) fn financing_schedules_supported(
    inst: &LeveredRealEstateEquity,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Vec<finstack_quant_cashflows::builder::CashFlowSchedule>> {
    let mut schedules = Vec::with_capacity(inst.financing.len());
    for financing in &inst.financing {
        schedules.push(financing.as_instrument().cashflow_schedule(market, as_of)?);
    }
    Ok(schedules)
}

pub(crate) fn outstanding_before(
    out_path: &[(Date, Money)],
    target: Date,
    initial: Money,
) -> Money {
    let mut last = initial;
    for (d, amt) in out_path {
        if *d < target {
            last = *amt;
        } else {
            break;
        }
    }
    last
}

pub(crate) fn equity_cashflows(
    inst: &LeveredRealEstateEquity,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Vec<(Date, f64)>> {
    validate_currency(inst)?;
    // The exit is the asset's valuation horizon, so PV, the equity cashflow
    // schedule and levered metrics share one holding period.
    let exit = super::pricer::horizon_date(&inst.asset, as_of)?;

    let purchase = super::pricer::required_purchase_price(&inst.asset)?;

    let mut flows: BTreeMap<Date, f64> = BTreeMap::new();
    let acq_cost = super::pricer::acquisition_cost_total(&inst.asset)?;
    *flows.entry(as_of).or_insert(0.0) += -(purchase + acq_cost);

    // Unlevered flows already stop at the horizon, which is the exit.
    for (d, a) in super::pricer::unlevered_flows(&inst.asset, as_of)? {
        *flows.entry(d).or_insert(0.0) += a;
    }

    let financing_schedules = financing_schedules_supported(inst, market, as_of)?;
    for sched in &financing_schedules {
        for cf in sched.get_flows() {
            if cf.date < as_of || cf.date > exit {
                continue;
            }
            if matches!(cf.kind, finstack_quant_core::cashflow::CFKind::Pik) {
                continue;
            }
            let is_principal = matches!(
                cf.kind,
                finstack_quant_core::cashflow::CFKind::Notional
                    | finstack_quant_core::cashflow::CFKind::Amortization
            );
            if cf.date == exit && is_principal {
                continue;
            }
            *flows.entry(cf.date).or_insert(0.0) += -cf.amount.amount();
        }
    }

    let sale = asset_sale_proceeds_at(inst, as_of, exit)?;
    *flows.entry(exit).or_insert(0.0) += sale;

    *flows.entry(exit).or_insert(0.0) += -payoff_at(&financing_schedules, exit, inst.currency)?;

    Ok(flows.into_iter().collect())
}

/// Financing payoff at the asset horizon (absolute amount); the caller has
/// validated the instrument, currencies included.
pub(crate) fn financing_payoff_at_exit(
    inst: &LeveredRealEstateEquity,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    let exit = super::pricer::horizon_date(&inst.asset, as_of)?;
    let schedules = financing_schedules_supported(inst, market, as_of)?;
    Money::new(payoff_at(&schedules, exit, inst.currency)?, inst.currency)
}

/// Total absolute outstanding of `schedules` just before `exit`.
fn payoff_at(
    schedules: &[finstack_quant_cashflows::builder::CashFlowSchedule],
    exit: Date,
    _currency: Currency,
) -> finstack_quant_core::Result<f64> {
    let mut payoff_amt = 0.0;
    for sched in schedules {
        let out_path = sched.outstanding_by_date()?;
        let payoff = outstanding_before(&out_path, exit, sched.get_notional().initial);
        payoff_amt += payoff.amount().abs();
    }
    Ok(payoff_amt)
}
