//! ILB real yield metric calculator.

use crate::inflation_linked_bond::InflationLinkedBond;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Real yield calculator for ILB
pub(crate) struct RealYieldCalculator;

impl MetricCalculator for RealYieldCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let ilb: &InflationLinkedBond = context.instrument_as()?;
        let clean_price = ilb.quoted_clean_price_pct("Real yield")?;
        ilb.real_yield(clean_price, context.as_of)
    }
}
