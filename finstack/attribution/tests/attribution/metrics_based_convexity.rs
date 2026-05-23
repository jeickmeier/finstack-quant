//! Integration tests for second-order metrics in attribution.
//!
//! Tests verify that second-order convexity terms (Gamma, Convexity, Volga, etc.)
//! reduce residuals in metrics-based attribution. Market-standard targets:
//! - First-order only (DV01, Theta): < 10%
//! - With second-order (Convexity, Gamma): < 5%

use crate::common::test_utils::TestInstrument;
use finstack_attribution::attribute_pnl_metrics_based;
use finstack_core::config::{results_meta, FinstackConfig};
use finstack_core::currency::Currency;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use finstack_valuations::results::ValuationResult;
use indexmap::IndexMap;
use std::sync::Arc;
use time::macros::date;

fn build_flat_curve(curve_id: &str, as_of: time::Date, rate: f64) -> DiscountCurve {
    let tenors = [0.0, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0, 30.0];
    let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, (-rate * t).exp())).collect();

    DiscountCurve::builder(curve_id)
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

/// Test that ValuationResult structure supports storing convexity metrics.
///
/// NOTE: This test verifies structural support for second-order metrics,
/// not that convexity actually reduces attribution residuals. For actual
/// residual reduction testing, see integration tests that run full attribution.
#[test]
fn test_valuation_result_supports_convexity_metric() {
    let t0 = date!(2025 - 01 - 15);

    // Simulate valuation results with first-order metrics only
    let meta = finstack_core::config::results_meta(&FinstackConfig::default());

    let mut measures_first_order = IndexMap::new();
    measures_first_order.insert(MetricId::Theta, -50.0);
    measures_first_order.insert(MetricId::Dv01, -4000.0); // High DV01 for $1M bond

    let val_t0_first = ValuationResult::stamped_with_meta(
        "TEST-BOND",
        t0,
        Money::new(1_050_000.0, Currency::USD),
        meta.clone(),
    )
    .with_measures(measures_first_order.clone());

    // Now add convexity to the first-order metrics
    let mut measures_with_convexity = measures_first_order;
    measures_with_convexity.insert(MetricId::Convexity, 8000.0); // Positive convexity for bonds

    let val_t0_second = ValuationResult::stamped_with_meta(
        "TEST-BOND",
        t0,
        Money::new(1_050_000.0, Currency::USD),
        meta,
    )
    .with_measures(measures_with_convexity);

    // Verify the structure supports second-order metrics
    assert!(val_t0_second.measures.contains_key(&MetricId::Convexity));
    assert_eq!(
        val_t0_second
            .measures
            .get(&MetricId::Convexity)
            .copied()
            .unwrap(),
        8000.0
    );

    // Verify first-order doesn't have convexity
    assert!(!val_t0_first.measures.contains_key(&MetricId::Convexity));
}

#[test]
fn test_second_order_metrics_available() {
    // Test that all second-order metric IDs are properly defined
    // Map each metric ID to its expected string name
    let expected_names: [(&MetricId, &str); 7] = [
        (&MetricId::Convexity, "convexity"),
        (&MetricId::IrConvexity, "ir_convexity"),
        (&MetricId::CsGamma, "cs_gamma"),
        (&MetricId::Gamma, "gamma"),
        (&MetricId::Volga, "volga"),
        (&MetricId::Vanna, "vanna"),
        (&MetricId::InflationConvexity, "inflation_convexity"),
    ];

    for (metric_id, expected_name) in expected_names {
        // Verify each metric ID has a valid string representation
        assert!(!metric_id.as_str().is_empty());
        // Verify the metric ID maps to the expected string name
        assert_eq!(
            metric_id.as_str(),
            expected_name,
            "Metric ID {:?} should have name '{}'",
            metric_id,
            expected_name
        );
    }
}

#[test]
fn test_valuation_result_supports_all_second_order_metrics() {
    // Test that ValuationResult can store all second-order metrics
    let as_of = date!(2025 - 01 - 15);
    let value = Money::new(1_000_000.0, Currency::USD);

    let mut measures = IndexMap::new();

    // Add all second-order metrics
    measures.insert(MetricId::Convexity, 5000.0);
    measures.insert(MetricId::IrConvexity, 4800.0);
    measures.insert(MetricId::CsGamma, 50.0);
    measures.insert(MetricId::Gamma, 0.05);
    measures.insert(MetricId::Volga, 2.0);
    measures.insert(MetricId::Vanna, 1.5);
    measures.insert(MetricId::InflationConvexity, 100.0);

    let meta = finstack_core::config::results_meta(&FinstackConfig::default());
    let val =
        ValuationResult::stamped_with_meta("TEST", as_of, value, meta).with_measures(measures);

    // Verify all metrics are present
    assert_eq!(val.measures.len(), 7);
    assert!(val.measures.contains_key(&MetricId::Convexity));
    assert!(val.measures.contains_key(&MetricId::IrConvexity));
    assert!(val.measures.contains_key(&MetricId::CsGamma));
    assert!(val.measures.contains_key(&MetricId::Gamma));
    assert!(val.measures.contains_key(&MetricId::Volga));
    assert!(val.measures.contains_key(&MetricId::Vanna));
    assert!(val.measures.contains_key(&MetricId::InflationConvexity));
}

#[test]
fn test_convexity_formula_correctness() {
    // Verify the mathematical correctness of the convexity term calculation.
    //
    // The implementation in metrics_based.rs uses the percentage convexity formula:
    //   Convexity P&L = ½ × P₀ × Convexity × (Δr)²
    //
    // Where:
    //   - P₀ = instrument price/value
    //   - Convexity = percentage convexity metric (dimensionless)
    //   - Δr = rate shift in decimal (e.g., 0.0050 for 50bp)
    //
    // Note: The shift must be converted from bp to decimal (divide by 10,000)
    // because convexity is a percentage metric.

    let p0: f64 = 1_000_000.0; // $1M bond price
    let convexity: f64 = 80.0; // Typical percentage convexity for a 5-year bond
    let shift_bp: f64 = 50.0; // 50bp shift
    let shift_decimal: f64 = shift_bp / 10_000.0; // 0.005

    // Second-order term matching implementation formula
    let convexity_pnl: f64 = 0.5 * p0 * convexity * shift_decimal * shift_decimal;

    // With $1M, convexity=80, 50bp shift:
    // 0.5 * 1,000,000 * 80 * 0.005 * 0.005 = 0.5 * 1,000,000 * 80 * 0.000025 = 1,000
    assert!((convexity_pnl - 1000.0).abs() < 0.01);

    // For smaller shifts (1bp), convexity effect is minimal
    let small_shift_bp: f64 = 1.0;
    let small_shift_decimal: f64 = small_shift_bp / 10_000.0; // 0.0001
    let small_convexity_pnl: f64 = 0.5 * p0 * convexity * small_shift_decimal * small_shift_decimal;

    // 0.5 * 1,000,000 * 80 * 0.0001 * 0.0001 = 0.4
    assert!((small_convexity_pnl - 0.4).abs() < 0.01);

    // Convexity effect scales with (Δr)²
    // 50bp shift is 50x larger than 1bp, so convexity P&L is 50² = 2500x larger
    let ratio: f64 = convexity_pnl / small_convexity_pnl;
    assert!((ratio - 2500.0).abs() < 1.0);
}

#[test]
fn test_metrics_based_convexity_reduces_residual() {
    use finstack_core::market_data::diff::{measure_discount_curve_shift, TenorSamplingMethod};

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, 0.04);
    let curve_t1 = build_flat_curve("USD-OIS", as_of_t1, 0.05); // ~100bp shift
    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    // Measure the actual shift from the curves (may differ slightly from 100bp
    // due to interpolation artifacts and tenor sampling)
    let measured_shift_bp = measure_discount_curve_shift(
        "USD-OIS",
        &market_t0,
        &market_t1,
        TenorSamplingMethod::Standard,
    )
    .unwrap();

    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new("METRICS-CONVEXITY", Money::new(1_000_000.0, Currency::USD))
            .with_discount_curves(&["USD-OIS"]),
    );

    let p0 = 1_000_000.0;
    let shift_decimal = measured_shift_bp / 10_000.0;
    let dv01 = -4_500.0; // $ per bp
    let convexity = 80.0; // dimensionless

    // Compute expected P&L using the actual measured shift for self-consistency
    let expected_convexity_pnl = 0.5 * p0 * convexity * shift_decimal * shift_decimal;
    let total_pnl = dv01 * measured_shift_bp + expected_convexity_pnl;

    let mut measures_first = IndexMap::new();
    measures_first.insert(MetricId::Theta, 0.0);
    measures_first.insert(MetricId::Dv01, dv01);

    let mut measures_second = measures_first.clone();
    measures_second.insert(MetricId::Convexity, convexity);

    let meta = results_meta(&FinstackConfig::default());
    let val_t0_first = ValuationResult::stamped_with_meta(
        "METRICS-CONVEXITY",
        as_of_t0,
        Money::new(p0, Currency::USD),
        meta.clone(),
    )
    .with_measures(measures_first.clone());

    let val_t1_first = ValuationResult::stamped_with_meta(
        "METRICS-CONVEXITY",
        as_of_t1,
        Money::new(p0 + total_pnl, Currency::USD),
        meta.clone(),
    )
    .with_measures(measures_first);

    let val_t0_second = ValuationResult::stamped_with_meta(
        "METRICS-CONVEXITY",
        as_of_t0,
        Money::new(p0, Currency::USD),
        meta.clone(),
    )
    .with_measures(measures_second.clone());

    let val_t1_second = ValuationResult::stamped_with_meta(
        "METRICS-CONVEXITY",
        as_of_t1,
        Money::new(p0 + total_pnl, Currency::USD),
        meta,
    )
    .with_measures(measures_second);

    let attr_first = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0_first,
        &val_t1_first,
        as_of_t0,
        as_of_t1,
    )
    .unwrap();

    let attr_second = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0_second,
        &val_t1_second,
        as_of_t0,
        as_of_t1,
    )
    .unwrap();

    let residual_first = attr_first.residual.amount().abs();
    let residual_second = attr_second.residual.amount().abs();

    // Without convexity metric, the residual should approximate the convexity P&L
    // (the second-order effect that wasn't captured by first-order DV01)
    assert!(
        (residual_first - expected_convexity_pnl).abs() < 1.0,
        "Without convexity, residual should match convexity P&L, expected {:.2}, got {:.2}",
        expected_convexity_pnl,
        residual_first
    );

    // With convexity metric, the residual should be near zero
    assert!(
        residual_second < 1.0,
        "With convexity, residual should be near zero, got {:.2}",
        residual_second
    );

    // Including convexity should always reduce the residual
    assert!(
        residual_second < residual_first,
        "Convexity should reduce residual, first={:.2}, second={:.2}",
        residual_first,
        residual_second
    );
}
