//! Amortization cashflow emission.

use crate::builder::orchestrator::{AmortizationSetup, PreparedAmortization};
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::decimal::{decimal_to_f64, f64_to_decimal};
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;

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
    ccy: Currency,
    outstanding: &mut Decimal,
    setup: &AmortizationSetup,
    linear_between_installment: Option<Decimal>,
    is_maturity: bool,
    new_flows: &mut Vec<CashFlow>,
) -> finstack_quant_core::Result<()> {
    let on_boundary = setup.amort_dates.contains(&d);
    let pay = match &setup.rule {
        PreparedAmortization::LinearTo {
            installment,
            final_notional,
        } if on_boundary => {
            if is_maturity {
                (*outstanding - *final_notional).max(Decimal::ZERO)
            } else {
                *installment
            }
        }
        PreparedAmortization::StepRemaining(map) => {
            if let Some(target) = map.get(&d) {
                let target = f64_to_decimal(target.amount())?;
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
        PreparedAmortization::LinearBetween { start, end, .. }
            if on_boundary && d > *start && d <= *end =>
        {
            if let Some(installment) = linear_between_installment {
                if d == *end {
                    *outstanding
                } else {
                    installment
                }
            } else {
                Decimal::ZERO
            }
        }
        PreparedAmortization::CustomPrincipal(map) => {
            let amount = map
                .get(&d)
                .map(|money| f64_to_decimal(money.amount()))
                .transpose()?
                .unwrap_or_default();
            if amount > *outstanding {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CustomPrincipal repayment {amount} exceeds outstanding principal {outstanding} on {d}"
                )));
            }
            amount
        }
        _ => Decimal::ZERO,
    };
    emit_principal_repayment(d, ccy, outstanding, pay, new_flows)
}
