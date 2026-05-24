//! FRA construction and builder tests.
//!
//! Validates FRA creation, builder patterns, and field validation following
//! market conventions for standard FRA quoting (e.g., 3x6, 6x12).

use super::common::*;
use finstack_core::currency::Currency;
use finstack_core::dates::DayCount;
use finstack_valuations::instruments::Instrument;
use rust_decimal::Decimal;
use time::macros::date;

#[test]
fn test_standard_fra_construction() {
    let fra = create_standard_fra();

    assert_eq!(fra.id(), "FRA_TEST");
    assert_eq!(fra.notional.amount(), 1_000_000.0);
    assert_eq!(fra.notional.currency(), Currency::USD);
    assert_eq!(
        fra.fixed_rate,
        Decimal::try_from(0.05).expect("valid decimal")
    );
    assert_eq!(fra.day_count, DayCount::Act360);
    assert_eq!(
        fra.side,
        finstack_valuations::instruments::rates::irs::PayReceive::Receive
    ); // receive fixed rate
}

#[test]
fn test_builder_standard_3x6() {
    // Market standard 3M x 6M FRA (3M forward, 3M tenor)
    let fra = TestFraBuilder::new()
        .id("FRA-3x6-USD")
        .notional(10_000_000.0, Currency::USD)
        .fixed_rate(0.045)
        .build();

    assert_eq!(fra.id(), "FRA-3x6-USD");
    assert_eq!(fra.notional.amount(), 10_000_000.0);
    assert_eq!(
        fra.fixed_rate,
        Decimal::try_from(0.045).expect("valid decimal")
    );
}

#[test]
fn test_builder_6x12_fra() {
    // 6M x 12M FRA (6M forward, 6M tenor)
    let fixing = date!(2024 - 07 - 01);
    let start = date!(2024 - 07 - 01);
    let end = date!(2025 - 01 - 01);

    let fra = TestFraBuilder::new()
        .id("FRA-6x12-USD")
        .dates(fixing, start, end)
        .fixed_rate(0.05)
        .build();

    assert_eq!(fra.start_date, start);
    assert_eq!(fra.maturity, end);
}

#[test]
fn test_builder_receive_vs_pay_fixed() {
    use finstack_valuations::instruments::rates::irs::PayReceive;
    // receive_fixed = true means receive fixed rate, pay floating
    // receive_fixed = false means pay fixed rate, receive floating
    let receive_fixed = TestFraBuilder::new().receive_fixed(true).build();
    let pay_fixed = TestFraBuilder::new().receive_fixed(false).build();

    assert_eq!(receive_fixed.side, PayReceive::Receive); // receive fixed rate
    assert_eq!(pay_fixed.side, PayReceive::Pay); // pay fixed rate
}

#[test]
fn test_builder_different_currencies() {
    let usd = TestFraBuilder::new()
        .notional(1_000_000.0, Currency::USD)
        .build();
    let eur = TestFraBuilder::new()
        .notional(1_000_000.0, Currency::EUR)
        .build();
    let gbp = TestFraBuilder::new()
        .notional(1_000_000.0, Currency::GBP)
        .build();

    assert_eq!(usd.notional.currency(), Currency::USD);
    assert_eq!(eur.notional.currency(), Currency::EUR);
    assert_eq!(gbp.notional.currency(), Currency::GBP);
}

#[test]
fn test_builder_day_count_conventions() {
    let act360 = TestFraBuilder::new().day_count(DayCount::Act360).build();
    let act365 = TestFraBuilder::new().day_count(DayCount::Act365F).build();
    let thirty360 = TestFraBuilder::new().day_count(DayCount::Thirty360).build();

    assert_eq!(act360.day_count, DayCount::Act360);
    assert_eq!(act365.day_count, DayCount::Act365F);
    assert_eq!(thirty360.day_count, DayCount::Thirty360);
}

#[test]
fn test_small_notional() {
    let fra = TestFraBuilder::new()
        .notional(1_000.0, Currency::USD)
        .build();

    assert_eq!(fra.notional.amount(), 1_000.0);
}

#[test]
fn test_large_notional() {
    let fra = TestFraBuilder::new()
        .notional(1_000_000_000.0, Currency::USD) // $1 billion
        .build();

    assert_eq!(fra.notional.amount(), 1_000_000_000.0);
}

#[test]
fn test_short_tenor_1m() {
    // 1 month FRA
    let start = date!(2024 - 02 - 01);
    let end = date!(2024 - 03 - 01);

    let fra = TestFraBuilder::new().dates(start, start, end).build();

    assert_eq!(fra.start_date, start);
    assert_eq!(fra.maturity, end);
}

#[test]
fn test_long_tenor_12m() {
    // 12 month FRA
    let start = date!(2024 - 04 - 01);
    let end = date!(2025 - 04 - 01);

    let fra = TestFraBuilder::new().dates(start, start, end).build();

    assert_eq!(fra.start_date, start);
    assert_eq!(fra.maturity, end);
}

#[test]
fn test_negative_rate_environment() {
    let fra = TestFraBuilder::new()
        .fixed_rate(-0.005) // -50bp
        .build();

    assert_eq!(
        fra.fixed_rate,
        Decimal::try_from(-0.005).expect("valid decimal")
    );
}

#[test]
fn test_high_rate_environment() {
    let fra = TestFraBuilder::new()
        .fixed_rate(0.15) // 15%
        .build();

    assert_eq!(
        fra.fixed_rate,
        Decimal::try_from(0.15).expect("valid decimal")
    );
}

#[test]
fn test_instrument_trait_id() {
    let fra = TestFraBuilder::new().id("TEST-FRA-123").build();
    assert_eq!(fra.id(), "TEST-FRA-123");
}

#[test]
fn test_instrument_trait_key() {
    use finstack_valuations::pricer::InstrumentType;
    let fra = create_standard_fra();
    assert_eq!(fra.key(), InstrumentType::FRA);
}

#[test]
fn test_instrument_trait_attributes() {
    let fra = create_standard_fra();
    let attrs = fra.attributes();
    assert!(attrs.tags.is_empty());
    assert!(attrs.meta.is_empty());
}

#[test]
fn test_clone_box() {
    let fra = create_standard_fra();
    let cloned = fra.clone_box();
    assert_eq!(cloned.id(), fra.id());
}

#[test]
fn test_multiple_curve_ids() {
    let fra = TestFraBuilder::new()
        .curves("USD_OIS", "USD_SOFR_3M")
        .build();

    assert_eq!(fra.discount_curve_id.as_str(), "USD_OIS");
    assert_eq!(fra.forward_curve_id.as_str(), "USD_SOFR_3M");
}

#[test]
fn test_eur_market_conventions() {
    // EUR market typically uses ACT/360 for money market instruments
    let fra = TestFraBuilder::new()
        .notional(5_000_000.0, Currency::EUR)
        .day_count(DayCount::Act360)
        .curves("EUR_OIS", "EUR_EURIBOR_3M")
        .build();

    assert_eq!(fra.notional.currency(), Currency::EUR);
    assert_eq!(fra.day_count, DayCount::Act360);
}

#[test]
fn test_gbp_market_conventions() {
    // GBP market typically uses ACT/365
    let fra = TestFraBuilder::new()
        .notional(5_000_000.0, Currency::GBP)
        .day_count(DayCount::Act365F)
        .curves("GBP_OIS", "GBP_SONIA_3M")
        .build();

    assert_eq!(fra.notional.currency(), Currency::GBP);
    assert_eq!(fra.day_count, DayCount::Act365F);
}
