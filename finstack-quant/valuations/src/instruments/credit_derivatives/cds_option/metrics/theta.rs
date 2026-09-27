//! Theta metric for `CdsOption`.

use crate::instruments::credit_derivatives::cds_option::CdsOption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// CDS-option theta calculator using the CDSO pricer's settlement-aware time convention.
pub(crate) struct ThetaCalculator;

impl MetricCalculator for ThetaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CdsOption = context.instrument_as()?;
        option.theta(&context.curves, context.as_of)
    }
}
