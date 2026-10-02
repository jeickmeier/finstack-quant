//! Validate the cash currency shared by the waterfall's payment priorities.

use crate::capital_structure::cashflows::CashflowBreakdown;
use crate::error::Result;
use indexmap::IndexMap;

/// Validate that all instruments share a single currency.
///
/// Also checks each breakdown's *internal* currency invariant. Comparing only
/// `interest_expense_cash` across instruments would let a breakdown whose other
/// legs carry a different currency through to the allocation steps, where
/// `Money`'s asserting `AddAssign` aborts the process (e.g. the Step 4b PIK
/// move). `execute_waterfall` is public and `CashflowBreakdown`'s fields are
/// `pub`, so an inconsistent breakdown is reachable input and must produce an
/// error rather than a panic (INVARIANTS.md §5).
pub(super) fn waterfall_currency(
    flows: &IndexMap<String, CashflowBreakdown>,
) -> Result<finstack_quant_core::currency::Currency> {
    for breakdown in flows.values() {
        breakdown.validate_currency_invariant()?;
    }

    let mut currencies = flows
        .values()
        .map(|cf| cf.interest_expense_cash.currency())
        .collect::<Vec<_>>();
    currencies.sort();
    currencies.dedup();
    match currencies.as_slice() {
        [currency] => Ok(*currency),
        [] => Err(crate::error::Error::capital_structure(
            "Waterfall execution requires at least one instrument with contractual flows; \
             cannot determine the waterfall cash currency from an empty set.",
        )),
        _ => Err(crate::error::Error::capital_structure(
            "Waterfall execution currently requires a single cash currency. \
             Use one currency per waterfall or add explicit FX allocation semantics.",
        )),
    }
}
