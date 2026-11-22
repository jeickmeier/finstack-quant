//! FX option fixture builders (also `#[path]`-included by the umbrella crate
//! and benches).

use finstack_quant_core::types::Attributes;
use finstack_quant_core::{
    currency::Currency,
    dates::Date,
    money::Money,
    types::{CurveId, InstrumentId},
};
use finstack_quant_models::types::OptionType;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_fx::FxSpotBasis;
use finstack_quant_valuations_fx::fx_option::{
    FxDeltaConvention, FxDeltaConventionKind, FxOption, FxOptionDelivery,
};

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
    let domestic = CurveId::new(format!("{}-OIS", quote_currency));
    let foreign = CurveId::new(format!("{}-OIS", base_currency));

    FxOption::builder()
        .spot_basis(FxSpotBasis::Cash)
        .delivery(FxOptionDelivery::CashAtExpiry)
        .id(id.into())
        .base_currency(base_currency)
        .quote_currency(quote_currency)
        .strike(strike)
        .option_type(OptionType::Call)
        .delta_convention(
            FxDeltaConvention::new(FxDeltaConventionKind::Forward, Currency::USD, "test")
                .expect("valid delta convention"),
        )
        .expiry(expiry)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .notional(notional)
        .domestic_discount_curve_id(domestic)
        .foreign_discount_curve_id(foreign)
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
    let domestic = CurveId::new(format!("{}-OIS", quote_currency));
    let foreign = CurveId::new(format!("{}-OIS", base_currency));

    FxOption::builder()
        .spot_basis(FxSpotBasis::Cash)
        .delivery(FxOptionDelivery::CashAtExpiry)
        .id(id.into())
        .base_currency(base_currency)
        .quote_currency(quote_currency)
        .strike(strike)
        .option_type(OptionType::Put)
        .delta_convention(
            FxDeltaConvention::new(FxDeltaConventionKind::Forward, Currency::USD, "test")
                .expect("valid delta convention"),
        )
        .expiry(expiry)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .notional(notional)
        .domestic_discount_curve_id(domestic)
        .foreign_discount_curve_id(foreign)
        .vol_surface_id(vol_surface_id.into())
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
}
