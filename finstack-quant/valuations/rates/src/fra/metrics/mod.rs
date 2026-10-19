//! FRA-specific metrics module.
//!
//! Provides metric calculators for FRAs. Metrics include:
//! - DV01 (parallel rate sensitivity via generic calculator)
//! - Par rate
//! - Bucketed DV01 (key-rate sensitivity)
//!
//! Note: PV is available in `ValuationResult.value`, not as a metric in measures.
//!
//! See unit tests and `examples/` for usage.

mod dv01;

use crate::fra::ForwardRateAgreement;
pub(crate) use dv01::FraRateCurveDv01Calculator;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Registers all FRA metrics to a registry.
///
/// Each metric is registered with the "FRA" instrument type to ensure
/// proper applicability filtering.
pub(crate) fn register_fra_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    // Standard metrics using macro
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Fra,
        metrics: [
            (Dv01, FraRateCurveDv01Calculator),
            (Pv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_forward_only())),
            (ForwardPv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_forward_only())),
            (ParRate, FraParRateCalculator),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}

/// Par rate for FRAs (fixed rate that zeroes PV).
///
/// Computes the fixed rate that makes the FRA's PV zero under current curves.
/// For standard FRA settlement-at-start conventions and consistent curves,
/// this equals the forward rate over the period:
///
/// par_rate = (projection_df(start) / projection_df(end) - 1) / index_accrual
///
/// Discount-factor time mapping uses the forward curve's own day count and base
/// date. Annualization uses the FRA's contractual index day count.
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

        fwd.as_ref().rate_between_dates(
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
