//! Delta and vega calculators for commodity Asian options.
//!
//! Both use bump-and-reprice (central finite difference) since there are no
//! closed-form analytical greeks for arithmetic Asian options.
//!
//! - **Delta**: Bumps the forward price curve (PriceCurve) by ±1% parallel
//!   and computes the central difference.
//! - **Vega**: Bumps the vol surface by ±1 vol point (absolute) and computes
//!   the central difference, scaled to per-1-vol-point sensitivity.

use crate::instruments::commodity::commodity_asian_option::CommodityAsianOption;
use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::{MetricCalculator, MetricContext};
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
/// where `bump_size` is 1% of the average forward price, and the PriceCurve
/// is bumped by ±1% parallel.
pub(super) struct AsianDeltaCalculator;

impl MetricCalculator for AsianDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let asian: &CommodityAsianOption = context.instrument_as()?;

        let bump_pct = crate::metrics::bump_sizes::SPOT; // 1% = 0.01

        // Get approximate forward price for bump_size calculation
        let (fwd_sum, fwd_count) = asian.future_forwards(&context.curves, context.as_of)?;
        if fwd_count == 0 {
            return Ok(0.0); // Fully observed, no forward sensitivity
        }
        let avg_fwd = fwd_sum / fwd_count as f64;
        let bump_size = avg_fwd * bump_pct;
        if bump_size <= 0.0 {
            return Ok(0.0);
        }

        // Bump forward curve up by 1%
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

        // Bump forward curve down by 1%
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
/// Vega = (PV_up - PV_down) / (2 * vol_bump)
/// ```
/// where `vol_bump` is 1 absolute vol point (0.01), giving sensitivity per
/// 1 vol point move in implied volatility.
pub(super) struct AsianVegaCalculator;

impl MetricCalculator for AsianVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let asian: &CommodityAsianOption = context.instrument_as()?;

        let vol_bump = crate::metrics::bump_sizes::VOLATILITY; // 1 vol point = 0.01

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
        // dPV/d(absolute volatility).  Because the scenarios already move the
        // surface by +/- one vol point, the central P&L is simply half the
        // difference between the two valuations.
        Ok((pv_up - pv_down) / 2.0)
    }
}
