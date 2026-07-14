//! Interest rate option metrics module.
//!
//! Provides metric calculators specific to `CapFloor`, split into
//! focused files. The calculators compose with the shared metrics framework
//! and are registered via `register_cap_floor_metrics`.
//!
//! Exposed metrics:
//! - Delta
//! - Gamma
//! - Vega (market normal-vol quote sensitivity)
//! - HwSigmaVega (direct Hull-White short-rate sigma sensitivity)
//! - Theta
//! - Rho
//! - ImpliedVol (placeholder)

mod common;
mod delta;
mod dv01;
mod forward_pv01;
mod gamma;
mod implied_vol;
mod theta;
mod vega;

use crate::metrics::MetricRegistry;

/// Register all CapFloor metrics with the registry
pub(crate) fn register_cap_floor_metrics(registry: &mut MetricRegistry) {
    use crate::instruments::rates::cap_floor::CapFloor;
    use crate::metrics::{Dv01CalculatorConfig, UnifiedDv01Calculator};
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CapFloor,
        metrics: [
            (Delta, delta::DeltaCalculator),
            (Gamma, gamma::GammaCalculator),
            (Vega, vega::VegaCalculator),
            (HwSigmaVega, vega::HwSigmaVegaCalculator),
            (Dv01, dv01::Dv01Calculator),
            (Theta, theta::ThetaCalculator),
            // Rho = parallel bump of the discount curve only. Routing through
            // the unified DV01 calculator keeps Rho aligned with the workspace
            // bump-size config and central-difference convention.
            (Rho, UnifiedDv01Calculator::<CapFloor>::new(
                Dv01CalculatorConfig::parallel_discount_only(),
            )),
            (ImpliedVol, implied_vol::ImpliedVolCalculator),
            (ForwardPv01, forward_pv01::ForwardPv01Calculator),
            (BucketedDv01, UnifiedDv01Calculator::<CapFloor>::new(
                Dv01CalculatorConfig::triangular_key_rate(),
            )),
        ]
    }
}
