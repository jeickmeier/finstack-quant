//! CDS Tranche Spread DV01 metric calculator.
//!
//! Measures the premium-leg PV change for a 1bp change in the running coupon.

use crate::instruments::credit_derivatives::cds_tranche::CDSTranche;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Spread DV01 calculator for CDS Tranche
pub(crate) struct SpreadDv01Calculator;

impl MetricCalculator for SpreadDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CDSTranche = context.instrument_as()?;
        // Propagate error when credit index data is missing rather than silently
        // returning zero, which would mask missing market data in risk reports.
        tranche.spread_dv01(&context.curves, context.as_of)
    }
}
