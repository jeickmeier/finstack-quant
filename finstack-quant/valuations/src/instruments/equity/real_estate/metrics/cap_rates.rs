//! Real estate cap-rate style metrics.

use crate::instruments::equity::real_estate::{pricer, RealEstateAsset};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Error;

/// Going-in cap rate.
///
/// If `purchase_price` is provided, returns `NOI_1 / purchase_price`.
/// Otherwise, returns `NOI_1 / base_value` (an implied cap rate).
#[derive(Debug, Default)]
pub(super) struct GoingInCapRate;

impl MetricCalculator for GoingInCapRate {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let asset: &RealEstateAsset = context.instrument_as()?;

        let noi_1 = pricer::first_noi(asset, context.as_of)?;

        let denom = pricer::purchase_price(asset)?.unwrap_or_else(|| context.base_value.amount());

        if denom <= 0.0 {
            return Err(Error::Validation(
                "GoingInCapRate: denominator must be positive".into(),
            ));
        }

        Ok(noi_1 / denom)
    }
}

/// Exit cap rate.
///
/// Returns the configured `terminal_cap_rate` (if present). If the instrument does not have
/// a terminal cap rate configured, this metric errors.
#[derive(Debug, Default)]
pub(super) struct ExitCapRate;

impl MetricCalculator for ExitCapRate {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let asset: &RealEstateAsset = context.instrument_as()?;

        asset
            .terminal_cap_rate
            .ok_or_else(|| Error::Validation("ExitCapRate requires terminal_cap_rate".into()))
    }
}
