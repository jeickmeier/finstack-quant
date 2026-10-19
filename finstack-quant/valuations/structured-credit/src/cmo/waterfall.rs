//! CMO waterfall engine.
//!
//! This module implements the waterfall logic for distributing collateral
//! cashflows to CMO tranches according to their priority and type.

use super::types::{CmoTranche, CmoTrancheType, CmoWaterfall};
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

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
    /// tranche's coupon (interest-deficient structures). For an accrual (Z)
    /// tranche during the accretion phase this is the unfunded portion of the
    /// period accrual (which is not capitalized).
    pub interest_shortfall: f64,
    /// Interest capitalized into the balance this period.
    ///
    /// Non-zero only for accrual (Z) tranches during the accretion phase: the
    /// funded accrual is added to the tranche balance and an equal amount of
    /// interest collections is redirected as accretion-directed principal.
    pub accreted_interest: f64,
    /// Beginning balance
    pub beginning_balance: f64,
    /// Ending balance
    pub ending_balance: f64,
}

/// Waterfall execution result for a single period.
#[derive(Debug, Clone)]
pub struct CmoWaterfallPeriodResult {
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

/// Execute waterfall while preserving scheduled-principal vs prepayment buckets.
///
/// `collateral_survival` is the fraction of the collateral balance that
/// survives this period (`ending / beginning`); IO strip notionals amortize
/// with it.
///
/// # Arguments
///
/// * `waterfall` - Mutable CMO waterfall; tranche balances and payment history
///   are updated in place.
/// * `scheduled_principal` - Level-pay collateral principal collected this
///   period before voluntary prepayments.
/// * `prepayment_principal` - Voluntary/prepaid collateral principal collected
///   this period.
/// * `available_interest` - Total collateral interest available this period.
/// * `collateral_survival` - Collateral ending/beginning balance ratio for
///   this period, in `[0, 1]`. An IO strip accrues on its current notional
///   and its notional is multiplied by this ratio at period end.
/// * `accrual_fraction` - Year fraction of this period's accrual month on the
///   collateral's day count (exactly 1/12 on 30/360). Tranche coupons and Z
///   accretion accrue over it, like the collateral interest that funds them.
/// * `payment_date` - Actual contractual payment date used to select each
///   PAC tranche's original principal target. Targets are never regenerated.
///
/// # Accrual (Z) tranches
///
/// While any current-pay (non-accrual) principal tranche is outstanding at the
/// start of the period ("accretion phase"), an accrual tranche receives no
/// cash. Its period accrual (`balance × coupon × accrual_fraction`) is funded from the
/// interest collections remaining after the current-pay coupons are paid,
/// capitalized into its balance, and an equal amount is redirected as
/// accretion-directed principal to the current-pay tranches in the normal
/// principal order. If remaining interest cannot fully fund the accrual, only
/// the funded portion is capitalized and the unfunded remainder is reported as
/// `interest_shortfall` on the accrual tranche. Once every current-pay tranche
/// is retired, the accrual tranche becomes current-pay: it receives cash
/// interest on its accreted balance plus principal until retired.
///
/// Accretion-directed principal is classified as *scheduled* principal in the
/// scheduled/prepayment breakdown: it is a deterministic, coupon-driven amount
/// (independent of prepayment behavior), so it joins the pool scheduled bucket
/// before the pro-rata source attribution.
///
/// Reference: Fabozzi, *The Handbook of Mortgage-Backed Securities* (7th
/// ed.), Ch. 21, accrual (Z) bonds in sequential-pay CMO structures.
pub fn execute_waterfall_with_principal_breakdown(
    waterfall: &mut CmoWaterfall,
    scheduled_principal: f64,
    prepayment_principal: f64,
    available_interest: f64,
    collateral_survival: f64,
    accrual_fraction: f64,
    payment_date: Date,
) -> finstack_quant_core::Result<CmoWaterfallPeriodResult> {
    let tranches = &waterfall.tranches;
    for tranche in tranches
        .iter()
        .filter(|tranche| tranche.tranche_type == CmoTrancheType::Pac)
    {
        tranche
            .pac_schedule
            .as_ref()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "PAC tranche '{}' requires its original dated principal schedule",
                    tranche.id
                ))
            })?
            .validate()?;
    }
    let n = tranches.len();
    let mut remaining_principal = scheduled_principal + prepayment_principal;
    let mut remaining_interest = available_interest;

    // Per-tranche buckets indexed like `waterfall.tranches`.
    let mut interest_allocations = vec![0.0_f64; n];
    let mut interest_shortfalls = vec![0.0_f64; n];
    let mut accretion_amounts = vec![0.0_f64; n];
    let mut principal_allocations = vec![0.0_f64; n];

    // Tranche indices in ascending `priority` (lower value = paid first).
    // A stable sort keeps tranches sharing a priority in insertion order.
    let mut by_priority: Vec<usize> = (0..n).collect();
    by_priority.sort_by_key(|&i| tranches[i].priority);

    // Accretion phase: determined at period start — an accrual (Z) tranche
    // capitalizes interest while any current-pay (non-accrual) principal
    // tranche is still outstanding.
    let accretion_phase = tranches.iter().any(|t| {
        t.receives_principal()
            && t.tranche_type != CmoTrancheType::Accrual
            && t.current_face.amount() > 0.0
    });

    // First pass: distribute interest to interest-bearing tranches in
    // priority order, so that on an interest shortfall senior tranches are
    // paid before juniors. Interest is always capped at what the collateral
    // delivered (`remaining_interest`); any unmet coupon demand is recorded
    // as a per-tranche shortfall.
    for &i in &by_priority {
        let tranche = &tranches[i];
        if !tranche.is_interest_bearing() {
            continue;
        }
        // Accrual (Z) tranches receive no cash interest during the accretion
        // phase; their accrual is capitalized after this loop.
        if accretion_phase && tranche.tranche_type == CmoTrancheType::Accrual {
            continue;
        }
        // An IO strip's current notional is not a principal balance: it
        // references a slice of the pool's interest and amortizes with the
        // collateral at period end (see the output loop).
        let notional = tranche.current_face.amount();
        if notional <= 0.0 {
            continue;
        }
        // Interest accrues over the collateral's accrual fraction.
        let monthly_interest = notional * tranche.coupon * accrual_fraction;
        let allocated_interest = monthly_interest.min(remaining_interest);
        remaining_interest -= allocated_interest;
        interest_allocations[i] = allocated_interest;
        interest_shortfalls[i] = (monthly_interest - allocated_interest).max(0.0);
    }

    // Accretion pass: fund each accrual (Z) tranche's period accrual from the
    // interest remaining after the current-pay coupons. The funded accrual is
    // capitalized into the Z balance (applied in the output loop below) and an
    // equal cash amount joins the principal pool as accretion-directed
    // principal — interest collections redirected, not new money, so per-period
    // cash is conserved. An unfunded remainder is reported as
    // `interest_shortfall` and is NOT capitalized (accrete only what is
    // funded).
    let mut accretion_directed = 0.0;
    if accretion_phase {
        for &i in &by_priority {
            let tranche = &tranches[i];
            if tranche.tranche_type != CmoTrancheType::Accrual {
                continue;
            }
            let balance = tranche.current_face.amount();
            if balance <= 0.0 || tranche.coupon <= 0.0 {
                continue;
            }
            let accrual = balance * tranche.coupon * accrual_fraction;
            let funded = accrual.min(remaining_interest);
            remaining_interest -= funded;
            accretion_directed += funded;
            accretion_amounts[i] = funded;
            interest_shortfalls[i] = (accrual - funded).max(0.0);
        }
        remaining_principal += accretion_directed;
    }

    // Second pass: distribute principal by priority group. During the
    // accretion phase accrual (Z) tranches are locked out of principal — they
    // only receive principal via the sweep below once every current-pay
    // tranche is retired. PO strips are NOT senior to all other classes: a PO
    // pays down at its own priority position like any other principal
    // tranche.
    let mut group_start = 0;
    let mut group: Vec<usize> = Vec::with_capacity(n);
    while group_start < n && remaining_principal > 0.0 {
        let priority = tranches[by_priority[group_start]].priority;
        let mut group_end = group_start;
        group.clear();
        while group_end < n && tranches[by_priority[group_end]].priority == priority {
            let i = by_priority[group_end];
            let tranche = &tranches[i];
            if tranche.receives_principal()
                && !(accretion_phase && tranche.tranche_type == CmoTrancheType::Accrual)
            {
                group.push(i);
            }
            group_end += 1;
        }
        group_start = group_end;
        if group.is_empty() {
            continue;
        }
        for (i, amount) in allocate_principal_to_group(
            tranches,
            &group,
            remaining_principal,
            waterfall.pro_rata_same_priority,
            payment_date,
        ) {
            remaining_principal -= amount;
            principal_allocations[i] = amount;
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
        // Accrual (Z) tranches sort behind every current-pay tranche: they
        // absorb leftover principal only after all current-pay balances are
        // exhausted (e.g. the transition period in which the last senior
        // retires). A Z's capacity includes the accretion capitalized this
        // period, which has not yet been applied to `current_face`.
        let mut sweep_order: Vec<usize> = (0..n)
            .filter(|&i| tranches[i].receives_principal())
            .collect();
        sweep_order.sort_by_key(|&i| {
            (
                tranches[i].tranche_type == CmoTrancheType::Accrual,
                tranches[i].priority,
            )
        });
        for i in sweep_order {
            if remaining_principal <= 1e-12 {
                break;
            }
            let capacity = (tranches[i].current_face.amount() + accretion_amounts[i]
                - principal_allocations[i])
                .max(0.0);
            let extra = capacity.min(remaining_principal);
            if extra > 0.0 {
                principal_allocations[i] += extra;
                remaining_principal -= extra;
            }
        }
    }

    // Classify contractual scheduled principal before allocating excess.
    // PAC targets define scheduled payments at the tranche level. A PAC tranche's
    // collar allocation is scheduled principal by construction (capped by
    // the PAC schedule); any excess above the PAC schedule is prepayment.
    // The remaining pool scheduled/prepayment is then split pro-rata across
    // every other tranche's allocation, which is order-independent and
    // conserves total principal. PACs can transform collateral prepayments
    // into scheduled tranche principal.
    // Accretion-directed principal joins the scheduled bucket: it is a
    // deterministic coupon-driven redirection of interest collections, not a
    // function of prepayment behavior (see function docs).
    let scheduled_attribution = attribute_scheduled_principal(
        tranches,
        &principal_allocations,
        scheduled_principal + accretion_directed,
        prepayment_principal,
        payment_date,
    );

    let mut total_principal = 0.0;
    let mut total_scheduled_principal = 0.0;
    let mut total_prepayment_principal = 0.0;
    let mut total_interest = 0.0;
    let mut allocations = Vec::with_capacity(n);

    // Output in deterministic priority order.
    for &i in &by_priority {
        let tranche = &mut waterfall.tranches[i];
        let principal = principal_allocations[i];
        let interest = interest_allocations[i];
        let (scheduled_principal, prepayment_principal) = scheduled_attribution[i];
        let accreted_interest = accretion_amounts[i];

        let beginning = tranche.current_face.amount();
        // IO strips receive no principal: their notional amortizes with the
        // collateral instead of via principal payments. An accrual (Z)
        // tranche's balance grows by the funded accrual capitalized this
        // period before any principal paydown.
        let ending = if tranche.tranche_type == CmoTrancheType::InterestOnly {
            beginning * collateral_survival.clamp(0.0, 1.0)
        } else {
            (beginning + accreted_interest - principal).max(0.0)
        };

        tranche.current_face = Money::new(ending, tranche.current_face.currency())?;

        allocations.push(TrancheAllocation {
            tranche_id: tranche.id.clone(),
            principal,
            scheduled_principal,
            prepayment_principal,
            interest,
            interest_shortfall: interest_shortfalls[i],
            accreted_interest,
            beginning_balance: beginning,
            ending_balance: ending,
        });

        total_principal += principal;
        total_scheduled_principal += scheduled_principal;
        total_prepayment_principal += prepayment_principal;
        total_interest += interest;
    }

    Ok(CmoWaterfallPeriodResult {
        allocations,
        total_principal,
        total_scheduled_principal,
        total_prepayment_principal,
        total_interest,
        residual_principal: remaining_principal,
        residual_interest: remaining_interest,
    })
}

/// Allocate principal to one priority group (tranche indices in insertion
/// order); returns `(tranche index, amount)` pairs.
fn allocate_principal_to_group(
    tranches: &[CmoTranche],
    group: &[usize],
    available: f64,
    pro_rata: bool,
    payment_date: Date,
) -> Vec<(usize, f64)> {
    let mut allocations = Vec::with_capacity(group.len());
    let mut remaining = available;

    // Separate PAC from others
    let (pac_tranches, other_tranches): (Vec<usize>, Vec<usize>) = group
        .iter()
        .partition(|&&i| tranches[i].tranche_type == CmoTrancheType::Pac);
    let balance = |i: usize| tranches[i].current_face.amount();

    // Each PAC claims its own dated target, capped by its live balance.
    // If the group is short, targets share the shortfall proportionally.
    // With no PAC in this group, ordinary pro-rata/sequential rules below
    // apply unchanged even if a junior group contains a PAC.
    let targets: Vec<_> = pac_tranches
        .iter()
        .map(|&i| {
            let target = tranches[i]
                .pac_schedule
                .as_ref()
                .map_or(0.0, |schedule| schedule.get_payment(payment_date));
            (i, balance(i).min(target).max(0.0))
        })
        .collect();
    let requested: f64 = targets.iter().map(|(_, amount)| amount).sum();
    let scale = if requested > 0.0 {
        (remaining / requested).min(1.0)
    } else {
        0.0
    };
    for (i, target) in targets {
        let paid = target * scale;
        allocations.push((i, paid));
        remaining -= paid;
    }

    // Support tranches absorb excess/shortfall
    // For sequential without PAC, just go in order
    if pro_rata {
        let mut to_allocate = remaining;
        let mut tranche_totals: Vec<f64> = vec![0.0; other_tranches.len()];
        let mut active: Vec<(usize, f64)> = other_tranches
            .iter()
            .enumerate()
            .map(|(k, &i)| (k, balance(i)))
            .filter(|(_, b)| *b > 0.0)
            .collect();

        while to_allocate > 1e-12 && !active.is_empty() {
            let total_balance: f64 = active.iter().map(|(_, b)| b).sum();
            let mut next = Vec::new();
            let mut round_alloc = 0.0;
            for &(k, bal) in &active {
                let share = (to_allocate * bal / total_balance).min(bal);
                tranche_totals[k] += share;
                round_alloc += share;
                let rem = bal - share;
                if rem > 1e-12 {
                    next.push((k, rem));
                }
            }
            to_allocate -= round_alloc;
            active = next;
        }

        for (k, &total) in tranche_totals.iter().enumerate() {
            if total > 0.0 {
                allocations.push((other_tranches[k], total));
            }
        }
    } else {
        // Sequential allocation (first tranche gets everything first)
        for &i in &other_tranches {
            if remaining <= 0.0 {
                break;
            }
            let bal = balance(i);
            if bal <= 0.0 {
                continue;
            }
            let allocated = bal.min(remaining);
            allocations.push((i, allocated));
            remaining -= allocated;
        }
    }

    allocations
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
/// Returns `(scheduled, prepayment)` per tranche index, whose sums equal
/// the distributed scheduled and prepayment principal.
fn attribute_scheduled_principal(
    tranches: &[CmoTranche],
    principal_allocations: &[f64],
    scheduled_principal: f64,
    prepayment_principal: f64,
    payment_date: Date,
) -> Vec<(f64, f64)> {
    let mut result = vec![(0.0_f64, 0.0_f64); tranches.len()];
    let is_pac = |i: usize| tranches[i].tranche_type == CmoTrancheType::Pac;

    let claims: Vec<_> = tranches
        .iter()
        .enumerate()
        .map(|(i, tranche)| {
            if is_pac(i) {
                tranche.pac_schedule.as_ref().map_or(0.0, |schedule| {
                    schedule
                        .get_payment(payment_date)
                        .min(principal_allocations[i])
                })
            } else {
                0.0
            }
        })
        .collect();
    let requested: f64 = claims.iter().sum();
    // PAC principal up to its contractual target is scheduled at the tranche
    // level, even when collateral prepayments funded that target.
    let pac_scheduled_claimed = requested;
    for (i, claim) in claims.into_iter().enumerate() {
        result[i].0 = claim;
    }

    // Remaining pool scheduled principal after PAC claims, plus the full
    // prepayment pool, are split pro-rata across all prepayment-eligible
    // amounts: every non-PAC allocation and any PAC excess over schedule.
    let remaining_scheduled = (scheduled_principal - pac_scheduled_claimed).max(0.0);
    let prepay_pool = prepayment_principal.max(0.0);

    // Prepayment-eligible amount per tranche index.
    let eligible: Vec<(usize, f64)> = (0..tranches.len())
        .filter(|&i| principal_allocations[i] > 0.0)
        .map(|i| {
            let alloc = principal_allocations[i];
            // PAC excess over its scheduled portion is prepayment-eligible.
            let amount = if is_pac(i) {
                (alloc - result[i].0).max(0.0)
            } else {
                alloc
            };
            (i, amount)
        })
        .filter(|&(_, amount)| amount > 0.0)
        .collect();

    let total_eligible: f64 = eligible.iter().map(|(_, a)| a).sum();
    if total_eligible <= 0.0 {
        return result;
    }

    // Pro-rata split of (remaining_scheduled, prepay_pool) over eligible
    // amounts. When the eligible total is below the pool total (residual
    // principal), both buckets shrink proportionally.
    let split_total = remaining_scheduled + prepay_pool;
    if split_total > 0.0 {
        for (i, amount) in eligible {
            let frac = amount / split_total;
            result[i].0 += remaining_scheduled * frac;
            result[i].1 += prepay_pool * frac;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmo::types::CmoTranche;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DateExt;
    const PAYMENT: Date = time::macros::date!(2024 - 02 - 25);
    fn pac_plan(amount: f64) -> super::super::tranches::pac_support::PacSchedule {
        use super::super::tranches::pac_support::{PacPayment, PacSchedule};
        PacSchedule {
            payments: (0..12)
                .map(|month| PacPayment {
                    date: PAYMENT.add_months(month).expect("payment date"),
                    principal: amount,
                })
                .collect(),
            collar: super::super::types::PacCollar::standard(),
        }
    }

    fn create_test_waterfall() -> CmoWaterfall {
        let tranches = vec![
            CmoTranche::sequential("A", Money::from((40_000_i64, Currency::USD)), 0.04, 1),
            CmoTranche::sequential("B", Money::from((30_000_i64, Currency::USD)), 0.05, 2),
            CmoTranche::sequential("C", Money::from((30_000_i64, Currency::USD)), 0.06, 3),
        ];

        CmoWaterfall::new(tranches)
    }

    #[test]
    fn test_sequential_waterfall() {
        let mut waterfall = create_test_waterfall();

        // Distribute 10,000 principal, enough interest
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            10_000.0,
            0.0,
            500.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid waterfall fixture");

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
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            50_000.0,
            0.0,
            500.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid waterfall fixture");

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
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            1_000.0,
            0.0,
            500.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid waterfall fixture");

        // Each tranche should get monthly interest based on balance × coupon / 12
        let a_alloc = result
            .allocations
            .iter()
            .find(|a| a.tranche_id == "A")
            .expect("A tranche allocation not found");

        // A: 40,000 × 0.04 / 12 = 133.33
        assert!(a_alloc.interest > 100.0 && a_alloc.interest < 200.0);
    }

    /// Finding 17: when collateral interest cannot cover the tranche coupons,
    /// the unmet demand is reported per tranche as `interest_shortfall`
    /// (juniors short first, matching the priority-ordered interest pass).
    #[test]
    fn interest_shortfall_reported_per_tranche() {
        let mut waterfall = create_test_waterfall();

        // Coupon demand: A 40,000×4% + B 30,000×5% + C 30,000×6% = 4,900/yr
        // ≈ 408.33/mo. Deliver only 200 of interest.
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            0.0,
            0.0,
            200.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid waterfall fixture");

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

    /// An IO strip accrues on its *current* notional and its notional
    /// amortizes by the collateral survival ratio, not with (nonexistent)
    /// principal payments. The IO here was issued at 100,000 and is now at
    /// 70,000 (collateral factor 0.7): interest = 70,000 × 4% / 12 = 233.33.
    /// Half the collateral pays down this period, so the IO ends at 35,000.
    #[test]
    fn io_accrues_on_current_notional_and_amortizes_with_collateral() {
        let mut io = CmoTranche::io_strip("IO", Money::from((100_000_i64, Currency::USD)), 0.04);
        io.current_face = Money::from((70_000_i64, Currency::USD));
        let tranches = vec![
            CmoTranche::sequential("A", Money::from((70_000_i64, Currency::USD)), 0.04, 1),
            io,
        ];
        let mut waterfall = CmoWaterfall::new(tranches);

        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            1_000.0,
            0.0,
            1_000.0,
            0.5,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid execute_waterfall_with_principal_breakdown fixture");

        let io = result
            .allocations
            .iter()
            .find(|x| x.tranche_id == "IO")
            .expect("IO allocation");
        assert!(
            (io.interest - 70_000.0 * 0.04 / 12.0).abs() < 1e-9,
            "IO interest must accrue on its current notional, got {}",
            io.interest
        );
        assert!(
            (io.ending_balance - 35_000.0).abs() < 1e-9,
            "IO balance must amortize with the collateral, got {}",
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
            CmoTranche::sequential("A", Money::from((40_000_i64, Currency::USD)), 0.04, 1),
            CmoTranche::sequential("B", Money::from((30_000_i64, Currency::USD)), 0.05, 2),
            CmoTranche::sequential("C", Money::from((30_000_i64, Currency::USD)), 0.06, 3),
            // PO strip referencing the junior slice (priority 2, alongside B).
            CmoTranche {
                priority: 2,
                ..CmoTranche::po_strip("PO", Money::from((30_000_i64, Currency::USD)))
            },
        ];
        let mut waterfall = CmoWaterfall::new(tranches);

        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            10_000.0,
            0.0,
            5_000.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid waterfall fixture");

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
        // Waterfall A: [SR, PAC]
        let tranches_a = vec![
            CmoTranche::sequential("SR", Money::from((50_000_i64, Currency::USD)), 0.04, 1),
            CmoTranche::pac(
                "PAC",
                Money::from((50_000_i64, Currency::USD)),
                0.04,
                1,
                pac_plan(4_000.0),
            ),
        ];
        let mut waterfall_a = CmoWaterfall::new(tranches_a);
        let result_a = execute_waterfall_with_principal_breakdown(
            &mut waterfall_a,
            6_000.0,
            3_000.0,
            5_000.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid execute_waterfall_with_principal_breakdown fixture");

        // Waterfall B: [PAC, SR]  — reversed insertion order
        let tranches_b = vec![
            CmoTranche::pac(
                "PAC",
                Money::from((50_000_i64, Currency::USD)),
                0.04,
                1,
                pac_plan(4_000.0),
            ),
            CmoTranche::sequential("SR", Money::from((50_000_i64, Currency::USD)), 0.04, 1),
        ];
        let mut waterfall_b = CmoWaterfall::new(tranches_b);
        let result_b = execute_waterfall_with_principal_breakdown(
            &mut waterfall_b,
            6_000.0,
            3_000.0,
            5_000.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid execute_waterfall_with_principal_breakdown fixture");

        let find = |res: &CmoWaterfallPeriodResult, id: &str| {
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

    /// Hand-computed accrual (Z) pin: A (current-pay) + Z over two periods.
    ///
    /// Structure: A 100,000 @ 6% (priority 1), Z 60,000 @ 6% accrual
    /// (priority 2). Monthly coupon rate = 0.06 / 12 = 0.005.
    ///
    /// Period 1 (scheduled 1,000, prepay 0, interest 900):
    ///   A cash interest  = 100,000 × 0.005 = 500
    ///   Z accrual        =  60,000 × 0.005 = 300 (funded: 400 remains ≥ 300)
    ///   principal pool   = 1,000 + 300 (accretion-directed) = 1,300 → all to A
    ///   A ending         = 100,000 − 1,300 = 98,700
    ///   Z ending         =  60,000 + 300   = 60,300 = 60,000 × 1.005
    ///   residual interest = 900 − 500 − 300 = 100
    ///
    /// Period 2 (same collateral cash):
    ///   A cash interest  = 98,700 × 0.005 = 493.50
    ///   Z accrual        = 60,300 × 0.005 = 301.50
    ///   A principal      = 1,000 + 301.50 = 1,301.50 → A ending 97,398.50
    ///   Z ending         = 60,300 + 301.50 = 60,601.50 = 60,000 × 1.005²
    #[test]
    fn accrual_z_pin_hand_computed() {
        let tranches = vec![
            CmoTranche::sequential("A", Money::from((100_000_i64, Currency::USD)), 0.06, 1),
            CmoTranche::accrual("Z", Money::from((60_000_i64, Currency::USD)), 0.06, 2),
        ];
        let mut waterfall = CmoWaterfall::new(tranches);
        let tol = 1e-9;

        // Period 1
        let r1 = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            1_000.0,
            0.0,
            900.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid execute_waterfall_with_principal_breakdown fixture");
        let a1 = r1
            .allocations
            .iter()
            .find(|a| a.tranche_id == "A")
            .expect("A allocation");
        let z1 = r1
            .allocations
            .iter()
            .find(|a| a.tranche_id == "Z")
            .expect("Z allocation");
        assert!(
            (a1.interest - 500.0).abs() < tol,
            "A interest {}",
            a1.interest
        );
        assert!(z1.interest.abs() < tol, "Z receives no cash interest");
        assert!(z1.principal.abs() < tol, "Z receives no principal");
        assert!(
            (z1.accreted_interest - 300.0).abs() < tol,
            "Z accretes 300, got {}",
            z1.accreted_interest
        );
        assert!(
            (a1.principal - 1_300.0).abs() < tol,
            "A principal = 1,000 collateral + 300 accretion-directed, got {}",
            a1.principal
        );
        assert!((a1.ending_balance - 98_700.0).abs() < tol);
        assert!(
            (z1.ending_balance - 60_000.0 * 1.005).abs() < tol,
            "Z balance accretes at (1 + coupon/12), got {}",
            z1.ending_balance
        );
        assert!((r1.residual_interest - 100.0).abs() < tol);

        // Period 2
        let r2 = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            1_000.0,
            0.0,
            900.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid execute_waterfall_with_principal_breakdown fixture");
        let a2 = r2
            .allocations
            .iter()
            .find(|a| a.tranche_id == "A")
            .expect("A allocation");
        let z2 = r2
            .allocations
            .iter()
            .find(|a| a.tranche_id == "Z")
            .expect("Z allocation");
        assert!((a2.interest - 493.50).abs() < tol);
        assert!((z2.accreted_interest - 301.50).abs() < tol);
        assert!((a2.principal - 1_301.50).abs() < tol);
        assert!((a2.ending_balance - 97_398.50).abs() < tol);
        assert!(
            (z2.ending_balance - 60_000.0 * 1.005 * 1.005).abs() < tol,
            "Z balance = 60,000 × 1.005², got {}",
            z2.ending_balance
        );
    }

    /// Transition period: the last current-pay tranche retires and leftover
    /// principal (including accretion-directed cash beyond A's capacity)
    /// flows to the Z via the sweep; the following period the Z is
    /// current-pay and receives cash interest on its accreted balance.
    ///
    /// Period 1: A 1,000 @ 6%, Z 60,000 @ 6%; scheduled 800, interest 400.
    ///   A interest = 1,000 × 0.005 = 5
    ///   Z accrual  = 60,000 × 0.005 = 300 (funded)
    ///   pool       = 800 + 300 = 1,100; A absorbs 1,000; sweep → Z gets 100
    ///   Z ending   = 60,000 + 300 − 100 = 60,200
    ///
    /// Period 2 (A retired → accretion phase over): scheduled 500, interest 310.
    ///   Z cash interest = 60,200 × 0.005 = 301; Z principal = 500
    #[test]
    fn accrual_transition_to_current_pay() {
        let tranches = vec![
            CmoTranche::sequential("A", Money::from((1_000_i64, Currency::USD)), 0.06, 1),
            CmoTranche::accrual("Z", Money::from((60_000_i64, Currency::USD)), 0.06, 2),
        ];
        let mut waterfall = CmoWaterfall::new(tranches);
        let tol = 1e-9;

        let r1 = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            800.0,
            0.0,
            400.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid execute_waterfall_with_principal_breakdown fixture");
        let a1 = r1
            .allocations
            .iter()
            .find(|a| a.tranche_id == "A")
            .expect("A allocation");
        let z1 = r1
            .allocations
            .iter()
            .find(|a| a.tranche_id == "Z")
            .expect("Z allocation");
        assert!((a1.principal - 1_000.0).abs() < tol, "A retires fully");
        assert!(a1.ending_balance.abs() < tol);
        assert!((z1.accreted_interest - 300.0).abs() < tol);
        assert!(
            (z1.principal - 100.0).abs() < tol,
            "leftover principal sweeps to the Z, got {}",
            z1.principal
        );
        assert!((z1.ending_balance - 60_200.0).abs() < tol);
        assert!(
            r1.residual_principal.abs() < tol,
            "no principal stranded, got {}",
            r1.residual_principal
        );

        // Period 2: Z is now current-pay.
        let r2 = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            500.0,
            0.0,
            310.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid execute_waterfall_with_principal_breakdown fixture");
        let z2 = r2
            .allocations
            .iter()
            .find(|a| a.tranche_id == "Z")
            .expect("Z allocation");
        assert!(
            (z2.interest - 301.0).abs() < tol,
            "Z pays cash interest on its accreted balance, got {}",
            z2.interest
        );
        assert!(z2.accreted_interest.abs() < tol, "no further accretion");
        assert!((z2.principal - 500.0).abs() < tol);
        assert!((z2.ending_balance - 59_700.0).abs() < tol);
    }

    /// Interest-short accretion: when remaining interest cannot fully fund
    /// the Z accrual, only the funded portion is capitalized and the
    /// unfunded remainder is reported as `interest_shortfall`.
    ///
    /// A 100,000 @ 6%, Z 60,000 @ 6%; interest 600 only.
    ///   A interest = 500; remaining 100 < Z accrual 300
    ///   Z accretes 100 (funded), shortfall 200; Z ending 60,100
    ///   A receives the 100 as accretion-directed principal.
    #[test]
    fn accrual_shortfall_accretes_only_funded_amount() {
        let tranches = vec![
            CmoTranche::sequential("A", Money::from((100_000_i64, Currency::USD)), 0.06, 1),
            CmoTranche::accrual("Z", Money::from((60_000_i64, Currency::USD)), 0.06, 2),
        ];
        let mut waterfall = CmoWaterfall::new(tranches);
        let tol = 1e-9;

        let r = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            0.0,
            0.0,
            600.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid execute_waterfall_with_principal_breakdown fixture");
        let a = r
            .allocations
            .iter()
            .find(|x| x.tranche_id == "A")
            .expect("A allocation");
        let z = r
            .allocations
            .iter()
            .find(|x| x.tranche_id == "Z")
            .expect("Z allocation");
        assert!((a.interest - 500.0).abs() < tol);
        assert!((z.accreted_interest - 100.0).abs() < tol);
        assert!(
            (z.interest_shortfall - 200.0).abs() < tol,
            "unfunded accrual reported as shortfall, got {}",
            z.interest_shortfall
        );
        assert!(
            (z.ending_balance - 60_100.0).abs() < tol,
            "only funded accrual capitalized"
        );
        assert!(
            (a.principal - 100.0).abs() < tol,
            "funded accretion redirected to A"
        );
        assert!(r.residual_interest.abs() < tol);
    }

    /// Per-period cash conservation with a Z tranche: total cash distributed
    /// to tranches plus residuals equals total collateral cash received. The
    /// accretion-directed principal is not new money — it is interest
    /// collections redirected — so
    /// `Σ tranche interest + Σ tranche principal + residual_interest +
    ///  residual_principal == collateral principal + collateral interest`.
    #[test]
    fn accrual_cash_conservation_per_period() {
        let tranches = vec![
            CmoTranche::sequential("A", Money::from((50_000_i64, Currency::USD)), 0.05, 1),
            CmoTranche::sequential("B", Money::from((30_000_i64, Currency::USD)), 0.055, 2),
            CmoTranche::accrual("Z", Money::from((20_000_i64, Currency::USD)), 0.06, 3),
        ];
        let mut waterfall = CmoWaterfall::new(tranches);

        // Run enough periods to cross the A and B retirements.
        for period in 0..120 {
            let scheduled = 1_500.0;
            let prepay = 500.0;
            let interest = 450.0;
            let r = execute_waterfall_with_principal_breakdown(
                &mut waterfall,
                scheduled,
                prepay,
                interest,
                1.0,
                1.0 / 12.0,
                PAYMENT,
            )
            .expect("valid execute_waterfall_with_principal_breakdown fixture");
            let cash_out: f64 = r.allocations.iter().map(|a| a.interest + a.principal).sum();
            let cash_in = scheduled + prepay + interest;
            assert!(
                (cash_out + r.residual_interest + r.residual_principal - cash_in).abs() < 1e-6,
                "period {period}: cash out {cash_out} + residuals ({}, {}) != cash in {cash_in}",
                r.residual_interest,
                r.residual_principal
            );
            // Scheduled/prepay breakdown stays consistent with the total.
            let breakdown: f64 = r
                .allocations
                .iter()
                .map(|a| a.scheduled_principal + a.prepayment_principal)
                .sum();
            assert!(
                (breakdown - r.total_principal).abs() < 1e-6,
                "period {period}: scheduled+prepay {breakdown} != principal {}",
                r.total_principal
            );
        }
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
            CmoTranche::sequential("A", Money::from((1_000_i64, Currency::USD)), 0.04, 1),
            CmoTranche::sequential("B", Money::from((500_i64, Currency::USD)), 0.04, 2),
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
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid execute_waterfall_with_principal_breakdown fixture");

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
    /// when an explicit zero contractual target receives excess principal
    /// after all support balances have retired. Source attribution still
    /// conserves scheduled and prepaid collateral buckets.
    #[test]
    fn test_pac_excess_over_schedule_labeled_prepayment() {
        // An explicit zero target makes the broken-PAC sweep's allocation
        // excess, funded pro rata from scheduled and prepaid collections.

        let tranches = vec![CmoTranche::pac(
            "PAC",
            Money::from((50_000_i64, Currency::USD)),
            0.04,
            1,
            pac_plan(0.0),
        )];
        let mut waterfall = CmoWaterfall::new(tranches);

        // Pool: 4,000 scheduled + 8,000 prepayment = 12,000 total.
        // Broken-PAC sweep: PAC receives 12,000 (balance >> available).
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
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid execute_waterfall_with_principal_breakdown fixture");

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
        // PAC 50,000 + a nearly-depleted support of 2,000 at the same priority.
        let tranches = vec![
            CmoTranche::pac(
                "PAC",
                Money::from((50_000_i64, Currency::USD)),
                0.04,
                1,
                pac_plan(4_000.0),
            ),
            CmoTranche::sequential("SUP", Money::from((2_000_i64, Currency::USD)), 0.05, 1),
        ];
        let mut waterfall = CmoWaterfall::new(tranches);

        // Fast-prepay pool: 4,000 scheduled + 6,000 prepayment = 10,000 total.
        // PAC schedule claims 4,000; support absorbs its full 2,000 balance
        // and is exhausted; the remaining 4,000 must go to the PAC.
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            4_000.0,
            6_000.0,
            1_000.0,
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid execute_waterfall_with_principal_breakdown fixture");

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
    /// `allocate_principal_to_group`. The PAC's collar allocation is, by
    /// construction, scheduled principal (capped by `pac_scheduled`). Under
    /// the buggy shared counter SR is iterated first and consumes
    /// `remaining_scheduled_principal`, mislabeling the PAC's genuine
    /// scheduled principal as prepayment.
    #[test]
    fn test_pac_scheduled_principal_labeled_scheduled_regardless_of_order() {
        // SR and PAC share priority 1 so the PAC/Support split applies.
        let tranches = vec![
            CmoTranche::sequential("SR", Money::from((50_000_i64, Currency::USD)), 0.04, 1),
            CmoTranche::pac(
                "PAC",
                Money::from((50_000_i64, Currency::USD)),
                0.04,
                1,
                pac_plan(4_000.0),
            ),
        ];
        let mut waterfall = CmoWaterfall::new(tranches);

        // PAC schedule with a known scheduled amount for period 0.

        // Period principal: 6,000 scheduled + 3,000 prepayment = 9,000 total.
        // Within the collar, `allocate_principal_to_group` gives the PAC its
        // scheduled amount (4,000) and the support SR the remaining 5,000.
        // The PAC's 4,000 is, by construction, scheduled principal.
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            6_000.0, // scheduled
            3_000.0, // prepayment
            5_000.0, // interest
            1.0,
            1.0 / 12.0,
            PAYMENT,
        )
        .expect("valid execute_waterfall_with_principal_breakdown fixture");

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

#[cfg(test)]
mod dated_pac_regressions {
    use super::super::tranches::pac_support::{PacPayment, PacSchedule};
    use super::super::PacCollar;
    use super::*;
    use finstack_quant_core::currency::Currency;
    use time::macros::date;

    fn pac(id: &str, principal: f64, priority: u32) -> CmoTranche {
        CmoTranche::pac(
            id,
            Money::from((100_i64, Currency::USD)),
            0.0,
            priority,
            PacSchedule {
                payments: vec![PacPayment {
                    date: date!(2025 - 02 - 25),
                    principal,
                }],
                collar: PacCollar::standard(),
            },
        )
    }

    #[test]
    fn junior_pac_does_not_change_senior_pro_rata() {
        let mut waterfall = CmoWaterfall::new(vec![
            CmoTranche::sequential("A", Money::from((50_i64, Currency::USD)), 0.0, 1),
            CmoTranche::sequential("B", Money::from((50_i64, Currency::USD)), 0.0, 1),
            pac("PAC", 20.0, 2),
        ]);
        waterfall.pro_rata_same_priority = true;
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            10.0,
            0.0,
            0.0,
            1.0,
            1.0 / 12.0,
            date!(2025 - 02 - 25),
        )
        .expect("waterfall");
        assert_eq!(result.allocations[0].principal, 5.0);
        assert_eq!(result.allocations[1].principal, 5.0);
        assert_eq!(result.allocations[2].principal, 0.0);
    }

    #[test]
    fn pacs_with_distinct_targets_get_their_own_contractual_principal() {
        let mut waterfall = CmoWaterfall::new(vec![
            pac("P1", 10.0, 1),
            pac("P2", 30.0, 1),
            CmoTranche::support("SUP", Money::from((100_i64, Currency::USD)), 0.0, 2),
        ]);
        let result = execute_waterfall_with_principal_breakdown(
            &mut waterfall,
            20.0,
            30.0,
            0.0,
            1.0,
            1.0 / 12.0,
            date!(2025 - 02 - 25),
        )
        .expect("waterfall");
        assert_eq!(result.allocations[0].principal, 10.0);
        assert_eq!(result.allocations[1].principal, 30.0);
        assert_eq!(result.allocations[2].principal, 10.0);
        assert!((result.total_scheduled_principal - 40.0).abs() < 1e-12);
        assert!((result.total_prepayment_principal - 10.0).abs() < 1e-12);
        for allocation in &result.allocations {
            assert!(
                (allocation.scheduled_principal + allocation.prepayment_principal
                    - allocation.principal)
                    .abs()
                    < 1e-12
            );
        }
    }
}
