//! Vega calculator for inflation cap/floor options.
//!
//! Computes vega using central finite differences for O(h²) accuracy:
//!
//! ```text
//! Vega = (PV_vol_up - PV_vol_down) / (2 × bump_size × VOL_POINTS_PER_ABSOLUTE_VOL)
//! ```
//!
//! This avoids the O(h) bias that one-sided differences can introduce,
//! especially important for curved volatility surfaces.
//!
//! # Output Convention
//!
//! **Vega is expressed per 1 volatility point (0.01 absolute vol change)**,
//! matching the workspace-wide convention used by swaption
//! (`VOL_PCT_SCALE`), nominal cap/floor, CMS, and `fd_greeks` vegas. The raw
//! central difference yields sensitivity per *unit* absolute vol; dividing by
//! `VOL_POINTS_PER_ABSOLUTE_VOL` (= 100) converts it to per-vol-point.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::inflation_cap_floor::InflationCapFloor;
use crate::metrics::bump_sizes;
use crate::metrics::bump_surface_vol_absolute;
use crate::metrics::{MetricCalculator, MetricContext, VOL_POINTS_PER_ABSOLUTE_VOL};
use finstack_quant_core::Result;

/// Vega calculator for inflation cap/floor options.
///
/// Uses central differences for improved accuracy on curved vol surfaces.
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &InflationCapFloor = context.instrument_as()?;
        let as_of = context.as_of;

        if as_of >= option.maturity {
            return Ok(0.0);
        }

        // Bump vol surface up
        let curves_up = bump_surface_vol_absolute(
            &context.curves,
            option.vol_surface_id.as_str(),
            bump_sizes::VOLATILITY,
        )?;
        let pv_up = option.value(&curves_up, as_of)?.amount();

        // Bump vol surface down
        let curves_down = bump_surface_vol_absolute(
            &context.curves,
            option.vol_surface_id.as_str(),
            -bump_sizes::VOLATILITY,
        )?;
        let pv_down = option.value(&curves_down, as_of)?.amount();

        // Central difference per unit vol, rescaled to per vol point (1% = 0.01
        // absolute vol) to match the workspace vega convention.
        Ok((pv_up - pv_down) / (2.0 * bump_sizes::VOLATILITY * VOL_POINTS_PER_ABSOLUTE_VOL))
    }
}
