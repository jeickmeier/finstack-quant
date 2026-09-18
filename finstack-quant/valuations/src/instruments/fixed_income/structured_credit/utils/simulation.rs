//! Simulation helpers for structured credit cashflow projection.
//!
//! This module contains internal helpers used by the pricing engine
//! for period-by-period simulation.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::money::Money;
use std::collections::VecDeque;

/// Recovery queue for delayed recovery processing.
///
/// Recoveries from defaulted assets are typically received 6-12 months after
/// the default event. This queue holds pending recoveries until they can be
/// released based on the configured recovery lag.
#[derive(Debug, Default)]
pub(crate) struct RecoveryQueue {
    /// Queue of pending claims: (default date, recovery amount, defaulted par).
    pending: VecDeque<(Date, Money, Money)>,
}

impl RecoveryQueue {
    /// Create a new empty recovery queue.
    pub(crate) fn new() -> Self {
        Self {
            pending: VecDeque::new(),
        }
    }

    /// Add a new recovery claim to the queue.
    ///
    /// `origination_date` is the default date the recovery lag runs from,
    /// `amount` the recovery cash expected on the claim and `par` the
    /// defaulted par the claim stands for, which the market-value coverage
    /// rule carries instead of the recovery and which the at-liquidation loss
    /// recognition books against the recovery when the claim settles. Claims
    /// with neither cash nor par are dropped.
    pub(crate) fn add_recovery(&mut self, origination_date: Date, amount: Money, par: Money) {
        if amount.amount() > 0.0 || par.amount() > 0.0 {
            let index = self
                .pending
                .partition_point(|(date, _, _)| *date <= origination_date);
            self.pending.insert(index, (origination_date, amount, par));
        }
    }

    /// Total pending (unreleased) recovery amount.
    pub(crate) fn pending_amount(&self, base_currency: Currency) -> Money {
        self.pending
            .iter()
            .fold(Money::from((0_i64, base_currency)), |acc, (_, amt, _)| {
                acc.checked_add(*amt).unwrap_or(acc)
            })
    }

    /// Total defaulted par behind the pending (unreleased) claims.
    pub(crate) fn pending_par(&self, base_currency: Currency) -> Money {
        self.pending
            .iter()
            .fold(Money::from((0_i64, base_currency)), |acc, (_, _, par)| {
                acc.checked_add(*par).unwrap_or(acc)
            })
    }

    /// Remove and return every pending recovery, in FIFO (default-date) order.
    ///
    /// Used at simulation termination: recoveries from defaults within the
    /// lag window of the final payment date never mature inside the period
    /// loop and must be drained explicitly rather than silently dropped.
    pub(crate) fn drain_pending(&mut self) -> Vec<(Date, Money, Money)> {
        self.pending.drain(..).collect()
    }

    /// Release all recoveries that have matured based on the lag period.
    ///
    /// Returns the recovery cash released this period and the defaulted par
    /// behind it.
    pub(crate) fn release_matured(
        &mut self,
        current_date: Date,
        recovery_lag_months: u32,
        base_currency: Currency,
    ) -> finstack_quant_core::Result<(Money, Money)> {
        let mut released = Money::from((0_i64, base_currency));
        let mut released_par = Money::from((0_i64, base_currency));

        while let Some((orig_date, _, _)) = self.pending.front() {
            let months = i32::try_from(recovery_lag_months).map_err(|_| {
                finstack_quant_core::Error::Validation(
                    "recovery lag exceeds supported calendar range".into(),
                )
            })?;
            if orig_date.add_months(months) <= current_date {
                if let Some((_, amount, par)) = self.pending.pop_front() {
                    released = released.checked_add(amount)?;
                    released_par = released_par.checked_add(par)?;
                }
            } else {
                break;
            }
        }

        Ok((released, released_par))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn production_waterfall_recovery_dates_use_full_lag_and_sort_claims() {
        let mut queue = RecoveryQueue::new();
        queue.add_recovery(
            date!(2024 - 02 - 29),
            Money::new(20.0, Currency::USD).expect("second claim"),
            Money::new(50.0, Currency::USD).expect("second par"),
        );
        queue.add_recovery(
            date!(2024 - 01 - 31),
            Money::new(10.0, Currency::USD).expect("first claim"),
            Money::new(25.0, Currency::USD).expect("first par"),
        );
        assert_eq!(
            queue
                .release_matured(date!(2024 - 03 - 01), 2, Currency::USD)
                .expect("early")
                .0
                .amount(),
            0.0
        );
        assert_eq!(
            queue
                .release_matured(date!(2024 - 03 - 31), 2, Currency::USD)
                .expect("first")
                .0
                .amount(),
            10.0
        );
        assert_eq!(
            queue
                .release_matured(date!(2024 - 04 - 29), 2, Currency::USD)
                .expect("second")
                .0
                .amount(),
            20.0
        );
        assert_eq!(
            queue
                .release_matured(date!(2024 - 04 - 30), 2, Currency::USD)
                .expect("repeat")
                .0
                .amount(),
            0.0
        );
    }
}
