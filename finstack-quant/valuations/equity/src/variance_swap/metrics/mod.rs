//! Variance swap metrics module.
//!
//! Split into focused calculators similar to other instruments. This `mod.rs`
//! re-exports the calculators and provides a registry hookup.

mod dv01;
mod expected_variance;
mod notional;
mod realized_variance;
mod strike_vol;
mod time_to_maturity;
mod variance_vega;
mod vega;

pub(crate) use dv01::Dv01Calculator;
pub(crate) use expected_variance::ExpectedVarianceCalculator;
pub(crate) use notional::VarianceNotionalCalculator;
pub(crate) use realized_variance::RealizedVarianceCalculator;
pub(crate) use strike_vol::StrikeVolCalculator;
pub(crate) use time_to_maturity::TimeToMaturityCalculator;
pub(crate) use variance_vega::VarianceVegaCalculator;
pub(crate) use vega::VegaCalculator;

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register variance swap metrics with the registry.
pub(crate) fn register_variance_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::VarianceSwap,
        metrics: [
            (Vega, VegaCalculator),
            (Dv01, Dv01Calculator),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            (VarianceVega, VarianceVegaCalculator),
            (ExpectedVariance, ExpectedVarianceCalculator),
            (RealizedVariance, RealizedVarianceCalculator),
            (VarianceNotional, VarianceNotionalCalculator),
            (VarianceStrikeVol, StrikeVolCalculator),
            (VarianceTimeToMaturity, TimeToMaturityCalculator),
        ]
    };
    Ok(())
}
