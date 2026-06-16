//! ILB index ratio metric calculator.

use crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
use crate::metrics::{MetricCalculator, MetricContext};

/// Index ratio calculator for ILB
pub(crate) struct IndexRatioCalculator;

impl MetricCalculator for IndexRatioCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let ilb: &InflationLinkedBond = context.instrument_as()?;
        ilb.index_ratio_from_market(context.as_of, context.curves.as_ref())
    }
}
