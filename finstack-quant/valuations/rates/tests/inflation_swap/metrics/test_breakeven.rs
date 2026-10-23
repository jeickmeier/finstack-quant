//! Par inflation rate and configured carry breakeven tests for InflationSwap.

use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::{
    BreakevenConfig, BreakevenMode, BreakevenTarget, PayReceive, PricingOptions,
};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_rates::inflation_swap::InflationSwap;
use finstack_quant_valuations_test_support::inflation::*;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_breakeven_requires_configuration_and_measures_carry_shift() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let mut swap = InflationSwap::builder()
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

    let error = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Breakeven],
            PricingOptions::default(),
        )
        .expect_err("unconfigured carry breakeven must not return the par inflation rate");
    assert!(error.to_string().contains("breakeven_config"), "{error}");

    swap.metric_pricing_overrides =
        MetricPricingOverrides::default().with_breakeven_config(BreakevenConfig {
            target: BreakevenTarget::Ytm,
            mode: BreakevenMode::Linear,
        });
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::Breakeven,
                MetricId::ParRate,
                MetricId::CarryTotal,
                MetricId::Dv01,
            ],
            PricingOptions::default(),
        )
        .expect("configured carry breakeven");
    let breakeven = result.measures["breakeven"];
    let carry = result.measures["carry_total"];
    let dv01 = result.measures["dv01"];
    assert!(dv01.abs() > 1e-12);
    assert!(breakeven.is_finite());
    assert!((breakeven - (-carry / dv01)).abs() < 1e-10);
    assert!(result.measures["par_rate"].is_finite());
}

#[test]
fn test_par_rate_gives_zero_pv() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwap::builder()
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
            &[MetricId::ParRate],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_rate = *result.measures.get("par_rate").unwrap();

    // Create swap at par_rate
    let be_swap = InflationSwap::builder()
        .id("ZCINF-BE-3".into())
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

    let pv = be_swap.value(&ctx, as_of).unwrap().amount();

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

    let ctx_low = standard_market(as_of, 0.01, 0.04);
    let ctx_high = standard_market(as_of, 0.03, 0.04);

    let swap = InflationSwap::builder()
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
            &[MetricId::ParRate],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_high = swap
        .price_with_metrics(
            &ctx_high,
            as_of,
            &[MetricId::ParRate],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let be_low = *result_low.measures.get("par_rate").unwrap();
    let be_high = *result_high.measures.get("par_rate").unwrap();

    assert!(
        be_high > be_low,
        "Par rate should increase with inflation expectations: {} vs {}",
        be_high,
        be_low
    );
}
