//! FX option metrics module.
//!
//! Splits FX option metrics into focused calculators per greek and registers
//! them with the `MetricRegistry`. Calculators reuse the pricing engine
//! helpers to ensure consistency between PV and greeks.

mod delta_conventions;
mod implied_vol;

use crate::metrics::MetricRegistry;

/// Register FX option metrics with the registry.
pub(crate) fn register_fx_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::metrics::{
        make_fx_bumper, make_rates_bumper, make_vol_bumper, CrossFactorCalculator, MetricId,
    };
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    // Standard metrics for rho split by domestic/foreign.
    registry.register_metric(
        MetricId::Rho,
        Arc::new(crate::metrics::OptionGreekCalculator::<
            crate::instruments::FxOption,
        >::rho()),
        &[InstrumentType::FxOption],
    )?;
    registry.register_metric(
        MetricId::ForeignRho,
        Arc::new(crate::metrics::OptionGreekCalculator::<
            crate::instruments::FxOption,
        >::foreign_rho()),
        &[InstrumentType::FxOption],
    )?;
    registry.register_metric(
        MetricId::DeltaForward,
        Arc::new(delta_conventions::DeltaForwardCalculator),
        &[InstrumentType::FxOption],
    )?;
    registry.register_metric(
        MetricId::DeltaPremiumAdjustedSpot,
        Arc::new(delta_conventions::DeltaPremiumAdjustedSpotCalculator),
        &[InstrumentType::FxOption],
    )?;
    registry.register_metric(
        MetricId::DeltaPremiumAdjustedForward,
        Arc::new(delta_conventions::DeltaPremiumAdjustedForwardCalculator),
        &[InstrumentType::FxOption],
    )?;

    registry.register_metric(
        MetricId::CrossGammaFxVol,
        Arc::new(CrossFactorCalculator::new(make_fx_bumper, make_vol_bumper)),
        &[InstrumentType::FxOption],
    )?;
    registry.register_metric(
        MetricId::CrossGammaFxRates,
        Arc::new(CrossFactorCalculator::new(
            make_fx_bumper,
            make_rates_bumper,
        )),
        &[InstrumentType::FxOption],
    )?;

    // Standard metrics using macro
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxOption,
        metrics: [
            (Delta, crate::metrics::OptionGreekCalculator::<crate::instruments::FxOption>::delta()),
            (Gamma, crate::metrics::OptionGreekCalculator::<crate::instruments::FxOption>::gamma()),
            (Vega, crate::metrics::OptionGreekCalculator::<crate::instruments::FxOption>::vega()),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::FxOption,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            // Override universal theta (carry) with model theta for FX options.
            (Theta, crate::metrics::OptionGreekCalculator::<crate::instruments::FxOption>::theta()),
            (ImpliedVol, implied_vol::ImpliedVolCalculator),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::FxOption,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            (Vanna, crate::metrics::OptionGreekCalculator::<crate::instruments::FxOption>::vanna()),
            (Volga, crate::metrics::OptionGreekCalculator::<crate::instruments::FxOption>::volga()),
        ]
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;

    #[test]
    fn registers_fx_delta_convention_metrics() {
        let mut registry = MetricRegistry::new();
        register_fx_option_metrics(&mut registry).expect("FX option metric registration");
        let metrics = registry.metrics_for_instrument(InstrumentType::FxOption);

        assert!(metrics.contains(&MetricId::DeltaForward));
        assert!(metrics.contains(&MetricId::DeltaPremiumAdjustedSpot));
        assert!(metrics.contains(&MetricId::DeltaPremiumAdjustedForward));
    }
}
