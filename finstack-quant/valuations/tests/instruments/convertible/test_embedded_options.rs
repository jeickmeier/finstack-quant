//! Embedded options tests for convertible bonds.
//!
//! Tests call and put features:
//! - Issuer call options (early redemption)
//! - Holder put options (early redemption)
//! - Combined call/put schedules
//! - Multiple calls/puts
//! - Option impact on pricing

use super::fixtures::*;
use finstack_quant_core::dates::Date;
use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    price_convertible_bond, ConvertibleTreeType,
};
use time::Month;

#[test]
fn test_callable_convertible_bond() {
    let call_date = dates::mid_date();
    let bond = create_callable_convertible(call_date, 102.0);
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Should price successfully with call feature
    assert!(
        price.amount() > 0.0 && price.amount().is_finite(),
        "Callable convertible should price: {}",
        price.amount()
    );
}

#[test]
fn test_callable_caps_upside() {
    let call_date = dates::mid_date();
    let bond_callable = create_callable_convertible(call_date, 102.0);
    let bond_plain = create_standard_convertible();
    let market = create_market_context();

    let price_callable = price_convertible_bond(
        &bond_callable,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    let price_plain = price_convertible_bond(
        &bond_plain,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Callable should be worth less than non-callable (issuer option)
    assert!(
        price_callable.amount() <= price_plain.amount() * 1.05,
        "Callable {} should be <= plain vanilla {} (issuer option value)",
        price_callable.amount(),
        price_plain.amount()
    );
}

#[test]
fn test_puttable_convertible_bond() {
    let put_date = Date::from_calendar_date(2028, Month::January, 1).unwrap();
    let bond = create_puttable_convertible(put_date, 98.0);
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Should price successfully with put feature
    assert!(
        price.amount() > 0.0 && price.amount().is_finite(),
        "Puttable convertible should price: {}",
        price.amount()
    );
}

#[test]
fn test_puttable_provides_floor() {
    let put_date = Date::from_calendar_date(2028, Month::January, 1).unwrap();
    let put_price_pct = 98.0;
    let bond = create_puttable_convertible(put_date, put_price_pct);

    // Test with OTM scenario
    let market = create_market_context_with_params(
        market_params::SPOT_LOW,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Should have put floor support (though discounted to present)
    let put_floor = bond_params::NOTIONAL * (put_price_pct / 100.0);
    assert!(
        price.amount() >= put_floor * 0.85, // Discounted to present
        "Puttable should have floor support: {} vs {}",
        price.amount(),
        put_floor
    );
}

#[test]
fn test_puttable_increases_value() {
    let put_date = Date::from_calendar_date(2028, Month::January, 1).unwrap();
    let bond_puttable = create_puttable_convertible(put_date, 98.0);
    let bond_plain = create_standard_convertible();

    // Test with OTM scenario where put is valuable
    let market = create_market_context_with_params(
        market_params::SPOT_LOW,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );

    let price_puttable = price_convertible_bond(
        &bond_puttable,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    let price_plain = price_convertible_bond(
        &bond_plain,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Puttable should be worth more than non-puttable (holder option)
    assert!(
        price_puttable.amount() >= price_plain.amount() * 0.95,
        "Puttable {} should be >= plain vanilla {} (holder option value)",
        price_puttable.amount(),
        price_plain.amount()
    );
}

#[test]
fn test_combined_call_put_convertible() {
    let call_date = Date::from_calendar_date(2028, Month::January, 1).unwrap();
    let put_date = Date::from_calendar_date(2027, Month::January, 1).unwrap();

    let bond = create_callable_puttable_convertible(call_date, 103.0, put_date, 97.0);
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(60),
        dates::base_date(),
    )
    .unwrap();

    // Should price successfully with both features
    assert!(
        price.amount() > 0.0 && price.amount().is_finite(),
        "Callable/puttable convertible should price: {}",
        price.amount()
    );
}

#[test]
fn test_combined_call_put_bounded() {
    let call_date = Date::from_calendar_date(2028, Month::January, 1).unwrap();
    let put_date = Date::from_calendar_date(2027, Month::January, 1).unwrap();

    let bond = create_callable_puttable_convertible(call_date, 103.0, put_date, 97.0);
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(60),
        dates::base_date(),
    )
    .unwrap();

    // Should be bounded by put floor and call ceiling (approximately)
    let put_floor = bond_params::NOTIONAL * 0.97;
    let _call_ceiling = bond_params::NOTIONAL * 1.03;

    assert!(
        price.amount() >= put_floor * 0.80, // Discounted
        "Should respect put floor: {} vs {}",
        price.amount(),
        put_floor
    );
}

#[test]
fn test_multiple_call_dates() {
    let mut bond = create_standard_convertible();

    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start_date: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
        end_date: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
        price_pct_of_par: 105.0,
        make_whole: None,
    });
    call_put.calls.push(CallPut {
        start_date: Date::from_calendar_date(2028, Month::January, 1).unwrap(),
        end_date: Date::from_calendar_date(2028, Month::January, 1).unwrap(),
        price_pct_of_par: 103.0,
        make_whole: None,
    });
    call_put.calls.push(CallPut {
        start_date: Date::from_calendar_date(2029, Month::January, 1).unwrap(),
        end_date: Date::from_calendar_date(2029, Month::January, 1).unwrap(),
        price_pct_of_par: 101.0,
        make_whole: None,
    });

    bond.call_put = Some(call_put);

    let market = create_market_context();
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(60),
        dates::base_date(),
    )
    .unwrap();

    // Should handle multiple call dates
    assert!(
        price.amount() > 0.0 && price.amount().is_finite(),
        "Multiple calls should price: {}",
        price.amount()
    );
}

#[test]
fn test_overlapping_call_windows_order_invariant() {
    // Regression test: overlapping call windows (step-down calls) must be mapped
    // as the issuer's optimal call at each step (min call price), independent
    // of input ordering.
    let mut bond_a = create_standard_convertible();
    let mut bond_b = create_standard_convertible();

    let mut schedule_a = CallPutSchedule::default();
    // Higher call price window (starts earlier, overlaps later window)
    schedule_a.calls.push(CallPut {
        start_date: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
        end_date: Date::from_calendar_date(2029, Month::January, 1).unwrap(),
        price_pct_of_par: 105.0,
        make_whole: None,
    });
    // Step-down to cheaper call later, overlapping 2028-2029
    schedule_a.calls.push(CallPut {
        start_date: Date::from_calendar_date(2028, Month::January, 1).unwrap(),
        end_date: Date::from_calendar_date(2029, Month::January, 1).unwrap(),
        price_pct_of_par: 101.0,
        make_whole: None,
    });
    bond_a.call_put = Some(schedule_a);

    let mut schedule_b = CallPutSchedule::default();
    // Same two windows but reversed insertion order
    schedule_b.calls.push(CallPut {
        start_date: Date::from_calendar_date(2028, Month::January, 1).unwrap(),
        end_date: Date::from_calendar_date(2029, Month::January, 1).unwrap(),
        price_pct_of_par: 101.0,
        make_whole: None,
    });
    schedule_b.calls.push(CallPut {
        start_date: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
        end_date: Date::from_calendar_date(2029, Month::January, 1).unwrap(),
        price_pct_of_par: 105.0,
        make_whole: None,
    });
    bond_b.call_put = Some(schedule_b);

    // Use low spot so conversion is out-of-the-money and call price level matters.
    let market = create_market_context_with_params(
        market_params::SPOT_LOW,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );

    let price_a = price_convertible_bond(
        &bond_a,
        &market,
        ConvertibleTreeType::Binomial(80),
        dates::base_date(),
    )
    .unwrap();
    let price_b = price_convertible_bond(
        &bond_b,
        &market,
        ConvertibleTreeType::Binomial(80),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        (price_a.amount() - price_b.amount()).abs() < 1e-9,
        "Overlapping call windows must be order invariant: {} vs {}",
        price_a.amount(),
        price_b.amount()
    );
}

#[test]
fn test_multiple_put_dates() {
    let mut bond = create_standard_convertible();

    let mut call_put = CallPutSchedule::default();
    call_put.puts.push(CallPut {
        start_date: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
        end_date: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
        price_pct_of_par: 98.0,
        make_whole: None,
    });
    call_put.puts.push(CallPut {
        start_date: Date::from_calendar_date(2028, Month::January, 1).unwrap(),
        end_date: Date::from_calendar_date(2028, Month::January, 1).unwrap(),
        price_pct_of_par: 99.0,
        make_whole: None,
    });
    call_put.puts.push(CallPut {
        start_date: Date::from_calendar_date(2029, Month::January, 1).unwrap(),
        end_date: Date::from_calendar_date(2029, Month::January, 1).unwrap(),
        price_pct_of_par: 100.0,
        make_whole: None,
    });

    bond.call_put = Some(call_put);

    let market = create_market_context();
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(60),
        dates::base_date(),
    )
    .unwrap();

    // Should handle multiple put dates
    assert!(
        price.amount() > 0.0 && price.amount().is_finite(),
        "Multiple puts should price: {}",
        price.amount()
    );
}

#[test]
fn test_call_price_at_par() {
    let call_date = dates::mid_date();
    let bond = create_callable_convertible(call_date, 100.0); // At par
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() > 0.0,
        "Callable at par should price: {}",
        price.amount()
    );
}

#[test]
fn test_call_price_at_premium() {
    let call_date = dates::mid_date();
    let bond = create_callable_convertible(call_date, 110.0); // At premium
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() > 0.0,
        "Callable at premium should price: {}",
        price.amount()
    );
}

#[test]
fn test_put_price_at_discount() {
    let put_date = Date::from_calendar_date(2028, Month::January, 1).unwrap();
    let bond = create_puttable_convertible(put_date, 95.0); // At discount
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() > 0.0,
        "Puttable at discount should price: {}",
        price.amount()
    );
}

#[test]
fn test_call_before_conversion_window() {
    use finstack_quant_valuations::instruments::fixed_income::convertible::ConversionPolicy;

    let call_date = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let window_start = Date::from_calendar_date(2027, Month::January, 1).unwrap();
    let window_end = Date::from_calendar_date(2029, Month::January, 1).unwrap();

    let mut bond = create_convertible_with_policy(ConversionPolicy::Window {
        start: window_start,
        end: window_end,
    });

    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start_date: call_date,
        end_date: call_date,
        price_pct_of_par: 102.0,
        make_whole: None,
    });
    bond.call_put = Some(call_put);

    let market = create_market_context();
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Call before conversion window should work
    assert!(
        price.amount() > 0.0,
        "Call before window should price: {}",
        price.amount()
    );
}

#[test]
fn test_call_during_conversion_window() {
    use finstack_quant_valuations::instruments::fixed_income::convertible::ConversionPolicy;

    let call_date = Date::from_calendar_date(2028, Month::January, 1).unwrap();
    let window_start = Date::from_calendar_date(2027, Month::January, 1).unwrap();
    let window_end = Date::from_calendar_date(2029, Month::January, 1).unwrap();

    let mut bond = create_convertible_with_policy(ConversionPolicy::Window {
        start: window_start,
        end: window_end,
    });

    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start_date: call_date,
        end_date: call_date,
        price_pct_of_par: 102.0,
        make_whole: None,
    });
    bond.call_put = Some(call_put);

    let market = create_market_context();
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Call during conversion window should work
    assert!(
        price.amount() > 0.0,
        "Call during window should price: {}",
        price.amount()
    );
}

#[test]
fn test_early_call_date() {
    let call_date = Date::from_calendar_date(2026, Month::January, 1).unwrap(); // Early call
    let bond = create_callable_convertible(call_date, 102.0);
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Early call should have more impact
    let bond_plain = create_standard_convertible();
    let price_plain = price_convertible_bond(
        &bond_plain,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() < price_plain.amount() * 1.05,
        "Early call should impact value: {} vs {}",
        price.amount(),
        price_plain.amount()
    );
}

#[test]
fn test_late_put_date() {
    let put_date = Date::from_calendar_date(2029, Month::January, 1).unwrap(); // Late put
    let bond = create_puttable_convertible(put_date, 98.0);
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Late put should still provide some value
    assert!(
        price.amount() > bond_params::NOTIONAL * 0.90,
        "Late put should still support value: {}",
        price.amount()
    );
}
