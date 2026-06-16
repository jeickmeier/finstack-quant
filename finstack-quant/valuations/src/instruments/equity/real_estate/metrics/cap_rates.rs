//! Real estate cap-rate style metrics.

use crate::instruments::equity::real_estate::RealEstateAsset;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Error as CoreError;

/// Going-in cap rate.
///
/// If `purchase_price` is provided, returns `NOI_1 / purchase_price`.
/// Otherwise, returns `NOI_1 / base_value` (an implied cap rate).
#[derive(Debug, Default)]
pub(super) struct GoingInCapRate;

impl MetricCalculator for GoingInCapRate {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let asset = context
            .instrument
            .as_any()
            .downcast_ref::<RealEstateAsset>()
            .ok_or_else(|| {
                CoreError::Validation("GoingInCapRate: instrument type mismatch".into())
            })?;

        let noi_1 = asset.first_noi(context.as_of)?;

        let denom = if let Some(px) = asset.purchase_price {
            if px.currency() != asset.currency {
                return Err(CoreError::Validation(
                    "purchase_price currency does not match instrument currency".into(),
                ));
            }
            px.amount()
        } else {
            context.base_value.amount()
        };

        if denom <= 0.0 {
            return Err(CoreError::Validation(
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
        let asset = context
            .instrument
            .as_any()
            .downcast_ref::<RealEstateAsset>()
            .ok_or_else(|| CoreError::Validation("ExitCapRate: instrument type mismatch".into()))?;

        asset
            .terminal_cap_rate
            .ok_or_else(|| CoreError::Validation("ExitCapRate requires terminal_cap_rate".into()))
    }
}
