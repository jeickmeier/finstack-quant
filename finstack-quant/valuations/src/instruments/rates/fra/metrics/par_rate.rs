//! FRA par rate metric calculator.
//!
//! Computes the fixed rate that makes the FRA's PV zero under current curves.
//! For standard FRA settlement-at-start conventions and consistent curves,
//! this equals the forward rate over the period:
//!
//! par_rate = (projection_df(start) / projection_df(end) - 1) / index_accrual
//!
//! Discount-factor time mapping uses the forward curve's own day count and base
//! date. Annualization uses the FRA's contractual index day count.

use crate::instruments::rates::fra::ForwardRateAgreement;
use crate::metrics::{MetricCalculator, MetricContext};

/// Par rate for FRAs (fixed rate that zeroes PV).
pub(crate) struct FraParRateCalculator;

impl MetricCalculator for FraParRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fra: &ForwardRateAgreement = context.instrument_as()?;

        // Forward rate over [t_start, t_end]
        let fwd = context.curves.get_forward(fra.forward_curve_id.as_str())?;

        let period_length = (fra.maturity - fra.start_date).whole_days();
        if period_length == 0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FRA '{}': period length is zero ({period_length}); cannot compute par rate",
                fra.id
            )));
        }

        crate::instruments::common_impl::pricing::time::rate_between_on_dates(
            fwd.as_ref(),
            fra.start_date,
            fra.maturity,
            fra.day_count.year_fraction(
                fra.start_date,
                fra.maturity,
                finstack_quant_core::dates::DayCountContext::default(),
            )?,
        )
    }
}
