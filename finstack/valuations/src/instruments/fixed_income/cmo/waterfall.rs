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
    /// Unpaid interest this period (coupon demand minus allocated interest).
    ///
    /// Non-zero when the collateral interest is insufficient to cover the
    /// tranche's coupon (interest-deficient structures).
    pub interest_shortfall: f64,
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
        1.0,
        pac_context,
    )
}

/// Execute waterfall while preserving scheduled-principal vs prepayment buckets.
///
/// `collateral_factor` is the collateral pool factor (current/original
/// balance) for this period; IO strip notionals amortize with it.
pub fn execute_waterfall_with_principal_breakdown(
    waterfall: &mut CmoWaterfall,
    scheduled_principal: f64,
    prepayment_principal: f64,
    available_interest: f64,
    collateral_factor: f64,
    pac_context: Option<&PacContext>,
) -> WaterfallPeriodResult {
    let mut remaining_principal = scheduled_principal + prepayment_principal;
    let mut remaining_interest = available_interest;

    // First pass: distribute interest to interest-bearing tranches.
    // Iterate in ascending `priority` order (lower priority value = paid
    // first) so that on an interest shortfall senior tranches are paid
    // before juniors, matching the principal pass which sorts by priority.
    // Use a stable sort so tranches sharing a priority keep insertion order.
    // Interest is always capped at what the collateral delivered
    // (`remaining_interest`); any unmet coupon demand is recorded as a
    // per-tranche shortfall.
    let mut interest_allocations: HashMap<String, f64> = HashMap::default();
    let mut interest_shortfalls: HashMap<String, f64> = HashMap::default();

    let mut interest_order: Vec<&CmoTranche> = waterfall.tranches.iter().collect();
    interest_order.sort_by_key(|t| t.priority);

    for tranche in interest_order {
        if !tranche.is_interest_bearing() {
            continue;
        }
        // An IO strip's notional is not a principal balance — it amortizes
        // with the collateral factor (the IO references a slice of the pool's
        // interest, which shrinks as the pool pays down).
        let notional = if tranche.tranche_type == CmoTrancheType::InterestOnly {
            tranche.original_face.amount() * collateral_factor.clamp(0.0, 1.0)
        } else {
            tranche.current_face.amount()
        };
        if notional <= 0.0 {
            continue;
        }
        // Interest = notional × coupon / 12
        let monthly_interest = notional * tranche.coupon / 12.0;
        let allocated_interest = monthly_interest.min(remaining_interest);
        remaining_interest -= allocated_interest;
        interest_allocations.insert(tranche.id.clone(), allocated_interest);
        interest_shortfalls.insert(
            tranche.id.clone(),
            (monthly_interest - allocated_interest).max(0.0),
        );
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

    // Broken-structure sweep (finding 16): when the regular allocation leaves
    // principal undistributed while tranches are still outstanding (e.g. a
    // broken PAC whose supports are exhausted, leaving the PAC capped at its
    // schedule), the excess accelerates the remaining tranches beyond their
    // schedules in priority order, balance-capped. Principal is conserved:
    // `residual_principal` is non-zero only when every principal-receiving
    // tranche is fully retired.
    if remaining_principal > 1e-12 {
        let mut sweep_order: Vec<&CmoTranche> = waterfall
            .tranches
            .iter()
            .filter(|t| t.receives_principal())
            .collect();
        sweep_order.sort_by_key(|t| t.priority);
        for tranche in sweep_order {
            if remaining_principal <= 1e-12 {
                break;
            }
            let already = principal_allocations
                .get(&tranche.id)
                .copied()
                .unwrap_or(0.0);
            let capacity = (tranche.current_face.amount() - already).max(0.0);
            let extra = capacity.min(remaining_principal);
            if extra > 0.0 {
                *principal_allocations
                    .entry(tranche.id.clone())
                    .or_insert(0.0) += extra;
                remaining_principal -= extra;
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
        let interest_shortfall = interest_shortfalls.get(&tranche.id).cloned().unwrap_or(0.0);
        let (scheduled_principal, prepayment_principal) = scheduled_attribution
            .get(&tranche.id)
            .cloned()
            .unwrap_or((0.0, 0.0));

        let beginning = tranche.current_face.amount();
        // IO strips receive no principal: their notional amortizes with the
        // collateral factor instead of via principal payments.
        let ending = if tranche.tranche_type == CmoTrancheType::InterestOnly {
            tranche.original_face.amount() * collateral_factor.clamp(0.0, 1.0)
        } else {
            (beginning - principal).max(0.0)
        };

        tranche.current_face = Money::new(ending, tranche.current_face.currency());

        allocations.push(TrancheAllocation {
            tranche_id: tranche.id.clone(),
            principal,
            scheduled_principal,
            prepayment_principal,
            interest,
            interest_shortfall,
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

    /// Finding 17: when collateral interest cannot cover the tranche coupons,
    /// the unmet demand is reported per tranche as `interest_shortfall`
    /// (juniors short first, matching the priority-ordered interest pass).
    #[test]
    fn interest_shortfall_reported_per_tranche() {
        let mut waterfall = create_test_waterfall();

        // Coupon demand: A 40,000×4% + B 30,000×5% + C 30,000×6% = 4,900/yr
        // ≈ 408.33/mo. Deliver only 200 of interest.
        let result = execute_waterfall(&mut waterfall, 0.0, 200.0);

        let total_shortfall: f64 = result
            .allocations
            .iter()
            .map(|a| a.interest_shortfall)
            .sum();
        let total_interest: f64 = result.allocations.iter().map(|a| a.interest).sum();
        assert!((total_interest - 200.0).abs() < 1e-9);
        assert!(
            (total_shortfall - (4_900.0 / 12.0 - 200.0)).abs() < 1e-6,
            "total shortfall must equal unmet coupon demand, got {total_shortfall}"
        );

        // Senior A is paid in full; the most junior C bears the shortfall.
        let a = result
            .allocations
            .iter()
            .find(|x| x.tranche_id == "A")
            .expect("A allocation");
        let c = result
            .allocations
            .iter()
            .find(|x| x.tranche_id == "C")
            .expect("C allocation");
        assert!(a.interest_shortfall < 1e-9, "senior tranche paid first");
        assert!(c.interest_shortfall > 0.0, "junior tranche bears shortfall");
    }

    /// Finding 17: in the waterfall interest pass an IO strip accrues on its
    /// factor-adjusted notional and its reported balance amortizes with the
    /// collateral factor, not with (nonexistent) principal payments.
    #[test]
    fn io_interest_and_balance_use_collateral_factor() {
        let tranches = vec![
            CmoTranche::sequential("A", Money::new(100_000.0, Currency::USD), 0.04, 1),
            CmoTranche::io_strip("IO", Money::new(100_000.0, Currency::USD), 0.04),
        ];
        let mut waterfall = CmoWaterfall::new(tranches);

        // Pool at factor 0.5: IO accrues on 50,000, not 100,000.
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            1_000.0,
            0.0,
            1_000.0,
            0.5,
            None,
        );

        let io = result
            .allocations
            .iter()
            .find(|x| x.tranche_id == "IO")
            .expect("IO allocation");
        // 100,000 × 0.5 × 0.04 / 12 = 166.67
        assert!(
            (io.interest - 100_000.0 * 0.5 * 0.04 / 12.0).abs() < 1e-6,
            "IO interest must accrue on factor-adjusted notional, got {}",
            io.interest
        );
        assert!(
            (io.ending_balance - 50_000.0).abs() < 1e-9,
            "IO balance must amortize with the collateral factor, got {}",
            io.ending_balance
        );
        assert!(io.principal.abs() < 1e-12, "IO receives no principal");
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

    /// Test 1: Order-permutation guard.
    ///
    /// The bug fixed by commit 8a3b125ef was ORDER-DEPENDENCE: the old code
    /// drained a single `remaining_scheduled_principal` counter in
    /// `priority_order` iteration, so the tranche visited first consumed as
    /// much scheduled principal as possible, leaving none for later tranches.
    /// The fix attributes scheduled/prepayment at source (pro-rata, not
    /// sequential drain), making the breakdown independent of insertion order.
    ///
    /// This test builds the same PAC + SR structure in both orders and asserts
    /// that every tranche's `scheduled_principal` / `prepayment_principal`
    /// breakdown is identical regardless of insertion order.
    ///
    /// # Why this would fail on the parent's logic
    ///
    /// In the parent, `priority_order` was built from the tranche `Vec` index,
    /// so "SR first" and "PAC first" produced different iteration sequences.
    /// Whichever tranche was visited first received `principal.min(remaining)`
    /// of scheduled principal.  In the `[SR, PAC]` order SR (which receives
    /// 5 000 from the support allocation) drained 5 000 from a pool of
    /// 6 000 scheduled, leaving only 1 000 for the PAC — but the PAC
    /// genuinely received 4 000 scheduled principal.  In the `[PAC, SR]`
    /// order the PAC was first, so it correctly claimed 4 000 scheduled.
    /// The two orderings therefore produced different `scheduled_principal`
    /// values for both SR and PAC, and this test would have caught that.
    #[test]
    fn test_order_permutation_scheduled_prepayment_identical() {
        use crate::instruments::fixed_income::cmo::tranches::pac_support::PacSchedule;
        use crate::instruments::fixed_income::cmo::types::PacCollar;

        let pac_schedule = PacSchedule {
            scheduled_payments: vec![4_000.0; 12],
            collar: PacCollar::standard(),
        };
        let pac_context = PacContext {
            schedule: Some(pac_schedule),
            period_index: 0,
            actual_psa: 2.0, // within the 100-300 collar
        };

        // Waterfall A: [SR, PAC]
        let tranches_a = vec![
            CmoTranche::sequential("SR", Money::new(50_000.0, Currency::USD), 0.04, 1),
            CmoTranche::pac(
                "PAC",
                Money::new(50_000.0, Currency::USD),
                0.04,
                1,
                PacCollar::standard(),
            ),
        ];
        let mut waterfall_a = CmoWaterfall::new(tranches_a);
        let result_a = execute_waterfall_with_principal_breakdown(
            &mut waterfall_a,
            6_000.0,
            3_000.0,
            5_000.0,
            1.0,
            Some(&pac_context),
        );

        // Waterfall B: [PAC, SR]  — reversed insertion order
        let tranches_b = vec![
            CmoTranche::pac(
                "PAC",
                Money::new(50_000.0, Currency::USD),
                0.04,
                1,
                PacCollar::standard(),
            ),
            CmoTranche::sequential("SR", Money::new(50_000.0, Currency::USD), 0.04, 1),
        ];
        let mut waterfall_b = CmoWaterfall::new(tranches_b);
        let result_b = execute_waterfall_with_principal_breakdown(
            &mut waterfall_b,
            6_000.0,
            3_000.0,
            5_000.0,
            1.0,
            Some(&pac_context),
        );

        // Helper to extract (scheduled, prepayment) for a given tranche ID.
        let find = |res: &WaterfallPeriodResult, id: &str| {
            res.allocations
                .iter()
                .find(|a| a.tranche_id == id)
                .map(|a| (a.scheduled_principal, a.prepayment_principal))
                .expect("tranche not found")
        };

        let (pac_sched_a, pac_prepay_a) = find(&result_a, "PAC");
        let (pac_sched_b, pac_prepay_b) = find(&result_b, "PAC");
        let (sr_sched_a, sr_prepay_a) = find(&result_a, "SR");
        let (sr_sched_b, sr_prepay_b) = find(&result_b, "SR");

        const TOL: f64 = 1e-9;
        assert!(
            (pac_sched_a - pac_sched_b).abs() < TOL,
            "PAC scheduled_principal differs by order: order=[SR,PAC] gave {pac_sched_a}, \
             order=[PAC,SR] gave {pac_sched_b}"
        );
        assert!(
            (pac_prepay_a - pac_prepay_b).abs() < TOL,
            "PAC prepayment_principal differs by order: order=[SR,PAC] gave {pac_prepay_a}, \
             order=[PAC,SR] gave {pac_prepay_b}"
        );
        assert!(
            (sr_sched_a - sr_sched_b).abs() < TOL,
            "SR scheduled_principal differs by order: order=[SR,PAC] gave {sr_sched_a}, \
             order=[PAC,SR] gave {sr_sched_b}"
        );
        assert!(
            (sr_prepay_a - sr_prepay_b).abs() < TOL,
            "SR prepayment_principal differs by order: order=[SR,PAC] gave {sr_prepay_a}, \
             order=[PAC,SR] gave {sr_prepay_b}"
        );

        // The per-tranche totals should also agree with the pool buckets.
        assert!(
            (result_a.total_scheduled_principal - 6_000.0).abs() < 1e-9,
            "total scheduled should equal pool's 6,000, got {}",
            result_a.total_scheduled_principal
        );
        assert!(
            (result_a.total_prepayment_principal - 3_000.0).abs() < 1e-9,
            "total prepayment should equal pool's 3,000, got {}",
            result_a.total_prepayment_principal
        );
    }

    /// Test 2: Residual-conservation assertion.
    ///
    /// When tranches are balance-capped so that some principal goes
    /// undistributed (residual > 0), the per-tranche breakdown must still
    /// conserve the distributed amounts: `total_scheduled_principal +
    /// total_prepayment_principal == total_principal_actually_distributed`.
    /// Additionally each total must be ≤ the corresponding pool bucket
    /// (scheduled and prepayment shrink pro-rata on a residual).
    #[test]
    fn test_residual_conservation_scheduled_prepayment() {
        // Two sequential tranches with small balances so most principal is
        // left as residual.
        let tranches = vec![
            CmoTranche::sequential("A", Money::new(1_000.0, Currency::USD), 0.04, 1),
            CmoTranche::sequential("B", Money::new(500.0, Currency::USD), 0.04, 2),
        ];
        let mut waterfall = CmoWaterfall::new(tranches);

        // Pool delivers 6,000 scheduled + 4,000 prepayment = 10,000 total,
        // but the tranches can only absorb 1,500 (A's 1,000 + B's 500).
        let scheduled_pool = 6_000.0_f64;
        let prepayment_pool = 4_000.0_f64;
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            scheduled_pool,
            prepayment_pool,
            0.0, // interest irrelevant here
            1.0,
            None,
        );

        // Residual must be positive (most principal undistributed).
        assert!(
            result.residual_principal > 0.0,
            "expected residual principal > 0, got {}",
            result.residual_principal
        );

        let total_distributed = result.total_principal;
        let tol = 1e-9;

        // Conservation: scheduled + prepayment == total distributed.
        assert!(
            (result.total_scheduled_principal + result.total_prepayment_principal
                - total_distributed)
                .abs()
                < tol,
            "scheduled {} + prepayment {} != distributed {total_distributed}",
            result.total_scheduled_principal,
            result.total_prepayment_principal
        );

        // Each bucket must not exceed the pool amount.
        assert!(
            result.total_scheduled_principal <= scheduled_pool + tol,
            "distributed scheduled {} exceeds pool scheduled {scheduled_pool}",
            result.total_scheduled_principal
        );
        assert!(
            result.total_prepayment_principal <= prepayment_pool + tol,
            "distributed prepayment {} exceeds pool prepayment {prepayment_pool}",
            result.total_prepayment_principal
        );

        // Both buckets are strictly positive (since 1,500 < 10,000 total the
        // distribution is non-trivial in both dimensions).
        assert!(
            result.total_scheduled_principal > 0.0,
            "expected some scheduled principal distributed"
        );
        assert!(
            result.total_prepayment_principal > 0.0,
            "expected some prepayment principal distributed"
        );
    }

    /// Test 3: PAC excess branch — allocation above PAC schedule is prepayment.
    ///
    /// `attribute_scheduled_principal` contains an explicit excess branch:
    /// when a PAC tranche's total principal allocation exceeds the PAC
    /// schedule amount (`total_pac_alloc > pac_scheduled`), the excess is
    /// prepayment-eligible rather than scheduled.  This branch is exercised
    /// when the PAC receives principal via the balance-limited fallback path
    /// (no amortization schedule in the PAC context), which means
    /// `pac_scheduled = 0`.  In that case the entire PAC allocation is
    /// "excess" and must be split pro-rata from the remaining-scheduled +
    /// prepayment pool — none of it is automatically labeled scheduled.
    ///
    /// Structure: PAC (priority 1) alone — no support tranche.  A
    /// `PacContext` with `schedule = None` is provided so the waterfall uses
    /// the fallback allocation (gives the PAC its balance-limited amount) and
    /// the attribution sees `pac_scheduled = 0.0`, triggering the excess
    /// path for the full allocation.
    ///
    /// Expected labeling: because `pac_scheduled_claimed = 0`, ALL of the
    /// PAC's allocation is prepayment-eligible and receives a pro-rata share
    /// of both the pool scheduled and prepayment buckets.  The PAC's
    /// `scheduled_principal` equals `scheduled_pool` (it is the only
    /// eligible tranche) and `prepayment_principal` equals `prepayment_pool`,
    /// with both pool buckets fully conserved.
    #[test]
    fn test_pac_excess_over_schedule_labeled_prepayment() {
        // No schedule → pac_scheduled = 0 in attribution → full allocation is
        // excess → entire allocation is prepayment-eligible.
        let pac_context = PacContext {
            schedule: None, // triggers balance-limited fallback allocation
            period_index: 0,
            actual_psa: 5.0,
        };

        let tranches = vec![CmoTranche::pac(
            "PAC",
            Money::new(50_000.0, Currency::USD),
            0.04,
            1,
            crate::instruments::fixed_income::cmo::types::PacCollar::standard(),
        )];
        let mut waterfall = CmoWaterfall::new(tranches);

        // Pool: 4,000 scheduled + 8,000 prepayment = 12,000 total.
        // Fallback allocation: PAC receives 12,000 (balance >> available).
        // Attribution: pac_scheduled = 0 → PAC excess = 12,000 → entire
        // 12,000 splits pro-rata from (4,000 scheduled + 8,000 prepay).
        // Since the PAC is the only eligible tranche (fraction = 1.0):
        //   PAC.scheduled_principal = 4,000
        //   PAC.prepayment_principal = 8,000
        let scheduled_pool = 4_000.0_f64;
        let prepayment_pool = 8_000.0_f64;
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            scheduled_pool,
            prepayment_pool,
            0.0,
            1.0,
            Some(&pac_context),
        );

        let pac = result
            .allocations
            .iter()
            .find(|a| a.tranche_id == "PAC")
            .expect("PAC allocation");

        // Fallback path: PAC receives the full 12,000.
        assert!(
            (pac.principal - 12_000.0).abs() < 1.0,
            "PAC should receive all 12,000 via fallback, got {}",
            pac.principal
        );

        // With pac_scheduled = 0, entire allocation is excess → the split
        // takes from both pool buckets pro-rata.  The PAC is the sole eligible
        // tranche so it absorbs the full scheduled pool (4,000).
        assert!(
            (pac.scheduled_principal - 4_000.0).abs() < 1.0,
            "PAC.scheduled_principal should equal pool scheduled 4,000 (excess path, \
             single eligible tranche), got {}",
            pac.scheduled_principal
        );

        // And the full prepayment pool (8,000) goes to the PAC as well.
        assert!(
            (pac.prepayment_principal - 8_000.0).abs() < 1.0,
            "PAC.prepayment_principal should equal pool prepayment 8,000 (excess path, \
             single eligible tranche), got {}",
            pac.prepayment_principal
        );

        // Both pool buckets conserved.
        let tol = 1e-9;
        assert!(
            (result.total_scheduled_principal + result.total_prepayment_principal - 12_000.0).abs()
                < tol,
            "scheduled {} + prepayment {} should equal 12,000",
            result.total_scheduled_principal,
            result.total_prepayment_principal
        );
    }

    /// Finding 16 (broken PAC): once the support tranche is exhausted, excess
    /// principal must accelerate the PAC beyond its schedule (balance-capped)
    /// instead of being stranded as `residual_principal` while tranches are
    /// still outstanding — principal must be conserved.
    #[test]
    fn broken_pac_excess_principal_accelerates_pac() {
        use crate::instruments::fixed_income::cmo::tranches::pac_support::PacSchedule;
        use crate::instruments::fixed_income::cmo::types::PacCollar;

        // PAC 50,000 + a nearly-depleted support of 2,000 at the same priority.
        let tranches = vec![
            CmoTranche::pac(
                "PAC",
                Money::new(50_000.0, Currency::USD),
                0.04,
                1,
                PacCollar::standard(),
            ),
            CmoTranche::sequential("SUP", Money::new(2_000.0, Currency::USD), 0.05, 1),
        ];
        let mut waterfall = CmoWaterfall::new(tranches);

        let pac_schedule = PacSchedule {
            scheduled_payments: vec![4_000.0; 12],
            collar: PacCollar::standard(),
        };
        let pac_context = PacContext {
            schedule: Some(pac_schedule),
            period_index: 0,
            actual_psa: 5.0, // fast prepay, above the 100-300 collar
        };

        // Fast-prepay pool: 4,000 scheduled + 6,000 prepayment = 10,000 total.
        // PAC schedule claims 4,000; support absorbs its full 2,000 balance
        // and is exhausted; the remaining 4,000 must go to the PAC.
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            4_000.0,
            6_000.0,
            1_000.0,
            1.0,
            Some(&pac_context),
        );

        let pac = result
            .allocations
            .iter()
            .find(|a| a.tranche_id == "PAC")
            .expect("PAC allocation");
        let sup = result
            .allocations
            .iter()
            .find(|a| a.tranche_id == "SUP")
            .expect("SUP allocation");

        assert!(
            (sup.principal - 2_000.0).abs() < 1e-9,
            "support must be fully depleted, got {}",
            sup.principal
        );
        assert!(
            (pac.principal - 8_000.0).abs() < 1e-9,
            "broken PAC must absorb the excess beyond schedule (4,000 + 4,000), got {}",
            pac.principal
        );
        // Conservation: every dollar of pool principal reaches a tranche.
        assert!(
            (result.total_principal - 10_000.0).abs() < 1e-9,
            "tranche principal must equal collateral principal, got {}",
            result.total_principal
        );
        assert!(
            result.residual_principal.abs() < 1e-9,
            "no residual principal while tranches are outstanding, got {}",
            result.residual_principal
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
            1.0,
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
