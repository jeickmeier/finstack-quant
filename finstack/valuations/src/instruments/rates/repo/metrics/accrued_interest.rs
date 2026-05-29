//! Accrued interest metric for repo instruments.
//!
//! Computes accrued interest between the repo start date and the valuation
//! date using the instrument's configured day count and effective rate. The
//! metric returns the currency amount (same units as the cash leg) so it can
//! be combined with other cash-based measures.
//!
//! # Market Standard
//!
//! Uses business-day adjusted dates for consistency with PV and interest
//! amount calculations. This ensures accrued interest aligns with the
//! actual accrual period used in pricing.

use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Calculator for repo accrued interest (currency amount).
///
/// Uses business-day adjusted start and maturity dates for consistency
/// with PV calculations.
pub(crate) struct AccruedInterestCalculator;

impl MetricCalculator for AccruedInterestCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let as_of = context.as_of;
        let (
            discount_curve_id,
            day_count,
            adj_start,
            adj_maturity,
            notional_amount,
            effective_rate,
        ) = {
            let repo = context.instrument_as::<crate::instruments::rates::repo::Repo>()?;
            // Use adjusted dates for consistency with PV and interest_amount()
            let (adj_start, adj_maturity) = repo.adjusted_dates()?;
            (
                repo.discount_curve_id.to_owned(),
                repo.day_count,
                adj_start,
                adj_maturity,
                repo.cash_amount.amount(),
                repo.effective_rate()?,
            )
        };

        let accrued_amount = if as_of <= adj_start {
            0.0
        } else {
            let accrual_end = if as_of >= adj_maturity {
                adj_maturity
            } else {
                as_of
            };
            let accrual_fraction = day_count.year_fraction(
                adj_start,
                accrual_end,
                finstack_core::dates::DayCountContext::default(),
            )?;
            notional_amount * effective_rate * accrual_fraction
        };

        context.discount_curve_id = Some(discount_curve_id);
        context.day_count = Some(day_count);

        Ok(accrued_amount)
    }
}
