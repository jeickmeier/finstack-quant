//! Conversion premium metric for `ConvertibleBond`.
//!
//! Computes conversion premium = bond_price / (spot * conversion_ratio) - 1.

use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

use crate::convertible::types::ConvertibleBond;

/// Calculator for conversion premium.
pub(crate) struct ConversionPremiumCalculator;

impl MetricCalculator for ConversionPremiumCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond = context.instrument_as::<ConvertibleBond>()?;
        let bond_price = context.base_value.amount();
        bond.conversion_premium(&context.curves, bond_price)
    }
}
