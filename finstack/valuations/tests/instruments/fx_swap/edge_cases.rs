//! Edge case and error handling tests for FX swaps.
//!
//! Tests boundary conditions, error handling, and unusual scenarios
//! to ensure robustness and proper validation.

use super::fixtures::*;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_valuations::instruments::fx::fx_swap::FxSwap;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::Month;

#[test]
fn test_zero_notional() {
    // Test swap with zero notional
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap("ZERO_NOTIONAL", dates.near_date, dates.far_date_1y, 0.0);

    let pv = swap.value(&market, dates.as_of).unwrap();

    // Zero notional should produce zero PV
    assert_eq!(pv.amount(), 0.0, "Zero notional should yield zero PV");
}

#[test]
fn test_very_large_notional() {
    // Test swap with very large notional (billions)
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "LARGE_NOTIONAL",
        dates.near_date,
        dates.far_date_1y,
        1_000_000_000.0, // 1 billion EUR
    );

    let pv = swap.value(&market, dates.as_of).unwrap();

    // Should handle large notionals without overflow
    assert!(
        pv.amount().is_finite(),
        "Large notional should produce finite PV"
    );
}

#[test]
fn test_same_near_far_dates() {
    // Test swap where near and far dates are the same (degenerate case)
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "SAME_DATES",
        dates.near_date,
        dates.near_date, // Same as near date
        1_000_000.0,
    );

    let pv = swap.value(&market, dates.as_of).unwrap();

    // Degenerate swap (same near/far dates) has no net cash flow, so PV = 0
    assert!(
        pv.amount().abs() < 1.0,
        "Swap with same near/far dates should have zero PV, got {}",
        pv.amount()
    );
}

#[test]
fn test_far_before_near() {
    let dates = TestDates::standard();

    let err = FxSwap::builder()
        .id("INVERTED_DATES".to_string().into())
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .near_date(dates.far_date_1y)
        .far_date(dates.near_date)
        .base_notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id("USD-OIS".into())
        .foreign_discount_curve_id("EUR-OIS".into())
        .build()
        .expect_err("builder should reject inverted dates");
    assert!(
        err.to_string().contains("near_date") && err.to_string().contains("far_date"),
        "Error should mention date ordering: {}",
        err
    );
}

#[test]
fn test_missing_fx_matrix() {
    // Test swap valuation when FX matrix is not provided
    let dates = TestDates::standard();

    // Create market without FX matrix
    let as_of = dates.as_of;
    let usd_curve = finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.9)])
        .interp(finstack_core::math::interp::InterpStyle::Linear)
        .build()
        .unwrap();

    let eur_curve = finstack_core::market_data::term_structures::DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.95)])
        .interp(finstack_core::math::interp::InterpStyle::Linear)
        .build()
        .unwrap();

    let market = finstack_core::market_data::context::MarketContext::new()
        .insert(usd_curve)
        .insert(eur_curve);
    // No FX matrix!

    let swap = create_standard_fx_swap(
        "NO_FX_MATRIX",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
    );

    // Should return error when FX matrix is missing
    let result = swap.value(&market, dates.as_of);
    assert!(result.is_err(), "Should error without FX matrix");
}

#[test]
fn test_missing_fx_matrix_with_contract_rates() {
    // Contract rates are cashflow terms, not substitutes for model FX spot.
    let dates = TestDates::standard();

    // Create market without FX matrix
    let as_of = dates.as_of;
    let usd_curve = finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.9)])
        .interp(finstack_core::math::interp::InterpStyle::Linear)
        .build()
        .unwrap();

    let eur_curve = finstack_core::market_data::term_structures::DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.95)])
        .interp(finstack_core::math::interp::InterpStyle::Linear)
        .build()
        .unwrap();

    let market = finstack_core::market_data::context::MarketContext::new()
        .insert(usd_curve)
        .insert(eur_curve);

    let swap = create_fx_swap_with_rates(
        "NO_FX_WITH_RATES",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
        1.10,
        1.15,
    );

    let result = swap.value(&market, dates.as_of);
    assert!(
        result.is_err(),
        "Should require FX matrix even when contract rates are provided"
    );
    let err = result.expect_err("expected missing FX matrix validation");
    assert!(err.to_string().contains("requires FxMatrix"));
}

#[test]
fn test_missing_discount_curve() {
    // Test swap valuation when discount curve is missing
    let dates = TestDates::standard();
    let market = finstack_core::market_data::context::MarketContext::new(); // Empty market

    let swap = create_standard_fx_swap("NO_CURVE", dates.near_date, dates.far_date_1y, 1_000_000.0);

    // Should return error when curves are missing
    let result = swap.value(&market, dates.as_of);
    assert!(result.is_err(), "Should error when discount curves missing");
}

#[test]
fn test_currency_mismatch_notional() {
    // Test swap with notional currency not matching base currency
    let dates = TestDates::standard();

    let err = FxSwap::builder()
        .id("CURRENCY_MISMATCH".to_string().into())
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .near_date(dates.near_date)
        .far_date(dates.far_date_1y)
        .base_notional(Money::new(1_000_000.0, Currency::GBP)) // Wrong currency!
        .domestic_discount_curve_id("USD-OIS".into())
        .foreign_discount_curve_id("EUR-OIS".into())
        .build()
        .expect_err("builder should reject notional currency mismatch");

    assert!(
        err.to_string().contains("base_notional currency"),
        "Error should mention notional currency mismatch: {}",
        err
    );
}

#[test]
fn test_valuation_at_near_date() {
    // Test valuation when as_of equals near date
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "AT_NEAR_DATE",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
    );

    let pv = swap.value(&market, dates.near_date).unwrap();

    // Should produce valid PV at near date
    assert!(pv.amount().is_finite(), "PV at near date should be finite");
}

#[test]
fn test_valuation_at_far_date() {
    // Test valuation when as_of equals far date
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_fx_swap_with_rates(
        "AT_FAR_DATE",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
        1.10,
        1.15,
    );

    let pv = swap.value(&market, dates.far_date_1y).unwrap();

    // At far date, remaining PV should be small
    assert!(pv.amount().is_finite(), "PV at far date should be finite");
}

#[test]
fn test_valuation_after_maturity() {
    // Test valuation after far date (expired swap)
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap("EXPIRED", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let as_of_after = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let pv = swap.value(&market, as_of_after).unwrap();

    // After maturity, PV should be close to zero or represent final settlement
    assert!(
        pv.amount().is_finite(),
        "PV after maturity should be finite"
    );
}

#[test]
fn test_extreme_contract_rates() {
    // Test swap with extreme contract rates
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_fx_swap_with_rates(
        "EXTREME_RATES",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
        0.01,  // Very low near rate
        100.0, // Very high far rate
    );

    let pv = swap.value(&market, dates.as_of).unwrap();

    // Should handle extreme rates without overflow
    assert!(pv.amount().is_finite(), "Should handle extreme rates");
}

#[test]
fn test_negative_contract_rates() {
    let dates = TestDates::standard();

    let err = FxSwap::builder()
        .id("NEGATIVE_RATES".to_string().into())
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .near_date(dates.near_date)
        .far_date(dates.far_date_1y)
        .base_notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id("USD-OIS".into())
        .foreign_discount_curve_id("EUR-OIS".into())
        .near_rate(1.10)
        .far_rate(-0.05)
        .build()
        .expect_err("builder should reject negative FX rates");
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("positive"),
        "Error should mention rate must be positive: {}",
        err_msg
    );
}

#[test]
fn test_builder_validation() {
    // Test that builder properly validates required fields
    // Attempt to build without required fields
    let result = FxSwap::builder()
        .id("INCOMPLETE".to_string().into())
        .base_currency(Currency::EUR)
        // Missing other required fields
        .build();

    assert!(result.is_err(), "Builder should validate required fields");
}

#[test]
fn test_metric_error_handling() {
    // Test that metrics handle missing data gracefully
    let dates = TestDates::standard();
    let market = finstack_core::market_data::context::MarketContext::new(); // Empty market

    let swap = create_standard_fx_swap(
        "METRIC_ERROR",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
    );

    // Should return error when trying to calculate metrics without market data
    let result = swap.price_with_metrics(
        &market,
        dates.as_of,
        &[MetricId::custom("forward_points")],
        finstack_valuations::instruments::PricingOptions::default(),
    );

    assert!(result.is_err(), "Metrics should error without market data");
}
