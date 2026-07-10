//! Conversion01 calculator for ConvertibleBond.
//!
//! Computes Conversion01 (conversion ratio/price sensitivity) using finite differences.
//! Conversion01 measures the change in PV for a 1% change in conversion ratio or price.
//!
//! # Formula
//! ```text
//! Conversion01 = (PV(conversion_ratio * 1.01) - PV(conversion_ratio * 0.99)) / (2 * bump_size)
//! ```
//! Where bump_size is 1% (0.01).
//!
//! # Note
//! This metric bumps the conversion ratio or conversion price (whichever is defined)
//! and reprices the convertible bond. An increase in conversion ratio (or decrease
//! in conversion price) makes conversion more favorable, increasing the bond value.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::convertible::ConvertibleBond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Standard conversion bump: 1% (0.01)
const CONVERSION_BUMP_PCT: f64 = 0.01;

/// Conversion01 calculator for ConvertibleBond.
pub(crate) struct Conversion01Calculator;

impl MetricCalculator for Conversion01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;
        let as_of = context.as_of;

        // Determine whether to bump ratio or price
        let bump_ratio = bond.conversion.ratio.is_some();

        // Bump conversion parameter up by 1%
        let mut bond_up = bond.clone();
        if bump_ratio {
            if let Some(ratio) = bond.conversion.ratio {
                bond_up.conversion.ratio = Some(ratio * (1.0 + CONVERSION_BUMP_PCT));
            }
        } else if let Some(price) = bond.conversion.price {
            // Decreasing price increases conversion value (same as increasing ratio)
            bond_up.conversion.price = Some(price * (1.0 - CONVERSION_BUMP_PCT));
        } else {
            // Neither ratio nor price defined - cannot calculate
            return Ok(0.0);
        }

        let pv_up = bond_up.value(context.curves.as_ref(), as_of)?.amount();

        // Bump conversion parameter down by 1%
        let mut bond_down = bond.clone();
        if bump_ratio {
            if let Some(ratio) = bond.conversion.ratio {
                bond_down.conversion.ratio = Some(ratio * (1.0 - CONVERSION_BUMP_PCT));
            }
        } else if let Some(price) = bond.conversion.price {
            bond_down.conversion.price = Some(price * (1.0 + CONVERSION_BUMP_PCT));
        }

        let pv_down = bond_down.value(context.curves.as_ref(), as_of)?.amount();

        // The two scenarios are already ±1%; the standardized metric is the
        // PV change for a 1% move, not the derivative per unit ratio/price.
        // For ratio: up = higher ratio = higher value, so PV_up > PV_down typically
        // For price: up = lower price = higher value, so PV_up > PV_down typically
        let conversion01 = (pv_up - pv_down) / 2.0;

        Ok(conversion01)
    }
}
