//! CDS Tranche correlation sensitivity metric calculator.
//!
//! Measures PV sensitivity to a shift in the base correlation curve, reported
//! **per 1% (0.01 absolute) correlation change** — the same per-1% unit as
//! `Recovery01`.

use crate::instruments::credit_derivatives::cds_tranche::CDSTranche;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Correlation01 calculator for CDS Tranche
pub(crate) struct Correlation01Calculator;

impl MetricCalculator for Correlation01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CDSTranche = context.instrument_as()?;
        tranche.correlation_delta(&context.curves, context.as_of)
    }
}
