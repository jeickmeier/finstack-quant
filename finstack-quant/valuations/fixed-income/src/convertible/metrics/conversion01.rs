//! Conversion01 calculator for ConvertibleBond.
//!
//! Computes Conversion01 (conversion ratio/price sensitivity) using finite differences.
//! Conversion01 is the PV change per 1% relative move in the conversion ratio
//! or price, in currency.
//!
//! # Formula
//! ```text
//! Conversion01 = (PV(conversion_ratio * 1.01) - PV(conversion_ratio * 0.99)) / 2
//! ```
//! i.e. `(PV_up - PV_down) / width × bump` with width 2% and bump 1%.
//!
//! # Note
//! This metric bumps the conversion ratio or conversion price (whichever is defined)
//! and reprices the convertible bond. An increase in conversion ratio (or decrease
//! in conversion price) makes conversion more favorable, increasing the bond value.

use crate::convertible::ConvertibleBond;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Standard conversion bump: 1% (0.01)
const CONVERSION_BUMP_PCT: f64 = 0.01;

/// Conversion01 calculator for ConvertibleBond.
pub(crate) struct Conversion01Calculator;

impl MetricCalculator for Conversion01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;
        let as_of = context.as_of;

        let apply_ratio_scale = |target: &mut ConvertibleBond, factor: f64| -> bool {
            let mut touched = false;
            if let Some(ratio) = target.conversion.ratio {
                let bumped = ratio * factor;
                target.conversion.ratio = Some(bumped);
                if target.conversion.price.is_some() {
                    target.conversion.price = Some(target.notional.amount() / bumped);
                }
                touched = true;
            } else if let Some(price) = target.conversion.price {
                target.conversion.price = Some(price / factor);
                touched = true;
            }
            if let crate::ConversionPolicy::MandatoryVariable {
                upper_conversion_price,
                lower_conversion_price,
                ..
            } = &mut target.conversion.policy
            {
                *upper_conversion_price /= factor;
                *lower_conversion_price /= factor;
                touched = true;
            }
            touched
        };

        // Bump effective conversion ratio up by +1% and down by -1% symmetrically.
        let mut bond_up = bond.clone();
        if !apply_ratio_scale(&mut bond_up, 1.0 + CONVERSION_BUMP_PCT) {
            return Ok(0.0);
        }
        let pv_up = super::model::model_value(context, &bond_up, context.curves.as_ref(), as_of)?;

        let mut bond_down = bond.clone();
        let _ = apply_ratio_scale(&mut bond_down, 1.0 - CONVERSION_BUMP_PCT);
        let pv_down =
            super::model::model_value(context, &bond_down, context.curves.as_ref(), as_of)?;

        Ok((pv_up - pv_down) / 2.0)
    }
}
