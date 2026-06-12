//! IR Future construction and builder tests.

use super::utils::*;
use finstack_core::currency::Currency;
use finstack_core::dates::DayCount;
use finstack_core::money::Money;
use finstack_valuations::instruments::rates::ir_future::{
    FutureContractSpecs, InterestRateFuture, Position,
};

#[test]
fn test_standard_construction() {
    let (_, start, end) = standard_dates();
    let future = create_standard_future(start, end);

    assert_eq!(future.id.as_str(), "IRF_TEST");
    assert_eq!(future.notional.amount(), 1_000_000.0);
    assert_eq!(future.notional.currency(), Currency::USD);
    assert_eq!(future.quoted_price, 97.50);
    assert_eq!(future.position, Position::Long);
}

#[test]
fn test_with_contract_specs() {
    let (_, start, end) = standard_dates();
    let custom_specs = FutureContractSpecs {
        face_value: 2_000_000.0,
        tick_size: 0.01,
        tick_value: 25.0,
        delivery_months: 6,
        convexity_adjustment: Some(0.0001),
    };

    let future = create_standard_future(start, end).with_contract_specs(custom_specs);

    assert_eq!(future.contract_specs.face_value, 2_000_000.0);
    assert_eq!(future.contract_specs.tick_size, 0.01);
    assert_eq!(future.contract_specs.tick_value, 25.0);
    assert_eq!(future.contract_specs.delivery_months, 6);
    assert_eq!(future.contract_specs.convexity_adjustment, Some(0.0001));
}

#[test]
fn test_default_contract_specs() {
    let specs = FutureContractSpecs::default();

    assert_eq!(specs.face_value, 1_000_000.0);
    assert_eq!(specs.tick_size, 0.0025);
    assert_eq!(specs.tick_value, 6.25);
    assert_eq!(specs.delivery_months, 3);
    assert!(specs.convexity_adjustment.is_none());
}

#[test]
fn test_sofr_specs() {
    let specs = create_sofr_specs();

    assert_eq!(specs.face_value, 1_000_000.0);
    assert_eq!(specs.tick_size, 0.0025);
    assert_eq!(specs.tick_value, 6.25);
}

#[test]
fn test_eurodollar_specs() {
    let specs = create_eurodollar_specs();

    assert_eq!(specs.face_value, 1_000_000.0);
    assert_eq!(specs.tick_size, 0.0025);
}

#[test]
fn test_multiple_contracts() {
    let (_, start, end) = standard_dates();

    // 5 contracts = 5 * face value
    let future = InterestRateFuture {
        id: "IRF_5_CONTRACTS".into(),
        notional: Money::new(5_000_000.0, Currency::USD),
        expiry: start,
        fixing_date: Some(start),
        period_start: Some(start),
        period_end: Some(end),
        quoted_price: 97.50,
        day_count: DayCount::Act360,
        position: Position::Long,
        contract_specs: FutureContractSpecs::default(),
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: None,
        pricing_overrides: finstack_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    // Verify notional is 5x face value
    assert_eq!(
        future.notional.amount(),
        5.0 * future.contract_specs.face_value
    );
}

#[test]
fn test_different_day_counts() {
    let (_, start, end) = standard_dates();

    let day_counts = vec![DayCount::Act360, DayCount::Act365F, DayCount::Thirty360];

    for dc in day_counts {
        let future = InterestRateFuture {
            id: "IRF_DC_TEST".into(),
            notional: Money::new(1_000_000.0, Currency::USD),
            expiry: start,
            fixing_date: Some(start),
            period_start: Some(start),
            period_end: Some(end),
            quoted_price: 97.50,
            day_count: dc,
            position: Position::Long,
            contract_specs: FutureContractSpecs::default(),
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            vol_surface_id: None,
            pricing_overrides: finstack_valuations::instruments::PricingOverrides::default(),
            attributes: Default::default(),
        };

        assert_eq!(future.day_count, dc);
    }
}

#[test]
fn test_long_and_short_positions() {
    let (_, start, end) = standard_dates();

    let long = create_custom_future(
        "LONG",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );
    let short = create_custom_future(
        "SHORT",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Short,
    );

    assert_eq!(long.position, Position::Long);
    assert_eq!(short.position, Position::Short);
}

#[test]
fn test_implied_rate_calculation() {
    let (_, start, end) = standard_dates();

    // Quoted price 97.50 => implied rate 2.50%
    let future = create_standard_future(start, end);
    let implied = future.implied_rate();

    assert!(
        (implied.as_decimal() - 0.025).abs() < 1e-10,
        "Expected 2.5%, got {}",
        implied
    );
}

#[test]
fn test_implied_rate_various_prices() {
    let (_, start, end) = standard_dates();

    let test_cases = vec![
        (100.0, 0.0), // Price 100 => 0% rate
        (99.0, 0.01), // Price 99 => 1% rate
        (98.0, 0.02), // Price 98 => 2% rate
        (95.0, 0.05), // Price 95 => 5% rate
    ];

    for (price, expected_rate) in test_cases {
        let mut future = create_standard_future(start, end);
        future.quoted_price = price;
        let implied = future.implied_rate();

        assert!(
            (implied.as_decimal() - expected_rate).abs() < 1e-10,
            "Price {} should imply rate {}, got {}",
            price,
            expected_rate,
            implied
        );
    }
}

#[test]
fn test_derived_tick_value() {
    let (_, start, end) = standard_dates();
    let future = create_standard_future(start, end);

    let tick_value = future.derived_tick_value().unwrap();

    // For a 3-month contract (approx 0.25 year), tick value should be reasonable
    // Actual calculation varies with exact dates (Act/360 day count)
    assert!(tick_value > 0.0, "Tick value should be positive");
    assert!(
        tick_value < 2000.0,
        "Tick value should be reasonable, got {}",
        tick_value
    );
}

#[test]
fn test_id_and_key() {
    use finstack_valuations::instruments::Instrument;
    use finstack_valuations::pricer::InstrumentType;

    let (_, start, end) = standard_dates();
    let future = create_standard_future(start, end);

    assert_eq!(future.id(), "IRF_TEST");
    assert_eq!(future.key(), InstrumentType::InterestRateFuture);
}

#[test]
fn test_attributes() {
    use finstack_valuations::instruments::Instrument;

    let (_, start, end) = standard_dates();
    let mut future = create_standard_future(start, end);

    // Test attributes access
    let attrs = future.attributes();
    assert!(attrs.meta.is_empty());
    assert!(attrs.tags.is_empty());

    // Test mutable attributes
    let attrs_mut = future.attributes_mut();
    attrs_mut
        .meta
        .insert("test_key".to_string(), "test_value".to_string());

    assert_eq!(
        future.attributes().meta.get("test_key"),
        Some(&"test_value".to_string())
    );
}

#[test]
fn test_clone_box() {
    use finstack_valuations::instruments::Instrument;

    let (_, start, end) = standard_dates();
    let future = create_standard_future(start, end);

    let boxed = future.clone_box();
    assert_eq!(boxed.id(), "IRF_TEST");
}
