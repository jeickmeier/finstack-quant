//! CDS Index risk metrics tests.
//!
//! Tests cover:
//! - DV01 (interest rate sensitivity)
//! - CS01 (credit spread sensitivity)
//! - Risky PV01 (premium spread sensitivity)
//! - Bucketed DV01 (term structure sensitivity)
//! - Risk metric scaling with notional
//! - Risk metric sign conventions

use super::test_utils::*;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::constants::isda::STANDARD_RECOVERY_SENIOR;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
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
            crate::test_support::credit::pricing_options(),
        )
        .unwrap();
    let rpv01 = *result.measures.get("risky_pv01").unwrap();

    assert_positive(rpv01, "Risky PV01");
    in_range(rpv01, 3_500.0, 5_500.0, "Risky PV01 for $10MM, 5Y");
}

#[test]
fn test_cs01_positive() {
    // Test: CS01 should be positive
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-CS01", start, end, 10_000_000.0);
    let ctx = replayable_standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            crate::test_support::credit::pricing_options(),
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
            crate::test_support::credit::pricing_options(),
        )
        .unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    // DV01 = PV(rate+1bp) - PV(base); sign depends on instrument structure
    assert!(dv01.is_finite(), "DV01 should be finite");
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
            crate::test_support::credit::pricing_options(),
        )
        .unwrap();
    let result_20mm = idx_20mm
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01],
            crate::test_support::credit::pricing_options(),
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
    let ctx = replayable_standard_market_context(as_of);

    let idx_3y = standard_single_curve_index("CDX-3Y", start, date!(2028 - 01 - 01), 10_000_000.0);
    let idx_5y = standard_single_curve_index("CDX-5Y", start, date!(2030 - 01 - 01), 10_000_000.0);

    let result_3y = idx_3y
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            crate::test_support::credit::pricing_options(),
        )
        .unwrap();
    let result_5y = idx_5y
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            crate::test_support::credit::pricing_options(),
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
fn test_standard_cs01_requires_replay_recipe() {
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-CS01", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);
    let provider = finstack_quant_calibration::recalibration::CachedRecalibrationProvider::new();

    let direct_error = idx
        .cs01(&ctx, as_of, &provider)
        .expect_err("standard CS01 requires quote-space replay");
    assert!(direct_error.to_string().contains("calibration recipe"));

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            finstack_quant_valuations::instruments::PricingOptions::default()
                .with_recalibration_provider(std::sync::Arc::new(provider)),
        )
        .expect_err("standard CS01 metric requires quote-space replay");
    assert!(result.to_string().contains("calibration recipe"));
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
            crate::test_support::credit::pricing_options(),
        )
        .unwrap();
    let result_const = idx_const
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::RiskyPv01],
            crate::test_support::credit::pricing_options(),
        )
        .unwrap();

    let rpv01_single = *result_single.measures.get("risky_pv01").unwrap();
    let rpv01_const = *result_const.measures.get("risky_pv01").unwrap();

    relative_eq(rpv01_single, rpv01_const, 0.05, "Risky PV01 parity");
}

#[test]
fn test_cs01_single_vs_constituents() {
    // Test: CS01 consistency across pricing modes
    //
    // Both modes use identical replayable par-spread quotes and recovery.
    // CS01 is computed by bumping those quotes by 1bp, re-bootstrapping the
    // hazard curves, and repricing.
    //
    // With identical curves, both should produce similar results.
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = replayable_multi_constituent_market_context(as_of, 5);

    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);
    let idx_const = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    let result_single = idx_single
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            crate::test_support::credit::pricing_options(),
        )
        .unwrap();
    let result_const = idx_const
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01],
            crate::test_support::credit::pricing_options(),
        )
        .unwrap();

    let cs01_single = *result_single.measures.get("cs01").unwrap();
    let cs01_const = *result_const.measures.get("cs01").unwrap();

    // 5% tolerance: aggregation of per-constituent CS01 vs single curve
    relative_eq(cs01_single, cs01_const, 0.05, "CS01 parity");
}

#[test]
fn test_bucketed_cs01_requires_replay_recipe() {
    // Manually built curves have no replay recipe.
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-BKT-SC", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let error = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            crate::test_support::credit::pricing_options(),
        )
        .expect_err("standard CDS index CS01 requires quote-space replay");
    assert!(error.to_string().contains("calibration recipe"));
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn bucketed_cs01_quote_single_curve_uses_each_off_grid_replay_quote_once() {
    let as_of = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let source = MarketContext::new().insert(flat_discount_curve("USD-OIS", as_of, 0.03));
    let hazard = crate::test_support::credit::calibrated_hazard_curve_with_pillars(
        &source,
        as_of,
        "HZ-INDEX",
        "INDEX",
        "USD-OIS",
        &[
            (365, 80.0),
            (3 * 365, 100.0),
            (4 * 365, 115.0),
            (5 * 365, 125.0),
            (10 * 365, 140.0),
        ],
    )
    .expect("off-grid index hazard calibration");
    let expected_count = hazard
        .hazard_calibration()
        .expect("replayable index hazard")
        .spread_risk_inputs
        .len();
    let market = source.insert(hazard);
    let index = standard_single_curve_index("CDX-QUOTE-OFFGRID", as_of, end, 10_000_000.0);

    let result = index
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            crate::test_support::credit::pricing_options(),
        )
        .expect("single-curve quote-space bucketed CS01");
    let prefix = "bucketed_cs01::HZ-INDEX::";
    let buckets: Vec<_> = result
        .measures
        .iter()
        .filter(|(key, _)| key.as_str().starts_with(prefix))
        .collect();
    let bucket_sum: f64 = buckets.iter().map(|(_, value)| **value).sum();

    assert_eq!(
        buckets.len(),
        expected_count,
        "each index replay quote must appear once: {buckets:?}"
    );
    assert!(
        buckets
            .iter()
            .any(|(key, _)| key.as_str() == "bucketed_cs01::HZ-INDEX::4y"),
        "off-grid 4Y index quote must be represented: {buckets:?}"
    );
    relative_eq(
        bucket_sum,
        result.measures[MetricId::Cs01.as_str()],
        0.02,
        "single-curve quote buckets vs parallel CS01",
    );
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn bucketed_cs01_quote_constituents_use_each_off_grid_replay_quote_once() {
    let as_of = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let source = MarketContext::new().insert(flat_discount_curve("USD-OIS", as_of, 0.03));
    let hz1 = crate::test_support::credit::calibrated_hazard_curve_with_pillars(
        &source,
        as_of,
        "HZ1",
        "NAME1",
        "USD-OIS",
        &[
            (365, 80.0),
            (3 * 365, 100.0),
            (4 * 365, 115.0),
            (5 * 365, 125.0),
            (10 * 365, 140.0),
        ],
    )
    .expect("first off-grid constituent hazard calibration");
    let hz2 = crate::test_support::credit::calibrated_hazard_curve_with_pillars(
        &source,
        as_of,
        "HZ2",
        "NAME2",
        "USD-OIS",
        &[
            (365, 90.0),
            (3 * 365, 105.0),
            (4 * 365, 118.0),
            (5 * 365, 130.0),
            (10 * 365, 145.0),
        ],
    )
    .expect("second off-grid constituent hazard calibration");
    let expected_per_curve = hz1
        .hazard_calibration()
        .expect("replayable constituent hazard")
        .spread_risk_inputs
        .len();
    let market = source.insert(hz1).insert(hz2).insert(flat_hazard_curve(
        "HZ-INDEX",
        as_of,
        STANDARD_RECOVERY_SENIOR,
        STANDARD_HAZARD_RATE,
    ));
    let index = standard_constituents_index("CDX-CONSTITUENT-OFFGRID", as_of, end, 10_000_000.0, 2);

    let result = index
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            crate::test_support::credit::pricing_options(),
        )
        .expect("constituent quote-space bucketed CS01");
    let curve_buckets = |curve_id: &str| {
        let prefix = format!("bucketed_cs01::{curve_id}::");
        result
            .measures
            .iter()
            .filter(|(key, _)| key.as_str().starts_with(&prefix))
            .collect::<Vec<_>>()
    };
    let hz1_buckets = curve_buckets("HZ1");
    let hz2_buckets = curve_buckets("HZ2");
    let bucket_sum: f64 = hz1_buckets
        .iter()
        .chain(&hz2_buckets)
        .map(|(_, value)| **value)
        .sum();

    assert_eq!(hz1_buckets.len(), expected_per_curve);
    assert_eq!(hz2_buckets.len(), expected_per_curve);
    assert!(hz1_buckets
        .iter()
        .any(|(key, _)| key.as_str() == "bucketed_cs01::HZ1::4y"));
    assert!(hz2_buckets
        .iter()
        .any(|(key, _)| key.as_str() == "bucketed_cs01::HZ2::4y"));
    relative_eq(
        bucket_sum,
        result.measures[MetricId::Cs01.as_str()],
        0.02,
        "constituent quote buckets vs parallel CS01",
    );
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn test_bucketed_cs01_reconciles_to_parallel_constituents() {
    // In `Constituents` mode, the bucketed calculator bumps each constituent's
    // replayable quote set one tenor at a time and reprices end-to-end.
    // Expensive under parallel CI load (N curves × tenors × central-diff reprices).
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = replayable_multi_constituent_market_context(as_of, 5);

    let idx = standard_constituents_index("CDX-BKT-CONST", start, end, 10_000_000.0, 5);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            crate::test_support::credit::pricing_options(),
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
    relative_eq(
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
    relative_eq(
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
    let ctx = replayable_standard_market_context(as_of);

    let metrics = vec![MetricId::RiskyPv01, MetricId::Cs01, MetricId::Dv01];

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &metrics,
            crate::test_support::credit::pricing_options(),
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
            crate::test_support::credit::pricing_options(),
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
    let ctx = replayable_standard_market_context(as_of);

    let metrics = vec![MetricId::RiskyPv01, MetricId::Cs01, MetricId::Dv01];

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &metrics,
            crate::test_support::credit::pricing_options(),
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

// Recovery01 is the PV sensitivity to a +1% recovery-rate bump. These tests
// guard against it silently regressing to zero/NaN or losing linearity in
// notional.

#[test]
fn test_recovery01_finite_and_nonzero() {
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);
    let idx = standard_single_curve_index("CDX-REC01", start, end, 10_000_000.0);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Recovery01],
            crate::test_support::credit::pricing_options(),
        )
        .unwrap();
    let recovery01 = *result.measures.get("recovery_01").unwrap();

    assert!(
        recovery01.is_finite(),
        "Recovery01 should be finite, got {}",
        recovery01
    );
    assert!(
        recovery01.abs() > 0.0,
        "Recovery01 should be non-zero for a live index, got {}",
        recovery01
    );
}

#[test]
fn test_recovery01_scales_with_notional() {
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx_10mm = standard_single_curve_index("CDX-REC01-10", start, end, 10_000_000.0);
    let idx_20mm = standard_single_curve_index("CDX-REC01-20", start, end, 20_000_000.0);

    let rec01_10mm = *idx_10mm
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Recovery01],
            crate::test_support::credit::pricing_options(),
        )
        .unwrap()
        .measures
        .get("recovery_01")
        .unwrap();
    let rec01_20mm = *idx_20mm
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Recovery01],
            crate::test_support::credit::pricing_options(),
        )
        .unwrap()
        .measures
        .get("recovery_01")
        .unwrap();

    assert_linear_scaling(
        rec01_10mm,
        10_000_000.0,
        rec01_20mm,
        20_000_000.0,
        "Recovery01",
        0.05,
    );
}
