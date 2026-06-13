//! Amortization cashflow emission.

use crate::builder::{AmortizationSpec, Notional};
use finstack_core::cashflow::{CFKind, CashFlow};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::decimal::{decimal_to_f64, f64_to_decimal};
use finstack_core::money::Money;
use rust_decimal::Decimal;

/// Amortization parameters for emission.
///
/// Contains precomputed values and maps needed by `emit_amortization_on` to
/// process various amortization specifications efficiently.
#[derive(Debug, Clone)]
pub(in crate::builder) struct AmortizationParams<'a> {
    pub(in crate::builder) ccy: Currency,
    pub(in crate::builder) amort_dates: &'a finstack_core::HashSet<Date>,
    pub(in crate::builder) linear_delta: Option<Decimal>,
    pub(in crate::builder) percent_per: Option<Decimal>,
    pub(in crate::builder) step_remaining_map: &'a Option<finstack_core::HashMap<Date, Money>>,
    pub(in crate::builder) custom_principal_map: &'a Option<finstack_core::HashMap<Date, Money>>,
}

fn emit_principal_repayment(
    d: Date,
    ccy: Currency,
    outstanding: &mut Decimal,
    pay: Decimal,
    new_flows: &mut Vec<CashFlow>,
) -> finstack_core::Result<()> {
    if pay <= Decimal::ZERO {
        return Ok(());
    }

    // Clamp to outstanding to guard against any numerical drift
    let pay = if pay < *outstanding {
        pay
    } else {
        *outstanding
    };
    if pay <= Decimal::ZERO {
        return Ok(());
    }

    new_flows.push(CashFlow {
        date: d,
        reset_date: None,
        amount: Money::new(decimal_to_f64(pay)?, ccy),
        kind: CFKind::Amortization,
        accrual_factor: 0.0,
        rate: None,
    });
    *outstanding -= pay;
    Ok(())
}

/// Emit amortization cashflows on a specific date.
///
/// Processes the notional's amortization specification to generate principal
/// repayment flows. Mutates the `outstanding` balance in-place to reflect
/// the reduction from amortization.
///
/// Supports:
/// - LinearTo: Equal installments over schedule
/// - StepRemaining: Specific remaining balance targets
/// - PercentOfOriginalPerPeriod: Percentage of original notional (capped by remaining)
/// - CustomPrincipal: Explicit payment amounts by date
///
/// All variants emit only the scheduled/configured amount as
/// `CFKind::Amortization` — including on the maturity date. Any residual
/// outstanding at maturity is redeemed downstream by the pipeline's
/// maturity handling as `CFKind::Notional`, so total principal cash is
/// unchanged by classification.
pub(in crate::builder) fn emit_amortization_on(
    d: Date,
    notional: &Notional,
    outstanding: &mut Decimal,
    params: &AmortizationParams,
    is_maturity: bool,
    new_flows: &mut Vec<CashFlow>,
) -> finstack_core::Result<()> {
    match &notional.amort {
        AmortizationSpec::None => {}
        AmortizationSpec::LinearTo { final_notional } => {
            if params.amort_dates.contains(&d) {
                if let Some(delta) = params.linear_delta {
                    let final_notional = f64_to_decimal(final_notional.amount())?;
                    let pay = if is_maturity {
                        let excess = *outstanding - final_notional;
                        if excess > Decimal::ZERO {
                            excess
                        } else {
                            Decimal::ZERO
                        }
                    } else if delta < *outstanding {
                        delta
                    } else {
                        *outstanding
                    };
                    emit_principal_repayment(d, params.ccy, outstanding, pay, new_flows)?;
                }
            }
        }
        AmortizationSpec::StepRemaining { .. } => {
            if let Some(map) = params.step_remaining_map {
                if let Some(rem_after) = map.get(&d) {
                    let target = f64_to_decimal(rem_after.amount())?;
                    // Pay down to the scheduled target on every date,
                    // including maturity. Any residual outstanding (a final
                    // non-zero target) is redeemed by `handle_maturity` as
                    // `CFKind::Notional`, consistent with the other variants.
                    let excess = *outstanding - target;
                    let positive_excess = if excess > Decimal::ZERO {
                        excess
                    } else {
                        Decimal::ZERO
                    };
                    let pay = if positive_excess < *outstanding {
                        positive_excess
                    } else {
                        *outstanding
                    };
                    emit_principal_repayment(d, params.ccy, outstanding, pay, new_flows)?;
                }
            }
        }
        AmortizationSpec::PercentOfOriginalPerPeriod { .. } => {
            if params.amort_dates.contains(&d) {
                if let Some(per) = params.percent_per {
                    // Pay the scheduled percentage on every date, including
                    // maturity. Any residual outstanding at maturity is
                    // redeemed by `handle_maturity` as `CFKind::Notional`,
                    // consistent with the other variants.
                    let pay = if per < *outstanding {
                        per
                    } else {
                        *outstanding
                    };
                    emit_principal_repayment(d, params.ccy, outstanding, pay, new_flows)?;
                }
            }
        }
        AmortizationSpec::CustomPrincipal { .. } => {
            // Honor the configured `amt` on every date, including maturity.
            // Any residual outstanding at maturity is redeemed by
            // `handle_maturity` as `CFKind::Notional`.
            if let Some(map) = params.custom_principal_map {
                if let Some(amt) = map.get(&d) {
                    let amount = f64_to_decimal(amt.amount())?;
                    let positive_amount = if amount > Decimal::ZERO {
                        amount
                    } else {
                        Decimal::ZERO
                    };
                    let pay = if positive_amount < *outstanding {
                        positive_amount
                    } else {
                        *outstanding
                    };
                    emit_principal_repayment(d, params.ccy, outstanding, pay, new_flows)?;
                }
            }
        }
    }
    Ok(())
}
