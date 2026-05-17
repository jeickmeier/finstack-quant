//! Expected variance metric (blend of realized and forward).

use super::super::types::VarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Calculate the expected variance (blend of realized and forward).
pub(crate) struct ExpectedVarianceCalculator;

impl MetricCalculator for ExpectedVarianceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;
        let as_of = context.as_of;

        if as_of >= swap.maturity {
            // At maturity, expected variance equals realized variance
            return swap.partial_realized_variance(&context.curves, as_of);
        }

        // If not started, expected variance is purely forward variance
        if as_of < swap.start_date {
            return swap.remaining_forward_variance(&context.curves, as_of);
        }

        // Partially observed: blend realized-to-date with forward for the
        // remaining period. Both variances are already annualized, so the
        // accrued-variance identity time-weights them by elapsed/remaining
        // day-count fractions, not observation counts (W-32).
        let realized = swap.partial_realized_variance(&context.curves, as_of)?;
        let forward = swap.remaining_forward_variance(&context.curves, as_of)?;
        let w = swap.time_elapsed_fraction(as_of);

        Ok(realized * w + forward * (1.0 - w))
    }
}
