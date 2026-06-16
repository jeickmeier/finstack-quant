//! Integration tests for covenant enforcement in private credit instruments.
//!
//! # Coverage
//!
//! - CovenantReport construction and fluent API
//! - Covenant pass/fail logic based on threshold tests
//! - Headroom calculation verification
//! - CovenantEngine spec management
//! - Covenant evaluation with custom metrics

use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_covenants::{
    Covenant, CovenantEngine, CovenantMetricId, CovenantReport, CovenantScope, CovenantSpec,
    CovenantType, HashMapMetricSource, ThresholdTest,
};

// =============================================================================
// CovenantReport Construction Tests
// =============================================================================

#[test]
fn test_covenant_report_smoke() {
    let report = CovenantReport::failed("Debt/EBITDA <= 4.00")
        .with_actual(5.0)
        .with_threshold(4.0);
    assert!(!report.passed);
}

#[test]
fn covenant_report_passed_with_all_fields() {
    let report = CovenantReport::passed("Interest Coverage >= 1.50x")
        .with_actual(2.5)
        .with_threshold(1.5)
        .with_headroom(0.667)
        .with_details("Comfortably above threshold");

    assert!(report.passed);
    assert_eq!(report.actual_value, Some(2.5));
    assert_eq!(report.threshold, Some(1.5));
    assert!((report.headroom.unwrap() - 0.667).abs() < 0.001);
    assert!(report.details.is_some());
}

#[test]
fn covenant_report_failed_with_negative_headroom() {
    let report = CovenantReport::failed("Debt/EBITDA <= 5.00x")
        .with_actual(5.5)
        .with_threshold(5.0)
        .with_headroom(-0.10);

    assert!(!report.passed);
    assert!(report.headroom.unwrap() < 0.0);
}

// =============================================================================
// Headroom Calculation Tests
// =============================================================================

#[test]
fn headroom_calculation_max_covenant() {
    // For MaxDebtToEBITDA: headroom = (threshold - actual) / threshold
    // If threshold = 5.0 and actual = 4.0, headroom = (5-4)/5 = 0.20 (20% cushion)
    let threshold: f64 = 5.0;
    let actual: f64 = 4.0;
    let headroom: f64 = (threshold - actual) / threshold;

    assert!(
        (headroom - 0.20).abs() < 0.001,
        "Headroom for max covenant should be 20%, got {}",
        headroom
    );

    let report = CovenantReport::passed("Debt/EBITDA <= 5.00x")
        .with_actual(actual)
        .with_threshold(threshold)
        .with_headroom(headroom);

    assert!(report.passed);
    assert!(report.headroom.unwrap() > 0.0);
}

#[test]
fn headroom_calculation_min_covenant() {
    // For MinInterestCoverage: headroom = (actual - threshold) / threshold
    // If threshold = 1.5 and actual = 2.0, headroom = (2-1.5)/1.5 = 0.333 (33% cushion)
    let threshold: f64 = 1.5;
    let actual: f64 = 2.0;
    let headroom: f64 = (actual - threshold) / threshold;

    assert!(
        (headroom - 0.333).abs() < 0.01,
        "Headroom for min covenant should be ~33%, got {}",
        headroom
    );
}

// =============================================================================
// CovenantEngine Spec Management Tests
// =============================================================================

#[test]
fn covenant_engine_add_specs() {
    let mut engine = CovenantEngine::new();

    // Add leverage covenant
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEBITDA { threshold: 5.0 },
            Tenor::quarterly(),
        ),
        CovenantMetricId::from("debt_to_ebitda"),
    ));

    // Add interest coverage covenant
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MinInterestCoverage { threshold: 1.5 },
            Tenor::quarterly(),
        ),
        CovenantMetricId::from("interest_coverage"),
    ));

    assert_eq!(engine.specs.len(), 2);
}

#[test]
fn same_type_covenants_with_distinct_labels_do_not_collide() {
    // Regression: two covenants of the same type+threshold (e.g. a senior and a
    // total leverage test) used to collide — reports were keyed by the (identical)
    // description and breaches by the discriminant-only `covenant_id`, so one
    // silently overwrote the other. Distinct instance labels must keep them apart.
    let mut engine = CovenantEngine::new();
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEBITDA { threshold: 4.0 },
            Tenor::quarterly(),
        )
        .with_label("senior_leverage"),
        CovenantMetricId::from("debt_to_ebitda"),
    ));
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEBITDA { threshold: 4.0 },
            Tenor::quarterly(),
        )
        .with_label("total_leverage"),
        CovenantMetricId::from("debt_to_ebitda"),
    ));

    let mut metrics = HashMapMetricSource::new();
    metrics.insert("debt_to_ebitda", 5.0); // breaches the 4.0 max

    let test_date = Date::from_calendar_date(2025, time::Month::March, 31).unwrap();
    let reports = engine.evaluate(&mut metrics, test_date).expect("evaluate");

    // Both covenants must be reported under their distinct labels — no collision.
    assert_eq!(
        reports.len(),
        2,
        "same-type covenants must not collide: {reports:?}"
    );
    assert!(reports.contains_key("senior_leverage"));
    assert!(reports.contains_key("total_leverage"));
    assert!(
        reports.values().all(|r| !r.passed),
        "both leverage tests breach 4.0x"
    );
}

#[test]
fn covenant_description_formatting() {
    let leverage = Covenant::new(
        CovenantType::MaxDebtToEBITDA { threshold: 4.5 },
        Tenor::quarterly(),
    );
    assert_eq!(leverage.description(), "Debt/EBITDA <= 4.50x");

    let coverage = Covenant::new(
        CovenantType::MinInterestCoverage { threshold: 2.0 },
        Tenor::quarterly(),
    );
    assert_eq!(coverage.description(), "Interest Coverage >= 2.00x");

    let custom = Covenant::new(
        CovenantType::Custom {
            metric: "DSCR".to_string(),
            test: ThresholdTest::Minimum(1.2),
        },
        Tenor::quarterly(),
    );
    assert_eq!(custom.description(), "DSCR >= 1.20");
}

// =============================================================================
// Covenant Type Pass/Fail Logic Tests
// =============================================================================

#[test]
fn max_covenant_type_pass_fail_logic() {
    // MaxDebtToEBITDA: passes when actual <= threshold
    let threshold = 5.0;

    // Pass case: actual (4.5) <= threshold (5.0)
    let actual_pass = 4.5;
    assert!(
        actual_pass <= threshold,
        "Should pass when actual <= threshold"
    );

    // Fail case: actual (5.5) > threshold (5.0)
    let actual_fail = 5.5;
    assert!(
        actual_fail > threshold,
        "Should fail when actual > threshold"
    );

    // Edge case: actual == threshold (should pass)
    let actual_edge = 5.0;
    assert!(
        actual_edge <= threshold,
        "Should pass when actual == threshold"
    );
}

#[test]
fn min_covenant_type_pass_fail_logic() {
    // MinInterestCoverage: passes when actual >= threshold
    let threshold = 1.5;

    // Pass case: actual (2.0) >= threshold (1.5)
    let actual_pass = 2.0;
    assert!(
        actual_pass >= threshold,
        "Should pass when actual >= threshold"
    );

    // Fail case: actual (1.2) < threshold (1.5)
    let actual_fail = 1.2;
    assert!(
        actual_fail < threshold,
        "Should fail when actual < threshold"
    );

    // Edge case: actual == threshold (should pass)
    let actual_edge = 1.5;
    assert!(
        actual_edge >= threshold,
        "Should pass when actual == threshold"
    );
}

// =============================================================================
// Covenant Consequence Tests
// =============================================================================

#[test]
fn covenant_with_multiple_consequences() {
    use finstack_quant_covenants::CovenantConsequence;

    let covenant = Covenant::new(
        CovenantType::MaxDebtToEBITDA { threshold: 5.0 },
        Tenor::quarterly(),
    )
    .with_consequence(CovenantConsequence::RateIncrease { bp_increase: 100.0 })
    .with_consequence(CovenantConsequence::BlockDistributions)
    .with_cure_period(Some(30));

    assert_eq!(covenant.consequences.len(), 2);
    assert_eq!(covenant.cure_period_days, Some(30));
}

// =============================================================================
// Covenant Scope Tests
// =============================================================================

#[test]
fn covenant_scope_maintenance_vs_incurrence() {
    let maintenance = Covenant::new(
        CovenantType::MaxDebtToEBITDA { threshold: 5.0 },
        Tenor::quarterly(),
    )
    .with_scope(CovenantScope::Maintenance);

    assert_eq!(maintenance.scope, CovenantScope::Maintenance);

    let incurrence = Covenant::new(
        CovenantType::MaxTotalLeverage { threshold: 6.0 },
        Tenor::annual(),
    )
    .with_scope(CovenantScope::Incurrence);

    assert_eq!(incurrence.scope, CovenantScope::Incurrence);
}

// =============================================================================
// Basket Covenant Tests
// =============================================================================

#[test]
fn basket_covenant_utilization() {
    let basket = Covenant::new(
        CovenantType::Basket {
            name: "permitted_investments".to_string(),
            limit: 50_000_000.0,
        },
        Tenor::quarterly(),
    );

    assert_eq!(
        basket.description(),
        "permitted_investments Utilization <= 50000000.00"
    );

    // Test utilization calculation
    let limit: f64 = 50_000_000.0;
    let used: f64 = 35_000_000.0;
    let utilization: f64 = used / limit;
    let available: f64 = limit - used;

    assert!((utilization - 0.70).abs() < 0.001, "70% utilized");
    assert!((available - 15_000_000.0).abs() < 1.0, "15M available");
}

// =============================================================================
// Threshold Test Variants
// =============================================================================

#[test]
fn threshold_test_maximum() {
    let test = ThresholdTest::Maximum(5.0);
    match test {
        ThresholdTest::Maximum(v) => assert_eq!(v, 5.0),
        ThresholdTest::Minimum(_) => panic!("Expected Maximum variant"),
    }
}

#[test]
fn threshold_test_minimum() {
    let test = ThresholdTest::Minimum(1.5);
    match test {
        ThresholdTest::Minimum(v) => assert_eq!(v, 1.5),
        ThresholdTest::Maximum(_) => panic!("Expected Minimum variant"),
    }
}
