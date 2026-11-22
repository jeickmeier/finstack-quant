//! Equity option fixture builders (also `#[path]`-included by the umbrella
//! crate for cross-family metrics and option-bounds suites).

use finstack_quant_core::types::Attributes;
use finstack_quant_core::{
    currency::Currency,
    dates::Date,
    types::{CurveId, InstrumentId},
};
use finstack_quant_models::types::{ExerciseStyle, OptionType};
use finstack_quant_valuations_core::instruments::SettlementType;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_equity::EquityUnderlyingParams;
use finstack_quant_valuations_equity::equity_option::EquityOption;

/// Create an Equity European call option using the builder pattern.
pub fn equity_option_european_call(
    id: impl Into<String>,
    ticker: impl Into<String>,
    strike: f64,
    expiry: Date,
    contract_size: f64,
) -> finstack_quant_core::Result<EquityOption> {
    let ticker = ticker.into();
    let underlying = EquityUnderlyingParams::new(ticker, "EQUITY-SPOT", Currency::USD)
        .with_div_yield_id("EQUITY-DIVYIELD");

    EquityOption::builder()
        .id(InstrumentId::new(id.into()))
        .underlying_ticker(underlying.ticker)
        .strike(strike)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(contract_size)
        .currency(underlying.currency)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .settlement(SettlementType::Cash)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id(underlying.spot_id)
        .vol_surface_id(CurveId::new("EQUITY-VOL"))
        .div_yield_id_opt(underlying.div_yield_id)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
}

/// Create an Equity European put option using the builder pattern.
pub fn equity_option_european_put(
    id: impl Into<String>,
    ticker: impl Into<String>,
    strike: f64,
    expiry: Date,
    contract_size: f64,
) -> finstack_quant_core::Result<EquityOption> {
    let ticker = ticker.into();
    let underlying = EquityUnderlyingParams::new(ticker, "EQUITY-SPOT", Currency::USD)
        .with_div_yield_id("EQUITY-DIVYIELD");

    EquityOption::builder()
        .id(InstrumentId::new(id.into()))
        .underlying_ticker(underlying.ticker)
        .strike(strike)
        .option_type(OptionType::Put)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(contract_size)
        .currency(underlying.currency)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .settlement(SettlementType::Cash)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id(underlying.spot_id)
        .vol_surface_id(CurveId::new("EQUITY-VOL"))
        .div_yield_id_opt(underlying.div_yield_id)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
}

/// Create an Equity American call option using the builder pattern.
pub fn equity_option_american_call(
    id: impl Into<String>,
    ticker: impl Into<String>,
    strike: f64,
    expiry: Date,
    contract_size: f64,
) -> finstack_quant_core::Result<EquityOption> {
    let ticker = ticker.into();
    let underlying = EquityUnderlyingParams::new(ticker, "EQUITY-SPOT", Currency::USD)
        .with_div_yield_id("EQUITY-DIVYIELD");

    EquityOption::builder()
        .id(InstrumentId::new(id.into()))
        .underlying_ticker(underlying.ticker)
        .strike(strike)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::American)
        .expiry(expiry)
        .quantity(contract_size)
        .currency(underlying.currency)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .settlement(SettlementType::Cash)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id(underlying.spot_id)
        .vol_surface_id(CurveId::new("EQUITY-VOL"))
        .div_yield_id_opt(underlying.div_yield_id)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
}
