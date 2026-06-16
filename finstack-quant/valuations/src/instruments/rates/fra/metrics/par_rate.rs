//! FRA par rate metric calculator.
//!
//! Computes the fixed rate that makes the FRA's PV zero under current curves.
//! For standard FRA settlement-at-start conventions and consistent curves,
//! this equals the forward rate over the period:
//!
//! par_rate = ForwardCurve::rate_period(t_start, t_end)
//!
//! Time mapping uses the instrument day-count measured from the discount
//! curve's base date, matching the engine and other rate instruments.

use crate::instruments::rates::fra::ForwardRateAgreement;
use crate::metrics::{MetricCalculator, MetricContext};

/// Minimum period length (in year fractions) for computing par rate.
const MIN_PERIOD_LENGTH: f64 = 1e-12;

/// Par rate for FRAs (fixed rate that zeroes PV).
pub(crate) struct FraParRateCalculator;

impl MetricCalculator for FraParRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fra: &ForwardRateAgreement = context.instrument_as()?;

        // Forward rate over [t_start, t_end]
        let fwd = context.curves.get_forward(fra.forward_curve_id.as_str())?;

        // Times must be calculated using the forward curve's basis
        let fwd_base = fwd.base_date();
        let fwd_dc = fwd.day_count();

        let t_start = fwd_dc
            .year_fraction(
                fwd_base,
                fra.start_date,
                finstack_quant_core::dates::DayCountContext::default(),
            )?
            .max(0.0);
        let t_end = fwd_dc
            .year_fraction(
                fwd_base,
                fra.maturity,
                finstack_quant_core::dates::DayCountContext::default(),
            )?
            .max(t_start);

        let period_length = t_end - t_start;
        if period_length.abs() < MIN_PERIOD_LENGTH {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FRA '{}': period length is zero or near-zero ({:.2e}); cannot compute par rate",
                fra.id, period_length
            )));
        }

        Ok(fwd.rate_period(t_start, t_end))
    }
}
