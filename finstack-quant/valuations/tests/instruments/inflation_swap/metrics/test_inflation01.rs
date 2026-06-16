//! Inflation01 (inflation rate sensitivity) metric tests for InflationSwap.

use crate::inflation_swap::fixtures::*;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use rust_decimal::Decimal;

use finstack_quant_valuations::instruments::rates::inflation_swap::InflationSwapBuilder;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::metrics::MetricId;
use time::Month;

#[test]
fn test_inflation01_finite_difference_validation() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-INFL01-FD".into())
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

    // Get analytic inflation01
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Inflation01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let infl01_analytic = *result.measures.get("inflation01").unwrap();

    // Compute finite difference inflation01
    let pv0 = swap.value(&ctx, as_of).unwrap().amount();

    let ctx_bumped = ctx
        .bump([MarketBump::Curve {
            id: "US-CPI-U".into(),
            spec: BumpSpec::parallel_bp(1.0),
        }])
        .unwrap();
    let pv1 = swap.value(&ctx_bumped, as_of).unwrap().amount();

    let infl01_fd = pv1 - pv0;

    // Check sign consistency
    assert_eq!(
        infl01_analytic.signum(),
        infl01_fd.signum(),
        "Inflation01 sign should match FD: analytic={}, FD={}",
        infl01_analytic,
        infl01_fd
    );
}

#[test]
fn test_inflation01_sign_pay_fixed() {
    // Pay: receive inflation, pay fixed
    // Higher inflation => more positive PV => positive inflation01
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-INFL01-SIGN1".into())
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
            &[MetricId::Inflation01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let infl01 = *result.measures.get("inflation01").unwrap();

    // Pay should benefit from higher inflation
    assert!(
        infl01 > 0.0,
        "Pay inflation01 should be positive: {}",
        infl01
    );
}

#[test]
fn test_inflation01_sign_receive_fixed() {
    // Receive: pay inflation, receive fixed
    // Higher inflation => more negative PV => negative inflation01
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-INFL01-SIGN2".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Receive)
        .attributes(Default::default())
        .build()
        .unwrap();

    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Inflation01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let infl01 = *result.measures.get("inflation01").unwrap();

    // Receive should lose from higher inflation
    assert!(
        infl01 < 0.0,
        "Receive inflation01 should be negative: {}",
        infl01
    );
}

#[test]
fn test_inflation01_scales_with_maturity() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let ctx = standard_market(as_of, 0.02, 0.04);

    let mut infl01s = Vec::new();
    for years in &[1, 2, 5, 10] {
        let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
        let swap = InflationSwapBuilder::new()
            .id("ZCINF-INFL01-MAT".into())
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
                &[MetricId::Inflation01],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let infl01 = result.measures.get("inflation01").unwrap().abs();
        infl01s.push(infl01);
    }

    // Inflation01 should increase with maturity
    for i in 1..infl01s.len() {
        assert!(
            infl01s[i] > infl01s[i - 1],
            "Inflation01 should increase with maturity"
        );
    }
}

#[test]
fn test_inflation01_scales_with_notional() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap1 = InflationSwapBuilder::new()
        .id("ZCINF-INFL01-N1".into())
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
        .id("ZCINF-INFL01-N2".into())
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
            &[MetricId::Inflation01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result2 = swap2
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Inflation01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let infl01_1 = result1.measures.get("inflation01").unwrap().abs();
    let infl01_2 = result2.measures.get("inflation01").unwrap().abs();

    let ratio = infl01_2 / infl01_1;
    let expected_ratio = large_notional().amount() / standard_notional().amount();

    assert!(
        (ratio - expected_ratio).abs() / expected_ratio < 0.01,
        "Inflation01 should scale linearly with notional"
    );
}
