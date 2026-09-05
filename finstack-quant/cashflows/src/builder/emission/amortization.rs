//! Amortization cashflow emission.

use crate::builder::{AmortizationSpec, Notional};
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::decimal::{decimal_to_f64, f64_to_decimal};
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;

/// Precomputed maps and deltas used by [`emit_amortization_on`].
#[derive(Debug, Clone)]
pub(in crate::builder) struct AmortizationParams<'a> {
    pub(in crate::builder) ccy: Currency,
    pub(in crate::builder) amort_dates: &'a finstack_quant_core::HashSet<Date>,
    pub(in crate::builder) linear_delta: Option<Decimal>,
    pub(in crate::builder) percent_per: Option<Decimal>,
    pub(in crate::builder) step_remaining_map:
        &'a Option<finstack_quant_core::HashMap<Date, Money>>,
    pub(in crate::builder) custom_principal_map:
        &'a Option<finstack_quant_core::HashMap<Date, Money>>,
}

fn emit_principal_repayment(
    d: Date,
    ccy: Currency,
    outstanding: &mut Decimal,
    pay: Decimal,
    new_flows: &mut Vec<CashFlow>,
) -> finstack_quant_core::Result<()> {
    if pay <= Decimal::ZERO {
        return Ok(());
    }

    let pay = pay.min(*outstanding);
    if pay <= Decimal::ZERO {
        return Ok(());
    }

    new_flows.push(CashFlow::new(
        d,
        None,
        Money::new(decimal_to_f64(pay)?, ccy)?,
        CFKind::Amortization,
        0.0,
        None,
    ));
    *outstanding -= pay;
    Ok(())
}

/// Emit scheduled amortization on `d` and reduce `outstanding`.
///
/// Residual outstanding at maturity is redeemed later as [`CFKind::Notional`].
pub(in crate::builder) fn emit_amortization_on(
    d: Date,
    notional: &Notional,
    outstanding: &mut Decimal,
    params: &AmortizationParams,
    is_maturity: bool,
    new_flows: &mut Vec<CashFlow>,
) -> finstack_quant_core::Result<()> {
    match &notional.amort {
        AmortizationSpec::None => {}
        AmortizationSpec::LinearTo { final_notional } => {
            if let Some(delta) = params
                .linear_delta
                .filter(|_| params.amort_dates.contains(&d))
            {
                let final_notional = f64_to_decimal(final_notional.amount())?;
                let pay = if is_maturity {
                    (*outstanding - final_notional).max(Decimal::ZERO)
                } else {
                    delta.min(*outstanding)
                };
                emit_principal_repayment(d, params.ccy, outstanding, pay, new_flows)?;
            }
        }
        AmortizationSpec::StepRemaining { .. } => {
            if let Some(rem_after) = params
                .step_remaining_map
                .as_ref()
                .and_then(|map| map.get(&d))
            {
                let target = f64_to_decimal(rem_after.amount())?;
                let pay = (*outstanding - target).max(Decimal::ZERO).min(*outstanding);
                emit_principal_repayment(d, params.ccy, outstanding, pay, new_flows)?;
            }
        }
        AmortizationSpec::PercentOfOriginalPerPeriod { .. } => {
            if let Some(per) = params
                .percent_per
                .filter(|_| params.amort_dates.contains(&d))
            {
                emit_principal_repayment(
                    d,
                    params.ccy,
                    outstanding,
                    per.min(*outstanding),
                    new_flows,
                )?;
            }
        }
        AmortizationSpec::CustomPrincipal { .. } => {
            if let Some(amt) = params
                .custom_principal_map
                .as_ref()
                .and_then(|map| map.get(&d))
            {
                let amount = f64_to_decimal(amt.amount())?.max(Decimal::ZERO);
                emit_principal_repayment(
                    d,
                    params.ccy,
                    outstanding,
                    amount.min(*outstanding),
                    new_flows,
                )?;
            }
        }
    }
    Ok(())
}
