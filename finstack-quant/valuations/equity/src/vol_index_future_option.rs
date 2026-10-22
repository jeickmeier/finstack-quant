//! Exchange-listed options on volatility-index futures.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_models::types::{ExerciseStyle, OptionType};
use finstack_quant_valuations_core::instruments::Position;
use finstack_quant_valuations_core::instruments::{
    FutureOptionModel, FutureOptionPremiumStyle, FutureOptionSettlement, FutureOptionTerms,
};

finstack_quant_valuations_core::define_future_option! {
    /// Exchange-listed option on a volatility-index futures contract.
    pub struct VolatilityIndexFutureOption;
    instrument_type: finstack_quant_valuations_core::pricer::InstrumentType::VolatilityIndexFutureOption,
    family: crate::EQUITY,
    register_metrics: register_vol_index_future_option_metrics,
}

impl VolatilityIndexFutureOption {
    /// Create a VSTOXX futures-option example for schema and serialization output.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use time::macros::date;

        let expiry = date!(2026 - 12 - 16);
        let terms = FutureOptionTerms::builder()
            .underlying("FVS".to_string())
            .futures_price(20.0)
            .option_reference_price(2.0)
            .strike(20.0)
            .contracts(1.0)
            .multiplier(100.0)
            .currency(Currency::EUR)
            .option_type(OptionType::Call)
            .position(Position::Long)
            .exercise_style(ExerciseStyle::American)
            .expiry(expiry)
            .settlement(FutureOptionSettlement::Future {
                underlying_last_trading_date: expiry,
                underlying_settlement_date: expiry,
                underlying_settlement_price: None,
            })
            .model(FutureOptionModel::Black76)
            .premium_style(FutureOptionPremiumStyle::FuturesStyle)
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("EUR-OIS"))
            .build()?;
        Ok(Self {
            instrument_pricing_overrides:
                finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::default()
                    .with_implied_volatility(0.50),
            ..Self::builder()
                .id(InstrumentId::new("VSTOXX-FUTURE-OPTION-EXAMPLE"))
                .terms(terms)
                .build()?
        })
    }
}
