//! FX option instrument construction and trait implementation tests.
//!
//! Tests builders, convenience constructors, and trait implementations.

use super::helpers::*;
use crate::test_support::equity_fx_options as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fx::fx_option::{
    FxDeltaConvention, FxDeltaConventionKind, FxOption,
};
use finstack_quant_valuations::instruments::FxUnderlyingParams;
use finstack_quant_valuations::instruments::OptionType;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_builder_pattern_creates_valid_option() {
    // Arrange & Act
    let option = FxOption::builder()
        .id(InstrumentId::new("TEST_CALL"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .strike(1.20)
        .option_type(OptionType::Call)
        .delta_convention(
            FxDeltaConvention::new(FxDeltaConventionKind::Forward, Currency::USD, "test")
                .expect("valid delta convention"),
        )
        .expiry(date!(2025 - 01 - 01))
        .day_count(DayCount::Act365F)
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .attributes(Attributes::new())
        .build();

    // Assert
    assert!(option.is_ok(), "Builder should create valid option");
    let opt = option.unwrap();
    assert_eq!(opt.id.as_str(), "TEST_CALL");
    assert_eq!(opt.strike, 1.20);
    assert_eq!(opt.option_type, OptionType::Call);
}

#[test]
fn test_european_call_convenience_constructor() {
    // Arrange & Act
    let call = test_utils::fx_option_european_call(
        "EUR_USD_CALL",
        Currency::EUR,
        Currency::USD,
        1.20,
        date!(2025 - 01 - 01),
        Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
        CurveId::new("EURUSD-VOL"),
    )
    .unwrap();

    // Assert
    assert_eq!(call.id.as_str(), "EUR_USD_CALL");
    assert_eq!(call.option_type, OptionType::Call);
    assert_eq!(call.strike, 1.20);
    assert_eq!(call.notional.amount(), 1_000_000.0);
    assert_eq!(call.notional.currency(), Currency::EUR);
}

#[test]
fn test_european_put_convenience_constructor() {
    // Arrange & Act
    let put = test_utils::fx_option_european_put(
        "EUR_USD_PUT",
        Currency::EUR,
        Currency::USD,
        1.20,
        date!(2025 - 01 - 01),
        Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
        CurveId::new("EURUSD-VOL"),
    )
    .unwrap();

    // Assert
    assert_eq!(put.id.as_str(), "EUR_USD_PUT");
    assert_eq!(put.option_type, OptionType::Put);
}

#[test]
fn test_builder_with_underlying_params() {
    // Arrange
    let underlying_params = FxUnderlyingParams::usd_eur();

    // Act
    let option = FxOption::builder()
        .id("TEST_OPTION".into())
        .base_currency(underlying_params.base_currency)
        .quote_currency(underlying_params.quote_currency)
        .strike(1.20)
        .option_type(OptionType::Call)
        .delta_convention(
            FxDeltaConvention::new(FxDeltaConventionKind::Forward, Currency::USD, "test")
                .expect("valid delta convention"),
        )
        .expiry(date!(2025 - 01 - 01))
        .day_count(DayCount::Act365F)
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
        .domestic_discount_curve_id(underlying_params.domestic_discount_curve_id.clone())
        .foreign_discount_curve_id(underlying_params.foreign_discount_curve_id.clone())
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .attributes(Attributes::new())
        .build()
        .expect("builder should create option");

    // Assert
    assert_eq!(option.id.as_str(), "TEST_OPTION");
    assert_eq!(option.base_currency, underlying_params.base_currency);
    assert_eq!(option.quote_currency, underlying_params.quote_currency);
    assert_eq!(option.strike, 1.20);
    assert_eq!(option.option_type, OptionType::Call);
}

#[test]
fn test_instrument_trait_value() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act: Call via trait
    let pv = call.value(&market, as_of).unwrap();

    // Assert
    assert!(pv.amount() > 0.0);
    assert_eq!(pv.currency(), QUOTE);
}

#[test]
fn test_value_method_returns_positive_pv() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let pv = call.value(&market, as_of).unwrap();

    // Assert
    assert!(pv.amount() > 0.0);
    assert_eq!(pv.currency(), QUOTE);
}

#[test]
fn test_value_method_consistency() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act: Call value() twice to verify consistency
    let pv1 = call.value(&market, as_of).unwrap();
    let pv2 = call.value(&market, as_of).unwrap();

    // Assert: Should be identical
    assert_eq!(pv1.amount(), pv2.amount());
    assert_eq!(pv1.currency(), pv2.currency());
}

#[test]
fn test_compute_greeks_method() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert
    assert!(greeks.delta.is_finite());
    assert!(greeks.gamma >= 0.0);
    assert!(greeks.vega > 0.0);
}

#[test]
fn test_implied_vol_method() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let pv = call.value(&market, as_of).unwrap();
    let iv = call.implied_vol(&market, as_of, pv.amount()).unwrap();

    // Assert
    assert_approx_eq(iv, 0.15, 1e-6, 1e-6, "IV method should recover market vol");
}

#[test]
fn test_price_with_metrics_matches_value() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let pv = call.value(&market, as_of).unwrap();
    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    assert_approx_eq(
        result.value.amount(),
        pv.amount(),
        1e-10,
        1e-10,
        "price_with_metrics PV matches value()",
    );
}

#[test]
fn test_pricing_overrides_applied() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let mut call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm()); // Market vol is 15%

    // Act
    let pv_surface = call.value(&market, as_of).unwrap();
    call.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.30); // Override to 30%
    let pv_override = call.value(&market, as_of).unwrap();

    // Assert: Higher vol should increase option PV
    assert!(
        pv_override.amount() > pv_surface.amount(),
        "Override vol should increase PV"
    );
}

#[test]
fn test_attributes_are_mutable() {
    // Arrange
    let mut call = build_call_option(
        date!(2024 - 01 - 01),
        date!(2025 - 01 - 01),
        1.20,
        1_000_000.0,
    );

    // Act
    call.attributes_mut()
        .meta
        .insert("trader".to_string(), "Alice".to_string());
    call.attributes_mut()
        .meta
        .insert("book".to_string(), "FX_OPTIONS".to_string());

    // Assert
    assert_eq!(
        call.attributes().meta.get("trader").map(|s| s.as_str()),
        Some("Alice")
    );
    assert_eq!(
        call.attributes().meta.get("book").map(|s| s.as_str()),
        Some("FX_OPTIONS")
    );
}

#[test]
fn test_clone_preserves_all_fields() {
    // Arrange
    let original = build_call_option(
        date!(2024 - 01 - 01),
        date!(2025 - 01 - 01),
        1.20,
        1_000_000.0,
    );

    // Act
    let cloned = original.clone();

    // Assert
    assert_eq!(cloned.id, original.id);
    assert_eq!(cloned.strike, original.strike);
    assert_eq!(cloned.option_type, original.option_type);
    assert_eq!(cloned.expiry, original.expiry);
    assert_eq!(cloned.notional.amount(), original.notional.amount());
}

#[test]
fn test_debug_trait_implemented() {
    // Arrange
    let call = build_call_option(
        date!(2024 - 01 - 01),
        date!(2025 - 01 - 01),
        1.20,
        1_000_000.0,
    );

    // Act: Format as debug string
    let debug_str = format!("{:?}", call);

    // Assert: Should contain key fields
    assert!(debug_str.contains("FxOption"));
    assert!(debug_str.contains("strike"));
}

#[test]
fn serde_rejects_unimplemented_fx_option_settlement_modes() {
    let call = build_call_option(
        date!(2024 - 01 - 01),
        date!(2025 - 01 - 01),
        1.20,
        1_000_000.0,
    );
    let mut value = serde_json::to_value(call).expect("serialize option");
    value["settlement"] = serde_json::json!("physical");

    let error =
        serde_json::from_value::<FxOption>(value).expect_err("settlement field must be rejected");
    assert!(error.to_string().contains("unknown field `settlement`"));
}

#[test]
fn serde_rejects_non_european_fx_option_exercise_styles() {
    let call = build_call_option(
        date!(2024 - 01 - 01),
        date!(2025 - 01 - 01),
        1.20,
        1_000_000.0,
    );
    let mut value = serde_json::to_value(call).expect("serialize option");
    value["exercise_style"] = serde_json::json!("american");

    let error = serde_json::from_value::<FxOption>(value)
        .expect_err("exercise_style field must be rejected");
    assert!(error.to_string().contains("unknown field `exercise_style`"));
}
