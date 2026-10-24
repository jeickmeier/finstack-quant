//! Charm and color calculators for equity options.
//!
//! Both are one-day forward differences of a spot greek:
//!
//! - Charm (∂²V/∂S∂t, delta decay) ≈ (Delta(t+h) − Delta(t)) / h
//! - Color (∂³V/∂S²∂t, gamma decay) ≈ (Gamma(t+h) − Gamma(t)) / h
//!
//! Delta and gamma come from central spot-bump stencils repriced at the
//! valuation date and one calendar day later.

use crate::equity_option::EquityOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::bump_scalar_price;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Charm calculator for equity options.
pub(crate) struct CharmCalculator;

impl MetricCalculator for CharmCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        spot_greek_decay(context, SpotGreek::Delta)
    }
}

/// Color calculator for equity options.
pub(crate) struct ColorCalculator;

impl MetricCalculator for ColorCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        spot_greek_decay(context, SpotGreek::Gamma)
    }
}

/// Spot greek whose one-day decay is measured.
#[derive(Clone, Copy)]
enum SpotGreek {
    /// Central first difference in spot (charm).
    Delta,
    /// Central second difference in spot (color).
    Gamma,
}

/// One-day forward difference of `greek`, per year on the option's `day_count`.
///
/// Returns `0.0` at or within two days of expiry, where the one-day time bump
/// would cross expiry.
fn spot_greek_decay(context: &MetricContext, greek: SpotGreek) -> Result<f64> {
    let option: &EquityOption = context.instrument_as()?;
    if option.exercise.is_some() {
        return Ok(0.0);
    }
    let as_of = context.as_of;
    let base_pv = match greek {
        SpotGreek::Delta => 0.0,
        SpotGreek::Gamma => context.reprice_instrument_raw(option, &context.curves, as_of)?,
    };

    let t = option.day_count.year_fraction(
        as_of,
        option.expiry,
        finstack_quant_core::dates::DayCountContext::default(),
    )?;
    if t <= 0.0 {
        return Ok(0.0);
    }

    let current_spot =
        finstack_quant_valuations_core::instruments::common_impl::helpers::scalar_price_amount(
            context.curves.get_price(&option.spot_id)?,
            option.currency,
        )?;

    let bump_pct = super::spot_bump_decimal(context, option, current_spot)?;
    let spot_bump = current_spot * bump_pct;

    let rolled_date = as_of + time::Duration::days(1);
    let h_years = option.day_count.year_fraction(
        as_of,
        rolled_date,
        finstack_quant_core::dates::DayCountContext::default(),
    )?;
    if h_years <= 0.0 || t < 2.0 * h_years {
        return Ok(0.0);
    }

    let spot_greek_at = |date, base_pv: f64| -> Result<f64> {
        let curves_up = bump_scalar_price(&context.curves, &option.spot_id, bump_pct)?;
        let pv_up = context.reprice_instrument_raw(option, &curves_up, date)?;
        let curves_down = bump_scalar_price(&context.curves, &option.spot_id, -bump_pct)?;
        let pv_down = context.reprice_instrument_raw(option, &curves_down, date)?;
        Ok(match greek {
            SpotGreek::Delta => (pv_up - pv_down) / (2.0 * spot_bump),
            SpotGreek::Gamma => (pv_up - 2.0 * base_pv + pv_down) / (spot_bump * spot_bump),
        })
    };

    let greek_t = spot_greek_at(as_of, base_pv)?;

    let base_pv_future = match greek {
        SpotGreek::Delta => 0.0,
        SpotGreek::Gamma => context.reprice_instrument_raw(option, &context.curves, rolled_date)?,
    };
    let greek_t_future = spot_greek_at(rolled_date, base_pv_future)?;

    Ok((greek_t_future - greek_t) / h_years)
}
