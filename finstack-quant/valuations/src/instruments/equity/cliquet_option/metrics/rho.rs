//! Rho calculator for cliquet options.
//!
//! Computes rho (interest rate sensitivity) via finite differences:
//! bump discount curve by +1bp, reprice, and return PV_change.
//!
//! Units & sign:
//! - Rho is per +1bp parallel discount move
//! - Rho = PV(rate + 1bp) − PV(base)
//! - Positive Rho means the instrument gains value when rates go up

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::cliquet_option::CliquetOption;
use crate::metrics::bump_discount_curve_parallel;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Rho calculator for cliquet options.
pub struct RhoCalculator;

impl MetricCalculator for RhoCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CliquetOption = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        let final_date = option.reset_dates.last().copied().unwrap_or(as_of);
        let t = option.day_count.year_fraction(
            as_of,
            final_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        // Bump discount curve by 1bp (0.0001)
        let bump_bp = 0.0001;
        let curves_bumped =
            bump_discount_curve_parallel(&context.curves, &option.discount_curve_id, bump_bp)?;

        // Reprice with bumped curve
        let pv_bumped = option.value(&curves_bumped, as_of)?.amount();

        // Rho = PV(rate + 1bp) − PV(base)
        let rho = pv_bumped - base_pv;

        Ok(rho)
    }
}
