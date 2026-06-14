//! Interest Rate Swap construction and builder tests.
//!
//! Tests cover:
//! - Standard construction methods
//! - Builder pattern with validation
//! - Convention-based construction
//! - Edge cases and error handling

use crate::finstack_test_utils as test_utils;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_core::money::Money;
use finstack_valuations::instruments::rates::irs::{InterestRateSwap, PayReceive};
use time::macros::date;

#[test]
fn test_irs_standard_construction() {
    // Standard USD swap using defaults
    let swap = test_utils::usd_irs_swap(
        "IRS-5Y",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        PayReceive::Pay,
    )
    .unwrap();

    assert_eq!(swap.id.as_str(), "IRS-5Y");
    assert_eq!(swap.notional.amount(), 1_000_000.0);
    assert_eq!(
        swap.fixed.rate,
        rust_decimal::Decimal::try_from(0.05).expect("valid")
    );
    assert_eq!(swap.side, PayReceive::Pay);
    assert_eq!(swap.fixed.discount_curve_id.as_ref(), "USD-OIS");
    assert_eq!(swap.float.forward_curve_id.as_ref(), "USD-SOFR-3M");
}

#[test]
fn test_irs_builder_pattern() {
    // Use builder for full control
    let swap = InterestRateSwap::builder()
        .id("IRS-CUSTOM".into())
        .notional(Money::new(5_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.0325).expect("valid"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            stub: StubKind::None,
            start: date!(2024 - 01 - 15),
            end: date!(2034 - 01 - 15),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(25.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            fixing_calendar_id: Some("usny".to_string()),
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 15),
            end: date!(2034 - 01 - 15),
        })
        .build();

    let swap = swap.expect("Builder should succeed");

    assert_eq!(swap.id.as_str(), "IRS-CUSTOM");
    assert_eq!(swap.notional.amount(), 5_000_000.0);
    assert_eq!(
        swap.fixed.rate,
        rust_decimal::Decimal::try_from(0.0325).expect("valid")
    );
    assert_eq!(
        swap.float.spread_bp,
        rust_decimal::Decimal::try_from(25.0).expect("valid")
    );
    assert_eq!(swap.fixed.frequency, Tenor::semi_annual());
    assert_eq!(swap.float.frequency, Tenor::quarterly());
}

#[test]
fn test_irs_receive_vs_pay() {
    let start = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let notional = Money::new(1_000_000.0, Currency::USD);
    let rate = 0.05;

    let swap_receive = test_utils::usd_irs_swap(
        "IRS-RECEIVE",
        notional,
        rate,
        start,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let swap_pay =
        test_utils::usd_irs_swap("IRS-PAY", notional, rate, start, end, PayReceive::Pay).unwrap();

    assert_eq!(swap_receive.side, PayReceive::Receive);
    assert_eq!(swap_pay.side, PayReceive::Pay);

    // Same parameters except direction
    assert_eq!(swap_receive.fixed.rate, swap_pay.fixed.rate);
    assert_eq!(swap_receive.notional, swap_pay.notional);
}

#[test]
fn test_irs_short_maturity() {
    // 6-month swap
    let swap = test_utils::usd_irs_swap(
        "IRS-6M",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2024 - 07 - 01),
        PayReceive::Pay,
    )
    .unwrap();

    assert_eq!(swap.id.as_str(), "IRS-6M");
    assert!(swap.fixed.end.year() - swap.fixed.start.year() < 1);
}

#[test]
fn test_irs_long_maturity() {
    // 30-year swap
    let swap = test_utils::usd_irs_swap(
        "IRS-30Y",
        Money::new(10_000_000.0, Currency::USD),
        0.04,
        date!(2024 - 01 - 01),
        date!(2054 - 01 - 01),
        PayReceive::Receive,
    )
    .unwrap();

    assert_eq!(swap.id.as_str(), "IRS-30Y");
    assert_eq!(swap.fixed.end.year() - swap.fixed.start.year(), 30);
}

#[test]
fn test_irs_zero_spread() {
    let swap = test_utils::usd_irs_swap(
        "IRS-ZERO-SPREAD",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        PayReceive::Receive,
    )
    .unwrap();

    assert_eq!(swap.float.spread_bp, rust_decimal::Decimal::ZERO);
}

#[test]
fn test_irs_with_spread() {
    let mut swap = test_utils::usd_irs_swap(
        "IRS-WITH-SPREAD",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        PayReceive::Receive,
    )
    .unwrap();

    swap.float.spread_bp = rust_decimal::Decimal::try_from(50.0).expect("valid");

    assert_eq!(
        swap.float.spread_bp,
        rust_decimal::Decimal::try_from(50.0).expect("valid")
    );
}

#[test]
fn test_irs_different_leg_frequencies() {
    // Fixed semiannual, float quarterly (standard)
    let swap = InterestRateSwap::builder()
        .id("IRS-DIFF-FREQ".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            start: date!(2024 - 01 - 01),
            end: date!(2029 - 01 - 01),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 01),
            end: date!(2029 - 01 - 01),
        })
        .build()
        .unwrap();

    assert_eq!(swap.fixed.frequency, Tenor::semi_annual());
    assert_eq!(swap.float.frequency, Tenor::quarterly());
}

#[test]
fn test_irs_attribute_management() {
    let mut swap = test_utils::usd_irs_swap(
        "IRS-ATTRS",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        PayReceive::Pay,
    )
    .unwrap();

    // Add attributes
    swap.attributes
        .meta
        .insert("book".to_string(), "trading".to_string());
    swap.attributes
        .meta
        .insert("desk".to_string(), "rates".to_string());

    assert_eq!(
        swap.attributes.meta.get("book"),
        Some(&"trading".to_string())
    );
    assert_eq!(swap.attributes.meta.get("desk"), Some(&"rates".to_string()));
}

#[test]
fn test_irs_calendar_specification() {
    let swap = InterestRateSwap::builder()
        .id("IRS-CAL".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            stub: StubKind::None,
            start: date!(2024 - 01 - 01),
            end: date!(2029 - 01 - 01),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            fixing_calendar_id: Some("usny".to_string()),
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 01),
            end: date!(2029 - 01 - 01),
        })
        .build()
        .unwrap();

    assert_eq!(swap.fixed.calendar_id, Some("usny".to_string()));
    assert_eq!(swap.float.calendar_id, Some("usny".to_string()));
}

#[test]
fn test_irs_stub_specification() {
    let swap = InterestRateSwap::builder()
        .id("IRS-STUB".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            start: date!(2024 - 01 - 15),
            end: date!(2029 - 01 - 01),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::ShortFront,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 15),
            end: date!(2029 - 01 - 01),
        })
        .build()
        .unwrap();

    assert_eq!(swap.fixed.stub, StubKind::ShortFront);
    assert_eq!(swap.float.stub, StubKind::ShortFront);
}
