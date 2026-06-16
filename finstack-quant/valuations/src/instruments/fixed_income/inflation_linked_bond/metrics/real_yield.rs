//! ILB real yield metric calculator.

use crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
use crate::metrics::{MetricCalculator, MetricContext};

/// Real yield calculator for ILB
pub(crate) struct RealYieldCalculator;

impl MetricCalculator for RealYieldCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let ilb: &InflationLinkedBond = context.instrument_as()?;
        let clean_price = ilb.quoted_clean.ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "inflation_linked_bond_quote".to_string(),
            })
        })?;
        ilb.real_yield(clean_price, &context.curves, context.as_of)
    }
}
