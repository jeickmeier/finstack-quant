//! CDS Tranche jump-to-default metric calculator.
//!
//! Computes the instantaneous tranche loss if an average constituent defaults.

use crate::cds_tranche::CdsTranche;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Jump-to-default calculator for CDS Tranche
pub(crate) struct JumpToDefaultCalculator;

impl MetricCalculator for JumpToDefaultCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CdsTranche = context.instrument_as()?;
        // Propagate error when credit index data is missing rather than silently
        // returning zero, which would mask missing market data in risk reports.
        tranche.jump_to_default(&context.curves, context.as_of)
    }
}
