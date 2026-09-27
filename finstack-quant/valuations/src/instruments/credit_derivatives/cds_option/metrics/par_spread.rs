//! Par-spread metric for `CdsOption`.

use crate::instruments::credit_derivatives::cds_option::pricer;
use crate::instruments::credit_derivatives::cds_option::CdsOption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Black forward CDS spread in basis points.
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CdsOption = context.instrument_as()?;
        pricer::forward_spread_bp(option, &context.curves, context.as_of)
    }
}
