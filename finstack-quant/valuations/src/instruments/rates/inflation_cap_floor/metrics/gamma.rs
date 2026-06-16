//! Gamma calculator for inflation cap/floor options.
//!
//! Computes gamma (second derivative of PV with respect to forward inflation rate)
//! using central finite differences:
//!
//! ```text
//! Gamma = (PV_up - 2×PV_base + PV_down) / (bump_size)²
//! ```
//!
//! Gamma measures the convexity of the option's value with respect to inflation,
//! which is important for hedging and risk management.
//!
//! # Units
//!
//! - Bump: 1bp = 0.01% applied to inflation curve via `BumpSpec::inflation_shift_pct`
//! - Result: raw second derivative `d²PV/dπ²` per unit² of inflation rate
//!   (consistent with the workspace raw-derivative convention; divide by 1e8
//!   for a per-bp² view).
//!
//! All three PVs are computed via `value_raw` (unrounded `f64`) so Money
//! quantization noise is not amplified by the `h² = 1e-8` divisor.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::inflation_cap_floor::InflationCapFloor;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::Result;

/// Inflation curve bump size: 1bp = 0.01% (as percentage for BumpSpec).
const GAMMA_BUMP_PCT: f64 = 0.01;

/// Bump size in decimal terms for scaling the result: 1bp = 0.0001.
const GAMMA_BUMP_DECIMAL: f64 = 0.0001;

/// Gamma calculator for inflation cap/floor options.
///
/// Computes the second derivative of PV with respect to the forward inflation rate.
/// Uses central finite differences for accurate second derivative estimation.
pub(crate) struct GammaCalculator;

impl MetricCalculator for GammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &InflationCapFloor = context.instrument_as()?;
        let as_of = context.as_of;

        // Expired options have zero gamma
        if as_of >= option.maturity {
            return Ok(0.0);
        }

        // Unrounded base PV: Money-quantized values divided by h² = 1e-8
        // would turn sub-cent rounding into large gamma noise.
        let base_pv = option.value_raw(context.curves.as_ref(), as_of)?;

        // Bump up by 1bp = 0.01%
        let bump_spec_up = BumpSpec::inflation_shift_pct(GAMMA_BUMP_PCT);
        let curves_up = context.curves.as_ref().bump([MarketBump::Curve {
            id: option.inflation_index_id.clone(),
            spec: bump_spec_up,
        }])?;
        let pv_up = option.value_raw(&curves_up, as_of)?;

        // Bump down by 1bp = 0.01%
        let bump_spec_down = BumpSpec::inflation_shift_pct(-GAMMA_BUMP_PCT);
        let curves_down = context.curves.as_ref().bump([MarketBump::Curve {
            id: option.inflation_index_id.clone(),
            spec: bump_spec_down,
        }])?;
        let pv_down = option.value_raw(&curves_down, as_of)?;

        // Second derivative via central difference:
        // Gamma = (PV_up - 2*PV_base + PV_down) / h²
        //
        // Where h = 1bp = 0.0001 in decimal form; dividing by h² yields the
        // raw second derivative d²PV/dπ² (per unit²).
        let h_squared = GAMMA_BUMP_DECIMAL * GAMMA_BUMP_DECIMAL;
        Ok((pv_up - 2.0 * base_pv + pv_down) / h_squared)
    }
}
