//! Rho calculator for range accrual instruments.
//!
//! Computes rho (interest rate sensitivity) via finite differences:
//! bump discount curve by +1bp, reprice, and return PV_change.
//!
//! Units & sign:
//! - Rho is per +1bp parallel discount move
//! - Rho = PV(rate + 1bp) − PV(base)
//! - Positive Rho means the instrument gains value when rates go up

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::range_accrual::RangeAccrual;
use crate::metrics::bump_discount_curve_parallel;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Rho calculator for range accrual instruments.
pub struct RhoCalculator;

impl MetricCalculator for RhoCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument: &RangeAccrual = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        // Use payment_date if provided, otherwise fall back to last observation date
        let final_date = instrument
            .payment_date
            .or_else(|| instrument.observation_dates.last().copied())
            .unwrap_or(as_of);
        let t = instrument.day_count.year_fraction(
            as_of,
            final_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        let bump_bp = instrument.metric_pricing_overrides.rho_bump_bp();
        let curves_bumped =
            bump_discount_curve_parallel(&context.curves, &instrument.discount_curve_id, bump_bp)?;

        // Reprice with bumped curve
        let pv_bumped = instrument.value(&curves_bumped, as_of)?.amount();

        // Rho = PV(rate + 1bp) − PV(base)
        let rho = pv_bumped - base_pv;

        Ok(rho)
    }
}
