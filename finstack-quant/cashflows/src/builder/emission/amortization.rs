//! Amortization cashflow emission.

use crate::builder::orchestrator::{AmortizationSetup, BuildState, PreparedAmortization};
use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowParams};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;

fn emit_principal_repayment(
    d: Date,
    payment_date: Date,
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

    // Amortization cash is par: the balance falls by the amount paid
    // (record CORE-CONVENTIONS-015 #4, #5).
    let cash = Money::from_decimal(pay, ccy);
    new_flows.push(
        CashFlowParams::new(payment_date, None, cash, CFKind::Amortization, 0.0, None)
            .with_principal_delta(cash.checked_neg())
            .with_principal_date(d)
            .build()?,
    );
    *outstanding -= pay;
    Ok(())
}

/// Emit scheduled amortization on `d` and reduce `outstanding`.
///
/// Residual outstanding at maturity is redeemed later as [`CFKind::Notional`].
pub(in crate::builder) fn emit_amortization_on(
    d: Date,
    ccy: Currency,
    setup: &AmortizationSetup,
    funded_today: Decimal,
    state: &mut BuildState,
) -> finstack_quant_core::Result<()> {
    let outstanding = &mut state.outstanding;
    let is_maturity = setup.final_amortization_date == Some(d);
    let on_boundary = setup.payment_dates.contains_key(&d);
    let pay = match &setup.rule {
        PreparedAmortization::LinearTo {
            installment,
            final_notional,
        } if on_boundary => {
            if is_maturity {
                if *outstanding < *final_notional {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "LinearTo maturity balance {outstanding} on {d} is below target final_notional {final_notional}"
                    )));
                }
                *outstanding - *final_notional
            } else {
                (*installment).min((*outstanding - *final_notional).max(Decimal::ZERO))
            }
        }
        PreparedAmortization::StepRemaining(map) => {
            if let Some(target) = map.get(&d) {
                let target = target.amount_decimal();
                if target > *outstanding {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "StepRemaining target {target} exceeds outstanding principal {outstanding} on {d}"
                    )));
                }
                *outstanding - target
            } else {
                Decimal::ZERO
            }
        }
        PreparedAmortization::OriginalPercent { installment } if on_boundary => *installment,
        PreparedAmortization::RemainingPercent { fraction } if on_boundary => {
            *outstanding * *fraction
        }
        PreparedAmortization::LinearBetween { boundaries } => {
            if let Ok(idx) = boundaries.binary_search(&d) {
                let remaining_boundaries = boundaries.len() - idx;
                if remaining_boundaries == 1 {
                    *outstanding
                } else {
                    (*outstanding - funded_today).max(Decimal::ZERO)
                        / Decimal::from(remaining_boundaries as u64)
                }
            } else {
                Decimal::ZERO
            }
        }
        PreparedAmortization::CustomPrincipal(map) => {
            let amount = map.get(&d).map(Money::amount_decimal).unwrap_or_default();
            if amount > *outstanding {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CustomPrincipal repayment {amount} exceeds outstanding principal {outstanding} on {d}"
                )));
            }
            amount
        }
        _ => Decimal::ZERO,
    };
    let payment_date = setup.payment_dates.get(&d).copied().unwrap_or(d);
    emit_principal_repayment(d, payment_date, ccy, outstanding, pay, &mut state.flows)
}
