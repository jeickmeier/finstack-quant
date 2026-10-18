//! CDS Tranche par spread metric calculator.
//!
//! Computes the running coupon in basis points that sets the tranche NPV to zero
//! using the Gaussian copula tranche pricer.

use crate::cds_tranche::CdsTranche;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Par spread calculator for CDS tranches.
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CdsTranche = context.instrument_as()?;
        tranche.par_spread(&context.curves, context.as_of)
    }
}
