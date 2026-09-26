//! Conversion value calculator for convertible bonds.
//!
//! Reports the absolute conversion value based on the current spot price and
//! conversion terms. Delegates to the canonical `compute_conversion_value`
//! function which handles all policies including `MandatoryVariable`
//! (PERCS/DECS/ACES).

use crate::instruments::fixed_income::convertible::ConvertibleBond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Conversion value calculator.
///
/// For standard policies: `conversion_ratio * spot`.
/// For `MandatoryVariable` (PERCS/DECS/ACES): variable delivery ratio based
/// on spot relative to upper/lower conversion prices.
pub(crate) struct ConversionValueCalculator;

impl MetricCalculator for ConversionValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;

        let spot_price = context.curves.get_price(&bond.spot_id)?;
        let spot = match spot_price {
            finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => money.amount(),
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(value) => *value,
        };

        crate::instruments::fixed_income::convertible::pricing::compute_conversion_value(bond, spot)
    }
}
