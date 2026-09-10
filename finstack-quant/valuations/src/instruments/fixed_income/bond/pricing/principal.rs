//! Principal recovery exposure until actual cash repayment.
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::{cashflow::CFKind, dates::Date, money::Money, Result};

/// Replay principal claims on cash dates, retaining economic PIK/default dates.
pub(crate) fn recovery_principal_path(schedule: &CashFlowSchedule) -> Result<Vec<(Date, Money)>> {
    let mut cash_schedule = schedule.clone();
    cash_schedule.update_flows(|flow| {
        if matches!(
            flow.kind,
            CFKind::Amortization
                | CFKind::PrePayment
                | CFKind::Notional
                | CFKind::RevolvingDraw
                | CFKind::RevolvingRepayment
        ) {
            flow.principal_date = None;
        }
    });
    cash_schedule.outstanding_by_date()
}
