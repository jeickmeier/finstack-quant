//! Autocallable metrics module.
//!
//! Provides full greek coverage for autocallable structured products using
//! finite difference methods. Delta, Gamma and Vanna run against the
//! spot-pinned instrument (see [`crate::spot_pinned`]).
//! Note: Autocallables exhibit discontinuities near autocall barrier levels.

use crate::autocallable::Autocallable;
use crate::spot_pinned::{SpotPinnable, SpotPinnedFd};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::types::PriceId;
use finstack_quant_valuations_core::metrics::MetricRegistry;

impl SpotPinnable for Autocallable {
    fn unpinned_spot(&self) -> Option<(&PriceId, Currency)> {
        self.initial_level
            .is_none()
            .then(|| (&self.spot_id, self.notional.currency()))
    }

    fn pinned_at(&self, spot: f64, _as_of: Date) -> Self {
        let mut pinned = self.clone();
        pinned.initial_level = Some(spot);
        pinned
    }
}

/// Register autocallable metrics with the registry.
pub(crate) fn register_autocallable_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::fd_greeks::{
        GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Autocallable,
        metrics: [
            (Vega, finstack_quant_valuations_core::metrics::sensitivities::fd_greeks::GenericFdVega::<crate::autocallable::Autocallable>::default()),
            (Rho, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            (Delta, SpotPinnedFd::<Autocallable, _>::new(GenericFdDelta::<Autocallable>::default())),
            (Gamma, SpotPinnedFd::<Autocallable, _>::new(GenericFdGamma::<Autocallable>::default())),
            (Vanna, SpotPinnedFd::<Autocallable, _>::new(GenericFdVanna::<Autocallable>::default())),
            (Volga, GenericFdVolga::<crate::autocallable::Autocallable>::default()),
        ]
    }
    Ok(())
}
