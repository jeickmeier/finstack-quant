//! Exchange-listed options on commodity futures.

use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations_core::instruments::FutureOptionTerms;

finstack_quant_valuations_core::define_future_option! {
    /// Exchange-listed option on an arbitrary commodity futures contract.
    pub struct CommodityFutureOption;
    instrument_type: finstack_quant_valuations_core::pricer::InstrumentType::CommodityFutureOption,
    family: crate::EXOTICS,
    register_metrics: register_commodity_future_option_metrics,
}

impl CommodityFutureOption {
    /// Create a neutral schema and serialization example.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Ok(Self {
            instrument_pricing_overrides:
                finstack_quant_valuations_core::instruments::InstrumentPricingOverrides::default()
                    .with_implied_volatility(0.20),
            ..Self::builder()
                .id(InstrumentId::new("COMMODITY-FUTURE-OPTION-EXAMPLE"))
                .terms(FutureOptionTerms::example()?)
                .build()?
        })
    }
}
