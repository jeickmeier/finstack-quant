//! FX Vega calculator for quanto options.
//!
//! Computes FX vega (FX volatility sensitivity) using a central finite
//! difference: bump FX volatility surface up and down by an absolute amount,
//! reprice, and return `(PV_up - PV_down) / (2 * bump)`.
//!
//! Vega is reported per 1 absolute volatility point (e.g. 20% -> 21%), matching
//! [`bump_sizes::VOLATILITY`] semantics.
//!
//! # Note
//!
//! Errors if `fx_vol_id` is not set on the instrument.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fx::quanto_option::QuantoOption;
use crate::metrics::{bump_sizes, bump_surface_vol_absolute, MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// FX Vega calculator for quanto options.
pub struct FxVegaCalculator;

impl MetricCalculator for FxVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &QuantoOption = context.instrument_as()?;
        let as_of = context.as_of;

        // Check if expired
        let t = option.day_count.year_fraction(
            as_of,
            option.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        // Require an FX vol surface id - silent zero hides config errors.
        let fx_vol_id = option.fx_vol_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "QuantoOption {}: fx_vol_id is required to compute FX Vega",
                option.id
            ))
        })?;

        // Absolute bump in vol units (e.g. 20% -> 21%); central difference for O(h^2).
        let bump = bump_sizes::VOLATILITY;
        let curves_up =
            bump_surface_vol_absolute(context.curves.as_ref(), fx_vol_id.as_str(), bump)?;
        let curves_down =
            bump_surface_vol_absolute(context.curves.as_ref(), fx_vol_id.as_str(), -bump)?;

        let pv_up = option.value(&curves_up, as_of)?.amount();
        let pv_down = option.value(&curves_down, as_of)?.amount();

        Ok((pv_up - pv_down) / (2.0 * bump))
    }
}
