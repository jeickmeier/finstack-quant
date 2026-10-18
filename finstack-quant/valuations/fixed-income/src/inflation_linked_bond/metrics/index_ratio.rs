//! ILB index ratio metric calculator.

use crate::inflation_linked_bond::InflationLinkedBond;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Index ratio calculator for ILB
pub(crate) struct IndexRatioCalculator;

impl MetricCalculator for IndexRatioCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let ilb: &InflationLinkedBond = context.instrument_as()?;
        ilb.index_ratio_from_market(context.as_of, context.curves.as_ref(), context.as_of)
    }
}
