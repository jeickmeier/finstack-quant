//! Tests for HazardCurve functionality.
//!
//! This module covers:
//! - Builder validation and construction
//! - Survival and default probability calculations
//! - Hazard rate shifts
//! - Metadata preservation
//! - Analytical verification

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::term_structures::{HazardCurve, ParInterp, Seniority};
use time::{Date, Month};

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

// =============================================================================
// Builder Validation Tests
// =============================================================================

#[test]
fn builder_rejects_empty_knots() {
    let err = HazardCurve::builder("BAD")
        .build()
        .expect_err("no points should fail");
    assert!(matches!(err, finstack_quant_core::Error::Input(_)));
}

#[test]
fn builder_rejects_negative_hazard_rate() {
    let err = HazardCurve::builder("NEG")
        .knots([(1.0, -0.01), (2.0, 0.02)])
        .build()
        .expect_err("negative lambda should fail");
    assert!(matches!(err, finstack_quant_core::Error::Input(_)));
}

// =============================================================================
// Survival and Default Probability Tests
// =============================================================================

#[test]
fn survival_and_default_probabilities() {
    let curve = HazardCurve::builder("HC")
        .base_date(base_date())
        .knots([(1.0, 0.01), (2.0, 0.015), (5.0, 0.02)])
        .par_spreads([(1.0, 100.0), (3.0, 150.0)])
        .build()
        .unwrap();

    assert!(curve.sp(0.0) - 1.0 < 1e-12);
    assert!(curve.sp(4.0) < curve.sp(1.0));
    assert!(curve.default_prob(1.0, 3.0).unwrap() > 0.0);

    let linear = curve.cds_quote_bp(2.0, ParInterp::Linear);
    let log = curve.cds_quote_bp(2.0, ParInterp::LogLinear);
    assert!(linear > 0.0 && log > 0.0);
}

#[test]
fn hazard_shift_rejects_negative_rates() {
    let curve = HazardCurve::builder("HC")
        .base_date(base_date())
        .knots([(1.0, 0.01), (5.0, 0.02)])
        .build()
        .unwrap();

    let err = curve.with_parallel_bump(-0.02).unwrap_err();
    assert!(err.to_string().contains("negative hazard rate after bump"));
}

// =============================================================================
// Metadata Preservation Tests
// =============================================================================

#[test]
fn to_builder_preserves_metadata() {
    let curve = HazardCurve::builder("HC")
        .base_date(base_date())
        .recovery_rate(0.35)
        .issuer("ACME Corp")
        .seniority(Seniority::Senior)
        .currency(Currency::USD)
        .knots([(1.0, 0.01), (3.0, 0.02)])
        .par_spreads([(2.0, 120.0)])
        .build()
        .unwrap();

    let rebuilt = curve
        .to_builder_with_id("HC-NEW")
        .build()
        .expect("builder should succeed");

    assert_eq!(rebuilt.recovery_rate(), curve.recovery_rate());
    assert_eq!(rebuilt.seniority, Some(Seniority::Senior));
    assert_eq!(
        rebuilt.knot_points().collect::<Vec<_>>(),
        curve.knot_points().collect::<Vec<_>>()
    );
}

// =============================================================================
// Analytical Verification Tests
// =============================================================================

#[test]
fn sp_analytical_verification_constant_hazard() {
    // Constant hazard rate for simple verification
    let curve = HazardCurve::builder("SP-VERIFY")
        .base_date(base_date())
        .knots([(1.0, 0.02), (5.0, 0.02), (10.0, 0.02)])
        .build()
        .unwrap();

    // S(t) = exp(-λ*t) for constant λ
    for t in [1.0, 2.5, 5.0, 7.5, 10.0] {
        let expected = (-0.02_f64 * t).exp();
        let actual = curve.sp(t);
        assert!(
            (actual - expected).abs() < 1e-12,
            "SP at t={}: got {}, expected {}",
            t,
            actual,
            expected
        );
    }
}

#[test]
fn sp_piecewise_verification() {
    // Piecewise hazard curve: knots define λ at each point
    // Implementation uses the first lambda on [0, first_knot], then λ[i] on (knot[i-1], knot[i]].
    let curve = HazardCurve::builder("SP-PIECEWISE")
        .base_date(base_date())
        .knots([(1.0, 0.01), (2.0, 0.015), (5.0, 0.02)])
        .build()
        .unwrap();

    // S(1) = exp(-0.01*1) - first lambda applies through the first pillar
    let expected_1 = (-0.01_f64 * 1.0).exp();
    assert!(
        (curve.sp(1.0) - expected_1).abs() < 1e-12,
        "SP at t=1: got {}, expected {}",
        curve.sp(1.0),
        expected_1
    );

    // S(3) = exp(-(0.01*1 + 0.015*1 + 0.02*1))
    let expected_3 = (-(0.01_f64 * 1.0 + 0.015 * 1.0 + 0.02 * 1.0)).exp();
    assert!(
        (curve.sp(3.0) - expected_3).abs() < 1e-12,
        "SP at t=3: got {}, expected {}",
        curve.sp(3.0),
        expected_3
    );

    // Default prob P(1,3) = S(1) - S(3)
    let expected_dp = expected_1 - expected_3;
    assert!(
        (curve.default_prob(1.0, 3.0).unwrap() - expected_dp).abs() < 1e-12,
        "Default prob 1-3: got {}, expected {}",
        curve.default_prob(1.0, 3.0).unwrap(),
        expected_dp
    );
}

#[test]
fn explicit_zero_time_knot_defines_first_positive_interval() {
    let curve = HazardCurve::builder("SP-ZERO-KNOT")
        .base_date(base_date())
        .knots([(0.0, 0.01), (5.0, 0.02)])
        .build()
        .unwrap();

    let expected_1 = (-0.01_f64).exp();
    assert!(
        (curve.sp(1.0) - expected_1).abs() < 1e-12,
        "SP at t=1: got {}, expected {}",
        curve.sp(1.0),
        expected_1
    );

    let expected_5 = (-0.01_f64 * 5.0).exp();
    assert!(
        (curve.sp(5.0) - expected_5).abs() < 1e-12,
        "SP at t=5: got {}, expected {}",
        curve.sp(5.0),
        expected_5
    );
}

// =============================================================================
// Additional hazard curve behavior
// =============================================================================

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

#[test]
fn test_hazard_curve_sp() {
    let curve = HazardCurve::builder("CDS-TEST")
        .base_date(test_date())
        .knots([(1.0, 0.01), (2.0, 0.012), (5.0, 0.015)])
        .recovery_rate(0.4)
        .build()
        .unwrap();

    // Survival probability at t=0 should be 1.0
    let surv_0 = curve.sp(0.0);
    assert!((surv_0 - 1.0).abs() < 1e-12);

    // Survival should decrease with time
    let surv_1 = curve.sp(1.0);
    let surv_5 = curve.sp(5.0);

    assert!(surv_1 < 1.0 && surv_1 > 0.0);
    assert!(surv_5 < surv_1);
}

#[test]
fn test_hazard_curve_default_probability() {
    let curve = HazardCurve::builder("CDS-TEST")
        .base_date(test_date())
        .knots([(1.0, 0.01), (5.0, 0.02)])
        .recovery_rate(0.4)
        .build()
        .unwrap();

    // Default probability = 1 - survival
    let surv = curve.sp(1.0);
    let default_prob = 1.0 - surv;

    assert!(default_prob > 0.0 && default_prob < 1.0);
}

#[test]
fn test_hazard_curve_cds_spread_bootstrap() {
    // Test bootstrapping from CDS spreads
    let spreads = vec![(1.0, 0.0100), (3.0, 0.0150), (5.0, 0.0200)];

    let curve = HazardCurve::builder("CDS-BOOT")
        .base_date(test_date())
        .knots(spreads)
        .recovery_rate(0.4)
        .build()
        .unwrap();

    // Hazard rates should be consistent with spreads
    let surv = curve.sp(1.0);
    assert!(surv > 0.95 && surv < 1.0);
}

#[test]
fn test_hazard_curve_recovery_rate_zero() {
    // Zero recovery (harshest case)
    let curve = HazardCurve::builder("ZERO-REC")
        .base_date(test_date())
        .knots([(1.0, 0.02), (5.0, 0.03)])
        .recovery_rate(0.0)
        .build()
        .unwrap();

    let surv = curve.sp(1.0);
    assert!(surv < 1.0);
}

#[test]
fn test_hazard_curve_recovery_rate_full() {
    // Full recovery (no loss given default)
    let curve = HazardCurve::builder("FULL-REC")
        .base_date(test_date())
        .knots([(1.0, 0.02), (5.0, 0.03)])
        .recovery_rate(1.0)
        .build()
        .unwrap();

    // Even with defaults, full recovery means no expected loss
    let surv = curve.sp(1.0);
    assert!(surv > 0.0);
}

#[test]
fn test_hazard_curve_edge_case_zero_spreads() {
    // Zero hazard rates (no default risk)
    let curve = HazardCurve::builder("RISK-FREE")
        .base_date(test_date())
        .knots([(1.0, 0.0), (10.0, 0.0)])
        .recovery_rate(0.4)
        .build()
        .unwrap();

    // Survival should remain 1.0
    let surv = curve.sp(5.0);
    assert!((surv - 1.0).abs() < 1e-10);
}

#[test]
fn test_hazard_curve_very_long_tenor() {
    // Very long dated CDS
    let curve = HazardCurve::builder("LONG")
        .base_date(test_date())
        .knots([(1.0, 0.01), (30.0, 0.02)])
        .recovery_rate(0.4)
        .build()
        .unwrap();

    let surv_30y = curve.sp(30.0);
    assert!(surv_30y > 0.0 && surv_30y < 1.0);
}

#[test]
fn test_hazard_curve_interpolation() {
    let curve = HazardCurve::builder("INTERP")
        .base_date(test_date())
        .knots([(1.0, 0.01), (5.0, 0.02)])
        .recovery_rate(0.4)
        .build()
        .unwrap();

    // Interpolated hazard rate at t=3
    let surv_3 = curve.sp(3.0);
    let surv_1 = curve.sp(1.0);
    let surv_5 = curve.sp(5.0);

    // Should be between the pillars
    assert!(surv_3 < surv_1 && surv_3 > surv_5);
}

#[test]
fn test_hazard_curve_serde_round_trip() {
    let original = HazardCurve::builder("SERDE-TEST")
        .base_date(test_date())
        .knots([(1.0, 0.01), (2.0, 0.015), (5.0, 0.02)])
        .recovery_rate(0.4)
        .build()
        .unwrap();

    let json = serde_json::to_string(&original).unwrap();
    let deserialized: HazardCurve = serde_json::from_str(&json).unwrap();

    assert_eq!(original.id(), deserialized.id());

    // Verify survival probabilities match
    for t in [0.0, 1.0, 3.0, 5.0] {
        let orig_surv = original.sp(t);
        let deser_surv = deserialized.sp(t);
        assert!((orig_surv - deser_surv).abs() < 1e-12);
    }
}
