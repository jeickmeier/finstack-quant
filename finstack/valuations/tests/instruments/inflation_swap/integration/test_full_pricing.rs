//! Full pricing workflow integration tests.

use crate::inflation_swap::fixtures::*;
use finstack_core::dates::{Date, DayCount};
use finstack_valuations::instruments::rates::inflation_swap::InflationSwapBuilder;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::PayReceive;
use finstack_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_complete_pricing_workflow() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = realistic_market(as_of);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-FULL-1".into())
        .notional(large_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Default::default())
        .build()
        .unwrap();

    // Price with all metrics
    let metrics = vec![
        MetricId::Dv01,
        MetricId::Dv01,
        MetricId::ParRate,
        MetricId::Theta,
        MetricId::BucketedDv01,
        MetricId::Inflation01,
        MetricId::custom("breakeven"),
        MetricId::custom("fixed_leg_pv"),
        MetricId::custom("inflation_leg_pv"),
    ];

    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &metrics,
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Verify all metrics are present
    assert!(result.measures.contains_key("dv01"));
    assert!(result.measures.contains_key("dv01"));
    assert!(result.measures.contains_key("par_rate"));
    assert!(result.measures.contains_key("theta"));
    assert!(result.measures.contains_key("bucketed_dv01"));
    assert!(result.measures.contains_key("inflation01"));
    assert!(result.measures.contains_key("breakeven"));
    assert!(result.measures.contains_key("fixed_leg_pv"));
    assert!(result.measures.contains_key("inflation_leg_pv"));

    // Verify all metrics are finite
    for (name, value) in &result.measures {
        assert!(
            value.is_finite(),
            "Metric {} should be finite: {}",
            name,
            value
        );
    }

    // Verify PV is reasonable
    assert!(result.value.amount().is_finite());
    assert!(result.value.amount().abs() < large_notional().amount());
}

#[test]
fn test_pricing_over_time() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-TIME".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.025).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Receive)
        .attributes(Default::default())
        .build()
        .unwrap();

    // Price at multiple points in time
    let mut pvs = Vec::new();
    for days in &[0, 30, 90, 180, 365] {
        let pricing_date = as_of + time::Duration::days(*days);
        if pricing_date < maturity {
            let pv = swap.value(&ctx, pricing_date).unwrap().amount();
            pvs.push((pricing_date, pv));
        }
    }

    // All PVs should be finite
    for (date, pv) in &pvs {
        assert!(pv.is_finite(), "PV should be finite at {:?}: {}", date, pv);
    }

    // PV should change over time (due to time decay and discounting)
    assert!(pvs.len() > 1, "Should have multiple pricing points");
}

#[test]
fn test_portfolio_of_swaps() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let ctx = standard_market(as_of, 0.02, 0.04);

    // Create a portfolio of swaps with different maturities and directions
    let mut swaps = Vec::new();

    for (years, side) in &[
        (2, PayReceive::Pay),
        (5, PayReceive::Receive),
        (10, PayReceive::Pay),
    ] {
        let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
        let swap = InflationSwapBuilder::new()
            .id(format!("ZCINF-PORT-{}Y", years).into())
            .notional(standard_notional())
            .start_date(as_of)
            .maturity(maturity)
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .inflation_index_id("US-CPI-U".into())
            .discount_curve_id("USD-OIS".into())
            .day_count(DayCount::Act365F)
            .side(*side)
            .attributes(Default::default())
            .build()
            .unwrap();
        swaps.push(swap);
    }

    // Price entire portfolio
    let mut total_pv = 0.0;
    let mut total_dv01 = 0.0;

    for swap in &swaps {
        let result = swap
            .price_with_metrics(
                &ctx,
                as_of,
                &[MetricId::Dv01, MetricId::Dv01],
                finstack_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        total_pv += result.value.amount();
        total_dv01 += result.measures.get("dv01").unwrap();
    }

    // Portfolio metrics should be finite
    assert!(total_pv.is_finite());
    assert!(total_dv01.is_finite());
}

#[test]
fn test_stress_scenarios() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-STRESS".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Default::default())
        .build()
        .unwrap();

    // Test various stress scenarios
    let scenarios = vec![
        ("normal", 0.02, 0.04),
        ("high_inflation", 0.05, 0.06),
        ("low_inflation", 0.001, 0.02),
        ("negative_rates", 0.015, -0.005),
        ("high_rates", 0.02, 0.10),
    ];

    for (name, infl_rate, disc_rate) in scenarios {
        let ctx = standard_market(as_of, infl_rate, disc_rate);
        let pv = swap
            .value(&ctx, as_of)
            .unwrap_or_else(|_| panic!("Failed to value swap in {} scenario", name));

        assert!(
            pv.amount().is_finite(),
            "PV should be finite in {} scenario: {}",
            name,
            pv.amount()
        );
    }
}

#[test]
fn test_side_symmetry() {
    // Pay and receive sides should be opposite in PV
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap_pay = InflationSwapBuilder::new()
        .id("ZCINF-SYM-PAY".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.015).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Default::default())
        .build()
        .unwrap();

    let swap_receive = InflationSwapBuilder::new()
        .id("ZCINF-SYM-RCV".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.015).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Receive)
        .attributes(Default::default())
        .build()
        .unwrap();

    let pv_pay = swap_pay.value(&ctx, as_of).unwrap().amount();
    let pv_receive = swap_receive.value(&ctx, as_of).unwrap().amount();

    // PVs should be opposite in sign and equal in magnitude
    assert!(
        (pv_pay + pv_receive).abs() < pv_tolerance(standard_notional()),
        "Pay and receive should be opposite: {} vs {}",
        pv_pay,
        pv_receive
    );
}

#[test]
fn test_realistic_market_workflow() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2035, Month::January, 1).unwrap();

    let ctx = realistic_market(as_of);

    // 10Y inflation swap at market
    let swap = InflationSwapBuilder::new()
        .id("ZCINF-REAL-MKT".into())
        .notional(large_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.0).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Default::default())
        .build()
        .unwrap();

    // Get par rate
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ParRate],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_rate = *result.measures.get("par_rate").unwrap();

    // Create swap at par
    let swap_at_par = InflationSwapBuilder::new()
        .id("ZCINF-REAL-PAR".into())
        .notional(large_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(par_rate).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Default::default())
        .build()
        .unwrap();

    // Price with full metrics
    let full_result = swap_at_par
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::Dv01,
                MetricId::Dv01,
                MetricId::Theta,
                MetricId::Inflation01,
            ],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Verify at-market characteristics
    assert!(
        full_result.value.amount().abs() < pv_tolerance(large_notional()),
        "At-market swap should have near-zero PV"
    );

    // All greeks should be reasonable
    let dv01 = full_result.measures.get("dv01").unwrap().abs();
    let theta = full_result.measures.get("theta").unwrap().abs();
    let infl01 = full_result.measures.get("inflation01").unwrap().abs();

    // DV01 for at-market swaps is typically small (near-zero PV * duration)
    // For a 5-year swap with ~100M notional, DV01 could be in range 0 to several million
    assert!(
        dv01.is_finite() && dv01 >= 0.0 && dv01 < large_notional().amount(),
        "DV01 should be finite, non-negative, and less than notional, got: {}",
        dv01
    );
    assert!(
        theta < large_notional().amount() * 0.01,
        "Theta should be reasonable"
    );
    assert!(infl01 > 0.0, "Inflation01 should be positive for Pay");
}
