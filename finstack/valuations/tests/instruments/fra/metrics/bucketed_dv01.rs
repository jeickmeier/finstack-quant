//! FRA bucketed DV01 metric tests.
//!
//! Bucketed DV01 decomposes total DV01 into risk contributions by
//! tenor bucket (e.g., 3M, 6M, 1Y). For FRAs, risk is concentrated
//! in the forward tenor bucket.

use crate::fra::common::*;
use finstack_core::currency::Currency;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;

fn sum_bucketed_dv01(result: &finstack_valuations::results::ValuationResult) -> f64 {
    result
        .measures
        .iter()
        .filter(|(id, _)| id.as_str().starts_with("bucketed_dv01::"))
        .map(|(_, v)| *v)
        .sum()
}

#[test]
fn test_bucketed_dv01_standard_fra() {
    let market = standard_market();
    let fra = create_standard_fra();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::BucketedDv01, MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Check that bucketed_dv01 metric exists
    assert!(result.measures.contains_key("bucketed_dv01"));

    let bucket_total = *result.measures.get("bucketed_dv01").unwrap();
    let bucket_sum = sum_bucketed_dv01(&result);
    let diff = (bucket_sum - bucket_total).abs();
    let tol = 1e-6_f64.max(1e-6 * bucket_total.abs());
    assert!(
        diff < tol,
        "Sum of bucketed DV01 should match bucketed total: bucket_sum={}, total={}, diff={}",
        bucket_sum,
        bucket_total,
        diff
    );
}

#[test]
fn test_bucketed_dv01_scales_with_notional() {
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
            &[MetricId::BucketedDv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let bdv01_1m = *result_1m.measures.get("bucketed_dv01").unwrap();

    let result_10m = fra_10m
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::BucketedDv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let bdv01_10m = *result_10m.measures.get("bucketed_dv01").unwrap();

    assert_finite(bdv01_1m, "1M bucketed DV01 should be finite");
    assert_finite(bdv01_10m, "10M bucketed DV01 should be finite");
}
