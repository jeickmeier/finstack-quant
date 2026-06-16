//! ILB real duration metric calculator.

use crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
use crate::metrics::{MetricCalculator, MetricContext};

/// Real duration calculator for ILB
pub(crate) struct RealDurationCalculator;

impl MetricCalculator for RealDurationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let ilb: &InflationLinkedBond = context.instrument_as()?;
        ilb.real_duration(&context.curves, context.as_of)
    }
}
