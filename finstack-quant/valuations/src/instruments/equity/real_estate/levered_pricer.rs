//! Levered real estate equity pricer implementation.

use super::levered::LeveredRealEstateEquity;
use crate::cashflow::traits::CashflowProvider;
use crate::instruments::{Instrument, InstrumentJson};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Error as CoreError;
use std::collections::BTreeMap;

/// Registry-facing pricer for levered real estate equity.
pub struct LeveredRealEstateDiscountingPricer;

pub(crate) fn compute_pv(
    inst: &LeveredRealEstateEquity,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    validate_currency(inst)?;
    let asset_pv = inst.asset.value(market, as_of)?;
    if asset_pv.currency() != inst.currency {
        return Err(CoreError::Validation("asset PV currency mismatch".into()));
    }
    let mut financing_pv = 0.0;
    for instrument in &inst.financing {
        let boxed = instrument.clone().into_boxed()?;
        let pv = boxed.value(market, as_of)?;
        if pv.currency() != inst.currency {
            return Err(CoreError::Validation(
                "financing PV currency mismatch".into(),
            ));
        }
        financing_pv += pv.amount();
    }
    Ok(Money::new(asset_pv.amount() - financing_pv, inst.currency))
}

pub(crate) fn validate_currency(inst: &LeveredRealEstateEquity) -> finstack_quant_core::Result<()> {
    if inst.asset.currency != inst.currency {
        return Err(CoreError::Validation(
            "asset currency must match levered equity currency".into(),
        ));
    }
    Ok(())
}

pub(crate) fn resolve_exit_date(
    inst: &LeveredRealEstateEquity,
    as_of: Date,
) -> finstack_quant_core::Result<Date> {
    if let Some(d) = inst.exit_date {
        return Ok(d);
    }
    let flows = inst.asset.unlevered_flows(as_of)?;
    flows
        .last()
        .map(|(d, _)| *d)
        .ok_or_else(|| CoreError::Validation("Missing cashflows for exit date".into()))
}

pub(crate) fn asset_sale_proceeds_at(
    inst: &LeveredRealEstateEquity,
    as_of: Date,
    exit: Date,
) -> finstack_quant_core::Result<f64> {
    let Some((_d, proceeds)) = inst.asset.sale_proceeds_at(as_of, exit)? else {
        return Err(CoreError::Validation(
            "sale_price or terminal_cap_rate is required to compute sale proceeds".into(),
        ));
    };
    Ok(proceeds)
}

pub(crate) fn financing_schedules_supported(
    inst: &LeveredRealEstateEquity,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Vec<crate::cashflow::builder::CashFlowSchedule>> {
    let mut schedules = Vec::with_capacity(inst.financing.len());
    for instrument in &inst.financing {
        let sched = match instrument {
            InstrumentJson::TermLoan(i) => i.cashflow_schedule(market, as_of)?,
            InstrumentJson::Bond(i) => i.cashflow_schedule(market, as_of)?,
            InstrumentJson::RevolvingCredit(i) => i.cashflow_schedule(market, as_of)?,
            InstrumentJson::Repo(i) => i.cashflow_schedule(market, as_of)?,
            _ => {
                return Err(CoreError::Validation(
                    "Unsupported financing instrument for cashflow-based metrics (supported: term_loan, bond, revolving_credit, repo)".into(),
                ));
            }
        };
        schedules.push(sched);
    }
    Ok(schedules)
}

pub(crate) fn outstanding_before(
    out_path: &[(Date, Money)],
    target: Date,
    currency: Currency,
) -> Money {
    let mut last = Money::new(0.0, currency);
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
    let exit = resolve_exit_date(inst, as_of)?;

    let purchase = inst
        .asset
        .purchase_price
        .ok_or_else(|| CoreError::Validation("purchase_price is required".into()))?;
    if purchase.currency() != inst.currency {
        return Err(CoreError::Validation(
            "purchase_price currency must match instrument currency".into(),
        ));
    }

    let mut flows: BTreeMap<Date, f64> = BTreeMap::new();
    let acq_cost = inst.asset.acquisition_cost_total()?;
    *flows.entry(as_of).or_insert(0.0) += -(purchase.amount() + acq_cost);

    for (d, a) in inst.asset.unlevered_flows(as_of)? {
        if d <= exit {
            *flows.entry(d).or_insert(0.0) += a;
        }
    }

    let financing_schedules = financing_schedules_supported(inst, market, as_of)?;
    for sched in &financing_schedules {
        for cf in sched.get_flows() {
            if cf.date < as_of || cf.date > exit {
                continue;
            }
            if matches!(cf.kind, finstack_quant_core::cashflow::CFKind::PIK) {
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

    let mut payoff_amt = 0.0;
    for sched in &financing_schedules {
        let out_path = sched.outstanding_by_date()?;
        let payoff = outstanding_before(&out_path, exit, inst.currency);
        payoff_amt += payoff.amount().abs();
    }
    *flows.entry(exit).or_insert(0.0) += -payoff_amt;

    Ok(flows.into_iter().collect())
}

pub(crate) fn financing_payoff_at_exit(
    inst: &LeveredRealEstateEquity,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    validate_currency(inst)?;
    let exit = resolve_exit_date(inst, as_of)?;
    let mut payoff_amt = 0.0;
    for sched in financing_schedules_supported(inst, market, as_of)? {
        let out_path = sched.outstanding_by_date()?;
        let payoff = outstanding_before(&out_path, exit, inst.currency);
        payoff_amt += payoff.amount().abs();
    }
    Ok(Money::new(payoff_amt, inst.currency))
}

impl Default for LeveredRealEstateDiscountingPricer {
    fn default() -> Self {
        Self
    }
}

impl Pricer for LeveredRealEstateDiscountingPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(
            InstrumentType::LeveredRealEstateEquity,
            ModelKey::Discounting,
        )
    }

    #[tracing::instrument(
        name = "real_estate.levered.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let inst = instrument
            .as_any()
            .downcast_ref::<LeveredRealEstateEquity>()
            .ok_or_else(|| {
                PricingError::type_mismatch(
                    InstrumentType::LeveredRealEstateEquity,
                    instrument.key(),
                )
            })?;
        let pv = compute_pv(inst, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(inst).model(ModelKey::Discounting),
            )
        })?;
        Ok(ValuationResult::stamped(inst.id(), as_of, pv))
    }
}
