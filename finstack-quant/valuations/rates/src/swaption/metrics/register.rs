//! Metric registration and calculators.

use super::{
    BermudanDeltaCalculator, BermudanGammaCalculator, BermudanVegaCalculator, DeltaCalculator,
    GammaCalculator, ImpliedVolCalculator, VegaCalculator,
};
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register swaption metrics with the registry
pub(crate) fn register_swaption_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Swaption,
        metrics: [
            (Delta, DeltaCalculator),
            (Gamma, GammaCalculator),
            (Vega, VegaCalculator),
            (HwSigmaVega, super::vega::HwSigmaVegaCalculator),
            (Dv01, crate::rate_bucketed_dv01::RateDv01(false)),
            // Rho bumps ONLY the discount/funding curve (holding the forward swap
            // rate fixed) — this is the correct option rho definition.  Using
            // `parallel_combined()` would also move the forward curve, conflating
            // discounting sensitivity with delta and producing the wrong sign for
            // receiver swaptions and wrong magnitude for payers.
            (Rho, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_discount_only())),
            (ImpliedVol, ImpliedVolCalculator),
            (BucketedDv01, crate::rate_bucketed_dv01::RateDv01(true)),
        ]
    }
    Ok(())
}

/// Register Bermudan swaption metrics with the registry.
///
/// # Arguments
///
/// * `registry` - The metric registry to register calculators with
/// # Important
///
/// `BermudanSwaption::value()` is not implemented (it requires a tree/LSMC pricer),
/// so generic calculators like `UnifiedDv01Calculator` that rely on
/// `Instrument::value()` cannot be used. Only bump-and-revalue calculators that use
/// the explicit tree pricer are registered.
pub(crate) fn register_bermudan_swaption_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::BermudanSwaption,
        metrics: [
            (Delta, BermudanDeltaCalculator),
            (Gamma, BermudanGammaCalculator {
                bump_bp: super::bermudan_greeks::DEFAULT_GAMMA_BUMP_BP,
            }),
            // Registered under `HwSigmaVega`, NOT `Vega`: this is a Hull-White
            // short-rate σ bump (a model-parameter vega) and lives on a
            // different vol axis than the Black-vol `Vega` reported for
            // European swaptions. Sharing the `vega` key would silently mix
            // incomparable units in cross-instrument aggregation.
            (HwSigmaVega, BermudanVegaCalculator),
            // Published by the Hull-White tree pricer from its own exercise
            // policy (`seed_metric_context`). Models that do not publish it
            // (LSMC rejects it, LMM has no tree) fail clearly instead of
            // recalibrating a Hull-White tree under a different model.
            (
                ExpectedExerciseTime,
                finstack_quant_valuations_core::metrics::PricerPublishedMetric::new(
                    finstack_quant_valuations_core::metrics::MetricId::ExpectedExerciseTime,
                )
            )
            // Note: UnifiedDv01Calculator and BucketedDv01 are NOT
            // registered here because BermudanSwaption::value() returns Err.
            // Use BermudanDeltaCalculator for rate sensitivity instead.
            // Theta is registered universally in crate::RATES.metrics()
            // but will fail for BermudanSwaption for the same reason.
        ]
    }
    Ok(())
}
