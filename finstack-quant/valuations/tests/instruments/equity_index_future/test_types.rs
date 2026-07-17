//! Tests for EquityIndexFuture types and construction.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::equity::equity_index_future::{
    EquityFutureSpecs, EquityIndexFuture,
};
use finstack_quant_valuations::instruments::Position;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use time::Month;

#[test]
fn test_equity_future_specs_all_contracts() {
    // E-mini S&P 500
    let es = EquityFutureSpecs::sp500_emini();
    assert_eq!(es.multiplier, 50.0);
    assert_eq!(es.tick_size, 0.25);
    assert_eq!(es.tick_value, 12.50);

    // E-mini Nasdaq-100
    let nq = EquityFutureSpecs::nasdaq100_emini();
    assert_eq!(nq.multiplier, 20.0);
    assert_eq!(nq.tick_size, 0.25);
    assert_eq!(nq.tick_value, 5.00);

    // Micro E-mini S&P 500
    let mes = EquityFutureSpecs::sp500_micro_emini();
    assert_eq!(mes.multiplier, 5.0);
    assert_eq!(mes.tick_size, 0.25);
    assert_eq!(mes.tick_value, 1.25);

    // Euro Stoxx 50
    let fesx = EquityFutureSpecs::euro_stoxx_50();
    assert_eq!(fesx.multiplier, 10.0);
    assert_eq!(fesx.tick_size, 1.0);
    assert_eq!(fesx.tick_value, 10.0);

    // DAX
    let fdax = EquityFutureSpecs::dax();
    assert_eq!(fdax.multiplier, 25.0);
    assert_eq!(fdax.tick_size, 0.5);
    assert_eq!(fdax.tick_value, 12.5);

    // FTSE 100
    let z = EquityFutureSpecs::ftse_100();
    assert_eq!(z.multiplier, 10.0);
    assert_eq!(z.tick_size, 0.5);
    assert_eq!(z.tick_value, 5.0);

    // Nikkei 225
    let nk = EquityFutureSpecs::nikkei_225();
    assert_eq!(nk.multiplier, 500.0);
    assert_eq!(nk.tick_size, 5.0);
    assert_eq!(nk.tick_value, 2500.0);
}

#[test]
fn test_equity_index_future_builder() {
    let expiry = Date::from_calendar_date(2025, Month::March, 21).unwrap();
    let last_trade = Date::from_calendar_date(2025, Month::March, 20).unwrap();

    let future = EquityIndexFuture::builder()
        .id(InstrumentId::new("ES-TEST"))
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
        .expect("should build");

    assert_eq!(future.id.as_str(), "ES-TEST");
    assert_eq!(future.underlying_ticker, "SPX");
    assert_eq!(future.notional, Money::new(2_250_000.0, Currency::USD));
    assert_eq!(future.entry_price, Some(4500.0));
    assert_eq!(future.quoted_price, Some(4550.0));
    assert_eq!(future.position, Position::Long);
}

#[test]
fn test_sp500_emini_convenience_constructor() {
    let expiry = Date::from_calendar_date(2025, Month::March, 21).unwrap();
    let last_trade = Date::from_calendar_date(2025, Month::March, 20).unwrap();

    let future = EquityIndexFuture::sp500_emini(
        "ESH5",
        Money::new(2_250_000.0, Currency::USD),
        expiry,
        last_trade,
        Some(4500.0),
        Position::Long,
        "USD-OIS",
    )
    .expect("should build");

    assert_eq!(future.id.as_str(), "ESH5");
    assert_eq!(future.underlying_ticker, "SPX");
    assert_eq!(future.notional.currency(), Currency::USD);
    assert_eq!(future.contract_specs.multiplier, 50.0);
    assert_eq!(future.spot_id.as_str(), "SPX-SPOT");
}

#[test]
fn test_nasdaq100_emini_convenience_constructor() {
    let expiry = Date::from_calendar_date(2025, Month::March, 21).unwrap();
    let last_trade = Date::from_calendar_date(2025, Month::March, 20).unwrap();

    let future = EquityIndexFuture::nasdaq100_emini(
        "NQH5",
        Money::new(1_500_000.0, Currency::USD),
        expiry,
        last_trade,
        Some(15000.0),
        Position::Short,
        "USD-OIS",
    )
    .expect("should build");

    assert_eq!(future.id.as_str(), "NQH5");
    assert_eq!(future.underlying_ticker, "NDX");
    assert_eq!(future.notional.currency(), Currency::USD);
    assert_eq!(future.contract_specs.multiplier, 20.0);
    assert_eq!(future.spot_id.as_str(), "NDX-SPOT");
    assert_eq!(future.position, Position::Short);
}

#[test]
fn test_position_sign() {
    let expiry = Date::from_calendar_date(2025, Month::March, 21).unwrap();
    let last_trade = Date::from_calendar_date(2025, Month::March, 20).unwrap();

    let long = EquityIndexFuture::sp500_emini(
        "ES-LONG",
        Money::new(2_250_000.0, Currency::USD),
        expiry,
        last_trade,
        None,
        Position::Long,
        "USD-OIS",
    )
    .unwrap();
    assert_eq!(long.position_sign(), 1.0);

    let short = EquityIndexFuture::sp500_emini(
        "ES-SHORT",
        Money::new(2_250_000.0, Currency::USD),
        expiry,
        last_trade,
        None,
        Position::Short,
        "USD-OIS",
    )
    .unwrap();
    assert_eq!(short.position_sign(), -1.0);
}

#[test]
fn test_delta_calculation() {
    let expiry = Date::from_calendar_date(2025, Month::March, 21).unwrap();
    let last_trade = Date::from_calendar_date(2025, Month::March, 20).unwrap();

    // Long 10 ES contracts at 4500: notional = 10 × 50 × 4500 = 2,250,000
    let long = EquityIndexFuture::sp500_emini(
        "ES-LONG",
        Money::new(2_250_000.0, Currency::USD),
        expiry,
        last_trade,
        Some(4500.0),
        Position::Long,
        "USD-OIS",
    )
    .unwrap();
    // Delta = 50 × 10 × 1 = 500
    assert_eq!(long.delta(), 500.0);

    // Short 5 NQ contracts at 15000: notional = 5 × 20 × 15000 = 1,500,000
    let short = EquityIndexFuture::nasdaq100_emini(
        "NQ-SHORT",
        Money::new(1_500_000.0, Currency::USD),
        expiry,
        last_trade,
        Some(15000.0),
        Position::Short,
        "USD-OIS",
    )
    .unwrap();
    // Delta = 20 × 5 × (-1) = -100
    assert_eq!(short.delta(), -100.0);
}

#[test]
fn test_num_contracts() {
    let future = EquityIndexFuture::example().unwrap();
    // At price 4500: contracts = 2,250,000 / (4500 × 50) = 10
    assert_eq!(future.num_contracts(4500.0), 10.0);
}

#[test]
fn test_instrument_trait() {
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::pricer::InstrumentType;

    let future = EquityIndexFuture::example().unwrap();

    assert_eq!(future.id(), "ES-2025M03");
    assert_eq!(future.key(), InstrumentType::EquityIndexFuture);
}

#[test]
fn test_market_dependencies() {
    let future = EquityIndexFuture::example().unwrap();
    let deps = future
        .market_dependencies()
        .expect("market_dependencies")
        .curves;

    assert_eq!(deps.discount_curves.len(), 1);
    assert_eq!(deps.discount_curves[0].as_str(), "USD-OIS");
}

#[test]
fn test_serde_roundtrip() {
    let future = EquityIndexFuture::example().unwrap();
    let json = serde_json::to_string(&future).expect("serialize");
    let recovered: EquityIndexFuture = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(future.id, recovered.id);
    assert_eq!(future.underlying_ticker, recovered.underlying_ticker);
    assert_eq!(future.notional, recovered.notional);
    assert_eq!(future.entry_price, recovered.entry_price);
    assert_eq!(future.quoted_price, recovered.quoted_price);
}
