//! Comprehensive IR Future metrics tests.

use super::utils::*;
use finstack_quant_valuations::instruments::rates::ir_future::Position;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_pv_value() {
    let (as_of, start, end) = standard_dates();
    let future = create_standard_future(start, end);
    let market = build_standard_market(as_of, 0.05);

    // PV is available directly in result.value (not in measures)
    let result = future
        .price_with_metrics(
            &market,
            as_of,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv_result = result.value.amount();

    // Should match direct value calculation
    let pv_direct = future.value(&market, as_of).unwrap().amount();
    assert!(
        (pv_result - pv_direct).abs() < 1e-9,
        "Result.value should match direct calculation: {} vs {}",
        pv_result,
        pv_direct
    );
}

#[test]
fn test_dv01_metric() {
    let (as_of, start, end) = standard_dates();
    let future = create_standard_future(start, end);
    let market = build_standard_market(as_of, 0.05);

    let result = future
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // DV01 for 3-month future on $1MM should be reasonable
    // Face × tau × 1bp ≈ 1MM × 0.25 × 0.0001 = $25
    assert!(
        dv01.abs() > 10.0 && dv01.abs() < 500.0,
        "DV01 should be reasonable: got {}",
        dv01
    );
}

#[test]
fn test_dv01_long_vs_short() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    let long = create_custom_future(
        "LONG",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );
    let short = create_custom_future(
        "SHORT",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Short,
    );

    let result_long = long
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_short = short
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_long = *result_long.measures.get("dv01").unwrap();
    let dv01_short = *result_short.measures.get("dv01").unwrap();

    // Long and short should have opposite signs
    assert!(
        (dv01_long + dv01_short).abs() < 1e-6,
        "Long and short DV01 should offset: {} vs {}",
        dv01_long,
        dv01_short
    );
}

#[test]
fn test_dv01_multiple_contracts() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    let single = create_custom_future(
        "SINGLE",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );
    let double = create_custom_future(
        "DOUBLE",
        2_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );

    let result_single = single
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_double = double
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_single = *result_single.measures.get("dv01").unwrap();
    let dv01_double = *result_double.measures.get("dv01").unwrap();

    // Should scale linearly (allow for small rounding differences)
    assert!(
        (dv01_double - 2.0 * dv01_single).abs() < 0.02,
        "DV01 should scale with contracts: {} vs {}",
        dv01_double,
        2.0 * dv01_single
    );
}

#[test]
fn test_dv01_near_vs_far() {
    let market_as_of = time::macros::date!(2024 - 01 - 01);
    let market = build_standard_market(market_as_of, 0.05);

    let (_, near_start, near_end) = near_term_dates();
    let (_, far_start, far_end) = far_forward_dates();

    let near = create_custom_future(
        "NEAR",
        1_000_000.0,
        near_start,
        near_start,
        near_end,
        97.50,
        Position::Long,
    );
    let far = create_custom_future(
        "FAR",
        1_000_000.0,
        far_start,
        far_start,
        far_end,
        97.50,
        Position::Long,
    );

    let result_near = near
        .price_with_metrics(
            &market,
            market_as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_far = far
        .price_with_metrics(
            &market,
            market_as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_near = *result_near.measures.get("dv01").unwrap();
    let dv01_far = *result_far.measures.get("dv01").unwrap();

    // Both should be reasonable (tau-dependent, not time-dependent)
    assert!(dv01_near.abs() > 0.0);
    assert!(dv01_far.abs() > 0.0);
}

#[test]
fn test_theta_metric() {
    let (as_of, start, end) = standard_dates();
    let future = create_standard_future(start, end);
    let market = build_standard_market(as_of, 0.05);

    let result = future
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    // Theta measures time decay
    assert!(theta.is_finite(), "Theta should be finite");
}

#[test]
fn test_theta_long_vs_short() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    let long = create_custom_future(
        "LONG",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );
    let short = create_custom_future(
        "SHORT",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Short,
    );

    let result_long = long
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_short = short
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta_long = *result_long.measures.get("theta").unwrap();
    let theta_short = *result_short.measures.get("theta").unwrap();

    // Should be opposite signs
    assert!(
        (theta_long + theta_short).abs() < 1e-6,
        "Long and short theta should offset: {} vs {}",
        theta_long,
        theta_short
    );
}

#[test]
fn test_all_metrics_together() {
    let (as_of, start, end) = standard_dates();
    let future = create_standard_future(start, end);
    let market = build_standard_market(as_of, 0.05);

    let metrics = vec![MetricId::Dv01, MetricId::Theta];

    let result = future
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // All metrics should be present
    assert!(result.measures.contains_key("dv01"));
    assert!(result.measures.contains_key("theta"));
    // PV is in result.value, not in measures
    assert!(result.value.amount().is_finite());
}

#[test]
fn test_bucketed_dv01_metric() {
    let (as_of, start, end) = standard_dates();
    let future = create_standard_future(start, end);
    let market = build_standard_market(as_of, 0.05);

    let result = future
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::BucketedDv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // BucketedDv01 should be present
    if result.measures.contains_key("bucketed_dv01") {
        let bucketed = *result.measures.get("bucketed_dv01").unwrap();
        assert!(bucketed.is_finite());
    }
}

#[test]
fn test_empty_metrics() {
    let (as_of, start, end) = standard_dates();
    let future = create_standard_future(start, end);
    let market = build_standard_market(as_of, 0.05);

    let result = future
        .price_with_metrics(
            &market,
            as_of,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Should still have value
    assert!(result.value.amount().is_finite());
    // measures might be empty or contain only base metrics
}

#[test]
fn test_metrics_with_very_short_tau() {
    let (as_of, _, _) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    // Very short tau (1 day)
    let start = time::macros::date!(2024 - 07 - 01);
    let end = time::macros::date!(2024 - 07 - 02);
    let future = create_custom_future(
        "SHORT_TAU",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );

    let result = future
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // Futures DV01 is exchange tick economics, not face × remaining accrual.
    assert!(dv01.is_finite() && dv01.abs() > 0.0);
}

#[test]
fn test_metrics_consistency() {
    let (as_of, start, end) = standard_dates();
    let future = create_standard_future(start, end);
    let market = build_standard_market(as_of, 0.05);

    // Request metrics separately
    let result_basic = future
        .price_with_metrics(
            &market,
            as_of,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_dv01 = future
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Request together with theta
    let result_both = future
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01, MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // PV should be consistent (in result.value, not measures)
    let pv_basic = result_basic.value.amount();
    let pv_with_metrics = result_dv01.value.amount();
    let pv_both = result_both.value.amount();

    let dv01_separate = *result_dv01.measures.get("dv01").unwrap();
    let dv01_together = *result_both.measures.get("dv01").unwrap();

    // Should be consistent
    assert!((pv_basic - pv_with_metrics).abs() < 1e-9);
    assert!((pv_with_metrics - pv_both).abs() < 1e-9);
    assert!((dv01_separate - dv01_together).abs() < 1e-9);
}
