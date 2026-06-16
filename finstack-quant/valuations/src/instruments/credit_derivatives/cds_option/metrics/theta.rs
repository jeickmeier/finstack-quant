//! Theta metric for `CDSOption`.

use crate::instruments::credit_derivatives::cds_option::CDSOption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// CDS-option theta calculator using the CDSO pricer's settlement-aware time convention.
pub(crate) struct ThetaCalculator;

impl MetricCalculator for ThetaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CDSOption = context.instrument_as()?;
        option.theta(&context.curves, context.as_of)
    }
}
