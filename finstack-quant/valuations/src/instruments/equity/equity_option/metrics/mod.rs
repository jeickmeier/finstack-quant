//! Equity option metrics module.
//!
//! Splits equity option metrics into focused calculators per greek and
//! registers them with the `MetricRegistry`. Calculators reuse the pricing
//! engine helpers to ensure consistency between PV and greeks.

mod charm;
mod color;
mod dividend_risk;
mod implied_vol;
mod speed;

use crate::metrics::MetricRegistry;

/// Register equity option metrics with the registry.
pub(crate) fn register_equity_option_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::{
        make_spot_bumper, make_vol_bumper, CrossFactorCalculator, CrossFactorPair, MetricId,
    };
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    // Custom metric: Dividend risk (dividend yield sensitivity per 1bp)
    registry.register_metric(
        MetricId::Dividend01,
        Arc::new(dividend_risk::DividendRiskCalculator),
        &[InstrumentType::EquityOption],
    );
    registry.register_metric(
        MetricId::CrossGammaSpotVol,
        Arc::new(CrossFactorCalculator::new(
            CrossFactorPair::SpotVol,
            make_spot_bumper,
            make_vol_bumper,
        )),
        &[InstrumentType::EquityOption],
    );

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::EquityOption,
        metrics: [
            (Delta, crate::metrics::OptionGreekCalculator::<crate::instruments::EquityOption>::delta()),
            (Gamma, crate::metrics::OptionGreekCalculator::<crate::instruments::EquityOption>::gamma()),
            (Vega, crate::metrics::OptionGreekCalculator::<crate::instruments::EquityOption>::vega()),
            (BucketedVega, crate::metrics::KeyRateVega::<
                crate::instruments::EquityOption,
            >::standard()),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::EquityOption,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::EquityOption,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            (Theta, crate::metrics::OptionGreekCalculator::<crate::instruments::EquityOption>::theta()),
            (Rho, crate::metrics::OptionGreekCalculator::<crate::instruments::EquityOption>::rho()),
            (ImpliedVol, implied_vol::ImpliedVolCalculator),
            (Vanna, crate::metrics::OptionGreekCalculator::<crate::instruments::EquityOption>::vanna()),
            (Volga, crate::metrics::OptionGreekCalculator::<crate::instruments::EquityOption>::volga()),
            (Charm, charm::CharmCalculator),
            (Color, color::ColorCalculator),
            (Speed, speed::SpeedCalculator),
        ]
    }
}
