//! Exchange-listed options on equity futures.

use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations_core::instruments::FutureOptionTerms;

finstack_quant_valuations_core::define_future_option! {
    /// Exchange-listed option on an arbitrary equity or equity-index futures contract.
    pub struct EquityFutureOption;
    instrument_type: finstack_quant_valuations_core::pricer::InstrumentType::EquityFutureOption,
    family: crate::EQUITY,
    register_metrics: register_equity_future_option_metrics,
}

impl EquityFutureOption {
    /// Create a neutral schema and serialization example.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Ok(Self {
            instrument_pricing_overrides:
                finstack_quant_valuations_core::instruments::InstrumentPricingOverrides::default()
                    .with_implied_volatility(0.20),
            ..Self::builder()
                .id(InstrumentId::new("EQUITY-FUTURE-OPTION-EXAMPLE"))
                .terms(FutureOptionTerms::example()?)
                .build()?
        })
    }
}
