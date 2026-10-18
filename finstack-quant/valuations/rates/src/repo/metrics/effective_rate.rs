//! Effective repo rate metric.
//!
//! Returns the effective rate accounting for any special collateral
//! adjustments configured on the instrument.

use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Calculate effective repo rate considering special collateral.
pub(crate) struct EffectiveRateCalculator;

impl MetricCalculator for EffectiveRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo = context.instrument_as::<crate::repo::Repo>()?;
        repo.effective_rate()
    }
}
