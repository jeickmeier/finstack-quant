//! Amortization cashflow emission.

use crate::builder::{AmortizationSpec, Notional};
use finstack_core::cashflow::{CFKind, CashFlow};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;

/// Amortization parameters for emission.
///
/// Contains precomputed values and maps needed by `emit_amortization_on` to
/// process various amortization specifications efficiently.
#[derive(Debug, Clone)]
pub(in crate::builder) struct AmortizationParams<'a> {
    pub(in crate::builder) ccy: Currency,
    pub(in crate::builder) amort_dates: &'a finstack_core::HashSet<Date>,
    pub(in crate::builder) linear_delta: Option<f64>,
    pub(in crate::builder) percent_per: Option<f64>,
    pub(in crate::builder) step_remaining_map: &'a Option<finstack_core::HashMap<Date, Money>>,
    pub(in crate::builder) custom_principal_map: &'a Option<finstack_core::HashMap<Date, Money>>,
}

fn emit_principal_repayment(
    d: Date,
    ccy: Currency,
    outstanding: &mut f64,
    pay: f64,
    new_flows: &mut Vec<CashFlow>,
) {
    if pay <= 0.0 {
        return;
    }

    // Clamp to outstanding to guard against any numerical drift
    let pay = pay.min(*outstanding);
    if pay <= 0.0 {
        return;
    }

    new_flows.push(CashFlow {
        date: d,
        reset_date: None,
        amount: Money::new(pay, ccy),
        kind: CFKind::Amortization,
        accrual_factor: 0.0,
        rate: None,
    });
    *outstanding -= pay;
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
    outstanding: &mut f64,
    params: &AmortizationParams,
    is_maturity: bool,
    new_flows: &mut Vec<CashFlow>,
) -> finstack_core::Result<()> {
    match &notional.amort {
        AmortizationSpec::None => {}
        AmortizationSpec::LinearTo { final_notional } => {
            if params.amort_dates.contains(&d) {
                if let Some(delta) = params.linear_delta {
                    let pay = if is_maturity {
                        (*outstanding - final_notional.amount()).max(0.0)
                    } else {
                        delta.min(*outstanding)
                    };
                    emit_principal_repayment(d, params.ccy, outstanding, pay, new_flows);
                }
            }
        }
        AmortizationSpec::StepRemaining { .. } => {
            if let Some(map) = params.step_remaining_map {
                if let Some(rem_after) = map.get(&d) {
                    let target = rem_after.amount();
                    // Pay down to the scheduled target on every date,
                    // including maturity. Any residual outstanding (a final
                    // non-zero target) is redeemed by `handle_maturity` as
                    // `CFKind::Notional`, consistent with the other variants.
                    let pay = (*outstanding - target).max(0.0).min(*outstanding);
                    emit_principal_repayment(d, params.ccy, outstanding, pay, new_flows);
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
                    let pay = per.min(*outstanding);
                    emit_principal_repayment(d, params.ccy, outstanding, pay, new_flows);
                }
            }
        }
        AmortizationSpec::CustomPrincipal { .. } => {
            // Honor the configured `amt` on every date, including maturity.
            // Any residual outstanding at maturity is redeemed by
            // `handle_maturity` as `CFKind::Notional`.
            if let Some(map) = params.custom_principal_map {
                if let Some(amt) = map.get(&d) {
                    let pay = amt.amount().max(0.0).min(*outstanding);
                    emit_principal_repayment(d, params.ccy, outstanding, pay, new_flows);
                }
            }
        }
    }
    Ok(())
}
