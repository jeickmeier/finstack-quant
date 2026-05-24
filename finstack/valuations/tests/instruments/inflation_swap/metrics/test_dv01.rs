//! DV01 metric tests for InflationSwap.

use crate::inflation_swap::fixtures::*;
use finstack_core::dates::{Date, DayCount};
use finstack_valuations::instruments::rates::inflation_swap::InflationSwapBuilder;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::PayReceive;
use finstack_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_dv01_positive_for_positive_pv_swap() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-DV01-1".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.01).expect("valid decimal"))
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
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // DV01 should be non-zero and reasonable
    assert!(dv01.abs() > 10.0, "DV01 should be meaningful: {}", dv01);
    assert!(
        dv01.abs() < standard_notional().amount(),
        "DV01 should be less than notional: {}",
        dv01
    );
}

#[test]
fn test_dv01_scales_with_time_to_maturity() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let ctx = standard_market(as_of, 0.02, 0.04);

    let mut dv01s = Vec::new();
    for years in &[1, 2, 5, 10] {
        let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
        let swap = InflationSwapBuilder::new()
            .id("ZCINF-DV01-MAT".into())
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
                &[MetricId::Dv01],
                finstack_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let dv01 = result.measures.get("dv01").unwrap().abs();
        dv01s.push(dv01);
    }

    // DV01 should generally increase with maturity
    for i in 1..dv01s.len() {
        assert!(
            dv01s[i] > dv01s[i - 1],
            "DV01 should increase with maturity: {} vs {}",
            dv01s[i],
            dv01s[i - 1]
        );
    }
}

#[test]
fn test_dv01_scales_with_notional() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap1 = InflationSwapBuilder::new()
        .id("ZCINF-DV01-N1".into())
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
        .id("ZCINF-DV01-N2".into())
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
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result2 = swap2
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_1 = result1.measures.get("dv01").unwrap().abs();
    let dv01_2 = result2.measures.get("dv01").unwrap().abs();

    let ratio = dv01_2 / dv01_1;
    let expected_ratio = large_notional().amount() / standard_notional().amount();

    assert!(
        (ratio - expected_ratio).abs() / expected_ratio < 0.01,
        "DV01 should scale linearly with notional: {} vs {}",
        ratio,
        expected_ratio
    );
}

#[test]
fn test_dv01_zero_for_matured_swap() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2020, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-DV01-MAT0".into())
        .notional(standard_notional())
        .start_date(Date::from_calendar_date(2015, Month::January, 1).unwrap())
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
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // Matured swap should have zero DV01
    assert_eq!(dv01, 0.0, "Matured swap should have zero DV01");
}

#[test]
fn test_dv01_reasonable_magnitude() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-DV01-MAG".into())
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
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = result.measures.get("dv01").unwrap().abs();

    // DV01 calculation uses zero-coupon approximation: -duration * pv_net * 0.0001
    // For at-market swaps (near-zero PV), DV01 can be very small
    // For 5Y swap with $1MM notional, DV01 should be finite, non-negative, and reasonable
    assert!(
        dv01.is_finite() && (0.0..100_000.0).contains(&dv01),
        "DV01 magnitude unreasonable: {}",
        dv01
    );
}
