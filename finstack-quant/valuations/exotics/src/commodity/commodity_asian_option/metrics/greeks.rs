//! Forward-delta calculator for commodity Asian options.
//!
//! Bump-and-reprice (central finite difference), since there are no
//! closed-form analytical greeks for arithmetic Asian options: the forward
//! price curve (PriceCurve) is bumped by ±`spot_bump_decimal` (default 1%) in
//! parallel. Vega is the core `GenericFdVega`.
//!
//! Bump sizes resolve from `metric_pricing_overrides.bump_config` and the
//! `valuations.sensitivities.v1` config extension.

use crate::commodity::bump_price_curve_pct;
use crate::commodity::commodity_asian_option::CommodityAsianOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Delta calculator for commodity Asian options (forward curve sensitivity).
///
/// Uses central finite difference on the forward price curve:
/// ```text
/// Delta = (PV_up - PV_down) / (2 * bump_size)
/// ```
/// where `bump_size` is the resolved spot bump (default 1%) of the average
/// forward price, and the PriceCurve is bumped by the same relative amount.
pub(super) struct AsianDeltaCalculator;

impl MetricCalculator for AsianDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let asian: &CommodityAsianOption = context.instrument_as()?;

        let future_count = asian
            .fixing_dates
            .iter()
            .filter(|date| **date > context.as_of)
            .count();
        if future_count == 0 {
            return Ok(0.0);
        }

        let bump_pct = sens_config::resolve(context)?.spot_bump_decimal;

        let (fwd_sum, fwd_count) = asian.future_forwards(&context.curves, context.as_of)?;
        let avg_fwd = fwd_sum / fwd_count as f64;
        let bump_size = avg_fwd * bump_pct;
        if bump_size <= 0.0 {
            return Ok(0.0);
        }

        let curve_id = asian.forward_curve_id.as_str();
        let market_up = bump_price_curve_pct(&context.curves, curve_id, bump_pct)?;
        let pv_up = asian.value(&market_up, context.as_of)?.amount();
        let market_down = bump_price_curve_pct(&context.curves, curve_id, -bump_pct)?;
        let pv_down = asian.value(&market_down, context.as_of)?.amount();

        // Central difference: dPV / d(forward_price)
        Ok((pv_up - pv_down) / (2.0 * bump_size))
    }
}
