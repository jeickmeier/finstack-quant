//! Swaption metrics module.
//!
//! Contains per-metric calculators split into separate files for clarity and
//! maintainability. The module exposes a registration helper to wire metrics
//! into the shared `MetricRegistry`.

pub(crate) mod bermudan_greeks;
mod delta;
mod gamma;
mod implied_vol;
mod vega;

pub(crate) use bermudan_greeks::{
    BermudanDeltaCalculator, BermudanGammaCalculator, BermudanVegaCalculator,
    ExerciseProbabilityCalculator,
};
pub(crate) use delta::DeltaCalculator;
pub(crate) use gamma::GammaCalculator;
pub(crate) use implied_vol::ImpliedVolCalculator;
pub(crate) use vega::VegaCalculator;

use crate::metrics::MetricRegistry;

/// Register swaption metrics with the registry
pub(crate) fn register_swaption_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Swaption,
        metrics: [
            (Delta, DeltaCalculator),
            (Gamma, GammaCalculator),
            (Vega, VegaCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Swaption,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            // Theta is now registered universally in metrics::standard_registry()
            // Rho bumps ONLY the discount/funding curve (holding the forward swap
            // rate fixed) — this is the correct option rho definition.  Using
            // `parallel_combined()` would also move the forward curve, conflating
            // discounting sensitivity with delta and producing the wrong sign for
            // receiver swaptions and wrong magnitude for payers.
            (Rho, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Swaption,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_discount_only())),
            (ImpliedVol, ImpliedVolCalculator),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Swaption,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
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
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::BermudanSwaption,
        metrics: [
            (Delta, BermudanDeltaCalculator {
                bump_bp: bermudan_greeks::DEFAULT_RATE_BUMP_BP,
            }),
            (Gamma, BermudanGammaCalculator {
                bump_bp: bermudan_greeks::DEFAULT_GAMMA_BUMP_BP,
            }),
            // Registered under `HwSigmaVega`, NOT `Vega`: this is a Hull-White
            // short-rate σ bump (a model-parameter vega) and lives on a
            // different vol axis than the Black-vol `Vega` reported for
            // European swaptions. Sharing the `vega` key would silently mix
            // incomparable units in cross-instrument aggregation.
            (HwSigmaVega, BermudanVegaCalculator {
                bump_pct: bermudan_greeks::DEFAULT_VOL_BUMP_PCT,
            })
            // Note: UnifiedDv01Calculator and BucketedDv01 are NOT
            // registered here because BermudanSwaption::value() returns Err.
            // Use BermudanDeltaCalculator for rate sensitivity instead.
            // Theta is registered universally in metrics::standard_registry()
            // but will fail for BermudanSwaption for the same reason.
        ]
    }
    // Register custom ExerciseProbability metric separately
    registry.register_metric(
        crate::metrics::MetricId::custom("exercise_probability"),
        std::sync::Arc::new(ExerciseProbabilityCalculator),
        &[InstrumentType::BermudanSwaption],
    )?;
    Ok(())
}

/// Swaption metrics configuration constants.
/// Centralizes scaling parameters to avoid magic numbers in calculators.
pub(crate) mod config {
    /// Vega scale for 1% volatility change.
    pub(crate) const VOL_PCT_SCALE: f64 = 100.0;
}
