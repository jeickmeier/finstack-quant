//! Edge case and error handling tests for convertible bonds.
//!
//! Tests boundary conditions and error scenarios:
//! - Currency safety and mismatches
//! - Time mapping edge cases
//! - Matured bonds
//! - Missing/invalid market data
//! - Floating coupon handling
//! - Day count propagation
//! - Numerical stability

use super::fixtures::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    price_convertible_bond, ConvertibleTreeType,
};
use time::Month;

#[test]
fn test_currency_safety_mismatch() {
    let bond = create_standard_convertible();
    let base_date = dates::base_date();

    // Create market with EUR instead of USD for equity
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;

    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (10.0, 0.741)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(discount_curve)
        .insert_price(
            "AAPL",
            MarketScalar::Price(Money::new(150.0, Currency::EUR)),
        ) // EUR mismatch!
        .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02));

    // Should detect currency mismatch and fail
    let result = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(20),
        dates::base_date(),
    );
    assert!(
        result.is_err(),
        "Should fail on currency mismatch between bond and equity"
    );
}

#[test]
fn test_currency_consistency_with_unitless_spot() {
    let bond = create_standard_convertible();
    let market = create_market_context(); // Uses unitless spot

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Should work with unitless spot and return bond's currency
    assert_eq!(
        price.currency(),
        Currency::USD,
        "Should return bond's currency"
    );
}

#[test]
fn test_floating_coupon_with_reset_events() {
    let bond = create_floating_convertible();
    let market = create_market_context();

    // Should handle floating coupons with reset events properly
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(20),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount().is_finite(),
        "Should handle floating coupons: {}",
        price.amount()
    );
}

#[test]
fn test_day_count_propagation_for_call_put() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};

    let issue = dates::issue();
    let maturity = dates::maturity_1y();
    let call_date = Date::from_calendar_date(2025, Month::July, 1).unwrap();

    let mut bond = create_standard_convertible();
    bond.issue_date = issue;
    bond.maturity = maturity;

    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start_date: call_date,
        end_date: call_date,
        price_pct_of_par: 101.0,
        make_whole: None,
    });
    bond.call_put = Some(call_put);

    let market = create_market_context();

    // Should use schedule day_count for mapping (not hardcoded)
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(10),
        dates::base_date(),
    )
    .unwrap();
    assert!(
        price.amount().is_finite(),
        "Day count propagation should work"
    );
}

#[test]
fn test_short_maturity_bond() {
    let bond = create_floating_convertible(); // 1 year maturity
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(20),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() > 0.0 && price.amount().is_finite(),
        "Short maturity should work: {}",
        price.amount()
    );
}

#[test]
fn test_very_few_tree_steps() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    // Test with minimum reasonable steps
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(3),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() > 0.0 && price.amount().is_finite(),
        "Should work with few steps: {}",
        price.amount()
    );
}

#[test]
fn test_time_mapping_with_quarterly_coupons() {
    use finstack_quant_cashflows::builder::specs::{CouponType, FixedCouponSpec};
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};

    let issue = dates::issue();
    let maturity = dates::maturity_1y();

    let conversion_spec = finstack_quant_valuations::instruments::fixed_income::convertible::ConversionSpec {
        ratio: Some(bond_params::CONVERSION_RATIO),
        price: None,
        policy: finstack_quant_valuations::instruments::fixed_income::convertible::ConversionPolicy::Voluntary,
        anti_dilution: finstack_quant_valuations::instruments::fixed_income::convertible::AntiDilutionPolicy::None,
        dividend_adjustment:
            finstack_quant_valuations::instruments::fixed_income::convertible::DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let fixed_coupon = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: rust_decimal::Decimal::try_from(0.06).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::quarterly(),

            dc: DayCount::Act365F,

            bdc: BusinessDayConvention::Following,

            calendar_id: "weekends_only".to_string(),

            stub: StubKind::None,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
        },
    };

    let bond = finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond {
        id: "TEST_QUARTERLY".to_string().into(),
        notional: Money::new(bond_params::NOTIONAL, Currency::USD),
        issue_date: issue,
        maturity,
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        settlement_days: None,
        recovery_rate: None,
        conversion: conversion_spec,
        underlying_equity_id: Some("AAPL".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: Some(fixed_coupon),
        floating_coupon: None,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = create_market_context();
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(10),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() > 0.0,
        "Quarterly coupons with time mapping should work: {}",
        price.amount()
    );
}

#[test]
fn test_narrow_conversion_window_with_few_steps() {
    use finstack_quant_valuations::instruments::fixed_income::convertible::ConversionPolicy;

    let window_start = Date::from_calendar_date(2027, Month::June, 1).unwrap();
    let window_end = Date::from_calendar_date(2027, Month::July, 1).unwrap();

    let bond = create_convertible_with_policy(ConversionPolicy::Window {
        start: window_start,
        end: window_end,
    });

    let market = create_market_context();

    // With few steps, window might map to single step
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(10),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() > bond_params::NOTIONAL * 0.5,
        "Narrow window with few steps should work: {}",
        price.amount()
    );
}

#[test]
fn test_call_put_on_same_date() {
    use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};

    let option_date = dates::mid_date();

    let mut bond = create_standard_convertible();
    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start_date: option_date,
        end_date: option_date,
        price_pct_of_par: 102.0,
        make_whole: None,
    });
    call_put.puts.push(CallPut {
        start_date: option_date,
        end_date: option_date,
        price_pct_of_par: 98.0,
        make_whole: None,
    });
    bond.call_put = Some(call_put);

    let market = create_market_context();

    // Should handle call and put on same date
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();
    assert!(price.amount().is_finite(), "Same date call/put should work");
}

#[test]
fn test_call_put_at_maturity() {
    use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};

    let mut bond = create_standard_convertible();
    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start_date: bond.maturity,
        end_date: bond.maturity,
        price_pct_of_par: 100.0,
        make_whole: None,
    });
    bond.call_put = Some(call_put);

    let market = create_market_context();

    // Call at maturity should work
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();
    assert!(price.amount().is_finite(), "Call at maturity should work");
}

#[test]
fn test_call_put_before_issue() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};

    let mut bond = create_standard_convertible();
    let mut call_put = CallPutSchedule::default();

    // Call date before issue (should be ignored)
    let before_issue = Date::from_calendar_date(2024, Month::January, 1).unwrap();
    call_put.calls.push(CallPut {
        start_date: before_issue,
        end_date: before_issue,
        price_pct_of_par: 102.0,
        make_whole: None,
    });
    bond.call_put = Some(call_put);

    let market = create_market_context();

    // Should handle gracefully (ignore past calls)
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();
    assert!(
        price.amount().is_finite(),
        "Past call dates should be ignored"
    );
}

#[test]
fn test_call_put_after_maturity() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};

    let mut bond = create_standard_convertible();
    let mut call_put = CallPutSchedule::default();

    // Call date after maturity (should be ignored)
    let after_maturity = Date::from_calendar_date(2031, Month::January, 1).unwrap();
    call_put.calls.push(CallPut {
        start_date: after_maturity,
        end_date: after_maturity,
        price_pct_of_par: 102.0,
        make_whole: None,
    });
    bond.call_put = Some(call_put);

    let market = create_market_context();

    // Should handle gracefully (ignore calls after maturity)
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();
    assert!(
        price.amount().is_finite(),
        "Call dates after maturity should be ignored"
    );
}

#[test]
fn test_zero_conversion_ratio() {
    use finstack_quant_valuations::instruments::fixed_income::convertible::{
        ConversionPolicy, ConversionSpec,
    };

    let mut bond = create_standard_convertible();
    bond.conversion = ConversionSpec {
        ratio: Some(0.0), // Zero conversion ratio - invalid but test handling
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution:
            finstack_quant_valuations::instruments::fixed_income::convertible::AntiDilutionPolicy::None,
        dividend_adjustment:
            finstack_quant_valuations::instruments::fixed_income::convertible::DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let market = create_market_context();

    // Should still price (as straight bond)
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();
    assert!(
        price.amount() > 0.0,
        "Zero conversion ratio should behave like straight bond"
    );
}

#[test]
fn test_very_high_conversion_ratio() {
    use finstack_quant_valuations::instruments::fixed_income::convertible::{
        ConversionPolicy, ConversionSpec,
    };

    let mut bond = create_standard_convertible();
    bond.conversion = ConversionSpec {
        ratio: Some(1000.0), // Very high conversion ratio
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution:
            finstack_quant_valuations::instruments::fixed_income::convertible::AntiDilutionPolicy::None,
        dividend_adjustment:
            finstack_quant_valuations::instruments::fixed_income::convertible::DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let market = create_market_context();

    // Should handle high conversion ratio
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();
    assert!(
        price.amount().is_finite() && price.amount() > 0.0,
        "Very high conversion ratio should work: {}",
        price.amount()
    );
}

#[test]
fn test_missing_underlying_equity() {
    let mut bond = create_standard_convertible();
    bond.underlying_equity_id = None; // Missing equity ID

    let market = create_market_context();

    // Should fail gracefully
    let result = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    );
    assert!(
        result.is_err(),
        "Should fail with missing underlying equity ID"
    );
}

#[test]
fn test_missing_volatility_data() {
    let bond = create_standard_convertible();

    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;

    let base_date = dates::base_date();
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (10.0, 0.741)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Market without volatility
    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(discount_curve)
        .insert_price("AAPL", MarketScalar::Unitless(150.0))
        // Missing volatility
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02));

    // Should fail due to missing volatility
    let result = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    );
    assert!(result.is_err(), "Should fail with missing volatility data");
}

#[test]
fn test_missing_discount_curve() {
    let bond = create_standard_convertible();

    // Market without discount curve
    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert_price("AAPL", MarketScalar::Unitless(150.0))
        .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02));

    // Should fail due to missing discount curve
    let result = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    );
    assert!(result.is_err(), "Should fail with missing discount curve");
}

#[test]
fn test_numerical_stability_extreme_parameters() {
    let bond = create_standard_convertible();

    // Extreme but valid parameters
    let market = create_market_context_with_params(
        1000.0, // Very high spot
        0.90,   // Very high vol
        0.10,   // High dividend yield
    );

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount().is_finite() && !price.amount().is_nan(),
        "Should remain numerically stable with extreme parameters: {}",
        price.amount()
    );
}

#[test]
fn test_empty_call_put_schedule() {
    let mut bond = create_standard_convertible();
    bond.call_put = Some(
        finstack_quant_valuations::instruments::fixed_income::bond::CallPutSchedule::default(),
    ); // Empty schedule

    let market = create_market_context();

    // Should work with empty call/put schedule
    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();
    assert!(price.amount() > 0.0, "Empty call/put schedule should work");
}
