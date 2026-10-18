//! Pricing and metric helpers for interest-rate instruments.
//!
use crate::deposit::Deposit;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Calculates year fraction for deposits.
///
/// Computes the time period between effective start and end dates using the deposit's
/// day count convention.
pub(crate) struct YearFractionCalculator;

impl MetricCalculator for YearFractionCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let deposit: &Deposit = context.instrument_as()?;
        let effective_start = deposit.effective_start_date()?;
        let effective_end = deposit.effective_end_date()?;

        if effective_end <= effective_start {
            return Err(finstack_quant_core::Error::Validation(format!(
                "YearFraction: effective end date ({}) must be after effective start date ({})",
                effective_end, effective_start
            )));
        }

        deposit.day_count.year_fraction(
            effective_start,
            effective_end,
            finstack_quant_core::dates::DayCountContext::default(),
        )
    }
}
