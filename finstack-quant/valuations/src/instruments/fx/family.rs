//! The FX family: spot, forwards, swaps, NDFs, futures and FX options.

use crate::metrics::{MetricRegistry, MetricRegistryError};
use crate::pricer::{
    register_generic, Family, InstrumentType, ModelKey, PricerRegistry, PricingError,
};

/// Registration unit for the FX instruments.
pub static FX: Family = Family::new("fx", register_pricers, register_metrics);

fn register_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    register_generic!(registry, InstrumentType::FxSpot, crate::instruments::FxSpot);
    register_generic!(
        registry,
        InstrumentType::FxForward,
        crate::instruments::FxForward
    );
    register_generic!(registry, InstrumentType::FxSwap, crate::instruments::FxSwap);
    register_generic!(registry, InstrumentType::Ndf, crate::instruments::Ndf);
    register_generic!(
        registry,
        InstrumentType::FxFuture,
        crate::instruments::FxFuture
    );
    register_generic!(
        registry,
        InstrumentType::FxFutureOption,
        crate::instruments::FxFutureOption
    );
    register_generic!(
        registry,
        InstrumentType::FxVarianceSwap,
        crate::instruments::FxVarianceSwap
    );

    // FX option: registered under the lognormal `Black76` key; the pricer
    // itself is Garman-Kohlhagen spot-form (mathematically equivalent via
    // the CIP forward). See `FxOption::default_model`.
    register_generic!(
        registry,
        InstrumentType::FxOption,
        crate::instruments::FxOption,
        ModelKey::Black76
    );
    register_generic!(
        registry,
        InstrumentType::FxDigitalOption,
        crate::instruments::FxDigitalOption,
        ModelKey::Black76
    );
    register_generic!(
        registry,
        InstrumentType::FxTouchOption,
        crate::instruments::FxTouchOption,
        ModelKey::Black76
    );
    registry.register(
        crate::instruments::fx::fx_barrier_option::pricer::FxBarrierOptionMcPricer::default(),
    )?;
    registry.register(
        crate::instruments::fx::fx_barrier_option::pricer::FxBarrierOptionAnalyticalPricer,
    )?;
    registry
        .register(crate::instruments::fx::quanto_option::pricer::QuantoOptionAnalyticalPricer)?;
    Ok(())
}

fn register_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    crate::instruments::fx::fx_spot::metrics::register_fx_spot_metrics(registry)?;
    crate::instruments::fx::fx_swap::metrics::register_fx_swap_metrics(registry)?;
    crate::instruments::fx::fx_forward::metrics::register_fx_forward_metrics(registry)?;
    crate::instruments::fx::fx_future::metrics::register_fx_future_metrics(registry)?;
    crate::instruments::fx::fx_future_option::metrics::register_fx_future_option_metrics(registry)?;
    crate::instruments::fx::ndf::metrics::register_ndf_metrics(registry)?;
    crate::instruments::fx::fx_option::metrics::register_fx_option_metrics(registry)?;
    crate::instruments::fx::fx_variance_swap::metrics::register_fx_variance_swap_metrics(registry)?;
    crate::instruments::fx::fx_barrier_option::metrics::register_fx_barrier_option_metrics(
        registry,
    )?;
    crate::instruments::fx::fx_digital_option::metrics::register_fx_digital_option_metrics(
        registry,
    )?;
    crate::instruments::fx::fx_touch_option::metrics::register_fx_touch_option_metrics(registry)?;
    crate::instruments::fx::quanto_option::metrics::register_quanto_option_metrics(registry)?;
    Ok(())
}
