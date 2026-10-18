//! The FX family: spot, forwards, swaps, NDFs, futures and FX options.

use finstack_quant_valuations_core::metrics::{MetricRegistry, MetricRegistryError};
use finstack_quant_valuations_core::pricer::{
    register_generic, Family, InstrumentType, ModelKey, PricerRegistry, PricingError,
};

/// Registration unit for the FX instruments.
pub static FX: Family = Family::new("fx", register_pricers, register_metrics);

fn register_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    register_generic!(registry, InstrumentType::FxSpot, crate::FxSpot);
    register_generic!(registry, InstrumentType::FxForward, crate::FxForward);
    register_generic!(registry, InstrumentType::FxSwap, crate::FxSwap);
    register_generic!(registry, InstrumentType::Ndf, crate::Ndf);
    register_generic!(registry, InstrumentType::FxFuture, crate::FxFuture);
    register_generic!(
        registry,
        InstrumentType::FxFutureOption,
        crate::FxFutureOption
    );
    register_generic!(
        registry,
        InstrumentType::FxVarianceSwap,
        crate::FxVarianceSwap
    );

    // FX option: registered under the lognormal `Black76` key; the pricer
    // itself is Garman-Kohlhagen spot-form (mathematically equivalent via
    // the CIP forward). See `FxOption::default_model`.
    register_generic!(
        registry,
        InstrumentType::FxOption,
        crate::FxOption,
        ModelKey::Black76
    );
    register_generic!(
        registry,
        InstrumentType::FxDigitalOption,
        crate::FxDigitalOption,
        ModelKey::Black76
    );
    register_generic!(
        registry,
        InstrumentType::FxTouchOption,
        crate::FxTouchOption,
        ModelKey::Black76
    );
    registry.register(crate::fx_barrier_option::pricer::FxBarrierOptionMcPricer::default())?;
    registry.register(crate::fx_barrier_option::pricer::FxBarrierOptionAnalyticalPricer)?;
    registry.register(crate::quanto_option::pricer::QuantoOptionAnalyticalPricer)?;
    Ok(())
}

fn register_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    crate::fx_spot::metrics::register_fx_spot_metrics(registry)?;
    crate::fx_swap::metrics::register_fx_swap_metrics(registry)?;
    crate::fx_forward::metrics::register_fx_forward_metrics(registry)?;
    crate::fx_future::metrics::register_fx_future_metrics(registry)?;
    crate::fx_future_option::metrics::register_fx_future_option_metrics(registry)?;
    crate::ndf::metrics::register_ndf_metrics(registry)?;
    crate::fx_option::metrics::register_fx_option_metrics(registry)?;
    crate::fx_variance_swap::metrics::register_fx_variance_swap_metrics(registry)?;
    crate::fx_barrier_option::metrics::register_fx_barrier_option_metrics(registry)?;
    crate::fx_digital_option::metrics::register_fx_digital_option_metrics(registry)?;
    crate::fx_touch_option::metrics::register_fx_touch_option_metrics(registry)?;
    crate::quanto_option::metrics::register_quanto_option_metrics(registry)?;
    Ok(())
}
