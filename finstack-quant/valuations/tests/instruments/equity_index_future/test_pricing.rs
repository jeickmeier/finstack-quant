//! Tests for EquityIndexFuture pricing.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::equity::equity_index_future::{
    EquityFutureSpecs, EquityIndexFuture,
};
use finstack_quant_valuations::instruments::rates::ir_future::Position;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::pricer::{standard_registry, ModelKey};
use time::Month;

/// Create a test market with discount curve and spot prices.
fn create_test_market() -> MarketContext {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    // Create flat 5% discount curve
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (0.5, 0.9753), (1.0, 0.9512)])
        .build()
        .expect("should succeed");

    MarketContext::new()
        .insert(discount_curve)
        .insert_price("SPX-SPOT", MarketScalar::Unitless(4500.0))
        .insert_price("NDX-SPOT", MarketScalar::Unitless(15000.0))
}

fn create_long_es_future_with_quoted() -> EquityIndexFuture {
    let expiry = Date::from_calendar_date(2025, Month::June, 20).expect("valid date");
    let last_trade = Date::from_calendar_date(2025, Month::June, 19).expect("valid date");

    EquityIndexFuture::builder()
        .id(InstrumentId::new("ES-QUOTED"))
        .underlying_ticker("SPX".to_string())
        .notional(Money::new(2_250_000.0, Currency::USD))
        .expiry(expiry)
        .last_trading_date(last_trade)
        .entry_price_opt(Some(4500.0))
        .quoted_price_opt(Some(4550.0))
        .position(Position::Long)
        .contract_specs(EquityFutureSpecs::sp500_emini())
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .attributes(Attributes::new())
        .build()
        .expect("should build")
}

fn create_short_es_future_with_quoted() -> EquityIndexFuture {
    let expiry = Date::from_calendar_date(2025, Month::June, 20).expect("valid date");
    let last_trade = Date::from_calendar_date(2025, Month::June, 19).expect("valid date");

    EquityIndexFuture::builder()
        .id(InstrumentId::new("ES-SHORT"))
        .underlying_ticker("SPX".to_string())
        .notional(Money::new(2_250_000.0, Currency::USD))
        .expiry(expiry)
        .last_trading_date(last_trade)
        .entry_price_opt(Some(4500.0))
        .quoted_price_opt(Some(4550.0))
        .position(Position::Short)
        .contract_specs(EquityFutureSpecs::sp500_emini())
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .attributes(Attributes::new())
        .build()
        .expect("should build")
}

fn create_es_future_fair_value() -> EquityIndexFuture {
    let expiry = Date::from_calendar_date(2025, Month::June, 20).expect("valid date");
    let last_trade = Date::from_calendar_date(2025, Month::June, 19).expect("valid date");

    EquityIndexFuture::builder()
        .id(InstrumentId::new("ES-FAIR"))
        .underlying_ticker("SPX".to_string())
        .notional(Money::new(2_250_000.0, Currency::USD))
        .expiry(expiry)
        .last_trading_date(last_trade)
        .entry_price_opt(Some(4500.0))
        // No quoted price - will use fair value
        .position(Position::Long)
        .contract_specs(EquityFutureSpecs::sp500_emini())
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .attributes(Attributes::new())
        .build()
        .expect("should build")
}

#[test]
fn test_quoted_price_long_profit() {
    let market = create_test_market();
    let future = create_long_es_future_with_quoted();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let npv = future.value(&market, as_of).expect("should price");

    // Long 10 contracts, entry 4500, quoted 4550
    // PV = (4550 - 4500) × 50 × 10 × 1 = 50 × 50 × 10 = 25,000
    assert_eq!(npv.currency(), Currency::USD);
    assert!((npv.amount() - 25_000.0).abs() < 0.01);
}

#[test]
fn test_quoted_price_short_loss() {
    let market = create_test_market();
    let future = create_short_es_future_with_quoted();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let npv = future.value(&market, as_of).expect("should price");

    // Short 10 contracts, entry 4500, quoted 4550
    // PV = (4550 - 4500) × 50 × 10 × (-1) = -25,000
    assert_eq!(npv.currency(), Currency::USD);
    assert!((npv.amount() + 25_000.0).abs() < 0.01);
}

#[test]
fn test_fair_value_pricing() {
    let market = create_test_market();
    let future = create_es_future_fair_value();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let npv = future.value(&market, as_of).expect("should price");

    // Fair value = S × exp((r - q) × T)
    // With S=4500, r≈5%, q=0, T≈0.47 (Jan to Jun)
    // F ≈ 4500 × exp(0.05 × 0.47) ≈ 4607
    // PV = (4607 - 4500) × 50 × 10 = 53,500 (approximately)
    assert_eq!(npv.currency(), Currency::USD);
    assert!(npv.amount() > 0.0); // Long position, forward > entry
    assert!(npv.amount() > 40_000.0 && npv.amount() < 70_000.0);
}

#[test]
fn test_expired_future_zero_value() {
    let market = create_test_market();
    let future = create_long_es_future_with_quoted();
    // Valuation date after expiry
    let as_of = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");

    let npv = future.value(&market, as_of).expect("should price");

    // Expired future should have zero value
    assert_eq!(npv.amount(), 0.0);
}

#[test]
fn test_fair_forward_calculation() {
    let market = create_test_market();
    let future = create_es_future_fair_value();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let fair_forward = future
        .fair_forward(&market, as_of)
        .expect("should calculate");

    // Fair forward should be above spot (positive carry, no dividends)
    // F = 4500 × exp(0.05 × 0.47) ≈ 4607
    assert!(fair_forward > 4500.0);
    assert!(fair_forward < 4700.0);
}

#[test]
fn test_registry_pricing() {
    let registry = standard_registry();
    let market = create_test_market();
    let future = create_long_es_future_with_quoted();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let result = registry
        .price_with_metrics(
            &future,
            ModelKey::Discounting,
            &market,
            as_of,
            &[],
            Default::default(),
        )
        .expect("should price");

    assert_eq!(result.instrument_id, "ES-QUOTED");
    assert!((result.value.amount() - 25_000.0).abs() < 0.01);
}

#[test]
fn test_value_via_instrument_trait() {
    let market = create_test_market();
    let future = create_long_es_future_with_quoted();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let value = future.value(&market, as_of).expect("should value");

    assert_eq!(value.currency(), Currency::USD);
    assert!((value.amount() - 25_000.0).abs() < 0.01);
}

#[test]
fn test_value_raw() {
    let market = create_test_market();
    let future = create_long_es_future_with_quoted();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let value_raw = future.value_raw(&market, as_of).expect("should value");

    assert!((value_raw - 25_000.0).abs() < 0.01);
}

#[test]
fn test_nq_future_pricing() {
    let market = create_test_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let expiry = Date::from_calendar_date(2025, Month::June, 20).expect("valid date");
    let last_trade = Date::from_calendar_date(2025, Month::June, 19).expect("valid date");

    let future = EquityIndexFuture::builder()
        .id(InstrumentId::new("NQ-TEST"))
        .underlying_ticker("NDX".to_string())
        .notional(Money::new(1_500_000.0, Currency::USD))
        .expiry(expiry)
        .last_trading_date(last_trade)
        .entry_price_opt(Some(15000.0))
        .quoted_price_opt(Some(15100.0))
        .position(Position::Long)
        .contract_specs(EquityFutureSpecs::nasdaq100_emini())
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("NDX-SPOT".into())
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = future.value(&market, as_of).expect("should price");

    // Long 5 contracts, entry 15000, quoted 15100
    // PV = (15100 - 15000) × 20 × 5 × 1 = 100 × 20 × 5 = 10,000
    assert_eq!(npv.currency(), Currency::USD);
    assert!((npv.amount() - 10_000.0).abs() < 0.01);
}

#[test]
fn test_at_the_money_future() {
    let market = create_test_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let expiry = Date::from_calendar_date(2025, Month::June, 20).expect("valid date");
    let last_trade = Date::from_calendar_date(2025, Month::June, 19).expect("valid date");

    // Entry = Quoted → zero PV
    let future = EquityIndexFuture::builder()
        .id(InstrumentId::new("ES-ATM"))
        .underlying_ticker("SPX".to_string())
        .notional(Money::new(2_250_000.0, Currency::USD))
        .expiry(expiry)
        .last_trading_date(last_trade)
        .entry_price_opt(Some(4500.0))
        .quoted_price_opt(Some(4500.0)) // Same as entry
        .position(Position::Long)
        .contract_specs(EquityFutureSpecs::sp500_emini())
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = future.value(&market, as_of).expect("should price");

    // At-the-money should have zero PV
    assert!(npv.amount().abs() < 0.01);
}
