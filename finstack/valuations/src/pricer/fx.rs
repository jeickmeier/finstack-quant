//! Pricer registrations for FX instruments.
//!
//! Covers: FxSpot, FxSwap, XccySwap, FxOption, FxVarianceSwap, FxForward, Ndf,
//! FxBarrierOption, FxDigitalOption, FxTouchOption.

use super::{register_generic, InstrumentType, ModelKey, PricerRegistry};

/// Register pricers for FX instruments.
pub(crate) fn register_fx_pricers(registry: &mut PricerRegistry) {
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

    // FX Option — registered under the lognormal `Black76` key; the pricer
    // itself is Garman-Kohlhagen spot-form (mathematically equivalent via
    // the CIP forward). See `FxOption::default_model`.
    register_generic!(
        registry,
        InstrumentType::FxOption,
        crate::instruments::FxOption,
        ModelKey::Black76
    );

    // FX Variance Swap
    register_generic!(
        registry,
        InstrumentType::FxVarianceSwap,
        crate::instruments::FxVarianceSwap
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
    register_generic!(
        registry,
        InstrumentType::FxDigitalOption,
        crate::instruments::FxDigitalOption,
        ModelKey::Black76
    );

    // FX Touch Option
    register_generic!(
        registry,
        InstrumentType::FxTouchOption,
        crate::instruments::FxTouchOption,
        ModelKey::Black76
    );
}
