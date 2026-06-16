//! Conversion premium metric for `ConvertibleBond`.
//!
//! Computes conversion premium = bond_price / (spot * conversion_ratio) - 1.

use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

use crate::instruments::fixed_income::convertible::types::ConvertibleBond;

/// Calculator for conversion premium.
pub(crate) struct ConversionPremiumCalculator;

impl MetricCalculator for ConversionPremiumCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond = context.instrument_as::<ConvertibleBond>()?;
        // Get current bond price from context
        let bond_price = context.base_value.amount();
        bond.conversion_premium(&context.curves, bond_price)
    }
}
