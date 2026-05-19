//! Par-spread metric for `CDSOption`.

use crate::instruments::credit_derivatives::cds_option::pricer;
use crate::instruments::credit_derivatives::cds_option::CDSOption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Black forward CDS spread in basis points.
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CDSOption = context.instrument_as()?;
        pricer::forward_spread_bp(option, &context.curves, context.as_of)
    }
}
