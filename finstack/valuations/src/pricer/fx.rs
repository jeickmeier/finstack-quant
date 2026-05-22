//! Pricer registrations for FX instruments.
//!
//! Covers: FxSpot, FxSwap, XccySwap, FxOption, FxVarianceSwap, FxForward, Ndf,
//! FxBarrierOption, FxDigitalOption, FxTouchOption.

use super::{register_generic, InstrumentType, ModelKey, PricerRegistry};

/// Register pricers for FX instruments.
pub fn register_fx_pricers(registry: &mut PricerRegistry) {
    // FX Spot
    register_generic!(registry, InstrumentType::FxSpot, crate::instruments::FxSpot);

    // FX Swap
    register_generic!(registry, InstrumentType::FxSwap, crate::instruments::FxSwap);

    // XCCY Swap
    register_generic!(
        registry,
        InstrumentType::XccySwap,
        crate::instruments::XccySwap
    );

    // FX Option
    registry.register(
        InstrumentType::FxOption,
        ModelKey::Black76,
        crate::instruments::fx::fx_option::pricer::SimpleFxOptionBlackPricer,
    );

    // FX Variance Swap
    registry.register(
        InstrumentType::FxVarianceSwap,
        ModelKey::Discounting,
        crate::instruments::fx::fx_variance_swap::pricer::SimpleFxVarianceSwapDiscountingPricer,
    );

    // FX Forward - uses GenericInstrumentPricer (curve dependencies)
    register_generic!(
        registry,
        InstrumentType::FxForward,
        crate::instruments::FxForward
    );

    // NDF (Non-Deliverable Forward) - uses GenericInstrumentPricer (curve dependencies)
    register_generic!(registry, InstrumentType::Ndf, crate::instruments::Ndf);

    // FX Barrier Option

    registry.register(
        InstrumentType::FxBarrierOption,
        ModelKey::MonteCarloGBM,
        crate::instruments::fx::fx_barrier_option::pricer::FxBarrierOptionMcPricer::default(),
    );
    registry.register(
        InstrumentType::FxBarrierOption,
        ModelKey::FxBarrierBSContinuous,
        crate::instruments::fx::fx_barrier_option::pricer::FxBarrierOptionAnalyticalPricer,
    );
    // Vanna-Volga remains an internal helper until market smile quotes are part
    // of the instrument/market contract. Do not register a standard route that
    // cannot be parameterized per trade.

    // FX Digital Option
    registry.register(
        InstrumentType::FxDigitalOption,
        ModelKey::Black76,
        crate::instruments::fx::fx_digital_option::SimpleFxDigitalOptionPricer,
    );

    // FX Touch Option
    registry.register(
        InstrumentType::FxTouchOption,
        ModelKey::Black76,
        crate::instruments::fx::fx_touch_option::SimpleFxTouchOptionPricer,
    );
}
