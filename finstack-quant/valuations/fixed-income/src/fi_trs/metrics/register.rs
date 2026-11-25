//! Metric registration and calculators.

use super::ParSpreadCalculator;
use crate::fi_trs::FiIndexTotalReturnSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::PayReceive;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::pricer::InstrumentType;

/// Registers all fixed income TRS metrics with the metric registry.
///
/// # Arguments
/// * `registry` — Metric registry to add FI TRS metrics to
pub(crate) fn register_fi_trs_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FiIndexTotalReturnSwap,
        metrics: [
            // FI TRS specific metrics
            //
            // Duration-based yield sensitivity. Registered under DurationDv01 (not IndexDelta)
            // because this measures yield sensitivity (N × D × 1bp), which is conceptually
            // distinct from equity IndexDelta (dV/dS per unit of index level change).
            (DurationDv01, DurationDv01Calculator),
            (ParSpread, ParSpreadCalculator),
            (FinancingAnnuity, FinancingAnnuityCalculator),
            // DV01 for financing leg sensitivity
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }

    Ok(())
}

/// Calculates the financing annuity for a fixed income index TRS.
///
/// The annuity is the discounted notional-weighted accrual sum used by the
/// par-spread solve. Multiplying it by `0.0001` gives the PV of one basis point
/// of financing spread.
pub(crate) struct FinancingAnnuityCalculator;

impl MetricCalculator for FinancingAnnuityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let trs: &FiIndexTotalReturnSwap = context.instrument_as()?;
        trs.financing_annuity(context.curves.as_ref(), context.as_of)
    }
}

/// Calculates duration-based DV01 for fixed income index TRS.
///
/// Measures the dollar value change for a 1 basis point yield shift:
///
/// ```text
/// DurationDv01 = Notional × Duration × 0.0001
/// ```
///
/// This is a yield sensitivity metric (not an index-level delta). For equity TRS,
/// use `IndexDelta` which measures `dV/dS` per unit of index level change.
///
/// # Sign Convention
///
/// Returns **negative** for `ReceiveTotalReturn` and **positive** for
/// `PayTotalReturn`. A total-return receiver is economically long the reference
/// bond index, so its signed rate sensitivity is negative: the position loses
/// value when yields rise. This matches the ISDA SIMM IR-delta convention (see
/// the `Marginable::simm_sensitivities` implementation for this instrument) and
/// the workspace CS01 convention (long credit → negative to spread widening).
///
/// Note: the underlying FI-TRS pricer is carry/income-only — it has no price
/// mark-to-market term, so `dV/dy` of the carry model itself would not produce
/// this sign. The convention here is chosen for economic consistency with the
/// rest of the workspace, not derived from the carry model.
///
/// # Errors
///
/// Requires `duration_id` and its finite unitless market scalar
/// in years. Missing inputs or a monetary scalar return an error.
pub(crate) struct DurationDv01Calculator;

impl MetricCalculator for DurationDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let trs: &FiIndexTotalReturnSwap = context.instrument_as()?;
        if context.as_of >= trs.schedule.end {
            return Ok(0.0);
        }

        let duration = trs.index_duration(context.curves.as_ref())?;

        // DV01 = Notional × Duration × 1bp
        let dv01 = trs.notional.amount() * duration * 0.0001;

        // Long the reference bond (receive TR) loses when yields rise → negative.
        Ok(match trs.side {
            PayReceive::Receive => -dv01,
            PayReceive::Pay => dv01,
        })
    }
}
