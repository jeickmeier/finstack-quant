//! CDS Index risk metrics tests.
//!
//! Tests cover:
//! - DV01 (interest rate sensitivity)
//! - CS01 (credit spread sensitivity)
//! - Risky PV01 (premium spread sensitivity)
//! - Hazard CS01 (hazard rate sensitivity)
//! - Bucketed DV01 (term structure sensitivity)
//! - Risk metric scaling with notional
//! - Risk metric sign conventions

use super::test_utils::*;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_risky_pv01_positive() {
    // Test: Risky PV01 should be positive
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-RPV01", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::RiskyPv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let rpv01 = *result.measures.get("risky_pv01").unwrap();

    assert_positive(rpv01, "Risky PV01");
    assert_in_range(rpv01, 3_500.0, 5_500.0, "Risky PV01 for $10MM, 5Y");
}

#[test]
fn test_cs01_positive() {
    // Test: CS01 should be positive
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-CS01", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let cs01 = *result.measures.get("cs01").unwrap();

    assert_positive(cs01, "CS01");
}

#[test]
fn test_dv01_calculation() {
    // Test: DV01 (interest rate sensitivity) calculation
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-DV01", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    // DV01 = PV(rate+1bp) - PV(base); sign depends on instrument structure
    assert!(dv01.is_finite(), "DV01 should be finite");
}

#[test]
fn test_hazard_cs01_calculation() {
    // Test: Hazard CS01 (parallel hazard bump sensitivity)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-HCS01", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // CS01 should be present
    let cs01 = result.measures.get("cs01").expect("CS01 should be present");
    assert!(cs01.is_finite(), "CS01 should be finite");
}

#[test]
fn test_dv01_scales_with_notional() {
    // Test: DV01 scales linearly with notional
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx_10mm = standard_single_curve_index("CDX-10MM", start, end, 10_000_000.0);
    let idx_20mm = standard_single_curve_index("CDX-20MM", start, end, 20_000_000.0);

    let result_10mm = idx_10mm
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_20mm = idx_20mm
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_10mm = *result_10mm.measures.get("dv01").unwrap();
    let dv01_20mm = *result_20mm.measures.get("dv01").unwrap();

    assert_linear_scaling(
        dv01_10mm,
        10_000_000.0,
        dv01_20mm,
        20_000_000.0,
        "DV01",
        0.01,
    );
}

#[test]
fn test_cs01_increases_with_maturity() {
    // Test: CS01 increases with longer maturity
    let start = date!(2025 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx_3y = standard_single_curve_index("CDX-3Y", start, date!(2028 - 01 - 01), 10_000_000.0);
    let idx_5y = standard_single_curve_index("CDX-5Y", start, date!(2030 - 01 - 01), 10_000_000.0);

    let result_3y = idx_3y
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_5y = idx_5y
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let cs01_3y = *result_3y.measures.get("cs01").unwrap();
    let cs01_5y = *result_5y.measures.get("cs01").unwrap();

    assert!(
        cs01_3y < cs01_5y,
        "CS01 should increase with maturity: 3Y={}, 5Y={}",
        cs01_3y,
        cs01_5y
    );
}

#[test]
fn test_cs01_matches_direct_method() {
    // Test: CS01 via metrics matches direct method
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-CS01", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let direct_cs01 = idx.cs01(&ctx, as_of).unwrap();

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let metric_cs01 = *result.measures.get("cs01").unwrap();

    assert_relative_eq(direct_cs01, metric_cs01, 0.001, "CS01: direct vs metric");
}

#[test]
fn test_risky_pv01_single_vs_constituents() {
    // Test: Risky PV01 consistency across pricing modes
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = multi_constituent_market_context(as_of, 5);

    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);
    let idx_const = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    let result_single = idx_single
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::RiskyPv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_const = idx_const
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::RiskyPv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let rpv01_single = *result_single.measures.get("risky_pv01").unwrap();
    let rpv01_const = *result_const.measures.get("risky_pv01").unwrap();

    assert_relative_eq(rpv01_single, rpv01_const, 0.05, "Risky PV01 parity");
}

#[test]
fn test_cs01_single_vs_constituents() {
    // Test: CS01 consistency across pricing modes
    //
    // Both modes use identical hazard rates (0.015) and recovery (40%).
    // CS01 is computed by bumping hazard curves by 1bp and repricing.
    // - Single-curve: bumps HZ-INDEX
    // - Constituents: bumps each HZ1..HZ5 independently and sums
    //
    // With identical curves, both should produce similar results.
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = multi_constituent_market_context(as_of, 5);

    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);
    let idx_const = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    let result_single = idx_single
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_const = idx_const
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let cs01_single = *result_single.measures.get("cs01").unwrap();
    let cs01_const = *result_const.measures.get("cs01").unwrap();

    // 5% tolerance: aggregation of per-constituent CS01 vs single curve
    assert_relative_eq(cs01_single, cs01_const, 0.05, "CS01 parity");
}

#[test]
fn test_bucketed_cs01_reconciles_to_parallel_single_curve() {
    // Key-rate (bucketed) par-spread CS01 must reconcile to the parallel `Cs01`.
    // The bucketed calculator applies the par-spread shock one standard tenor at
    // a time; each curve par point is bumped by exactly one bucket, so the
    // per-tenor series sums to the parallel CS01.
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-BKT-SC", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let cs01 = *result.measures.get("cs01").expect("cs01 present");
    let bucketed = *result
        .measures
        .get("bucketed_cs01")
        .expect("bucketed_cs01 present");
    assert!(
        cs01.is_finite() && bucketed.is_finite(),
        "CS01 metrics must be finite (cs01={cs01}, bucketed={bucketed})"
    );
    assert_positive(bucketed, "BucketedCs01");
    assert_relative_eq(bucketed, cs01, 0.02, "BucketedCs01 total vs parallel Cs01");

    let series_sum: f64 = result
        .measures
        .iter()
        .filter(|(k, _)| k.as_str().starts_with("bucketed_cs01::"))
        .map(|(_, v)| *v)
        .sum();
    assert_relative_eq(series_sum, cs01, 0.02, "per-tenor series vs parallel Cs01");
}

#[test]
fn test_bucketed_cs01_reconciles_to_parallel_constituents() {
    // Same reconciliation in `Constituents` mode: the bucketed calculator bumps
    // every constituent curve at each tenor and reprices the index end-to-end.
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = multi_constituent_market_context(as_of, 5);

    let idx = standard_constituents_index("CDX-BKT-CONST", start, end, 10_000_000.0, 5);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let cs01 = *result.measures.get("cs01").expect("cs01 present");
    let bucketed = *result
        .measures
        .get("bucketed_cs01")
        .expect("bucketed_cs01 present");
    assert!(
        cs01.is_finite() && bucketed.is_finite(),
        "CS01 metrics must be finite (cs01={cs01}, bucketed={bucketed})"
    );
    assert_relative_eq(
        bucketed,
        cs01,
        0.02,
        "BucketedCs01 total vs parallel Cs01 (constituents)",
    );

    let series_sum: f64 = result
        .measures
        .iter()
        .filter(|(k, _)| k.as_str().starts_with("bucketed_cs01::"))
        .map(|(_, v)| *v)
        .sum();
    assert_relative_eq(
        series_sum,
        cs01,
        0.02,
        "per-tenor series vs parallel Cs01 (constituents)",
    );
}

#[test]
fn test_all_risk_metrics_together() {
    // Test: All risk metrics computed together
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-ALL-RISK", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let metrics = vec![MetricId::RiskyPv01, MetricId::Cs01, MetricId::Dv01];

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &metrics,
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert!(result.measures.contains_key("risky_pv01"));
    assert!(result.measures.contains_key("cs01"));
    assert!(result.measures.contains_key("dv01"));
}

#[test]
fn test_dv01_reasonable_magnitude() {
    // Test: DV01 has reasonable magnitude
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-DV01", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    // DV01 computed via bump-and-reprice; magnitude should be meaningful but not a simple closed-form
    assert!(dv01.is_finite(), "DV01 should be finite");
    // DV01 can be small for credit instruments where protection leg dominates premium leg
    assert!(
        dv01.abs() > 1.0,
        "DV01 magnitude should be non-trivial for $10MM notional"
    );
}

#[test]
fn test_risk_metrics_finite() {
    // Test: All risk metrics are finite
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-FINITE", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let metrics = vec![MetricId::RiskyPv01, MetricId::Cs01, MetricId::Dv01];

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &metrics,
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    for (name, value) in &result.measures {
        assert!(
            value.is_finite(),
            "Risk metric '{}' is not finite: {}",
            name,
            value
        );
    }
}
