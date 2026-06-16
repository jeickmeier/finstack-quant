//! Breakeven inflation metric tests for InflationSwap.

use crate::inflation_swap::fixtures::*;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_valuations::instruments::rates::inflation_swap::InflationSwapBuilder;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_breakeven_equals_par_rate() {
    // Breakeven and par_rate should be equivalent for zero-coupon inflation swaps
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-BE-1".into())
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

    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::custom("breakeven"), MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let breakeven = *result.measures.get("breakeven").unwrap();
    let par_rate = *result.measures.get("par_rate").unwrap();

    assert!(
        (breakeven - par_rate).abs() < rate_tolerance(),
        "Breakeven should equal par rate: {} vs {}",
        breakeven,
        par_rate
    );
}

#[test]
fn test_breakeven_gives_zero_pv() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-BE-2".into())
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

    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::custom("breakeven")],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let breakeven = *result.measures.get("breakeven").unwrap();

    // Create swap at breakeven
    let be_swap = InflationSwapBuilder::new()
        .id("ZCINF-BE-3".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(breakeven).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Default::default())
        .build()
        .unwrap();

    let pv = be_swap.value(&ctx, as_of).unwrap().amount();

    assert!(
        pv.abs() < pv_tolerance(standard_notional()),
        "Breakeven should give zero PV: pv={}, breakeven={}",
        pv,
        breakeven
    );
}

#[test]
fn test_breakeven_increases_with_inflation() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx_low = standard_market(as_of, 0.01, 0.04);
    let ctx_high = standard_market(as_of, 0.03, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-BE-INFL".into())
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

    let result_low = swap
        .price_with_metrics(
            &ctx_low,
            as_of,
            &[MetricId::custom("breakeven")],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_high = swap
        .price_with_metrics(
            &ctx_high,
            as_of,
            &[MetricId::custom("breakeven")],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let be_low = *result_low.measures.get("breakeven").unwrap();
    let be_high = *result_high.measures.get("breakeven").unwrap();

    assert!(
        be_high > be_low,
        "Breakeven should increase with inflation expectations: {} vs {}",
        be_high,
        be_low
    );
}
