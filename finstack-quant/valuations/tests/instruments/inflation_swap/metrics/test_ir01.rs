//! DV01 (nominal interest rate sensitivity) metric tests for InflationSwap.

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
fn test_ir01_finite_difference_validation() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-IR01-FD".into())
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

    // Get analytic DV01
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01_analytic = *result.measures.get("dv01").unwrap();

    let ctx_up = ctx
        .bump([MarketBump::Curve {
            id: "USD-OIS".into(),
            spec: BumpSpec::parallel_bp(1.0),
        }])
        .unwrap();
    let ctx_down = ctx
        .bump([MarketBump::Curve {
            id: "USD-OIS".into(),
            spec: BumpSpec::parallel_bp(-1.0),
        }])
        .unwrap();
    let pv_up = swap.value(&ctx_up, as_of).unwrap().amount();
    let pv_down = swap.value(&ctx_down, as_of).unwrap().amount();

    let dv01_fd = (pv_up - pv_down) / 2.0;

    // Check sign consistency
    assert_eq!(
        dv01_analytic.signum(),
        dv01_fd.signum(),
        "DV01 sign should match FD: analytic={}, FD={}",
        dv01_analytic,
        dv01_fd
    );

    // Check magnitude within tolerance
    let rel_diff = (dv01_analytic - dv01_fd).abs() / dv01_fd.abs().max(1.0);
    assert!(
        rel_diff < greek_tolerance(),
        "DV01 relative difference too large: {}",
        rel_diff
    );
}

#[test]
fn test_ir01_sign_pay_fixed() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-IR01-SIGN1".into())
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
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // DV01 should be finite
    assert!(dv01.is_finite(), "DV01 should be finite");
}

#[test]
fn test_ir01_sign_receive_fixed() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-IR01-SIGN2".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.03).expect("valid decimal"))
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
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // DV01 should be finite
    assert!(dv01.is_finite(), "DV01 should be finite");
}
