//! Theta metric for `CdsOption`.

use crate::cds_option::CdsOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// CDS-option theta calculator using the CDSO pricer's settlement-aware time convention.
pub(crate) struct ThetaCalculator;

impl MetricCalculator for ThetaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CdsOption = context.instrument_as()?;
        option.theta(&context.curves, context.as_of)
    }
}
