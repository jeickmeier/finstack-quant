//! Dated principal claims whose cash and economic dates can cross reporting
//! periods.
//!
//! Outstanding economic principal follows the contract's effective dates,
//! independently of cash settlement. A scheduled principal payment is a
//! separate cash obligation with its own due date: unpaid after its due date
//! it is arrears, and paid before its face reduction takes effect it leaves a
//! reserve that cannot be prepaid again.

use super::CashflowBreakdown;
use crate::error::{Error, Result};
use finstack_quant_core::dates::{Date, Period};
use finstack_quant_core::money::Money;

/// One payment made against a principal claim.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PrincipalPayment {
    /// Modeled payment date: the due date when paid in its own period, the
    /// first day of the paying period when paid late.
    pub(crate) date: Date,
    /// Cash paid, in the instrument's native currency.
    pub(crate) amount: Money,
}

/// A scheduled principal cash claim with its independent economic date.
///
/// The claim keeps its identity, dates and payment history when it is carried
/// to another reporting period, so a delayed settlement cannot reduce
/// outstanding principal a second time and an early one cannot be repaid.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PrincipalClaim {
    /// Stable identity within the instrument: due date, effective date and
    /// ordinal of the schedule row.
    pub(crate) id: String,
    /// Date on which this principal cash payment is contractually due.
    pub(crate) payment_date: Date,
    /// Date on which the contractual outstanding principal is reduced.
    pub(crate) balance_date: Date,
    /// Nonnegative principal cash still owed, in the instrument's native
    /// currency.
    pub(crate) amount: Money,
    /// Nonnegative face reduction the cash still owed settles. It differs
    /// from the cash when principal is redeemed above or below par.
    pub(crate) face: Money,
    /// Payments already made against the claim, oldest first.
    pub(crate) payments: Vec<PrincipalPayment>,
}

impl PrincipalClaim {
    /// An unpaid claim settled at par.
    #[cfg(test)]
    pub(crate) fn at_par(payment_date: Date, balance_date: Date, amount: Money) -> Self {
        Self {
            id: format!("{payment_date}/{balance_date}/0"),
            payment_date,
            balance_date,
            amount,
            face: amount,
            payments: Vec::new(),
        }
    }
}

/// The dated principal cash claims that fall due in one reporting period.
#[derive(Debug, Clone)]
pub(crate) struct PeriodPrincipalFlows {
    pub start: Date,
    pub end: Date,
    pub claims: Vec<PrincipalClaim>,
}

impl PeriodPrincipalFlows {
    pub(crate) fn new(period: &Period, claims: Vec<PrincipalClaim>) -> Result<Self> {
        if period.end <= period.start {
            return Err(Error::capital_structure(
                "Principal claim period must have positive length",
            ));
        }
        for claim in &claims {
            if claim.payment_date < period.start || claim.payment_date >= period.end {
                return Err(Error::capital_structure(
                    "Current principal cash claims must settle inside the model period",
                ));
            }
            if claim.amount.amount() < 0.0 || claim.face.amount() < 0.0 {
                return Err(Error::capital_structure(
                    "Principal cash claims cannot be negative",
                ));
            }
        }
        Ok(Self {
            start: period.start,
            end: period.end,
            claims,
        })
    }
}

/// The principal position of one instrument at a period close.
pub(crate) struct PrincipalClose {
    /// Outstanding economic principal: contractual principal at the close
    /// less prepayments made in the period.
    pub(crate) economic: Money,
    /// Claims due and still unpaid, oldest due first.
    pub(crate) arrears: Vec<PrincipalClaim>,
    /// Paid claims whose face reduction is effective after the period.
    pub(crate) advances: Vec<PrincipalClaim>,
    /// Face of `advances`: principal paid for and reserved until effective.
    pub(crate) reserve: Money,
}

/// The principal ledger of one instrument for one reporting period.
///
/// Economic principal starts from the contractual close of the dated
/// schedule, which already holds every reduction effective by then, and falls
/// only by prepayments. Scheduled claims are cash obligations paid oldest due
/// first; a claim whose face reduction is still in the future is limited to
/// the principal that remains to be reduced.
pub(crate) struct PrincipalAllocation {
    economic: Money,
    reserve: Money,
    advances: Vec<PrincipalClaim>,
    start: Date,
    end: Date,
    claims: Vec<PrincipalClaim>,
}

impl PrincipalAllocation {
    pub(crate) fn new(
        period: &PeriodPrincipalFlows,
        contractual: &CashflowBreakdown,
        mut carried: Vec<PrincipalClaim>,
        carried_advances: Vec<PrincipalClaim>,
    ) -> Result<Self> {
        let economic = contractual.debt_balance;
        let currency = economic.currency();
        let zero = Money::from((0_i64, currency));
        if economic.amount() < 0.0 {
            return Err(Error::capital_structure(
                "Economic principal cannot be negative before allocation",
            ));
        }
        // Face already paid stays in economic principal until its reduction
        // takes effect; until then it cannot be prepaid again. An advance
        // that became effective is in the contractual close and is released.
        let mut reserve = zero;
        let mut advances = Vec::new();
        for claim in carried_advances {
            check(&claim, currency)?;
            if claim.balance_date >= period.end {
                reserve = reserve.checked_add(claim.face)?;
                advances.push(claim);
            }
        }
        carried.sort_by_key(|claim| (claim.payment_date, claim.balance_date));
        let mut current = period.claims.clone();
        current.sort_by_key(|claim| (claim.payment_date, claim.balance_date));
        let mut current_total = zero;
        for claim in &current {
            check(claim, currency)?;
            current_total = current_total.checked_add(claim.amount)?;
        }
        for claim in &carried {
            check(claim, currency)?;
        }
        if current_total != contractual.principal_payment {
            return Err(Error::capital_structure(format!(
                "Dated principal claims ({} {currency}) do not reconcile to contractual cash \
                 principal ({} {currency})",
                current_total.amount(),
                contractual.principal_payment.amount()
            )));
        }
        // Carried claims are older and are paid first.
        carried.extend(current);
        Ok(Self {
            economic,
            reserve,
            advances,
            start: period.start,
            end: period.end,
            claims: carried,
        })
    }

    /// Principal that can still be prepaid: economic principal less the face
    /// already paid for.
    pub(crate) fn get_prepayment_capacity(&self) -> Money {
        let zero = Money::from((0_i64, self.economic.currency()));
        match self.economic.checked_sub(self.reserve) {
            Ok(capacity) if capacity.amount() > 0.0 => capacity,
            _ => zero,
        }
    }

    pub(crate) fn pay_prepayment(&mut self, amount: Money) -> Result<()> {
        validate_payment(amount, self.get_prepayment_capacity())?;
        self.economic = self.economic.checked_sub(amount)?;
        Ok(())
    }

    pub(crate) fn get_scheduled_capacity(&self) -> Result<Money> {
        let mut total = Money::from((0_i64, self.economic.currency()));
        for (_, payable) in self.payable()? {
            total = total.checked_add(payable)?;
        }
        Ok(total)
    }

    pub(crate) fn pay_scheduled(&mut self, amount: Money) -> Result<()> {
        validate_payment(amount, self.get_scheduled_capacity()?)?;
        let mut budget = amount;
        let payable = self.payable()?;
        for (index, payable) in payable {
            let claim = &mut self.claims[index];
            // A future reduction truncated by prepayments is no longer owed.
            let unreachable = claim.amount.checked_sub(payable)?;
            let paid = if budget.amount() < payable.amount() {
                budget
            } else {
                payable
            };
            let face_paid = if paid == claim.amount {
                claim.face
            } else if claim.amount.amount() > 0.0 {
                Money::new(
                    claim.face.amount() * paid.amount() / claim.amount.amount(),
                    paid.currency(),
                )?
            } else {
                Money::from((0_i64, paid.currency()))
            };
            let face_unreachable = if unreachable.amount() > 0.0 {
                Money::new(
                    claim.face.amount() * unreachable.amount() / claim.amount.amount(),
                    paid.currency(),
                )?
            } else {
                Money::from((0_i64, paid.currency()))
            };
            claim.amount = payable.checked_sub(paid)?;
            claim.face = claim
                .face
                .checked_sub(face_paid)?
                .checked_sub(face_unreachable)?;
            budget = budget.checked_sub(paid)?;
            if paid.amount() > 0.0 {
                let payment = PrincipalPayment {
                    date: claim.payment_date.max(self.start),
                    amount: paid,
                };
                claim.payments.push(payment.clone());
                if claim.balance_date >= self.end {
                    self.reserve = self.reserve.checked_add(face_paid)?;
                    self.advances.push(PrincipalClaim {
                        amount: paid,
                        face: face_paid,
                        payments: vec![payment],
                        ..claim.clone()
                    });
                }
            }
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<PrincipalClose> {
        let arrears = self
            .payable()?
            .into_iter()
            .filter(|(_, payable)| payable.amount() > 0.0)
            .map(|(index, payable)| {
                let claim = &self.claims[index];
                let face = if payable == claim.amount {
                    claim.face
                } else {
                    Money::new(
                        claim.face.amount() * payable.amount() / claim.amount.amount(),
                        payable.currency(),
                    )?
                };
                Ok(PrincipalClaim {
                    amount: payable,
                    face,
                    ..claim.clone()
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(PrincipalClose {
            economic: self.economic,
            arrears,
            advances: self.advances,
            reserve: self.reserve,
        })
    }

    /// Cash still payable on each claim, by claim index, oldest due first.
    ///
    /// A claim whose face reduction is already in the contractual close is
    /// owed in full. A claim whose reduction is still in the future is owed
    /// only up to the principal that remains to be reduced after the face
    /// already paid for and the older future claims.
    fn payable(&self) -> Result<Vec<(usize, Money)>> {
        let mut headroom = self.get_prepayment_capacity();
        let mut payable = Vec::with_capacity(self.claims.len());
        for (index, claim) in self.claims.iter().enumerate() {
            let owed = if claim.balance_date < self.end {
                claim.amount
            } else if claim.face.amount() <= headroom.amount() {
                headroom = headroom.checked_sub(claim.face)?;
                claim.amount
            } else {
                let share = if claim.face.amount() > 0.0 {
                    headroom.amount() / claim.face.amount()
                } else {
                    0.0
                };
                headroom = Money::from((0_i64, headroom.currency()));
                Money::new(claim.amount.amount() * share, claim.amount.currency())?
            };
            payable.push((index, owed));
        }
        Ok(payable)
    }
}

fn check(claim: &PrincipalClaim, currency: finstack_quant_core::currency::Currency) -> Result<()> {
    for money in [claim.amount, claim.face] {
        if money.currency() != currency {
            return Err(Error::CurrencyMismatch(currency, money.currency()));
        }
        if money.amount() < 0.0 {
            return Err(Error::capital_structure(
                "Principal cash claims cannot be negative",
            ));
        }
    }
    Ok(())
}

fn validate_payment(amount: Money, capacity: Money) -> Result<()> {
    if amount.currency() != capacity.currency() {
        return Err(Error::CurrencyMismatch(
            capacity.currency(),
            amount.currency(),
        ));
    }
    if amount.amount() < 0.0 || amount.amount() > capacity.amount() + 1e-6 {
        return Err(Error::capital_structure(
            "Principal payment exceeds its available capacity",
        ));
    }
    Ok(())
}
