//! Par rate metric tests for InflationSwap.

use crate::inflation_swap::fixtures::*;
use finstack_core::dates::{Date, DayCount};
use finstack_valuations::instruments::rates::inflation_swap::InflationSwapBuilder;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::PayReceive;
use finstack_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_par_rate_gives_zero_pv() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-PR-1".into())
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
            &[MetricId::ParRate],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_rate = *result.measures.get("par_rate").unwrap();

    // Use par rate to create new swap
    let par_swap = InflationSwapBuilder::new()
        .id("ZCINF-PR-2".into())
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

    let pv = par_swap.value(&ctx, as_of).unwrap().amount();

    assert!(
        pv.abs() < pv_tolerance(standard_notional()),
        "Par rate should give zero PV: pv={}, par_rate={}",
        pv,
        par_rate
    );
}

#[test]
fn test_par_rate_increases_with_inflation() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx_low = standard_market(as_of, 0.015, 0.04);
    let ctx_high = standard_market(as_of, 0.025, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-PR-INFL".into())
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
            &[MetricId::ParRate],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_high = swap
        .price_with_metrics(
            &ctx_high,
            as_of,
            &[MetricId::ParRate],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_low = *result_low.measures.get("par_rate").unwrap();
    let par_high = *result_high.measures.get("par_rate").unwrap();

    assert!(
        par_high > par_low,
        "Par rate should increase with inflation: {} vs {}",
        par_high,
        par_low
    );
}

#[test]
fn test_par_rate_reasonable_range() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-PR-RANGE".into())
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
            &[MetricId::ParRate],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_rate = *result.measures.get("par_rate").unwrap();

    // For reasonable market, par rate should be positive and < 10%
    assert!(
        par_rate > 0.0 && par_rate < 0.10,
        "Par rate should be reasonable: {}",
        par_rate
    );
}

#[test]
fn test_par_rate_independent_of_side() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap_pay = InflationSwapBuilder::new()
        .id("ZCINF-PR-PAY".into())
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

    let swap_receive = InflationSwapBuilder::new()
        .id("ZCINF-PR-RCV".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.0).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Receive)
        .attributes(Default::default())
        .build()
        .unwrap();

    let result_pay = swap_pay
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ParRate],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_receive = swap_receive
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ParRate],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_pay = *result_pay.measures.get("par_rate").unwrap();
    let par_receive = *result_receive.measures.get("par_rate").unwrap();

    assert!(
        (par_pay - par_receive).abs() < rate_tolerance(),
        "Par rate should be independent of side: {} vs {}",
        par_pay,
        par_receive
    );
}
