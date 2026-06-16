//! CDS Tranche expected loss metric calculator.
//!
//! Computes the total expected loss at maturity using the Gaussian Copula engine.

use crate::instruments::credit_derivatives::cds_tranche::CDSTranche;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Expected Loss calculator for CDS Tranche
pub(crate) struct ExpectedLossCalculator;

impl MetricCalculator for ExpectedLossCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CDSTranche = context.instrument_as()?;
        // Propagate error when credit index data is missing rather than silently
        // returning zero, which would mask missing market data in risk reports.
        tranche.expected_loss(&context.curves)
    }
}
