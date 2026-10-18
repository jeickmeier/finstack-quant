//! Conversion value calculator for convertible bonds.
//!
//! Reports the absolute conversion value based on the current spot price and
//! conversion terms. Delegates to the canonical `compute_conversion_value`
//! function which handles all policies including `MandatoryVariable`
//! (PERCS/DECS/ACES).

use crate::convertible::ConvertibleBond;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Conversion value calculator.
///
/// For standard policies: `conversion_ratio * spot`.
/// For `MandatoryVariable` (PERCS/DECS/ACES): variable delivery ratio based
/// on spot relative to upper/lower conversion prices.
pub(crate) struct ConversionValueCalculator;

impl MetricCalculator for ConversionValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;

        let spot = bond.spot_value(context.curves.get_price(&bond.spot_id)?)?;

        crate::convertible::pricing::compute_conversion_value(bond, spot)
    }
}
