//! Integration tests for covenant report construction, engine specs, and
//! instance-label identity.

use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_covenants::{
    Covenant, CovenantEngine, CovenantMetricId, CovenantReport, CovenantScope, CovenantSpec,
    CovenantType, HashMapMetricSource, ThresholdTest,
};

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

#[test]
fn covenant_engine_add_specs() {
    let mut engine = CovenantEngine::new();
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 5.0 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        ),
        CovenantMetricId::from("debt_to_ebitda"),
    ));

    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MinInterestCoverage { threshold: 1.5 },
            Tenor::quarterly(),
            "min_interest_coverage",
        ),
        CovenantMetricId::from("interest_coverage"),
    ));

    assert_eq!(engine.specs.len(), 2);
}

#[test]
fn same_type_covenants_with_distinct_labels_do_not_collide() {
    let mut engine = CovenantEngine::new();
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.0 },
            Tenor::quarterly(),
            "senior_leverage",
        ),
        CovenantMetricId::from("debt_to_ebitda"),
    ));
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.0 },
            Tenor::quarterly(),
            "total_leverage",
        ),
        CovenantMetricId::from("debt_to_ebitda"),
    ));

    let mut metrics = HashMapMetricSource::new();
    metrics.insert("debt_to_ebitda", 5.0);

    let test_date = Date::from_calendar_date(2025, time::Month::March, 31).unwrap();
    let reports = engine.evaluate(&metrics, test_date).expect("evaluate");

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
        CovenantType::MaxDebtToEbitda { threshold: 4.5 },
        Tenor::quarterly(),
        "max_debt_ebitda",
    );
    assert_eq!(leverage.description(), "Debt/EBITDA <= 4.50x");

    let coverage = Covenant::new(
        CovenantType::MinInterestCoverage { threshold: 2.0 },
        Tenor::quarterly(),
        "min_interest_coverage",
    );
    assert_eq!(coverage.description(), "Interest Coverage >= 2.00x");

    let custom = Covenant::new(
        CovenantType::Custom {
            metric: "DSCR".to_string(),
            test: ThresholdTest::Minimum(1.2),
        },
        Tenor::quarterly(),
        "custom",
    );
    assert_eq!(custom.description(), "DSCR >= 1.20");
}

#[test]
fn covenant_with_multiple_consequences() {
    use finstack_quant_covenants::CovenantConsequence;

    let covenant = Covenant::new(
        CovenantType::MaxDebtToEbitda { threshold: 5.0 },
        Tenor::quarterly(),
        "max_debt_ebitda",
    )
    .with_consequence(CovenantConsequence::RateIncrease { bp_increase: 100.0 })
    .with_consequence(CovenantConsequence::BlockDistributions)
    .with_cure_period(Some(30));

    assert_eq!(covenant.consequences.len(), 2);
    assert_eq!(covenant.cure_period_days, Some(30));
}

#[test]
fn covenant_scope_maintenance_vs_incurrence() {
    let maintenance = Covenant::new(
        CovenantType::MaxDebtToEbitda { threshold: 5.0 },
        Tenor::quarterly(),
        "max_debt_ebitda",
    )
    .with_scope(CovenantScope::Maintenance);

    assert_eq!(maintenance.scope, CovenantScope::Maintenance);

    let incurrence = Covenant::new(
        CovenantType::MaxTotalLeverage { threshold: 6.0 },
        Tenor::annual(),
        "max_total_leverage",
    )
    .with_scope(CovenantScope::Incurrence);

    assert_eq!(incurrence.scope, CovenantScope::Incurrence);
}

#[test]
fn basket_covenant_utilization() {
    let basket = Covenant::new(
        CovenantType::Basket {
            name: "permitted_investments".to_string(),
            limit: 50_000_000.0,
        },
        Tenor::quarterly(),
        "basket",
    );

    assert_eq!(
        basket.description(),
        "permitted_investments Utilization <= 50000000.00"
    );
}
