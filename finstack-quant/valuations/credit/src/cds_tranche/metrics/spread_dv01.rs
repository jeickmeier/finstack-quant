//! CDS Tranche Spread DV01 metric calculator.
//!
//! Measures the premium-leg PV change for a 1bp change in the running coupon.

use crate::cds_tranche::CdsTranche;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Spread DV01 calculator for CDS Tranche
pub(crate) struct SpreadDv01Calculator;

impl MetricCalculator for SpreadDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CdsTranche = context.instrument_as()?;
        // Propagate error when credit index data is missing rather than silently
        // returning zero, which would mask missing market data in risk reports.
        tranche.spread_dv01(&context.curves, context.as_of)
    }
}
