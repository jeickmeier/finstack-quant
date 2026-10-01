//! Tests for BaseCorrelationCurve functionality.
//!
//! This module covers:
//! - Builder validation and construction
//! - Serialization roundtrips

use finstack_quant_core::market_data::bumps::{BumpSpec, Bumpable};
use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;

// Serialization Tests

mod serde_tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let curve = BaseCorrelationCurve::builder("CDX")
            .knots([(3.0, 0.25), (7.0, 0.45), (10.0, 0.60)])
            .build()
            .unwrap();

        let json = serde_json::to_string_pretty(&curve).unwrap();
        let deserialized: BaseCorrelationCurve = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id(), curve.id());
        assert_eq!(deserialized.detachment_points(), curve.detachment_points());
        assert_eq!(deserialized.correlations(), curve.correlations());
    }
}

#[test]
fn builder_accepts_non_monotonic_quotes_with_explicit_shape_diagnostics() {
    let curve = BaseCorrelationCurve::builder("CDX")
        .knots([(3.0, 0.50), (7.0, 0.40), (10.0, 0.60)])
        .build()
        .unwrap();
    let report = curve.validate_shape();
    assert!(!report.is_monotonic);
    assert_eq!(report.violations.len(), 1);
    assert!((report.max_violation_magnitude - 0.10).abs() < 1e-14);
}

#[test]
fn builder_rejects_correlation_outside_unit_interval() {
    let result = BaseCorrelationCurve::builder("CDX")
        .knots([(3.0, 0.25), (7.0, 1.20)])
        .build();

    assert!(
        result.is_err(),
        "base-correlation builder should reject correlations outside [0, 1]"
    );
}

#[test]
fn bucket_bump_filters_and_clamps_correlations() {
    let curve = BaseCorrelationCurve::builder("CDX")
        .knots([(3.0, 0.25), (7.0, 0.45), (10.0, 0.60)])
        .build()
        .unwrap();

    let bumped = curve
        .apply_bucket_bump(Some(&[7.0]), 0.75)
        .expect("filtered bucket bump should rebuild the curve");

    assert_eq!(bumped.correlations()[0], 0.25);
    assert_eq!(bumped.correlations()[1], 1.0);
    assert_eq!(bumped.correlations()[2], 0.60);
    assert!(
        !bumped.validate_shape().is_monotonic,
        "single-bucket stress can intentionally break monotonicity"
    );
}

#[test]
fn malformed_wire_rejects_unmatched_detachments_or_correlations() {
    for (json, detachment_len, correlation_len) in [
        (
            r#"{"id":"CDX","detachment_points":[3,7,10],"correlations":[0.25,0.45]}"#,
            3,
            2,
        ),
        (
            r#"{"id":"CDX","detachment_points":[3,7],"correlations":[0.25,0.45,0.90]}"#,
            2,
            3,
        ),
    ] {
        let error = serde_json::from_str::<BaseCorrelationCurve>(json).unwrap_err();
        assert!(error.to_string().contains(&format!(
            "Base correlation curve 'CDX': detachment_points length {detachment_len} does not match correlations length {correlation_len}"
        )));
    }
}

#[test]
fn bucket_stress_survives_serde_and_identity_bumps() {
    let curve = BaseCorrelationCurve::builder("CDX")
        .knots([(3.0, 0.25), (7.0, 0.45), (10.0, 0.60)])
        .build()
        .unwrap();
    let stressed = curve.apply_bucket_bump(Some(&[7.0]), 0.75).unwrap();
    let restored: BaseCorrelationCurve =
        serde_json::from_str(&serde_json::to_string(&stressed).unwrap()).unwrap();
    let identity = stressed.apply_bump(BumpSpec::multiplier(1.0)).unwrap();
    for candidate in [restored, identity] {
        assert_eq!(candidate.detachment_points(), stressed.detachment_points());
        assert_eq!(candidate.correlations(), stressed.correlations());
        for detachment in [0.0, 3.0, 5.0, 7.0, 8.5, 10.0, 100.0] {
            assert_eq!(
                candidate.correlation(detachment),
                stressed.correlation(detachment)
            );
        }
    }
}

#[test]
fn correlation_boundaries_are_supported_by_construction_bumps_and_serde() {
    let curve = BaseCorrelationCurve::builder("BOUNDARIES")
        .knots([(0.0, 0.0), (3.0, 0.0), (100.0, 1.0)])
        .build()
        .unwrap();
    assert_eq!(curve.correlation(0.0), 0.0);
    assert_eq!(curve.correlation(3.0), 0.0);
    assert_eq!(curve.correlation(100.0), 1.0);
    let restored: BaseCorrelationCurve =
        serde_json::from_str(&serde_json::to_string(&curve).unwrap()).unwrap();
    assert_eq!(restored.correlations(), &[0.0, 0.0, 1.0]);
    let all_zero = curve.apply_bucket_bump(None, -1.0).unwrap();
    assert_eq!(all_zero.correlations(), &[0.0, 0.0, 0.0]);
    let all_one = curve.apply_bucket_bump(None, 1.0).unwrap();
    assert_eq!(all_one.correlations(), &[1.0, 1.0, 1.0]);
}

#[test]
fn constructors_and_bucket_bumps_reject_invalid_domains() {
    for detachment in [-1.0, 101.0, f64::NAN, f64::INFINITY] {
        assert!(BaseCorrelationCurve::builder("INVALID")
            .knots([(3.0, 0.25), (detachment, 0.45)])
            .build()
            .is_err());
    }
    for correlation in [-0.01, 1.01, f64::NAN, f64::INFINITY] {
        assert!(BaseCorrelationCurve::builder("INVALID")
            .knots([(3.0, 0.25), (7.0, correlation)])
            .build()
            .is_err());
    }
    let curve = BaseCorrelationCurve::builder("CDX")
        .knots([(3.0, 0.25), (7.0, 0.45)])
        .build()
        .unwrap();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(curve.apply_bucket_bump(None, value).is_none());
        assert!(curve.apply_bucket_bump(Some(&[value]), 0.01).is_none());
    }
}

#[test]
fn shape_report_makes_no_arbitrage_certification() {
    let curve = BaseCorrelationCurve::builder("STEEP")
        .knots([(3.0, 0.01), (7.0, 0.99)])
        .build()
        .unwrap();
    let report = curve.validate_shape();
    assert!(report.is_monotonic);
    assert_eq!(report.warnings.len(), 2);
    let wire = serde_json::to_value(report).unwrap();
    assert_eq!(wire["is_monotonic"], true);
    assert!(wire.get("is_arbitrage_free").is_none());
}
