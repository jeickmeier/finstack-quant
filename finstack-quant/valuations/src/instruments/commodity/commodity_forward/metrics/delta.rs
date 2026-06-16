//! Delta calculator for commodity forwards.
//!
//! Delta measures the sensitivity of the forward's NPV to changes in the forward price.
//! For a commodity forward with NPV = sign(position) × (F - K) × Q × M × DF:
//!
//! Delta = ∂NPV/∂F = sign(position) × Q × M × DF
//!
//! Where:
//! - sign(position) = +1 for long, -1 for short
//! - Q = quantity
//! - M = multiplier
//! - DF = discount factor from as_of to settlement

use crate::instruments::commodity::commodity_forward::CommodityForward;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Delta calculator for commodity forwards (per 1.0 unit of forward price).
///
/// Returns the change in NPV for a 1.0 unit increase in the forward price,
/// accounting for position direction (long = positive delta, short = negative delta).
///
/// Uses `df_between_dates(as_of, settlement)` for base-date-safe discounting.
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fwd: &CommodityForward = context.instrument_as()?;

        // Expired forward has zero delta
        if fwd.maturity < context.as_of {
            return Ok(0.0);
        }

        let disc = context
            .curves
            .get_discount(fwd.discount_curve_id.as_str())?;

        // Use df_between_dates for base-date-safe discounting (consistent with npv())
        let df = disc.df_between_dates(context.as_of, fwd.maturity)?;

        // Delta = sign(position) × Q × M × DF
        Ok(fwd.position.sign() * fwd.quantity * fwd.multiplier * df)
    }
}
