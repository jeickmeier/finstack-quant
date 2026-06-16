//! Pricer integration tests for FX Spot.

use super::super::common::*;
use finstack_quant_core::{currency::Currency, market_data::context::MarketContext, money::Money};
use finstack_quant_valuations::{
    instruments::Instrument,
    pricer::{standard_registry, InstrumentType, ModelKey, PricerKey},
};

#[test]
fn test_registry_pricer_key() {
    let key = PricerKey::new(InstrumentType::FxSpot, ModelKey::Discounting);
    let registry = standard_registry();

    assert!(
        registry.get_pricer(key).is_some(),
        "FxSpot discounting pricer should be registered"
    );
}

#[test]
fn test_registry_prices_fx_spot() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let registry = standard_registry();
    let market = MarketContext::new();
    let as_of = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
        .unwrap();

    let result = registry
        .price_with_metrics(
            &fx,
            ModelKey::Discounting,
            &market,
            as_of,
            &[],
            Default::default(),
        )
        .unwrap();

    assert_eq!(result.instrument_id, "EURUSD");
    assert_approx_eq(result.value.amount(), 1_200_000.0, EPSILON, "PV");
}

#[test]
fn test_pricer_with_various_instruments() {
    let registry = standard_registry();
    let market = MarketContext::new();

    let instruments: Vec<Box<dyn Instrument>> = vec![
        Box::new(eurusd_with_notional(1_000_000.0, 1.20)),
        Box::new(
            sample_gbpusd()
                .with_notional(Money::new(500_000.0, Currency::GBP))
                .unwrap()
                .with_rate(1.40)
                .expect("test rate"),
        ),
        Box::new(
            sample_usdjpy()
                .with_notional(Money::new(100_000.0, Currency::USD))
                .unwrap()
                .with_rate(110.0)
                .expect("test rate"),
        ),
    ];

    let as_of = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
        .unwrap();
    for inst in instruments {
        let result = registry.price_with_metrics(
            inst.as_ref(),
            ModelKey::Discounting,
            &market,
            as_of,
            &[],
            Default::default(),
        );
        assert!(result.is_ok(), "Pricer should price all FX instruments");
    }
}

#[test]
fn test_pricer_consistent_with_instrument_value() {
    let fx = eurusd_with_notional(2_500_000.0, 1.22);
    let registry = standard_registry();
    let market = MarketContext::new();
    let as_of = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
        .unwrap();

    let pricer_result = registry
        .price_with_metrics(
            &fx,
            ModelKey::Discounting,
            &market,
            as_of,
            &[],
            Default::default(),
        )
        .unwrap();

    // Price via instrument
    let instrument_value = fx.value(&market, test_date()).unwrap();

    assert_approx_eq(
        pricer_result.value.amount(),
        instrument_value.amount(),
        EPSILON,
        "Pricer and instrument values match",
    );
}

#[test]
fn test_pricer_with_fx_matrix() {
    let fx = sample_eurusd()
        .with_notional(Money::new(1_000_000.0, Currency::EUR))
        .unwrap();
    let registry = standard_registry();
    let market = market_with_fx_matrix(); // EUR/USD = 1.20
    let as_of = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
        .unwrap();

    let result = registry
        .price_with_metrics(
            &fx,
            ModelKey::Discounting,
            &market,
            as_of,
            &[],
            Default::default(),
        )
        .unwrap();

    assert_approx_eq(
        result.value.amount(),
        1_200_000.0,
        LARGE_EPSILON,
        "FX matrix pricing",
    );
}

#[test]
fn test_pricer_valuation_result_structure() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let registry = standard_registry();
    let market = MarketContext::new();
    let as_of = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
        .unwrap();

    let result = registry
        .price_with_metrics(
            &fx,
            ModelKey::Discounting,
            &market,
            as_of,
            &[],
            Default::default(),
        )
        .unwrap();

    assert_eq!(result.instrument_id, "EURUSD");
    assert_eq!(result.value.currency(), Currency::USD);
    assert!(result.measures.is_empty(), "No metrics by default");
}
