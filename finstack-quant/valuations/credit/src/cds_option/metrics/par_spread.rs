//! Par-spread metric for `CdsOption`.

use crate::cds_option::pricer;
use crate::cds_option::CdsOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Black forward CDS spread in basis points.
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CdsOption = context.instrument_as()?;
        pricer::forward_spread_bp(option, &context.curves, context.as_of)
    }
}
