//! Inflation convexity metric tests for InflationSwap.
//!
//! Tests the second-order sensitivity to inflation rate changes.

use crate::inflation_swap::fixtures::*;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_valuations::instruments::rates::inflation_swap::InflationSwapBuilder;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_inflation_convexity_par_swap_nonzero() {
    // Key test: Convexity should be non-zero even for par swaps (where PV = 0).
    // This validates the fix for the base_pv == 0.0 check removal.
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    // First, get the par rate
    let temp_swap = InflationSwapBuilder::new()
        .id("ZCINF-CONV-PAR-TEMP".into())
        .notional(standard_notional())
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

    let par_rate = temp_swap.par_rate(&ctx, as_of).unwrap();

    // Create a swap at par rate (should have ~zero PV)
    let par_swap = InflationSwapBuilder::new()
        .id("ZCINF-CONV-PAR".into())
        .notional(standard_notional())
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

    // Verify PV is approximately zero
    let pv = par_swap.value(&ctx, as_of).unwrap().amount();
    assert!(
        pv.abs() < pv_tolerance(standard_notional()),
        "Par swap should have near-zero PV: {}",
        pv
    );

    // Get inflation convexity
    let result = par_swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::InflationConvexity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let convexity = *result.measures.get("inflation_convexity").unwrap();

    // Key assertion: convexity should be non-zero for par swap
    // (This was previously returning 0.0 incorrectly)
    assert!(
        convexity.is_finite(),
        "Convexity should be finite: {}",
        convexity
    );
    assert!(
        convexity.abs() > 0.0,
        "Convexity should be non-zero for par swap: {}",
        convexity
    );
}

#[test]
fn test_inflation_convexity_finite_and_nonzero() {
    // Test that convexity is computed correctly (finite and non-zero).
    // Note: The sign of convexity depends on the instrument structure and market conditions.
    // For inflation swaps, convexity can be negative due to the interaction between
    // the exponential inflation growth and discounting effects.
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-CONV-CHECK".into())
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

    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::InflationConvexity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let convexity = *result.measures.get("inflation_convexity").unwrap();

    // Convexity should be finite and non-zero
    assert!(
        convexity.is_finite(),
        "Convexity should be finite: {}",
        convexity
    );
    assert!(
        convexity.abs() > 1e-10,
        "Convexity should be non-zero: {}",
        convexity
    );
}

#[test]
fn test_inflation_convexity_varies_with_maturity() {
    // Convexity varies with maturity. For inflation swaps, the behavior is complex
    // due to the interaction between inflation growth and discounting effects.
    // This test verifies that convexity changes with maturity and remains computable.
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let ctx = standard_market(as_of, 0.02, 0.04);

    let mut convexities = Vec::new();
    for years in &[2, 5, 10] {
        let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
        let swap = InflationSwapBuilder::new()
            .id("ZCINF-CONV-MAT".into())
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

        let result = swap
            .price_with_metrics(
                &ctx,
                as_of,
                &[MetricId::InflationConvexity],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let convexity = *result.measures.get("inflation_convexity").unwrap();
        convexities.push((*years, convexity));
    }

    // Verify all convexities are finite and non-zero
    for (years, conv) in &convexities {
        assert!(
            conv.is_finite(),
            "Convexity should be finite for {}Y: {}",
            years,
            conv
        );
        assert!(
            conv.abs() > 1e-10,
            "Convexity should be non-zero for {}Y: {}",
            years,
            conv
        );
    }

    // Verify convexities are not all identical (they should vary with maturity)
    let first_conv = convexities[0].1;
    let all_same = convexities
        .iter()
        .all(|(_, c)| (c - first_conv).abs() < 1e-10);
    assert!(
        !all_same,
        "Convexity should vary with maturity: {:?}",
        convexities
    );
}

#[test]
fn test_inflation_convexity_scales_with_notional() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap1 = InflationSwapBuilder::new()
        .id("ZCINF-CONV-N1".into())
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

    let swap2 = InflationSwapBuilder::new()
        .id("ZCINF-CONV-N2".into())
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

    let result1 = swap1
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::InflationConvexity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result2 = swap2
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::InflationConvexity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let conv1 = result1.measures.get("inflation_convexity").unwrap().abs();
    let conv2 = result2.measures.get("inflation_convexity").unwrap().abs();

    let ratio = conv2 / conv1;
    let expected_ratio = large_notional().amount() / standard_notional().amount();

    // Convexity should scale linearly with notional
    assert!(
        (ratio - expected_ratio).abs() / expected_ratio < 0.01,
        "Convexity should scale linearly with notional: ratio={}, expected={}",
        ratio,
        expected_ratio
    );
}

#[test]
fn test_inflation_convexity_finite_for_edge_cases() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Test with various inflation/rate combinations
    let scenarios = vec![
        ("normal", 0.02, 0.04),
        ("high_inflation", 0.05, 0.06),
        ("low_inflation", 0.005, 0.02),
        ("zero_rate", 0.02, 0.001),
    ];

    for (name, infl_rate, disc_rate) in scenarios {
        let ctx = standard_market(as_of, infl_rate, disc_rate);
        let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

        let swap = InflationSwapBuilder::new()
            .id("ZCINF-CONV-EDGE".into())
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

        let result = swap
            .price_with_metrics(
                &ctx,
                as_of,
                &[MetricId::InflationConvexity],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let convexity = *result.measures.get("inflation_convexity").unwrap();

        assert!(
            convexity.is_finite(),
            "Convexity should be finite in {} scenario: {}",
            name,
            convexity
        );
    }
}
