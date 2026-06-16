//! FRA DV01 metric tests.
//!
//! DV01 (Dollar Value of 01 basis point) measures the sensitivity
//! of FRA value to a 1bp parallel shift in both discount and forward curves.
//!
//! Note: Uses GenericParallelDv01 which bumps all referenced curves.

use crate::fra::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_dv01_standard_fra() {
    let market = standard_market();
    let fra = create_standard_fra();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    assert_finite(dv01, "DV01 should be finite");
    // 3M FRA on $1MM should have reasonable DV01 (10-500 range)
    assert_in_range(
        dv01.abs(),
        10.0,
        500.0,
        "DV01 should be in reasonable range",
    );
}

#[test]
fn test_dv01_scales_with_notional() {
    let market = standard_market();

    let fra_1m = TestFraBuilder::new()
        .notional(1_000_000.0, Currency::USD)
        .build();
    let fra_10m = TestFraBuilder::new()
        .notional(10_000_000.0, Currency::USD)
        .build();

    let result_1m = fra_1m
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01_1m = *result_1m.measures.get("dv01").unwrap();

    let result_10m = fra_10m
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01_10m = *result_10m.measures.get("dv01").unwrap();

    assert_approx_equal(
        dv01_10m,
        dv01_1m * 10.0,
        1.0,
        "DV01 should scale linearly with notional",
    );
}

#[test]
fn test_dv01_scales_with_tenor() {
    let market = standard_market();

    // 1M FRA
    let start_1m = date!(2024 - 04 - 01);
    let end_1m = date!(2024 - 05 - 01);
    let fra_1m = TestFraBuilder::new()
        .dates(start_1m, start_1m, end_1m)
        .build();

    // 6M FRA
    let start_6m = date!(2024 - 04 - 01);
    let end_6m = date!(2024 - 10 - 01);
    let fra_6m = TestFraBuilder::new()
        .dates(start_6m, start_6m, end_6m)
        .build();

    let dv01_1m = fra_1m
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap()
        .abs();

    let dv01_6m = fra_6m
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap()
        .abs();

    assert!(
        dv01_6m > dv01_1m,
        "Longer tenor should have higher DV01: 6M={}, 1M={}",
        dv01_6m,
        dv01_1m
    );
}

#[test]
fn test_dv01_receive_fixed_negative() {
    let market = standard_market();
    let fra = TestFraBuilder::new().receive_fixed(true).build(); // true = receive fixed

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // Receive fixed: when rates rise, you lose value (negative DV01)
    assert_negative(dv01, "Receive-fixed FRA should have negative DV01");
}

#[test]
fn test_dv01_pay_fixed_positive() {
    let market = standard_market();
    let fra = TestFraBuilder::new().receive_fixed(false).build(); // false = pay fixed

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // Pay fixed: when rates rise, you gain value (positive DV01)
    assert_positive(dv01, "Pay-fixed FRA should have positive DV01");
}

#[test]
fn test_fra_pv01_matches_forward_pv01_br01() {
    let market = standard_market();
    let fra = create_standard_fra();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Pv01, MetricId::ForwardPv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv01 = *result.measures.get("pv01").unwrap();
    let forward_pv01 = *result.measures.get("forward_pv01").unwrap();

    assert_negative(pv01, "Receive-fixed FRA forward PV01 should be negative");
    assert_approx_equal(
        pv01,
        forward_pv01,
        1e-10,
        "FRA pv01 should alias forward_pv01",
    );
}

#[test]
fn test_fra_dv01_is_not_pv01_for_off_market_fra() {
    let market = standard_market();
    let fra = TestFraBuilder::new().fixed_rate(0.045).build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01, MetricId::Pv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();
    let pv01 = *result.measures.get("pv01").unwrap();

    assert!(
        (dv01 - pv01).abs() > 1e-6,
        "FRA dv01 should include total rate-curve risk, not just forward BR01"
    );
}

#[test]
fn test_dv01_short_period() {
    let market = standard_market();

    let start = date!(2024 - 02 - 01);
    let end = date!(2024 - 03 - 01);
    let fra = TestFraBuilder::new().dates(start, start, end).build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    assert_finite(dv01, "Short period DV01 should be finite");
    // For short-period FRA (1 month), DV01 is smaller than for longer-period FRA
    // But still significant: for 1M notional, typically $2-$10
    assert!(
        dv01.abs() < 10.0,
        "Short FRA DV01 should be smaller than long-period: got {}",
        dv01.abs()
    );
}

#[test]
fn test_dv01_long_period() {
    let market = standard_market();

    let start = date!(2024 - 07 - 01);
    let end = date!(2025 - 01 - 01); // 6M
    let fra = TestFraBuilder::new().dates(start, start, end).build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    assert_finite(dv01, "Long period DV01 should be finite");
}

#[test]
fn test_dv01_zero_tau() {
    let market = standard_market();
    let same_date = date!(2024 - 04 - 01);

    let fra = TestFraBuilder::new()
        .dates(same_date, same_date, same_date)
        .build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    assert_eq!(dv01, 0.0, "Zero tau should produce zero DV01");
}

#[test]
fn test_dv01_different_day_counts() {
    let market = standard_market();

    let fra_360 = TestFraBuilder::new().day_count(DayCount::Act360).build();
    let fra_365 = TestFraBuilder::new().day_count(DayCount::Act365F).build();

    let dv01_360 = fra_360
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap()
        .abs();

    let dv01_365 = fra_365
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap()
        .abs();

    // ACT/360 should have slightly higher DV01 than ACT/365 (larger tau)
    assert!(
        dv01_360 > dv01_365,
        "ACT/360 should have higher DV01 than ACT/365"
    );
}

#[test]
fn test_pv01_alias() {
    let market = standard_market();
    let fra = create_standard_fra();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Pv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv01 = *result.measures.get("pv01").unwrap();

    assert_finite(pv01, "PV01 (DV01 alias) should work");
}
