//! Credit exposure metrics that make the undrawn commitment visible:
//! exposure at default and the expected loss embedded in the valuation.

use crate::instruments::fixed_income::revolving_credit::types::{
    CreditSpreadProcessSpec, DrawRepaySpec,
};
use crate::instruments::{Instrument, RevolvingCredit};
use crate::metrics::{MetricCalculator, MetricContext};

use super::drawn_balance_as_of;

/// Exposure at default on the valuation date, in the facility currency:
/// the drawn balance plus `leq` of the undrawn commitment plus the LC
/// sub-facility's `leq` of its outstanding face (Basel credit-conversion
/// treatment of the contingent exposure).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ExposureAtDefaultCalculator;

impl MetricCalculator for ExposureAtDefaultCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        let as_of = context.as_of;
        let drawn = drawn_balance_as_of(facility, as_of)?.amount();
        let lc = facility.lc_outstanding_at(as_of).amount();
        let undrawn = (facility.commitment_at(as_of).amount() - drawn - lc).max(0.0);
        Ok(drawn + facility.leq * undrawn + facility.lc_exposure_at_default(as_of))
    }
}

/// Expected credit loss embedded in the valuation, in the facility currency:
/// the value of the facility with its default model removed (no hazard
/// curve, no loan-equivalent or LC draw at default) less the base value.
/// Zero for a facility without a credit curve. A stochastic facility is
/// repriced on the same seed with a zero constant spread, so the difference
/// carries no Monte Carlo noise from the utilization paths.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ExpectedLossCalculator;

impl MetricCalculator for ExpectedLossCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        if facility.credit_curve_id.is_none() {
            return Ok(0.0);
        }
        let mut risk_free = facility.clone();
        risk_free.credit_curve_id = None;
        risk_free.leq = 0.0;
        if let Some(lc) = risk_free.lc.as_mut() {
            lc.leq = 0.0;
        }
        if let DrawRepaySpec::Stochastic(spec) = &mut risk_free.draw_repay_spec {
            // Zero the spread process but keep the rest of the configuration
            // (seed, correlation, rate process) so every utilization path is
            // identical to the base run and the difference carries no Monte
            // Carlo noise.
            if let Some(config) = spec.mc_config.as_mut() {
                config.credit_spread_process = CreditSpreadProcessSpec::Constant(0.0);
            }
        }
        let pv_risk_free = risk_free.value(&context.curves, context.as_of)?.amount();
        Ok(pv_risk_free - context.base_value.amount())
    }
}
