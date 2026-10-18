//! Rho calculator for range accrual instruments.
//!
//! Computes rho (interest rate sensitivity) via finite differences: bump the
//! discount curve by the resolved `rate_bump_bp` (default 1bp), reprice, and
//! report the PV change per 1bp.
//!
//! Units & sign:
//! - Rho is per +1bp parallel discount move
//! - Rho = (PV(rate + bump) − PV(base)) / rate_bump_bp
//! - Positive Rho means the instrument gains value when rates go up

use crate::exotics::range_accrual::RangeAccrual;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::common_impl::traits::Instrument;
use finstack_quant_valuations_core::metrics::bump_discount_curve_parallel;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Rho calculator for range accrual instruments.
pub struct RhoCalculator;

impl MetricCalculator for RhoCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bump_bp = sens_config::resolve(context)?.rate_bump_bp;
        let instrument: &RangeAccrual = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        // Use payment_date if provided, otherwise fall back to last observation date
        let final_date = instrument
            .terms
            .payment_date
            .or_else(|| instrument.terms.observation_dates.last().copied())
            .unwrap_or(as_of);
        let t = instrument.terms.day_count.year_fraction(
            as_of,
            final_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        let curves_bumped = bump_discount_curve_parallel(
            &context.curves,
            &instrument.terms.discount_curve_id,
            bump_bp,
        )?;

        // Reprice with bumped curve
        let pv_bumped = instrument.value(&curves_bumped, as_of)?.amount();

        // Rho per 1bp = (PV(rate + bump) − PV(base)) / bump_bp
        Ok((pv_bumped - base_pv) / bump_bp)
    }
}
