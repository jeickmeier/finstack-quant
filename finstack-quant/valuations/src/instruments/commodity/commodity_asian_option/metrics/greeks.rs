//! Delta and vega calculators for commodity Asian options.
//!
//! Both use bump-and-reprice (central finite difference) since there are no
//! closed-form analytical greeks for arithmetic Asian options.
//!
//! - **Delta**: Bumps the forward price curve (PriceCurve) by ±`spot_bump_decimal`
//!   (default 1%) parallel and computes the central difference.
//! - **Vega**: Bumps the vol surface by ±`vol_bump_decimal` (default 1 vol point,
//!   absolute) and reports the central difference per 1 vol point.
//!
//! Bump sizes resolve from `metric_pricing_overrides.bump_config` and the
//! `valuations.sensitivities.v1` config extension.

use crate::instruments::commodity::commodity_asian_option::CommodityAsianOption;
use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext, VOL_POINTS_PER_ABSOLUTE_VOL};
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;

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
        if fwd_count == 0 {
            return Ok(0.0); // Fully observed, no forward sensitivity
        }
        let avg_fwd = fwd_sum / fwd_count as f64;
        let bump_size = avg_fwd * bump_pct;
        if bump_size <= 0.0 {
            return Ok(0.0);
        }

        // Bump forward curve up
        let curve_id = CurveId::new(asian.forward_curve_id.as_str());
        let market_up = context.curves.bump([MarketBump::Curve {
            id: curve_id.clone(),
            spec: BumpSpec {
                bump_type: BumpType::Parallel,
                mode: BumpMode::Additive,
                units: BumpUnits::Percent,
                value: bump_pct * 100.0, // percent units
            },
        }])?;
        let pv_up = asian.value(&market_up, context.as_of)?.amount();

        // Bump forward curve down
        let market_down = context.curves.bump([MarketBump::Curve {
            id: curve_id,
            spec: BumpSpec {
                bump_type: BumpType::Parallel,
                mode: BumpMode::Additive,
                units: BumpUnits::Percent,
                value: -bump_pct * 100.0,
            },
        }])?;
        let pv_down = asian.value(&market_down, context.as_of)?.amount();

        // Central difference: dPV / d(forward_price)
        Ok((pv_up - pv_down) / (2.0 * bump_size))
    }
}

/// Vega calculator for commodity Asian options (vol surface sensitivity).
///
/// Uses central finite difference on the vol surface:
/// ```text
/// Vega = (PV_up - PV_down) / (2 * vol_bump * 100)
/// ```
/// where `vol_bump` is the resolved absolute vol bump (default 0.01), giving
/// sensitivity per 1 vol point move in implied volatility.
pub(super) struct AsianVegaCalculator;

impl MetricCalculator for AsianVegaCalculator {
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

        let vol_bump = sens_config::resolve(context)?.vol_bump_decimal;

        // Bump vol surface up
        let market_up = crate::metrics::bump_surface_vol_absolute(
            &context.curves,
            asian.vol_surface_id.as_str(),
            vol_bump,
        )?;
        let pv_up = asian.value(&market_up, context.as_of)?.amount();

        // Bump vol surface down
        let market_down = crate::metrics::bump_surface_vol_absolute(
            &context.curves,
            asian.vol_surface_id.as_str(),
            -vol_bump,
        )?;
        let pv_down = asian.value(&market_down, context.as_of)?.amount();

        // `MetricId::Vega` is cash P&L for a one-vol-point move, not
        // dPV/d(absolute volatility): normalise by the bump width expressed in
        // vol points (2.0 at the 1 vol-point default).
        Ok((pv_up - pv_down) / (2.0 * vol_bump * VOL_POINTS_PER_ABSOLUTE_VOL))
    }
}
