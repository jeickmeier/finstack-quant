//! Fixed and inflation leg PV metric tests.

use crate::inflation_swap::fixtures::*;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_valuations::instruments::rates::inflation_swap::InflationSwapBuilder;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_fixed_leg_pv_metric() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-FLG-1".into())
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
            &[MetricId::custom("fixed_leg_pv")],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let fixed_leg_pv = *result.measures.get("fixed_leg_pv").unwrap();

    // Fixed leg PV should be positive and reasonable
    assert!(fixed_leg_pv > 0.0, "Fixed leg PV should be positive");
    assert!(
        fixed_leg_pv < standard_notional().amount() * 0.5,
        "Fixed leg PV should be reasonable"
    );
}

#[test]
fn test_inflation_leg_pv_metric() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-ILG-1".into())
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
            &[MetricId::custom("inflation_leg_pv")],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let inflation_leg_pv = *result.measures.get("inflation_leg_pv").unwrap();

    // Inflation leg PV should be positive and reasonable
    assert!(
        inflation_leg_pv > 0.0,
        "Inflation leg PV should be positive"
    );
    assert!(
        inflation_leg_pv < standard_notional().amount() * 0.5,
        "Inflation leg PV should be reasonable"
    );
}

#[test]
fn test_leg_pvs_sum_to_npv() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-LEGS-SUM".into())
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

    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::custom("fixed_leg_pv"),
                MetricId::custom("inflation_leg_pv"),
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let fixed_leg_pv = *result.measures.get("fixed_leg_pv").unwrap();
    let inflation_leg_pv = *result.measures.get("inflation_leg_pv").unwrap();
    let npv = result.value.amount();

    // Pay: NPV = inflation_leg - fixed_leg
    let expected_npv = inflation_leg_pv - fixed_leg_pv;

    assert!(
        (npv - expected_npv).abs() < 1e-6,
        "Leg PVs should sum to NPV: {} vs {}",
        npv,
        expected_npv
    );
}

#[test]
fn test_leg_pvs_scale_with_notional() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap1 = InflationSwapBuilder::new()
        .id("ZCINF-LEGS-N1".into())
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
        .id("ZCINF-LEGS-N2".into())
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
            &[
                MetricId::custom("fixed_leg_pv"),
                MetricId::custom("inflation_leg_pv"),
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let result2 = swap2
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::custom("fixed_leg_pv"),
                MetricId::custom("inflation_leg_pv"),
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let fixed1 = *result1.measures.get("fixed_leg_pv").unwrap();
    let fixed2 = *result2.measures.get("fixed_leg_pv").unwrap();
    let infl1 = *result1.measures.get("inflation_leg_pv").unwrap();
    let infl2 = *result2.measures.get("inflation_leg_pv").unwrap();

    let expected_ratio = large_notional().amount() / standard_notional().amount();

    let fixed_ratio = fixed2 / fixed1;
    let infl_ratio = infl2 / infl1;

    assert!(
        (fixed_ratio - expected_ratio).abs() / expected_ratio < 0.01,
        "Fixed leg should scale with notional"
    );
    assert!(
        (infl_ratio - expected_ratio).abs() / expected_ratio < 0.01,
        "Inflation leg should scale with notional"
    );
}
