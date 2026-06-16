//! Closing balances, cumulative metric updates, and end-of-period invariants.

use crate::capital_structure::cashflows::CashflowBreakdown;
use crate::capital_structure::state::CapitalStructureState;
use crate::error::Result;
use finstack_quant_core::money::Money;

/// Update cumulative metrics in state after a period closes.
pub(super) fn update_cumulative_metrics(
    state: &mut CapitalStructureState,
    instrument_id: &str,
    breakdown: &CashflowBreakdown,
    currency: finstack_quant_core::currency::Currency,
) -> Result<()> {
    let current_cash = state
        .cumulative_interest_cash
        .get(instrument_id)
        .copied()
        .unwrap_or_else(|| Money::new(0.0, currency));
    state.cumulative_interest_cash.insert(
        instrument_id.to_string(),
        current_cash.checked_add(breakdown.interest_expense_cash)?,
    );

    let current_pik = state
        .cumulative_interest_pik
        .get(instrument_id)
        .copied()
        .unwrap_or_else(|| Money::new(0.0, currency));
    state.cumulative_interest_pik.insert(
        instrument_id.to_string(),
        current_pik.checked_add(breakdown.interest_expense_pik)?,
    );

    let current_principal = state
        .cumulative_principal
        .get(instrument_id)
        .copied()
        .unwrap_or_else(|| Money::new(0.0, currency));
    state.cumulative_principal.insert(
        instrument_id.to_string(),
        current_principal.checked_add(breakdown.principal_payment)?,
    );

    Ok(())
}
