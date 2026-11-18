//! Sweep capacity, pro-rata allocation, available-cash caps, and
//! the [`StagedInstrumentFlow`] working struct.

use crate::capital_structure::cashflows::{CashflowBreakdown, add_optional_money};
use crate::capital_structure::principal::PrincipalAllocation;
use crate::error::Result;
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;

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
    /// Cash coupon moved into the PIK bucket by the PIK toggle this period;
    /// it capitalizes into the closing balance at period close.
    pub toggled_pik_moved: Money,
}

/// Selects the principal component of a [`CashflowBreakdown`]
/// (`scheduled_principal`, `mandatory_prepayment`, `sweep_prepayment` or
/// `voluntary_prepayment`) that a payment rung records its cash in.
pub(super) type PrincipalComponent = fn(&mut CashflowBreakdown) -> &mut Option<Money>;

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
    component: PrincipalComponent,
) -> Result<()> {
    let currency = remaining_cash.currency();
    if requested.amount_decimal().is_sign_negative() && !requested.amount_decimal().is_zero() {
        return Err(crate::error::Error::capital_structure(
            "A prepayment request cannot be negative",
        ));
    }
    // The rung spends the lesser of its request and the cash left.
    let budget = if requested.amount_decimal() < remaining_cash.amount_decimal() {
        requested
    } else {
        *remaining_cash
    };
    let mut unallocated = budget;
    let allocations = allocate_by_class(staged, &mut unallocated, |s| {
        Ok(if target.is_some_and(|id| id != s.instrument_id) {
            Money::from((0_i64, currency))
        } else {
            s.principal.get_prepayment_capacity()
        })
    })?;
    *remaining_cash = remaining_cash.checked_sub(budget.checked_sub(unallocated)?)?;
    for (s, payment) in staged.iter_mut().zip(allocations) {
        s.principal.pay_prepayment(payment)?;
        s.breakdown.principal_payment = s.breakdown.principal_payment.checked_add(payment)?;
        add_optional_money(component(&mut s.breakdown), Some(payment))?;
    }
    Ok(())
}

/// Pay one category (fees, interest) across instruments from the remaining
/// cash, replacing each claim with the amount paid.
///
/// A negative claim in an outflow category is an upstream sign-convention
/// error, not a receivable: it is rejected rather than netted against other
/// instruments' claims.
pub(super) fn apply_cash_cap_to_category<F>(
    staged: &mut [StagedInstrumentFlow],
    remaining_cash: &mut Money,
    category: &str,
    mut field: F,
) -> Result<()>
where
    F: FnMut(&mut StagedInstrumentFlow) -> &mut Money,
{
    let allocations = allocate_by_class(staged, remaining_cash, |s| {
        let claim = *field(s);
        if claim.amount_decimal().is_sign_negative() && !claim.amount_decimal().is_zero() {
            return Err(crate::error::Error::capital_structure(format!(
                "instrument '{}' has a negative {category} claim of {} {}; an outflow claim \
                 cannot be negative",
                s.instrument_id,
                claim.amount(),
                claim.currency()
            )));
        }
        Ok(claim)
    })?;
    for (s, paid) in staged.iter_mut().zip(allocations) {
        *field(s) = paid;
    }
    Ok(())
}

/// The payment quantum of a currency: one minor unit (a cent for USD, one
/// yen for JPY).
fn payment_quantum(currency: finstack_quant_core::currency::Currency) -> Decimal {
    Decimal::new(1, u32::from(currency.decimals()))
}

/// Split `cash` across the claims of one payment class.
///
/// Claims are paid exactly and in full when the cash covers them. Otherwise
/// each claim's pro-rata quota is floored to the payment quantum, and the
/// whole quanta that flooring leaves over go one at a time to the claims
/// with the largest fractional remainder, ties broken by instrument id, never
/// above a claim. Cash that cannot be allocated stays in the pool.
///
/// Returns the amount paid on each claim, in claim order.
fn allocate_class(
    claims: &[(&str, Decimal)],
    cash: Decimal,
    quantum: Decimal,
) -> Result<Vec<Decimal>> {
    let overflow = || crate::error::Error::capital_structure("Waterfall allocation overflowed");
    let mut total = Decimal::ZERO;
    for (_, claim) in claims {
        total = total.checked_add(*claim).ok_or_else(overflow)?;
    }
    if total.is_zero() || cash <= Decimal::ZERO {
        return Ok(vec![Decimal::ZERO; claims.len()]);
    }
    if cash >= total {
        return Ok(claims.iter().map(|(_, claim)| *claim).collect());
    }
    let floor_to_quantum = |amount: Decimal| -> Result<Decimal> {
        (amount.checked_div(quantum).ok_or_else(overflow)?)
            .floor()
            .checked_mul(quantum)
            .ok_or_else(overflow)
    };
    let payable = floor_to_quantum(cash)?;
    let mut paid = Vec::with_capacity(claims.len());
    let mut remainders = Vec::with_capacity(claims.len());
    let mut allocated = Decimal::ZERO;
    for (index, (id, claim)) in claims.iter().enumerate() {
        // Exact when the product is representable; for very large amounts
        // the share is formed first, which is exact to 28 digits.
        let quota = payable
            .checked_mul(*claim)
            .and_then(|product| product.checked_div(total))
            .or_else(|| {
                claim
                    .checked_div(total)
                    .and_then(|share| share.checked_mul(payable))
            })
            .ok_or_else(overflow)?;
        let base = floor_to_quantum(quota)?;
        allocated = allocated.checked_add(base).ok_or_else(overflow)?;
        remainders.push((quota - base, *id, index));
        paid.push(base);
    }
    // Largest fractional remainder first; a stable id breaks ties.
    remainders.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(right.1)));
    if allocated > payable {
        return Err(overflow());
    }
    let mut leftover = payable - allocated;
    let mut progressed = true;
    while leftover >= quantum && progressed {
        progressed = false;
        for (_, _, index) in &remainders {
            if leftover < quantum {
                break;
            }
            if paid[*index] + quantum <= claims[*index].1 {
                paid[*index] += quantum;
                leftover -= quantum;
                progressed = true;
            }
        }
    }
    Ok(paid)
}

/// Allocate `remaining` across staged rows: senior payment classes first,
/// then within each class by [`allocate_class`]. `claim` returns each row's
/// non-negative claim. `remaining` is reduced by exactly the cash allocated.
pub(super) fn allocate_by_class(
    staged: &mut [StagedInstrumentFlow],
    remaining: &mut Money,
    mut claim: impl FnMut(&mut StagedInstrumentFlow) -> Result<Money>,
) -> Result<Vec<Money>> {
    let currency = remaining.currency();
    let quantum = payment_quantum(currency);
    let mut claims = Vec::with_capacity(staged.len());
    for s in staged.iter_mut() {
        let amount = claim(s)?;
        if amount.currency() != currency {
            return Err(crate::error::Error::CurrencyMismatch(
                currency,
                amount.currency(),
            ));
        }
        claims.push(amount.amount_decimal().max(Decimal::ZERO));
    }
    let mut allocations = vec![Money::from((0_i64, currency)); staged.len()];
    let mut ranks: Vec<u32> = staged.iter().map(|s| s.class_rank).collect();
    ranks.sort_unstable();
    ranks.dedup();
    for rank in ranks {
        let members: Vec<usize> = (0..staged.len())
            .filter(|index| staged[*index].class_rank == rank)
            .collect();
        let class_claims: Vec<(&str, Decimal)> = members
            .iter()
            .map(|index| (staged[*index].instrument_id.as_str(), claims[*index]))
            .collect();
        let paid = allocate_class(&class_claims, remaining.amount_decimal(), quantum)?;
        for (index, amount) in members.into_iter().zip(paid) {
            let payment = Money::from_decimal(amount, currency);
            *remaining = remaining.checked_sub(payment)?;
            allocations[index] = payment;
        }
    }
    Ok(allocations)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cents(value: i64) -> Decimal {
        Decimal::new(value, 2)
    }

    /// The record's example: five cents across three equal claims of two
    /// cents go two, two and one, by stable id.
    #[test]
    fn five_cents_across_three_equal_claims_follow_the_largest_remainder_rule() {
        let claims = [("b", cents(2)), ("c", cents(2)), ("a", cents(2))];
        let paid = allocate_class(&claims, cents(5), cents(1)).expect("allocation");
        // Ties go to the lowest ids, whatever the row order.
        assert_eq!(paid, [cents(2), cents(1), cents(2)]);
    }

    #[test]
    fn full_cover_pays_claims_exactly_and_shortfalls_keep_every_cent() {
        let third = Decimal::new(333_333, 6);
        let claims = [("a", third), ("b", cents(10))];
        // Enough cash: each claim is paid exactly, sub-cent part included.
        assert_eq!(
            allocate_class(&claims, cents(100), cents(1)).expect("allocation"),
            [third, cents(10)]
        );
        // Short cash is split in whole cents and never exceeds a claim.
        let paid = allocate_class(&claims, cents(20), cents(1)).expect("allocation");
        assert_eq!(paid.iter().sum::<Decimal>(), cents(20));
        assert!(paid[0] <= third && paid[1] <= cents(10));
        // Cash below one quantum cannot be allocated and stays in the pool.
        let paid = allocate_class(&claims, Decimal::new(4, 3), cents(1)).expect("allocation");
        assert_eq!(paid, [Decimal::ZERO, Decimal::ZERO]);
    }

    /// The row order and the scale of the amounts do not change the total:
    /// every allocatable quantum is paid and none is lost.
    #[test]
    fn allocation_conserves_cash_across_row_orders_and_scales() {
        for scale in [1_i64, 1_000, 1_000_000_000] {
            let claims = [
                ("x", cents(94_892_500 * scale)),
                ("y", cents(14_753_112 * scale)),
                ("z", cents(1)),
            ];
            let cash = cents(99_798_274 * scale);
            let mut reversed = claims;
            reversed.reverse();
            let forward = allocate_class(&claims, cash, cents(1)).expect("allocation");
            let backward = allocate_class(&reversed, cash, cents(1)).expect("allocation");
            assert_eq!(forward.iter().sum::<Decimal>(), cash);
            let mut backward = backward;
            backward.reverse();
            assert_eq!(forward, backward);
        }
    }
}
