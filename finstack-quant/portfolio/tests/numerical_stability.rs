//! Numerical stability tests for portfolio.

mod common;

use common::*;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::{Error, PortfolioBuilder};
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use std::sync::Arc;
use time::Duration;

#[test]
fn test_compensated_summation_large_portfolio() {
    // Test that compensated summation handles large portfolios with mixed-sign values
    // Create 1000 positions with alternating ±1e12 and 1e0 values
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let mut builder = PortfolioBuilder::new("LARGE_PORTFOLIO")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENTITY_A"));

    for i in 0..1000 {
        let value: f64 = if i % 2 == 0 {
            1e12 // Large positive
        } else {
            -1e12 // Large negative
        };

        let deposit = Deposit::builder()
            .id(format!("DEP_{}", i).into())
            .notional(Money::new(value.abs(), Currency::USD))
            .start_date(as_of)
            .maturity(end_date)
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .unwrap();

        let quantity = if value < 0.0 { -1.0 } else { 1.0 };
        let position = Position::new(
            format!("POS_{}", i),
            "ENTITY_A",
            format!("DEP_{}", i),
            Arc::new(deposit),
            quantity,
            PositionUnit::Units,
        )
        .unwrap();

        builder = builder.position(position);
    }

    let portfolio = builder.build().unwrap();
    let market = market_with_usd();
    let config = FinstackConfig::default();

    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();

    // With alternating ±1e12, the total should be close to zero
    // Compensated summation should maintain accuracy
    let total = valuation.total_base_ccy.amount();

    // Verify that compensated summation produces a reasonable result
    // The exact value depends on discounting, but should be finite and not NaN/Inf
    assert!(total.is_finite(), "Total should be finite");
    // With flat curve (DF=1) and alternating ±1e12 positions,
    // the total should be very close to 0
    // Allow for small discounting effects from the 30-day deposit
    assert!(
        total.abs() < 1e9, // Much tighter than 1e15
        "Compensated sum of alternating ±1e12 should be near 0, got: {}",
        total
    );
}

#[test]
fn test_aggregated_metrics_are_finite() {
    // Verify that standard instruments produce finite metrics
    // and that aggregation preserves finiteness.
    // Note: This does not test NaN exclusion - a mock instrument
    // returning NaN would be needed for that.
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let deposit = Deposit::builder()
        .id("DEP_NAN".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let position = Position::new(
        "POS_NAN",
        "ENTITY_A",
        "DEP_NAN",
        Arc::new(deposit),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("TEST")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENTITY_A"))
        .position(position)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();

    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();
    let metrics = finstack_quant_portfolio::metrics::aggregate_metrics(
        &valuation,
        Currency::USD,
        &market,
        as_of,
    )
    .unwrap();

    // Verify that all aggregated metrics are finite
    for (metric_id, agg_metric) in &metrics.aggregated {
        assert!(
            agg_metric.total.is_finite(),
            "Metric {} total should be finite, got: {}",
            metric_id,
            agg_metric.total
        );
        for (entity_id, value) in &agg_metric.by_entity {
            assert!(
                value.is_finite(),
                "Metric {} for entity {} should be finite, got: {}",
                metric_id,
                entity_id,
                value
            );
        }
    }
}

#[test]
fn test_inf_quantity_rejected() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let deposit = Deposit::builder()
        .id("DEP_INF".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    // Try to create position with Inf quantity
    let result = Position::new(
        "POS_INF",
        "ENTITY_A",
        "DEP_INF",
        Arc::new(deposit),
        f64::INFINITY,
        PositionUnit::Units,
    );

    assert!(
        result.is_err(),
        "Position::new() should reject Inf quantity"
    );

    match result {
        Err(Error::InvalidInput(msg)) => {
            assert!(
                msg.contains("finite"),
                "Error message should mention 'finite'"
            );
        }
        other => panic!("Expected InvalidInput error, got: {:?}", other),
    }
}

#[test]
fn test_nan_quantity_rejected() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let deposit = Deposit::builder()
        .id("DEP_NAN".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    // Try to create position with NaN quantity
    let result = Position::new(
        "POS_NAN",
        "ENTITY_A",
        "DEP_NAN",
        Arc::new(deposit),
        f64::NAN,
        PositionUnit::Units,
    );

    assert!(
        result.is_err(),
        "Position::new() should reject NaN quantity"
    );

    match result {
        Err(Error::InvalidInput(msg)) => {
            assert!(
                msg.contains("finite"),
                "Error message should mention 'finite'"
            );
        }
        other => panic!("Expected InvalidInput error, got: {:?}", other),
    }
}

#[test]
fn test_neumaier_sum_accuracy() {
    // Direct test of Neumaier summation accuracy
    // Create alternating large positive and negative values
    let values: Vec<f64> = (0..1000)
        .map(|i| if i % 2 == 0 { 1e12 } else { -1e12 })
        .collect();

    let neumaier_result = neumaier_sum(values.iter().copied());
    let naive_result: f64 = values.iter().sum();

    // True sum of alternating ±1e12 (500 each) is exactly 0.0
    let true_sum = 0.0;
    let neumaier_error = (neumaier_result - true_sum).abs();
    let naive_error = (naive_result - true_sum).abs();

    // Neumaier should be at least as accurate as naive summation
    assert!(
        neumaier_error <= naive_error + 1e-6, // Small epsilon for floating point comparison
        "Neumaier error ({}) should be <= naive error ({})",
        neumaier_error,
        naive_error
    );

    // Both should be reasonably close to the true sum
    // Even naive sum should be within ~1e4 for this test case
    assert!(
        neumaier_error < 1e6,
        "Neumaier result should be close to true sum 0.0, got error: {}",
        neumaier_error
    );
}
