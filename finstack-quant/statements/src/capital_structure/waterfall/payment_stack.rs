//! Priority ordering helpers and rules like equity-before-sweep behavior.

use crate::capital_structure::cashflows::CashflowBreakdown;
use crate::capital_structure::waterfall_spec::PaymentPriority;
use crate::error::Result;
use indexmap::IndexMap;

/// Find the position of a priority level in the waterfall stack.
///
/// Returns `usize::MAX` when `target` is not present.
pub(super) fn priority_index(priorities: &[PaymentPriority], target: PaymentPriority) -> usize {
    priorities
        .iter()
        .position(|priority| *priority == target)
        .unwrap_or(usize::MAX)
}

/// Earliest position of any extra-principal action (sweep, mandatory/voluntary prepayment).
pub(super) fn extra_principal_priority(priorities: &[PaymentPriority]) -> usize {
    [
        PaymentPriority::MandatoryPrepayment,
        PaymentPriority::VoluntaryPrepayment,
        PaymentPriority::Sweep,
    ]
    .into_iter()
    .map(|priority| priority_index(priorities, priority))
    .min()
    .unwrap_or(usize::MAX)
}

/// Validate that all instruments share a single currency.
pub(super) fn waterfall_currency(
    flows: &IndexMap<String, CashflowBreakdown>,
) -> Result<finstack_quant_core::currency::Currency> {
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
