use crate::instruments::rates::deposit::Deposit;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculates year fraction for deposits.
///
/// Computes the time period between effective start and end dates using the deposit's
/// day count convention. This is fundamental for all other deposit calculations.
///
/// # Validation
///
/// Explicitly validates that `effective_end > effective_start` before computing
/// the year fraction. This defensive check catches malformed instruments early
/// with a clear error message, rather than propagating a negative year fraction
/// that could cause confusing downstream errors.
///
/// See unit tests and `examples/` for usage.
pub(crate) struct YearFractionCalculator;

impl MetricCalculator for YearFractionCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let deposit: &Deposit = context.instrument_as()?;
        let effective_start = deposit.effective_start_date()?;
        let effective_end = deposit.effective_end_date()?;

        // Defensive check: ensure effective dates are properly ordered.
        // This catches malformed instruments before they produce confusing
        // negative year fractions that propagate to par rate calculations.
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
