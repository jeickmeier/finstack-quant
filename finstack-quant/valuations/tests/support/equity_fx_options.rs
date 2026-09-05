use finstack_quant_core::{
    currency::Currency,
    dates::Date,
    money::Money,
    types::{CurveId, InstrumentId},
};
use finstack_quant_valuations::instruments::fx::fx_option::{
    FxDeltaConvention, FxDeltaConventionKind,
};
use finstack_quant_valuations::instruments::{
    Attributes, EquityOption, EquityUnderlyingParams, ExerciseStyle, FxOption, FxUnderlyingParams,
    InstrumentPricingOverrides, OptionType, SettlementType,
};

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
        .with_dividend_yield("EQUITY-DIVYIELD");

    EquityOption::builder()
        .id(InstrumentId::new(id.into()))
        .underlying_ticker(underlying.ticker)
        .strike(strike)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .notional(Money::new(contract_size, underlying.currency).expect("valid money fixture"))
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
        .with_dividend_yield("EQUITY-DIVYIELD");

    EquityOption::builder()
        .id(InstrumentId::new(id.into()))
        .underlying_ticker(underlying.ticker)
        .strike(strike)
        .option_type(OptionType::Put)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .notional(Money::new(contract_size, underlying.currency).expect("valid money fixture"))
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
        .with_dividend_yield("EQUITY-DIVYIELD");

    EquityOption::builder()
        .id(InstrumentId::new(id.into()))
        .underlying_ticker(underlying.ticker)
        .strike(strike)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::American)
        .expiry(expiry)
        .notional(Money::new(contract_size, underlying.currency).expect("valid money fixture"))
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

/// Create an FX European call option using the builder pattern.
pub fn fx_option_european_call(
    id: impl Into<InstrumentId>,
    base_currency: Currency,
    quote_currency: Currency,
    strike: f64,
    expiry: Date,
    notional: Money,
    vol_surface_id: impl Into<CurveId>,
) -> finstack_quant_core::Result<FxOption> {
    let fx_underlying = if quote_currency == Currency::USD && base_currency == Currency::EUR {
        FxUnderlyingParams::usd_eur()
    } else if quote_currency == Currency::USD && base_currency == Currency::GBP {
        FxUnderlyingParams::gbp_usd()
    } else {
        let domestic = CurveId::new(format!("{}-OIS", quote_currency));
        let foreign = CurveId::new(format!("{}-OIS", base_currency));
        FxUnderlyingParams::new(base_currency, quote_currency, domestic, foreign)
    };

    FxOption::builder()
        .id(id.into())
        .base_currency(fx_underlying.base_currency)
        .quote_currency(fx_underlying.quote_currency)
        .strike(strike)
        .option_type(OptionType::Call)
        .delta_convention(
            FxDeltaConvention::new(FxDeltaConventionKind::Forward, Currency::USD, "test")
                .expect("valid delta convention"),
        )
        .expiry(expiry)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .notional(notional)
        .domestic_discount_curve_id(fx_underlying.domestic_discount_curve_id)
        .foreign_discount_curve_id(fx_underlying.foreign_discount_curve_id)
        .vol_surface_id(vol_surface_id.into())
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
}

/// Create an FX European put option using the builder pattern.
pub fn fx_option_european_put(
    id: impl Into<InstrumentId>,
    base_currency: Currency,
    quote_currency: Currency,
    strike: f64,
    expiry: Date,
    notional: Money,
    vol_surface_id: impl Into<CurveId>,
) -> finstack_quant_core::Result<FxOption> {
    let fx_underlying = if quote_currency == Currency::USD && base_currency == Currency::EUR {
        FxUnderlyingParams::usd_eur()
    } else if quote_currency == Currency::USD && base_currency == Currency::GBP {
        FxUnderlyingParams::gbp_usd()
    } else {
        let domestic = CurveId::new(format!("{}-OIS", quote_currency));
        let foreign = CurveId::new(format!("{}-OIS", base_currency));
        FxUnderlyingParams::new(base_currency, quote_currency, domestic, foreign)
    };

    FxOption::builder()
        .id(id.into())
        .base_currency(fx_underlying.base_currency)
        .quote_currency(fx_underlying.quote_currency)
        .strike(strike)
        .option_type(OptionType::Put)
        .delta_convention(
            FxDeltaConvention::new(FxDeltaConventionKind::Forward, Currency::USD, "test")
                .expect("valid delta convention"),
        )
        .expiry(expiry)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .notional(notional)
        .domestic_discount_curve_id(fx_underlying.domestic_discount_curve_id)
        .foreign_discount_curve_id(fx_underlying.foreign_discount_curve_id)
        .vol_surface_id(vol_surface_id.into())
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
}
