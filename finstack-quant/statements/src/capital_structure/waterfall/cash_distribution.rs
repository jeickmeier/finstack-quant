//! Sweep capacity, pro-rata allocation, available-cash caps, and
//! the [`StagedInstrumentFlow`] working struct.

use crate::capital_structure::cashflows::CashflowBreakdown;
use finstack_quant_core::money::Money;

/// Per-instrument working state during waterfall allocation.
///
/// Named fields make the allocation logic readable and resilient to
/// future field additions.
pub(super) struct StagedInstrumentFlow {
    /// Instrument identifier (e.g. "TL-1")
    pub instrument_id: String,
    /// Cashflow breakdown (mutated during allocation)
    pub breakdown: CashflowBreakdown,
    /// Balance at the start of this period
    pub opening_balance: Money,
    /// Extra principal from sweep allocation
    pub extra_principal: Money,
    /// Scheduled (contractual) principal payment
    pub scheduled_principal: Money,
    /// Net new funding (revolver draws + initial-exchange notional) for this
    /// period. The payable balance is `opening_balance + net_new_funding`, and
    /// the period-close balance adds it back so in-period draws are preserved.
    pub net_new_funding: Money,
    /// Cash coupon moved into the PIK bucket by the PIK toggle this period.
    /// Tracked so toggle-driven capitalization can be accumulated in
    /// `CapitalStructureState::cumulative_toggled_pik`.
    pub toggled_pik_moved: Money,
}

/// Cap a single category (fees, interest) across instruments using a pro-rata
/// allocation of remaining cash.
///
/// Negative planned values are treated as zero claims: they receive no
/// allocation and the category field is set to zero. A negative contractual
/// amount in an outflow bucket indicates an upstream sign-convention problem
/// rather than a receivable, so it is neutralized instead of being netted
/// against other instruments' claims.
pub(super) fn apply_cash_cap_to_category<F>(
    staged: &mut [StagedInstrumentFlow],
    remaining_cash: &mut Money,
    period_id: finstack_quant_core::dates::PeriodId,
    category: &str,
    warnings: &mut Vec<crate::evaluator::EvalWarning>,
    mut field: F,
) where
    F: FnMut(&mut StagedInstrumentFlow) -> &mut Money,
{
    let planned: Vec<f64> = staged
        .iter_mut()
        .map(|s| {
            let amount = field(s).amount();
            if amount < 0.0 {
                // A negative outflow claim is neutralized to zero. Surface it:
                // it usually signals an upstream sign-convention bug, and it
                // silently changes the model's totals when a cash cap is active.
                warnings.push(
                    crate::evaluator::EvalWarning::CapitalStructureCashflowIgnored {
                        period: period_id,
                        kind: format!(
                            "negative_{category}_neutralized(instrument={}, amount={amount:.4})",
                            s.instrument_id
                        ),
                        cashflow_date: period_id.to_string(),
                    },
                );
            }
            amount.max(0.0)
        })
        .collect();
    let allocations = allocate_pro_rata(&planned, remaining_cash);
    for (s, allocated) in staged.iter_mut().zip(allocations.into_iter()) {
        let currency = field(s).currency();
        *field(s) = Money::new(allocated, currency);
    }
}

/// Distribute `remaining_cash` proportionally across `planned` amounts.
///
/// If enough cash exists to fund all planned amounts, each is paid in
/// full. Otherwise, each entry receives its pro-rata share, with any
/// residual rounding error assigned to the last entry to preserve the
/// total exactly.
pub(super) fn allocate_pro_rata(planned: &[f64], remaining_cash: &mut Money) -> Vec<f64> {
    let total_planned: f64 = planned.iter().sum();
    if total_planned <= 0.0 || remaining_cash.amount() <= 0.0 {
        return vec![0.0; planned.len()];
    }
    if remaining_cash.amount() >= total_planned {
        *remaining_cash = Money::new(
            remaining_cash.amount() - total_planned,
            remaining_cash.currency(),
        );
        return planned.to_vec();
    }

    let cash_before = remaining_cash.amount();
    let mut allocations = Vec::with_capacity(planned.len());
    for (idx, planned_value) in planned.iter().enumerate() {
        if idx + 1 == planned.len() {
            let allocated_so_far: f64 = allocations.iter().sum();
            allocations.push(
                (cash_before - allocated_so_far)
                    .max(0.0)
                    .min(*planned_value),
            );
        } else {
            allocations.push((cash_before * (*planned_value / total_planned)).min(*planned_value));
        }
    }
    *remaining_cash = Money::new(0.0, remaining_cash.currency());
    allocations
}
