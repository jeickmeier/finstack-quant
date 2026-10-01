//! Principal claims whose cash and economic dates can cross reporting periods.

use super::CashflowBreakdown;
use crate::error::{Error, Result};
use finstack_quant_core::dates::{Date, Period};
use finstack_quant_core::money::Money;

/// A scheduled principal cash claim with its independent economic date.
///
/// Unpaid claims keep these dates when carried to another reporting period,
/// so an advance payment cannot create debt before its principal movement and
/// a delayed settlement cannot reduce outstanding a second time.
#[derive(Debug, Clone, PartialEq)]
pub struct PrincipalClaim {
    /// Date on which this principal cash payment is contractually due.
    pub payment_date: Date,
    /// Date on which the contractual outstanding principal is reduced.
    pub balance_date: Date,
    /// Nonnegative principal cash owed, in the instrument's native currency.
    pub amount: Money,
}

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
            if claim.amount.amount() < 0.0 {
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

struct AllocatedClaim {
    claim: PrincipalClaim,
    /// A newly due settlement whose principal already left an earlier period.
    settlement_only: bool,
}

/// Two principal capacities: debt that can be prepaid and settlements of
/// principal already removed from an earlier period's outstanding balance.
pub(crate) struct PrincipalAllocation {
    economic: Money,
    settlements: Money,
    future_paid: Money,
    advances: Vec<PrincipalClaim>,
    end: Date,
    claims: Vec<AllocatedClaim>,
}

impl PrincipalAllocation {
    pub(crate) fn new(
        period: Option<&PeriodPrincipalFlows>,
        opening: Money,
        funding: Money,
        contractual: &CashflowBreakdown,
        mut carried: Vec<PrincipalClaim>,
        carried_advances: Vec<PrincipalClaim>,
    ) -> Result<Self> {
        let currency = opening.currency();
        let zero = Money::from((0_i64, currency));
        let mut economic;
        let mut settlements = zero;
        let mut future_paid = zero;
        let mut advances = Vec::new();
        let mut claims = Vec::new();
        let end;
        carried.sort_by_key(|claim| (claim.payment_date, claim.balance_date));
        for claim in &carried_advances {
            if claim.amount.currency() != currency {
                return Err(Error::currency_mismatch(currency, claim.amount.currency()));
            }
            if claim.amount.amount() < 0.0 {
                return Err(Error::capital_structure(
                    "Principal advances cannot be negative",
                ));
            }
        }
        if let Some(period) = period {
            end = period.end;
            economic = contractual.debt_balance;
            // Cash already settled against a future principal date cannot
            // be prepaid again while its economic notional remains visible.
            for claim in carried_advances {
                if claim.balance_date >= period.end {
                    economic = economic.checked_sub(claim.amount)?;
                    future_paid = future_paid.checked_add(claim.amount)?;
                    advances.push(claim);
                }
            }
            // Old effective arrears are already part of opening and the
            // residual path. Only this period's newly effective carried
            // principal needs to be restored before allocating its cash.
            for claim in &carried {
                if claim.balance_date >= period.start && claim.balance_date < period.end {
                    economic = economic.checked_add(claim.amount)?;
                }
            }
            for claim in carried {
                claims.push(AllocatedClaim {
                    claim,
                    settlement_only: false,
                });
            }
            let mut current = period.claims.clone();
            current.sort_by_key(|claim| (claim.payment_date, claim.balance_date));
            let mut current_total = zero;
            for claim in current {
                current_total = current_total.checked_add(claim.amount)?;
                let settlement_only = claim.balance_date < period.start;
                if settlement_only {
                    settlements = settlements.checked_add(claim.amount)?;
                } else if claim.balance_date < period.end {
                    economic = economic.checked_add(claim.amount)?;
                }
                claims.push(AllocatedClaim {
                    claim,
                    settlement_only,
                });
            }
            if (current_total.amount() - contractual.principal_payment.amount()).abs() > 1e-6 {
                return Err(Error::capital_structure(
                    "Dated principal claims do not reconcile to contractual cash principal",
                ));
            }
        } else {
            // Aggregate-only callers supply no timing distinction: all cash
            // and principal movements belong to this allocation period. The
            // market-aware runtime always supplies the actual dated claims.
            end = Date::MAX;
            if !carried_advances.is_empty() {
                return Err(Error::capital_structure(
                    "Dated principal advances require exact period principal flows",
                ));
            }
            economic = opening
                .checked_add(funding)?
                .checked_add(contractual.interest_expense_pik)?;
            for claim in carried {
                claims.push(AllocatedClaim {
                    claim,
                    settlement_only: false,
                });
            }
            claims.push(AllocatedClaim {
                claim: PrincipalClaim {
                    payment_date: Date::MIN,
                    balance_date: Date::MIN,
                    amount: contractual.principal_payment,
                },
                settlement_only: false,
            });
        }
        for claim in &claims {
            if claim.claim.amount.currency() != currency {
                return Err(Error::currency_mismatch(
                    currency,
                    claim.claim.amount.currency(),
                ));
            }
            if claim.claim.amount.amount() < 0.0 {
                return Err(Error::capital_structure(
                    "Principal cash claims cannot be negative",
                ));
            }
        }
        if economic.currency() != currency || settlements.currency() != currency {
            return Err(Error::capital_structure(
                "Principal allocation currencies must match the instrument",
            ));
        }
        if economic.amount() < -1e-6 {
            return Err(Error::capital_structure(
                "Economic principal cannot be negative before allocation",
            ));
        }
        Ok(Self {
            economic: Money::new(economic.amount().max(0.0), currency)?,
            settlements,
            future_paid,
            advances,
            end,
            claims,
        })
    }

    pub(crate) fn get_prepayment_capacity(&self) -> Money {
        self.economic
    }

    pub(crate) fn pay_prepayment(&mut self, amount: Money) -> Result<()> {
        validate_payment(amount, self.economic)?;
        self.economic = self.economic.checked_sub(amount)?;
        Ok(())
    }

    pub(crate) fn get_scheduled_capacity(&self) -> Result<Money> {
        let mut total = Money::from((0_i64, self.economic.currency()));
        for claim in self.remaining_claims()? {
            total = total.checked_add(claim.amount)?;
        }
        Ok(total)
    }

    pub(crate) fn pay_scheduled(&mut self, amount: Money) -> Result<()> {
        validate_payment(amount, self.get_scheduled_capacity()?)?;
        let mut budget = amount;
        let mut economic_capacity = self.economic;
        let mut settlement_capacity = self.settlements;
        for allocated in &mut self.claims {
            let claim_capacity = if allocated.settlement_only {
                &mut settlement_capacity
            } else {
                &mut economic_capacity
            };
            let owed = Money::new(
                allocated
                    .claim
                    .amount
                    .amount()
                    .min(claim_capacity.amount())
                    .max(0.0),
                amount.currency(),
            )?;
            *claim_capacity = claim_capacity.checked_sub(owed)?;
            let paid = Money::new(owed.amount().min(budget.amount()), amount.currency())?;
            allocated.claim.amount = owed.checked_sub(paid)?;
            budget = budget.checked_sub(paid)?;
            if allocated.settlement_only {
                self.settlements = self.settlements.checked_sub(paid)?;
            } else {
                self.economic = self.economic.checked_sub(paid)?;
                if allocated.claim.balance_date >= self.end {
                    self.future_paid = self.future_paid.checked_add(paid)?;
                    if paid.amount() > 0.0 {
                        self.advances.push(PrincipalClaim {
                            amount: paid,
                            ..allocated.claim.clone()
                        });
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<(Money, Vec<PrincipalClaim>, Vec<PrincipalClaim>)> {
        let remaining = self.remaining_claims()?;
        let closing = self
            .economic
            .checked_add(self.settlements)?
            .checked_add(self.future_paid)?;
        Ok((closing, remaining, self.advances))
    }

    fn remaining_claims(&self) -> Result<Vec<PrincipalClaim>> {
        let mut economic = self.economic;
        let mut settlements = self.settlements;
        let mut remaining = Vec::new();
        for allocated in &self.claims {
            let capacity = if allocated.settlement_only {
                &mut settlements
            } else {
                &mut economic
            };
            let amount = Money::new(
                allocated
                    .claim
                    .amount
                    .amount()
                    .min(capacity.amount())
                    .max(0.0),
                capacity.currency(),
            )?;
            *capacity = capacity.checked_sub(amount)?;
            if amount.amount() > 0.0 {
                remaining.push(PrincipalClaim {
                    amount,
                    ..allocated.claim.clone()
                });
            }
        }
        Ok(remaining)
    }
}

fn validate_payment(amount: Money, capacity: Money) -> Result<()> {
    if amount.currency() != capacity.currency() {
        return Err(Error::currency_mismatch(
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
