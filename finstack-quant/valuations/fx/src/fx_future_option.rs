//! Exchange-listed options on FX futures.

use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations_core::instruments::FutureOptionTerms;

finstack_quant_valuations_core::define_future_option! {
    /// Exchange-listed option on an arbitrary FX futures contract.
    pub struct FxFutureOption;
    instrument_type: finstack_quant_valuations_core::pricer::InstrumentType::FxFutureOption,
    family: crate::FX,
    register_metrics: register_fx_future_option_metrics,
}

impl FxFutureOption {
    /// Create a neutral schema and serialization example.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Ok(Self {
            instrument_pricing_overrides:
                finstack_quant_valuations_core::instruments::InstrumentPricingOverrides::default()
                    .with_implied_volatility(0.20),
            ..Self::builder()
                .id(InstrumentId::new("FX-FUTURE-OPTION-EXAMPLE"))
                .terms(FutureOptionTerms::example()?)
                .build()?
        })
    }
}
