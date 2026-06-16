//! CDS Tranche par spread metric calculator.
//!
//! Computes the running coupon in basis points that sets the tranche NPV to zero
//! using the Gaussian copula tranche pricer.

use crate::instruments::credit_derivatives::cds_tranche::CDSTranche;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Par spread calculator for CDS tranches.
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CDSTranche = context.instrument_as()?;
        tranche.par_spread(&context.curves, context.as_of)
    }
}
