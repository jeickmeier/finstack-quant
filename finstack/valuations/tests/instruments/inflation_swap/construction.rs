//! Construction and builder validation tests for InflationSwap.

use crate::inflation_swap::fixtures::*;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::scalars::InflationLag;
use finstack_core::money::Money;
use finstack_valuations::instruments::rates::inflation_swap::{
    InflationSwap, InflationSwapBuilder,
};
use finstack_valuations::instruments::Attributes;
use finstack_valuations::instruments::PayReceive;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_builder_creates_valid_swap() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-001".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    assert_eq!(swap.id.as_str(), "ZCINF-001");
    assert_eq!(swap.notional.amount(), 1_000_000.0);
    assert_eq!(swap.start_date, as_of);
    assert_eq!(swap.maturity, maturity);
    assert_eq!(
        swap.fixed_rate,
        Decimal::try_from(0.02).expect("valid decimal")
    );
    assert_eq!(swap.side, PayReceive::Pay);
}

#[test]
fn test_builder_with_lag_override() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-002".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.025).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Receive)
        .lag_override(InflationLag::Months(2))
        .attributes(Attributes::new())
        .build()
        .unwrap();

    assert_eq!(swap.lag_override, Some(InflationLag::Months(2)));
}

#[test]
fn test_pay_receive_inflation_display() {
    assert_eq!(PayReceive::Pay.to_string(), "pay");
    assert_eq!(PayReceive::Receive.to_string(), "receive");
}

#[test]
fn test_pay_receive_inflation_from_str() {
    use std::str::FromStr;

    assert_eq!(PayReceive::from_str("pay_fixed").unwrap(), PayReceive::Pay);
    assert_eq!(PayReceive::from_str("pay").unwrap(), PayReceive::Pay);
    assert_eq!(
        PayReceive::from_str("receive_fixed").unwrap(),
        PayReceive::Receive
    );
    assert_eq!(
        PayReceive::from_str("receive").unwrap(),
        PayReceive::Receive
    );
    assert_eq!(PayReceive::from_str("PAY-FIXED").unwrap(), PayReceive::Pay);
    assert!(PayReceive::from_str("invalid").is_err());
}

#[test]
fn test_swap_with_different_day_counts() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    for dc in &[
        DayCount::Act360,
        DayCount::Act365F,
        DayCount::Thirty360,
        DayCount::ActActIsma,
    ] {
        let swap = InflationSwapBuilder::new()
            .id("ZCINF-DC".into())
            .notional(standard_notional())
            .start_date(as_of)
            .maturity(maturity)
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .inflation_index_id("US-CPI-U".into())
            .discount_curve_id("USD-OIS".into())
            .day_count(*dc)
            .side(PayReceive::Pay)
            .attributes(Attributes::new())
            .build()
            .unwrap();

        assert_eq!(swap.day_count, *dc);
    }
}

#[test]
fn test_swap_with_different_notionals() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    for notional_amt in &[1_000.0, 100_000.0, 1_000_000.0, 100_000_000.0] {
        let swap = InflationSwapBuilder::new()
            .id("ZCINF-NOT".into())
            .notional(Money::new(*notional_amt, Currency::USD))
            .start_date(as_of)
            .maturity(maturity)
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .inflation_index_id("US-CPI-U".into())
            .discount_curve_id("USD-OIS".into())
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .attributes(Attributes::new())
            .build()
            .unwrap();

        assert_eq!(swap.notional.amount(), *notional_amt);
    }
}

#[test]
fn test_swap_with_various_maturities() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for years in &[1, 2, 5, 10, 30] {
        let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
        let swap = InflationSwapBuilder::new()
            .id("ZCINF-MAT".into())
            .notional(standard_notional())
            .start_date(as_of)
            .maturity(maturity)
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .inflation_index_id("US-CPI-U".into())
            .discount_curve_id("USD-OIS".into())
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .attributes(Attributes::new())
            .build()
            .unwrap();

        assert_eq!(swap.maturity, maturity);
    }
}

#[test]
fn test_swap_with_negative_fixed_rate() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    // Negative real rates are valid (e.g., during high inflation)
    let swap = InflationSwapBuilder::new()
        .id("ZCINF-NEG".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(-0.01).expect("valid decimal")) // -1% real rate
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    assert_eq!(
        swap.fixed_rate,
        Decimal::try_from(-0.01).expect("valid decimal")
    );
}

#[test]
fn test_swap_with_attributes() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let attrs = Attributes::new()
        .with_tag("portfolio:hedge_fund_a")
        .with_tag("strategy:inflation_protection");

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-ATTR".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(attrs)
        .build()
        .unwrap();

    assert!(swap.attributes.has_tag("portfolio:hedge_fund_a"));
    assert!(swap.attributes.has_tag("strategy:inflation_protection"));
}

#[test]
fn test_instrument_trait_implementations() {
    use finstack_valuations::instruments::Instrument;

    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-TRAIT".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    assert_eq!(swap.id(), "ZCINF-TRAIT");
    assert_eq!(
        swap.key(),
        finstack_valuations::pricer::InstrumentType::InflationSwap
    );
    assert!(swap.as_any().is::<InflationSwap>());
}

#[test]
fn test_clone_and_equality() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let swap1 = InflationSwapBuilder::new()
        .id("ZCINF-CLONE".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let swap2 = swap1.clone();

    assert_eq!(swap1.id, swap2.id);
    assert_eq!(swap1.notional.amount(), swap2.notional.amount());
    assert_eq!(swap1.start_date, swap2.start_date);
    assert_eq!(swap1.maturity, swap2.maturity);
    assert_eq!(swap1.fixed_rate, swap2.fixed_rate);
    assert_eq!(swap1.side, swap2.side);
}
