//! Simulation helpers for structured credit cashflow projection.
//!
//! This module contains internal helpers used by the pricing engine
//! for period-by-period simulation.

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DateExt};
use finstack_core::money::Money;
use std::collections::{HashMap, VecDeque};

/// Recovery queue for delayed recovery processing.
///
/// Recoveries from defaulted assets are typically received 6-12 months after
/// the default event. This queue holds pending recoveries until they can be
/// released based on the configured recovery lag.
#[derive(Debug, Default)]
pub(crate) struct RecoveryQueue {
    /// Queue of pending recoveries: (origination_date, recovery_amount)
    pending: VecDeque<(Date, Money)>,
}

impl RecoveryQueue {
    /// Create a new empty recovery queue.
    pub(crate) fn new() -> Self {
        Self {
            pending: VecDeque::new(),
        }
    }

    /// Add a new recovery to the queue.
    pub(crate) fn add_recovery(&mut self, origination_date: Date, amount: Money) {
        if amount.amount() > 0.0 {
            self.pending.push_back((origination_date, amount));
        }
    }

    /// Total pending (unreleased) recovery amount.
    pub(crate) fn pending_amount(&self, base_currency: Currency) -> Money {
        self.pending
            .iter()
            .fold(Money::new(0.0, base_currency), |acc, (_, amt)| {
                acc.checked_add(*amt).unwrap_or(acc)
            })
    }

    /// Remove and return every pending recovery, in FIFO (default-date) order.
    ///
    /// Used at simulation termination: recoveries from defaults within the
    /// lag window of the final payment date never mature inside the period
    /// loop and must be drained explicitly rather than silently dropped.
    pub(crate) fn drain_pending(&mut self) -> Vec<(Date, Money)> {
        self.pending.drain(..).collect()
    }

    /// Release all recoveries that have matured based on the lag period.
    ///
    /// Returns the total amount of recoveries released this period.
    pub(crate) fn release_matured(
        &mut self,
        current_date: Date,
        recovery_lag_months: u32,
        base_currency: Currency,
    ) -> finstack_core::Result<Money> {
        let mut released = Money::new(0.0, base_currency);

        while let Some((orig_date, _)) = self.pending.front() {
            let months_elapsed = orig_date.months_until(current_date);
            if months_elapsed >= recovery_lag_months {
                if let Some((_, amount)) = self.pending.pop_front() {
                    released = released.checked_add(amount)?;
                }
            } else {
                break;
            }
        }

        Ok(released)
    }
}

/// Cashflows generated in a single payment period.
#[allow(dead_code)]
pub(crate) struct PeriodFlows {
    /// Interest collected from pool assets.
    pub(crate) interest_collections: Money,
    /// Principal from prepayments.
    pub(crate) prepayments: Money,
    /// Principal lost to defaults (gross).
    pub(crate) defaults: Money,
    /// Recoveries received this period.
    pub(crate) recoveries: Money,
}

impl PeriodFlows {
    // NOTE: total cash helper removed (unused). Callers can compute:
    // interest_collections + prepayments + recoveries explicitly with currency-safe ops.
}

/// Update tranche balance after payment.
#[allow(dead_code)]
pub(crate) fn update_tranche_balance(
    tranche_balances: &mut HashMap<String, Money>,
    tranche_id: &str,
    payment: Money,
    interest_portion: Money,
) -> finstack_core::Result<()> {
    let principal_payment = payment
        .checked_sub(interest_portion)
        .unwrap_or(Money::new(0.0, payment.currency()));

    if let Some(current) = tranche_balances.get_mut(tranche_id) {
        *current = current.checked_sub(principal_payment).unwrap_or(*current);
    }

    Ok(())
}
