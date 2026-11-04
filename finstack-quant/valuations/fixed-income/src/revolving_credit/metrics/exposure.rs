//! Credit exposure metrics that make the undrawn commitment visible: exposure
//! at default, the expected loss embedded in the valuation, recovery01 and
//! jump-to-default.

use crate::revolving_credit::RevolvingCredit;
use crate::revolving_credit::types::{CreditSpreadProcessSpec, DrawRepaySpec};
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::sensitivities::recovery01::hazard_recovery01;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

use super::{drawn_at, undrawn_at};

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
        let drawn = drawn_at(facility, as_of, as_of)?.amount();
        let undrawn = undrawn_at(facility, as_of, drawn);
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

/// The hazard curve of a deterministic facility priced on one, or the error
/// saying it is not one. A stochastic facility takes survival from its simulated
/// spread path, not from the curve.
fn hazard_curve_id(
    facility: &RevolvingCredit,
    metric: &str,
) -> finstack_quant_core::Result<CurveId> {
    match &facility.credit_curve_id {
        Some(id) if facility.is_deterministic() => Ok(id.clone()),
        _ => Err(finstack_quant_core::Error::Validation(format!(
            "{metric} for revolving credit '{}' requires hazard pricing: a credit_curve_id \
             and a deterministic draw schedule",
            facility.id
        ))),
    }
}

/// `recovery01`: PV change in currency per +1% (0.01 absolute) move in the
/// facility's `recovery_rate`, the recovery its pricer applies; the hazard curve's
/// own recovery enters no default leg.
///
/// A central finite difference of the full PV at `R ± 0.01`, one-sided at a bound.
/// The convention is the shared one
/// ([`finstack_quant_valuations_core::metrics::sensitivities::recovery01`]): a
/// curve carrying its calibration recipe is re-bootstrapped to the same par spreads
/// with its recovery moved by the same amount; a hand-built curve keeps its hazard
/// rates. A higher recovery also recovers more on a draw at default.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Recovery01Calculator;

impl MetricCalculator for Recovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        let hazard_id = hazard_curve_id(facility, "recovery01")?;
        let as_of = context.as_of;
        hazard_recovery01(
            context,
            hazard_id.as_str(),
            facility.discount_curve_id.clone(),
            Some(facility.recovery_rate),
            "Revolving credit Recovery01",
            |market, recovery| {
                let mut bumped = facility.clone();
                bumped.recovery_rate = recovery;
                bumped.value_raw(market, as_of)
            },
        )
    }
}

/// `jump_to_default`: value on immediate default less current value, in currency,
/// for the whole facility, from the lender's perspective.
///
/// ```text
/// value on default = R × drawn + (R − 1) × (leq × undrawn + LC draw)
/// jump_to_default  = value on default − PV(as_of)
/// ```
///
/// The value on default is what the default leg of the pricer pays at `as_of`,
/// with no accrued interest: `R` of the drawn balance, plus the draw at default
/// (see [`ExposureAtDefaultCalculator`]), funded at par and recovered at `R`.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct JumpToDefaultCalculator;

impl MetricCalculator for JumpToDefaultCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        hazard_curve_id(facility, "jump_to_default")?;
        let as_of = context.as_of;
        let recovery = facility.recovery_rate;
        let drawn = drawn_at(facility, as_of, as_of)?.amount();
        let draw_at_default = facility.leq * undrawn_at(facility, as_of, drawn)
            + facility.lc_exposure_at_default(as_of);
        Ok(recovery * drawn + (recovery - 1.0) * draw_at_default - context.base_value.amount())
    }
}
