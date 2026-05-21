//! Swaption metrics module.
//!
//! Contains per-metric calculators split into separate files for clarity and
//! maintainability. The module exposes a registration helper to wire metrics
//! into the shared `MetricRegistry`.

#![allow(dead_code)] // WIP: public API not yet wired into main pricing paths

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
pub(crate) fn register_swaption_metrics(registry: &mut MetricRegistry) {
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
            (Rho, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Swaption,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (ImpliedVol, ImpliedVolCalculator),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Swaption,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}

/// Register Bermudan swaption metrics with the registry.
///
/// # Arguments
///
/// * `registry` - The metric registry to register calculators with
/// * `hw_params` - Calibrated Hull-White parameters for the Bermudan Greek calculators.
///   These should be calibrated to co-terminal European swaption prices from
///   the volatility surface. Using uncalibrated defaults can produce Bermudan
///   premium errors of 10-30% of the early exercise value.
///
/// # Important
///
/// `BermudanSwaption::value()` is not implemented (it requires a tree/LSMC pricer),
/// so generic calculators like `UnifiedDv01Calculator` that rely on
/// `Instrument::value()` cannot be used. Only bump-and-revalue calculators that use
/// the explicit tree pricer are registered.
///
/// # Example
///
/// ```ignore
/// use finstack_valuations::instruments::rates::swaption::HullWhiteParams;
///
/// // Calibrate parameters to your vol surface first
/// let calibrated_params = HullWhiteParams::new(0.05, 0.008);
///
/// let mut registry = MetricRegistry::new();
/// register_bermudan_swaption_metrics(&mut registry, calibrated_params);
/// ```
#[allow(dead_code)]
pub(crate) fn register_bermudan_swaption_metrics(
    registry: &mut MetricRegistry,
    hw_params: crate::calibration::hull_white::HullWhiteParams,
) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::BermudanSwaption,
        metrics: [
            (Delta, BermudanDeltaCalculator::new()
                .with_hw_params(hw_params.kappa, hw_params.sigma)),
            (Gamma, BermudanGammaCalculator::new()
                .with_hw_params(hw_params.kappa, hw_params.sigma)),
            (Vega, BermudanVegaCalculator::new()
                .with_hw_params(hw_params.kappa, hw_params.sigma))
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
        std::sync::Arc::new(ExerciseProbabilityCalculator::new_with_hw(
            hw_params.kappa,
            hw_params.sigma,
        )),
        &[InstrumentType::BermudanSwaption],
    );
}

/// Convert a (possibly lognormal) volatility to a normal (Bachelier) vol for a
/// swaption greek's negative-rate fallback.
///
/// The swaption Black greeks (`delta`, `gamma`, `vega`) fall back to the
/// Bachelier model when the forward swap rate or strike is non-positive. In
/// that case `inputs.sigma` resolved from SABR or a lognormal vol surface is a
/// **lognormal** vol — feeding it straight into a Bachelier greek mis-scales
/// the result by roughly a factor of the forward rate. This helper converts it
/// via the standard lognormal→normal mapping, using any configured SABR shift
/// so the conversion can operate on positive shifted rates.
///
/// (When the swaption's `vol_model` is itself `Normal`, `sigma` is already a
/// normal vol and callers must NOT invoke this helper.)
pub(super) fn resolved_normal_sigma(
    option: &crate::instruments::rates::swaption::Swaption,
    forward: f64,
    strike: f64,
    sigma: f64,
    time_to_expiry: f64,
) -> f64 {
    let shift = option.sabr_params.as_ref().and_then(|p| p.shift);
    crate::instruments::rates::swaption::types::lognormal_to_normal_vol(
        sigma,
        forward,
        strike,
        time_to_expiry,
        shift,
    )
}

/// Swaption metrics configuration constants.
/// Centralizes scaling parameters to avoid magic numbers in calculators.
pub(crate) mod config {

    /// Basis points per unit (1.0 = 1 bp; 10000.0 = 100%).
    pub(crate) const BP_PER_UNIT: f64 = 10000.0;
    /// Vega scale for 1% volatility change.
    pub(crate) const VOL_PCT_SCALE: f64 = 100.0;
    /// Trading days per year for theta scaling (daily).
    pub(crate) const THETA_DAYS_PER_YEAR: f64 = 365.0;
    /// Discount curve parallel bump size in basis points for rho.
    pub(crate) const DISC_BUMP_BP: f64 = 1.0;
    /// Multiplier to convert 1bp PV change to per 1% rate move.
    pub(crate) const RHO_PCT_PER_BP: f64 = 100.0;
}
