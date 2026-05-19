//! CDS Option metrics module.
//!
//! Provides metric calculators specific to `CDSOption`, split into focused
//! files. The calculators compose with the shared metrics framework and are
//! registered via `register_cds_option_metrics`.
//!
//! Exposed metrics:
//! - Delta, Gamma, Vega, Theta
//! - CS01 (quoted CDS spread sensitivity)
//! - DV01 (swap-curve quote sensitivity; canonical IR sensitivity)
//! - ParSpread (Bloomberg CDSO ATM forward spread in bp)
//! - Implied Volatility (placeholder)

mod cs01;
mod delta;
mod dv01;
mod gamma;
mod implied_vol;
mod par_spread;
mod recovery01;
mod spread_dv01;
mod theta;
// risk_bucketed_dv01 - now using generic implementation
mod vega;

use crate::metrics::MetricRegistry;

/// Register all CDS Option metrics with the registry
pub(crate) fn register_cds_option_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    // Recovery01 (custom metric - recovery rate sensitivity)
    registry.register_metric(
        MetricId::Recovery01,
        Arc::new(recovery01::Recovery01Calculator),
        &[InstrumentType::CDSOption],
    );

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CDSOption,
        metrics: [
            (Delta, delta::DeltaCalculator),
            (Gamma, gamma::GammaCalculator),
            (Vega, vega::VegaCalculator),
            (Cs01, cs01::Cs01Calculator),
            (SpreadDv01, spread_dv01::UnderlyingSpreadDv01Calculator),
            (Dv01, dv01::CdsOptionDv01Calculator),
            (Theta, theta::ThetaCalculator),
            (ParSpread, par_spread::ParSpreadCalculator),
            (ImpliedVol, implied_vol::ImpliedVolCalculator),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::CDSOption,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
