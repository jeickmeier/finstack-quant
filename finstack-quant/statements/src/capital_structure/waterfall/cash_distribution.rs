//! Sweep capacity, pro-rata allocation, available-cash caps, and
//! the [`StagedInstrumentFlow`] working struct.

use crate::capital_structure::cashflows::CashflowBreakdown;
use crate::capital_structure::principal::PrincipalAllocation;
use crate::error::Result;
use crate::evaluator::{CapitalStructureClaimCategory, CapitalStructureWarning, EvalWarning};
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
    /// Dated principal claims and the economic balance available to repay.
    pub principal: PrincipalAllocation,
    /// Payment-class rank (`0` = most senior). Empty `payment_classes` uses `0`.
    pub class_rank: u32,
    /// Scheduled principal claim, replaced by its paid amount at Amortization.
    pub scheduled_principal: Money,
    /// Cash coupon moved into the PIK bucket by the PIK toggle this period.
    /// Tracked so toggle-driven capitalization can be accumulated in
    /// `CapitalStructureState::cumulative_toggled_pik`.
    pub toggled_pik_moved: Money,
}

/// Apply a prepayment at its position in the priority stack.
///
/// Earlier payments consume principal capacity; later scheduled claims and
/// prepayments reserve none. This preserves payment-class seniority within
/// each rung even when a later sweep targets a particular instrument.
pub(super) fn apply_prepayment(
    staged: &mut [StagedInstrumentFlow],
    remaining_cash: &mut Money,
    requested: Money,
    target: Option<&str>,
) -> Result<()> {
    let budget = Money::new(
        requested.amount().min(remaining_cash.amount()).max(0.0),
        remaining_cash.currency(),
    )?;
    let mut unallocated = budget;
    let allocations = allocate_by_class(staged, &mut unallocated, |s| {
        if target.is_some_and(|id| id != s.instrument_id) {
            return 0.0;
        }
        s.principal.get_prepayment_capacity().amount()
    })?;
    *remaining_cash = remaining_cash.checked_sub(budget.checked_sub(unallocated)?)?;
    for (s, allocated) in staged.iter_mut().zip(allocations) {
        let payment = Money::new(allocated, remaining_cash.currency())?;
        s.principal.pay_prepayment(payment)?;
        s.breakdown.principal_payment = s.breakdown.principal_payment.checked_add(payment)?;
    }
    Ok(())
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
    category: CapitalStructureClaimCategory,
    warnings: &mut Vec<EvalWarning>,
    mut field: F,
) -> Result<()>
where
    F: FnMut(&mut StagedInstrumentFlow) -> &mut Money,
{
    for s in staged.iter_mut() {
        let amount = field(s).amount();
        if amount < 0.0 {
            warnings.push(EvalWarning::CapitalStructure {
                period: period_id,
                warning: CapitalStructureWarning::NegativeClaimNeutralized {
                    category,
                    instrument_id: s.instrument_id.clone(),
                    amount,
                },
            });
            let currency = field(s).currency();
            *field(s) = Money::from((0_i64, currency));
        }
    }
    let mut ranks: Vec<u32> = staged.iter().map(|s| s.class_rank).collect();
    ranks.sort_unstable();
    ranks.dedup();
    for rank in ranks {
        let planned: Vec<f64> = staged
            .iter_mut()
            .map(|s| {
                if s.class_rank == rank {
                    field(s).amount().max(0.0)
                } else {
                    0.0
                }
            })
            .collect();
        let allocations = allocate_pro_rata(&planned, remaining_cash)?;
        for (s, allocated) in staged.iter_mut().zip(allocations) {
            if s.class_rank != rank {
                continue;
            }
            let currency = field(s).currency();
            *field(s) = Money::new(allocated, currency)?;
        }
    }
    Ok(())
}

/// Distribute `remaining_cash` proportionally across `planned` amounts.
///
/// If enough cash exists to fund all planned amounts, each is paid in
/// full. Otherwise, each entry receives its pro-rata share, with any
/// residual rounding error assigned to the last entry to preserve the
/// total exactly.
pub(super) fn allocate_pro_rata(planned: &[f64], remaining_cash: &mut Money) -> Result<Vec<f64>> {
    let total_planned: f64 = planned.iter().sum();
    if total_planned <= 0.0 || remaining_cash.amount() <= 0.0 {
        return Ok(vec![0.0; planned.len()]);
    }
    if remaining_cash.amount() >= total_planned {
        *remaining_cash = Money::new(
            remaining_cash.amount() - total_planned,
            remaining_cash.currency(),
        )?;
        return Ok(planned.to_vec());
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
    *remaining_cash = Money::from((0_i64, remaining_cash.currency()));
    Ok(allocations)
}

/// Allocate `remaining` across staged rows by class rank, then pro-rata
/// within each class using `planned`.
pub(super) fn allocate_by_class(
    staged: &[StagedInstrumentFlow],
    remaining: &mut Money,
    planned: impl Fn(&StagedInstrumentFlow) -> f64,
) -> Result<Vec<f64>> {
    let mut allocations = vec![0.0; staged.len()];
    let mut ranks: Vec<u32> = staged.iter().map(|s| s.class_rank).collect();
    ranks.sort_unstable();
    ranks.dedup();
    for rank in ranks {
        let class_planned: Vec<f64> = staged
            .iter()
            .map(|s| {
                if s.class_rank == rank {
                    planned(s).max(0.0)
                } else {
                    0.0
                }
            })
            .collect();
        let class_alloc = allocate_pro_rata(&class_planned, remaining)?;
        for (idx, amount) in class_alloc.into_iter().enumerate() {
            if staged[idx].class_rank == rank {
                allocations[idx] = amount;
            }
        }
    }
    Ok(allocations)
}
