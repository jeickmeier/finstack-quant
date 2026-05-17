//! CMO waterfall engine.
//!
//! This module implements the waterfall logic for distributing collateral
//! cashflows to CMO tranches according to their priority and type.

use super::tranches::pac_support::{allocate_pac_support, PacSchedule};
use super::types::{CmoTranche, CmoTrancheType, CmoWaterfall};
use finstack_core::money::Money;
use finstack_core::HashMap;

/// Cashflow allocation for a single period.
#[derive(Debug, Clone)]
pub struct TrancheAllocation {
    /// Tranche ID
    pub tranche_id: String,
    /// Principal allocated
    pub principal: f64,
    /// Scheduled principal allocated
    pub scheduled_principal: f64,
    /// Prepayment principal allocated
    pub prepayment_principal: f64,
    /// Interest allocated
    pub interest: f64,
    /// Beginning balance
    pub beginning_balance: f64,
    /// Ending balance
    pub ending_balance: f64,
}

/// Waterfall execution result for a single period.
#[derive(Debug, Clone)]
pub struct WaterfallPeriodResult {
    /// Allocations by tranche
    pub allocations: Vec<TrancheAllocation>,
    /// Total principal distributed
    pub total_principal: f64,
    /// Total scheduled principal distributed
    pub total_scheduled_principal: f64,
    /// Total prepayment principal distributed
    pub total_prepayment_principal: f64,
    /// Total interest distributed
    pub total_interest: f64,
    /// Residual principal (if any)
    pub residual_principal: f64,
    /// Residual interest (if any)
    pub residual_interest: f64,
}

/// Execute waterfall for a single period.
///
/// Distributes principal and interest from collateral to tranches
/// according to waterfall rules.
///
/// # Arguments
///
/// * `waterfall` - Waterfall configuration with tranches
/// * `available_principal` - Total principal available for distribution
/// * `available_interest` - Total interest available for distribution
///
/// # Returns
///
/// Waterfall execution result with allocations by tranche
/// Optional PAC context for waterfall execution.
#[derive(Debug, Clone, Default)]
pub struct PacContext {
    /// PAC schedule for scheduled payment lookup.
    pub schedule: Option<PacSchedule>,
    /// Current period index into the schedule.
    pub period_index: usize,
    /// Actual PSA speed for collar check.
    pub actual_psa: f64,
}

/// Execute waterfall for a single period (convenience entry point).
pub fn execute_waterfall(
    waterfall: &mut CmoWaterfall,
    available_principal: f64,
    available_interest: f64,
) -> WaterfallPeriodResult {
    execute_waterfall_with_pac(waterfall, available_principal, available_interest, None)
}

/// Execute waterfall with optional PAC schedule context.
pub fn execute_waterfall_with_pac(
    waterfall: &mut CmoWaterfall,
    available_principal: f64,
    available_interest: f64,
    pac_context: Option<&PacContext>,
) -> WaterfallPeriodResult {
    execute_waterfall_with_principal_breakdown(
        waterfall,
        available_principal,
        0.0,
        available_interest,
        pac_context,
    )
}

/// Execute waterfall while preserving scheduled-principal vs prepayment buckets.
pub fn execute_waterfall_with_principal_breakdown(
    waterfall: &mut CmoWaterfall,
    scheduled_principal: f64,
    prepayment_principal: f64,
    available_interest: f64,
    pac_context: Option<&PacContext>,
) -> WaterfallPeriodResult {
    let mut remaining_principal = scheduled_principal + prepayment_principal;
    let mut remaining_interest = available_interest;

    // First pass: distribute interest to interest-bearing tranches
    let mut interest_allocations: HashMap<String, f64> = HashMap::default();

    for tranche in &waterfall.tranches {
        if tranche.is_interest_bearing() && tranche.current_face.amount() > 0.0 {
            // Interest = balance × coupon / 12
            let monthly_interest = tranche.current_face.amount() * tranche.coupon / 12.0;
            let allocated_interest = monthly_interest.min(remaining_interest);
            remaining_interest -= allocated_interest;
            interest_allocations.insert(tranche.id.clone(), allocated_interest);
        }
    }

    // Second pass: distribute principal based on tranche type and priority
    // Group tranches by priority
    let mut priority_groups: HashMap<u32, Vec<&CmoTranche>> = HashMap::default();
    for tranche in &waterfall.tranches {
        if tranche.receives_principal() {
            priority_groups
                .entry(tranche.priority)
                .or_default()
                .push(tranche);
        }
    }

    let mut priorities: Vec<u32> = priority_groups.keys().cloned().collect();
    priorities.sort();

    let mut principal_allocations: HashMap<String, f64> = HashMap::default();

    // PO strips are NOT senior to all other classes. A PO strip is a
    // principal strip of a defined collateral slice that pays down at its
    // own priority position, so it flows through the normal priority-group
    // allocation below like any other principal-receiving tranche
    // (`receives_principal()` is true for `PrincipalOnly`).
    for priority in priorities {
        if remaining_principal <= 0.0 {
            break;
        }

        // Priority groups are built from tranches above, so get() always succeeds
        if let Some(tranches) = priority_groups.get(&priority) {
            // Determine allocation mode for this priority group
            let allocation = allocate_principal_to_group(
                tranches,
                remaining_principal,
                waterfall.pro_rata_same_priority,
                pac_context,
            );

            for (id, amount) in allocation {
                remaining_principal -= amount;
                principal_allocations.insert(id, amount);
            }
        }
    }

    // Attribute scheduled vs prepayment principal AT SOURCE rather than by
    // draining a single shared counter in priority order. A PAC tranche's
    // collar allocation is scheduled principal by construction (capped by
    // the PAC schedule); any excess above the PAC schedule is prepayment.
    // The remaining pool scheduled/prepayment is then split pro-rata across
    // every other tranche's allocation, which is order-independent and
    // conserves both pool buckets exactly.
    let scheduled_attribution = attribute_scheduled_principal(
        waterfall,
        &principal_allocations,
        scheduled_principal,
        prepayment_principal,
        pac_context,
    );

    // Iterate tranches in a deterministic priority order for output.
    let mut priority_order: Vec<usize> = (0..waterfall.tranches.len()).collect();
    priority_order.sort_by_key(|&i| waterfall.tranches[i].priority);

    let mut total_principal = 0.0;
    let mut total_scheduled_principal = 0.0;
    let mut total_prepayment_principal = 0.0;
    let mut total_interest = 0.0;
    let mut allocations = Vec::with_capacity(waterfall.tranches.len());

    for &idx in &priority_order {
        let tranche = &mut waterfall.tranches[idx];
        let principal = principal_allocations
            .get(&tranche.id)
            .cloned()
            .unwrap_or(0.0);
        let interest = interest_allocations
            .get(&tranche.id)
            .cloned()
            .unwrap_or(0.0);
        let (scheduled_principal, prepayment_principal) = scheduled_attribution
            .get(&tranche.id)
            .cloned()
            .unwrap_or((0.0, 0.0));

        let beginning = tranche.current_face.amount();
        let ending = (beginning - principal).max(0.0);

        tranche.current_face = Money::new(ending, tranche.current_face.currency());

        allocations.push(TrancheAllocation {
            tranche_id: tranche.id.clone(),
            principal,
            scheduled_principal,
            prepayment_principal,
            interest,
            beginning_balance: beginning,
            ending_balance: ending,
        });

        total_principal += principal;
        total_scheduled_principal += scheduled_principal;
        total_prepayment_principal += prepayment_principal;
        total_interest += interest;
    }

    WaterfallPeriodResult {
        allocations,
        total_principal,
        total_scheduled_principal,
        total_prepayment_principal,
        total_interest,
        residual_principal: remaining_principal,
        residual_interest: remaining_interest,
    }
}

/// Allocate principal to a group of tranches at the same priority.
fn allocate_principal_to_group(
    tranches: &[&CmoTranche],
    available: f64,
    pro_rata: bool,
    pac_context: Option<&PacContext>,
) -> Vec<(String, f64)> {
    let mut allocations = Vec::new();
    let mut remaining = available;

    // Separate PAC from others
    let (pac_tranches, other_tranches): (Vec<&&CmoTranche>, Vec<&&CmoTranche>) = tranches
        .iter()
        .partition(|t| t.tranche_type == CmoTrancheType::Pac);

    // When PAC schedule is available, use proper PAC/Support allocation
    if let Some(ctx) = pac_context {
        if let Some(ref schedule) = ctx.schedule {
            let pac_balance: f64 = pac_tranches.iter().map(|t| t.current_face.amount()).sum();
            let support_balance: f64 = other_tranches.iter().map(|t| t.current_face.amount()).sum();
            let pac_scheduled = schedule.scheduled_at(ctx.period_index);

            let (pac_alloc, support_alloc) = allocate_pac_support(
                remaining,
                pac_balance,
                support_balance,
                pac_scheduled,
                ctx.actual_psa,
                &schedule.collar,
            );

            // Distribute PAC allocation pro-rata among PAC tranches
            if pac_balance > 0.0 && pac_alloc > 0.0 {
                for tranche in &pac_tranches {
                    let proportion = tranche.current_face.amount() / pac_balance;
                    let alloc = pac_alloc * proportion;
                    allocations.push((tranche.id.clone(), alloc));
                }
            }
            // Distribute support allocation among other tranches
            if support_alloc > 0.0 {
                let mut support_remaining = support_alloc;
                for tranche in &other_tranches {
                    if support_remaining <= 0.0 {
                        break;
                    }
                    let balance = tranche.current_face.amount();
                    let alloc = balance.min(support_remaining);
                    allocations.push((tranche.id.clone(), alloc));
                    support_remaining -= alloc;
                }
            }

            return allocations;
        }
    }

    // Fallback: balance-limited allocation when no PAC schedule is available
    for tranche in &pac_tranches {
        if remaining <= 0.0 {
            break;
        }
        let balance = tranche.current_face.amount();
        if balance <= 0.0 {
            continue;
        }
        let allocated = balance.min(remaining);
        allocations.push((tranche.id.clone(), allocated));
        remaining -= allocated;
    }

    // Support tranches absorb excess/shortfall
    // For sequential without PAC, just go in order
    if pro_rata {
        let mut to_allocate = remaining;
        let mut tranche_totals: Vec<f64> = vec![0.0; other_tranches.len()];
        let mut active: Vec<(usize, f64)> = other_tranches
            .iter()
            .enumerate()
            .map(|(i, t)| (i, t.current_face.amount()))
            .filter(|(_, b)| *b > 0.0)
            .collect();

        while to_allocate > 1e-12 && !active.is_empty() {
            let total_balance: f64 = active.iter().map(|(_, b)| b).sum();
            let mut next = Vec::new();
            let mut round_alloc = 0.0;
            for &(i, balance) in &active {
                let share = (to_allocate * balance / total_balance).min(balance);
                tranche_totals[i] += share;
                round_alloc += share;
                let rem = balance - share;
                if rem > 1e-12 {
                    next.push((i, rem));
                }
            }
            to_allocate -= round_alloc;
            active = next;
        }

        for (i, &total) in tranche_totals.iter().enumerate() {
            if total > 0.0 {
                allocations.push((other_tranches[i].id.clone(), total));
            }
        }
    } else {
        // Sequential allocation (first tranche gets everything first)
        for tranche in &other_tranches {
            if remaining <= 0.0 {
                break;
            }
            let balance = tranche.current_face.amount();
            if balance <= 0.0 {
                continue;
            }
            let allocated = balance.min(remaining);
            allocations.push((tranche.id.clone(), allocated));
            remaining -= allocated;
        }
    }

    allocations
}

/// Allocate IO cashflows.
///
/// IO strips receive interest based on their notional and coupon,
/// but their notional decreases as the underlying pool pays down.
pub fn allocate_io_cashflow(io_tranche: &CmoTranche, collateral_factor: f64) -> f64 {
    // IO payment = notional × factor × coupon / 12
    let adjusted_notional = io_tranche.original_face.amount() * collateral_factor;
    adjusted_notional * io_tranche.coupon / 12.0
}

/// Attribute scheduled-vs-prepayment principal to each tranche at source.
///
/// The collateral pool delivers a fixed `scheduled_principal` (level-pay
/// amortization) and `prepayment_principal` (SMM-driven) for the period.
/// This function maps that split onto per-tranche principal allocations
/// without depending on iteration order:
///
/// * A PAC tranche's allocation is scheduled principal up to the PAC
///   schedule amount for the period; any excess (fast-prepay) is
///   prepayment.
/// * The remaining pool scheduled principal (after PAC claims) and the
///   pool prepayment principal are split pro-rata across every other
///   tranche's allocation, and across any PAC excess.
///
/// Returns a map of `tranche_id -> (scheduled, prepayment)` whose sums
/// equal the distributed scheduled and prepayment principal.
fn attribute_scheduled_principal(
    waterfall: &CmoWaterfall,
    principal_allocations: &HashMap<String, f64>,
    scheduled_principal: f64,
    prepayment_principal: f64,
    pac_context: Option<&PacContext>,
) -> HashMap<String, (f64, f64)> {
    let mut result: HashMap<String, (f64, f64)> = HashMap::default();

    // PAC schedule amount for this period (0.0 when no PAC context).
    let pac_scheduled = pac_context
        .and_then(|ctx| {
            ctx.schedule
                .as_ref()
                .map(|s| s.scheduled_at(ctx.period_index))
        })
        .unwrap_or(0.0);

    // Total principal allocated to PAC tranches this period.
    let total_pac_alloc: f64 = waterfall
        .tranches
        .iter()
        .filter(|t| t.tranche_type == CmoTrancheType::Pac)
        .map(|t| principal_allocations.get(&t.id).cloned().unwrap_or(0.0))
        .sum();

    // PACs collectively claim scheduled principal up to the schedule amount.
    let pac_scheduled_claimed = total_pac_alloc.min(pac_scheduled).max(0.0);

    // Per-PAC scheduled portion, pro-rata to PAC allocation. Any PAC
    // allocation above the schedule is prepayment-eligible (handled below).
    for tranche in &waterfall.tranches {
        if tranche.tranche_type != CmoTrancheType::Pac {
            continue;
        }
        let alloc = principal_allocations
            .get(&tranche.id)
            .cloned()
            .unwrap_or(0.0);
        let sched = if total_pac_alloc > 0.0 {
            pac_scheduled_claimed * (alloc / total_pac_alloc)
        } else {
            0.0
        };
        result.insert(tranche.id.clone(), (sched, 0.0));
    }

    // Remaining pool scheduled principal after PAC claims, plus the full
    // prepayment pool, are split pro-rata across all prepayment-eligible
    // amounts: every non-PAC allocation and any PAC excess over schedule.
    let remaining_scheduled = (scheduled_principal - pac_scheduled_claimed).max(0.0);
    let prepay_pool = prepayment_principal.max(0.0);

    // Collect prepayment-eligible amounts: (tranche_id, eligible_amount).
    let mut eligible: Vec<(String, f64)> = Vec::with_capacity(waterfall.tranches.len());
    for tranche in &waterfall.tranches {
        let alloc = principal_allocations
            .get(&tranche.id)
            .cloned()
            .unwrap_or(0.0);
        if alloc <= 0.0 {
            continue;
        }
        if tranche.tranche_type == CmoTrancheType::Pac {
            // PAC excess over its scheduled portion is prepayment-eligible.
            let sched = result.get(&tranche.id).map(|(s, _)| *s).unwrap_or(0.0);
            let excess = (alloc - sched).max(0.0);
            if excess > 0.0 {
                eligible.push((tranche.id.clone(), excess));
            }
        } else {
            eligible.push((tranche.id.clone(), alloc));
        }
    }

    let total_eligible: f64 = eligible.iter().map(|(_, a)| a).sum();
    if total_eligible <= 0.0 {
        return result;
    }

    // Pro-rata split of (remaining_scheduled, prepay_pool) over eligible
    // amounts. When the eligible total is below the pool total (residual
    // principal), both buckets shrink proportionally.
    let split_total = remaining_scheduled + prepay_pool;
    for (id, amount) in eligible {
        let (sched, prepay) = if split_total > 0.0 {
            let frac = amount / split_total;
            (remaining_scheduled * frac, prepay_pool * frac)
        } else {
            (0.0, 0.0)
        };
        let entry = result.entry(id).or_insert((0.0, 0.0));
        entry.0 += sched;
        entry.1 += prepay;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::cmo::types::CmoTranche;
    use finstack_core::currency::Currency;

    fn create_test_waterfall() -> CmoWaterfall {
        let tranches = vec![
            CmoTranche::sequential("A", Money::new(40_000.0, Currency::USD), 0.04, 1),
            CmoTranche::sequential("B", Money::new(30_000.0, Currency::USD), 0.05, 2),
            CmoTranche::sequential("C", Money::new(30_000.0, Currency::USD), 0.06, 3),
        ];

        CmoWaterfall::new(tranches)
    }

    #[test]
    fn test_sequential_waterfall() {
        let mut waterfall = create_test_waterfall();

        // Distribute 10,000 principal, enough interest
        let result = execute_waterfall(&mut waterfall, 10_000.0, 500.0);

        // A should get all principal (it's first priority)
        let a_alloc = result
            .allocations
            .iter()
            .find(|a| a.tranche_id == "A")
            .expect("A tranche allocation not found");
        assert!((a_alloc.principal - 10_000.0).abs() < 1.0);

        // B and C should get nothing yet
        let b_alloc = result
            .allocations
            .iter()
            .find(|a| a.tranche_id == "B")
            .expect("B tranche allocation not found");
        assert!(b_alloc.principal < 1.0);
    }

    #[test]
    fn test_waterfall_payoff_a() {
        let mut waterfall = create_test_waterfall();

        // Distribute enough to pay off A completely plus some to B
        let result = execute_waterfall(&mut waterfall, 50_000.0, 500.0);

        // A should be paid off
        let a_alloc = result
            .allocations
            .iter()
            .find(|a| a.tranche_id == "A")
            .expect("A tranche allocation not found");
        assert!((a_alloc.principal - 40_000.0).abs() < 1.0);
        assert!(a_alloc.ending_balance < 1.0);

        // B should get remaining
        let b_alloc = result
            .allocations
            .iter()
            .find(|a| a.tranche_id == "B")
            .expect("B tranche allocation not found");
        assert!((b_alloc.principal - 10_000.0).abs() < 1.0);
    }

    #[test]
    fn test_interest_allocation() {
        let mut waterfall = create_test_waterfall();

        // Run waterfall with interest
        let result = execute_waterfall(&mut waterfall, 1_000.0, 500.0);

        // Each tranche should get monthly interest based on balance × coupon / 12
        let a_alloc = result
            .allocations
            .iter()
            .find(|a| a.tranche_id == "A")
            .expect("A tranche allocation not found");

        // A: 40,000 × 0.04 / 12 = 133.33
        assert!(a_alloc.interest > 100.0 && a_alloc.interest < 200.0);
    }

    #[test]
    fn test_io_allocation() {
        let io = CmoTranche::io_strip("IO", Money::new(100_000.0, Currency::USD), 0.04);

        // At 100% factor
        let payment = allocate_io_cashflow(&io, 1.0);
        // 100,000 × 0.04 / 12 = 333.33
        assert!((payment - 333.33).abs() < 1.0);

        // At 50% factor
        let payment_half = allocate_io_cashflow(&io, 0.5);
        assert!((payment_half - 166.67).abs() < 1.0);
    }

    /// Defect (a): a PO strip is NOT senior to all other classes. It is a
    /// principal strip that pays down at its own priority position, not a
    /// pre-loop 100% drain of pool principal.
    ///
    /// Structure: sequential A (priority 1), B (priority 2), C (priority 3)
    /// plus a PO strip placed at priority 2 (junior to A, pari passu with B).
    /// Distributing 10,000 — less than A's 40,000 balance — must pay class A
    /// in full and leave the PO with nothing, because the PO is junior to A.
    #[test]
    fn test_po_strip_does_not_starve_senior_tranche() {
        let tranches = vec![
            CmoTranche::sequential("A", Money::new(40_000.0, Currency::USD), 0.04, 1),
            CmoTranche::sequential("B", Money::new(30_000.0, Currency::USD), 0.05, 2),
            CmoTranche::sequential("C", Money::new(30_000.0, Currency::USD), 0.06, 3),
            // PO strip referencing the junior slice (priority 2, alongside B).
            CmoTranche {
                priority: 2,
                ..CmoTranche::po_strip("PO", Money::new(30_000.0, Currency::USD))
            },
        ];
        let mut waterfall = CmoWaterfall::new(tranches);

        let result = execute_waterfall(&mut waterfall, 10_000.0, 5_000.0);

        let a = result
            .allocations
            .iter()
            .find(|x| x.tranche_id == "A")
            .expect("A allocation");
        let po = result
            .allocations
            .iter()
            .find(|x| x.tranche_id == "PO")
            .expect("PO allocation");

        // Class A is senior (priority 1) and must receive the full 10,000.
        assert!(
            (a.principal - 10_000.0).abs() < 1.0,
            "class A should receive all 10,000 of principal, got {}",
            a.principal
        );
        // The PO is junior (priority 2) and must receive nothing this period.
        assert!(
            po.principal < 1.0,
            "PO strip should receive 0 while senior class A is outstanding, got {}",
            po.principal
        );
    }

    /// Defect (b): scheduled-vs-prepayment principal must be attributed per
    /// tranche at source, not by draining one shared counter in priority
    /// order. A PAC tranche that genuinely receives PAC-scheduled principal
    /// must see it labeled `scheduled`, even when another tranche is
    /// iterated first.
    ///
    /// Structure: a senior sequential SR and a PAC sharing priority 1, with
    /// a PAC schedule so the PAC receives its scheduled amount via
    /// `allocate_pac_support`. The PAC's collar allocation is, by
    /// construction, scheduled principal (capped by `pac_scheduled`). Under
    /// the buggy shared counter SR is iterated first and consumes
    /// `remaining_scheduled_principal`, mislabeling the PAC's genuine
    /// scheduled principal as prepayment.
    #[test]
    fn test_pac_scheduled_principal_labeled_scheduled_regardless_of_order() {
        use crate::instruments::fixed_income::cmo::tranches::pac_support::PacSchedule;
        use crate::instruments::fixed_income::cmo::types::PacCollar;

        // SR and PAC share priority 1 so the PAC/Support split applies.
        let tranches = vec![
            CmoTranche::sequential("SR", Money::new(50_000.0, Currency::USD), 0.04, 1),
            CmoTranche::pac(
                "PAC",
                Money::new(50_000.0, Currency::USD),
                0.04,
                1,
                PacCollar::standard(),
            ),
        ];
        let mut waterfall = CmoWaterfall::new(tranches);

        // PAC schedule with a known scheduled amount for period 0.
        let pac_schedule = PacSchedule {
            scheduled_payments: vec![4_000.0; 12],
            collar: PacCollar::standard(),
        };
        let pac_context = PacContext {
            schedule: Some(pac_schedule),
            period_index: 0,
            actual_psa: 2.0, // within the 100-300 collar
        };

        // Period principal: 6,000 scheduled + 3,000 prepayment = 9,000 total.
        // Within the collar, `allocate_pac_support` gives the PAC its
        // scheduled amount (4,000) and the support SR the remaining 5,000.
        // The PAC's 4,000 is, by construction, scheduled principal.
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            6_000.0, // scheduled
            3_000.0, // prepayment
            5_000.0, // interest
            Some(&pac_context),
        );

        let pac = result
            .allocations
            .iter()
            .find(|x| x.tranche_id == "PAC")
            .expect("PAC allocation");

        // PAC receives its scheduled collar allocation of 4,000.
        assert!(
            (pac.principal - 4_000.0).abs() < 1.0,
            "PAC should receive 4,000 of scheduled collar principal, got {}",
            pac.principal
        );
        // The PAC's principal is scheduled principal (its PAC schedule
        // covers it), NOT prepayment. The buggy shared counter labels most
        // of it prepayment because SR consumed `remaining_scheduled_principal`.
        assert!(
            (pac.scheduled_principal - pac.principal).abs() < 1.0,
            "PAC principal {} should be labeled scheduled, but scheduled_principal={} \
             prepayment_principal={}",
            pac.principal,
            pac.scheduled_principal,
            pac.prepayment_principal
        );
        assert!(
            pac.prepayment_principal < 1.0,
            "PAC should have zero prepayment principal, got {}",
            pac.prepayment_principal
        );

        // The split is conserved: scheduled + prepayment across all tranches
        // equals the pool's 6,000 scheduled + 3,000 prepayment. The PAC's
        // 4,000 scheduled leaves 2,000 of pool-scheduled principal for the
        // support SR, whose remaining 3,000 is prepayment.
        let sr = result
            .allocations
            .iter()
            .find(|x| x.tranche_id == "SR")
            .expect("SR allocation");
        assert!(
            (sr.scheduled_principal - 2_000.0).abs() < 1.0,
            "support SR should be labeled 2,000 scheduled, got {}",
            sr.scheduled_principal
        );
        assert!(
            (result.total_scheduled_principal - 6_000.0).abs() < 1.0,
            "total scheduled principal should equal the pool's 6,000, got {}",
            result.total_scheduled_principal
        );
        assert!(
            (result.total_prepayment_principal - 3_000.0).abs() < 1.0,
            "total prepayment principal should equal the pool's 3,000, got {}",
            result.total_prepayment_principal
        );
    }
}
