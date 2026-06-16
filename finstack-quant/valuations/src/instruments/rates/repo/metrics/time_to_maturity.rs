//! Time to maturity metric for `Repo`.
//!
//! Computes the year fraction from `as_of` to repo maturity using the
//! instrument's day-count convention.
//!
//! # Market Standard
//!
//! Uses business-day adjusted maturity date for consistency with PV and
//! other metric calculations.

use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculate time to maturity in years.
///
/// Uses business-day adjusted maturity for consistency with PV calculations.
pub(crate) struct TimeToMaturityCalculator;

impl MetricCalculator for TimeToMaturityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo = context.instrument_as::<crate::instruments::rates::repo::Repo>()?;
        // Use adjusted maturity for consistency with PV and interest calculations
        let (_, adj_maturity) = repo.adjusted_dates()?;
        let ttm = repo.day_count.year_fraction(
            context.as_of,
            adj_maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        Ok(ttm)
    }
}
