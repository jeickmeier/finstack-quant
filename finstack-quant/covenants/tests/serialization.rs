//! Roundtrip serialization tests for covenant types.
//!
//! Ensures all covenant types serialize to JSON and deserialize back correctly.

use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_covenants::{
    validate_covenant_engine_json, validate_covenant_report_json, BoundKind,
    ConsequenceApplication, Covenant, CovenantBreach, CovenantConsequence, CovenantEngine,
    CovenantForecast, CovenantForecastConfig, CovenantMetricId, CovenantReport, CovenantScope,
    CovenantSpec, CovenantTestSpec, CovenantType, CovenantWindow, FutureBreach, SpringingCondition,
    ThresholdSchedule, ThresholdTest,
};
use time::Month;

fn date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(
        year,
        Month::try_from(month).expect("Valid month (1-12)"),
        day,
    )
    .expect("Valid test date")
}

/// Helper to perform JSON roundtrip and assert equality.
fn roundtrip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string_pretty(value).expect("serialize to JSON");
    serde_json::from_str(&json).expect("deserialize from JSON")
}

// ============================================================================
// mod_types.rs
// ============================================================================

#[test]
fn covenant_report_roundtrip() {
    let report = CovenantReport::failed("Debt/EBITDA <= 4.00")
        .with_actual(5.2)
        .with_threshold(4.0)
        .with_details("Exceeded by 1.2x")
        .with_headroom(-0.30);

    let rt = roundtrip(&report);
    assert_eq!(report, rt);
}

#[test]
fn covenant_report_passed_roundtrip() {
    let report = CovenantReport::passed("Interest Coverage >= 1.50x")
        .with_actual(2.0)
        .with_threshold(1.5)
        .with_headroom(0.33);

    let rt = roundtrip(&report);
    assert_eq!(report, rt);
}

// ============================================================================
// engine.rs
// ============================================================================

#[test]
fn covenant_scope_roundtrip() {
    let maintenance = CovenantScope::Maintenance;
    let incurrence = CovenantScope::Incurrence;

    assert_eq!(maintenance, roundtrip(&maintenance));
    assert_eq!(incurrence, roundtrip(&incurrence));
}

#[test]
fn threshold_test_roundtrip() {
    let max = ThresholdTest::Maximum(5.0);
    let min = ThresholdTest::Minimum(1.5);

    assert_eq!(max, roundtrip(&max));
    assert_eq!(min, roundtrip(&min));
}

#[test]
fn covenant_type_all_variants_roundtrip() {
    let variants = vec![
        CovenantType::MaxDebtToEBITDA { threshold: 4.5 },
        CovenantType::MinInterestCoverage { threshold: 1.5 },
        CovenantType::MinFixedChargeCoverage { threshold: 1.25 },
        CovenantType::MaxTotalLeverage { threshold: 6.0 },
        CovenantType::MaxSeniorLeverage { threshold: 3.5 },
        CovenantType::MinAssetCoverage { threshold: 1.1 },
        CovenantType::Negative {
            restriction: "No additional secured debt".to_string(),
        },
        CovenantType::Affirmative {
            requirement: "Maintain insurance coverage".to_string(),
        },
        CovenantType::Custom {
            metric: "custom_liquidity_ratio".to_string(),
            test: ThresholdTest::Minimum(1.0),
        },
        CovenantType::Basket {
            name: "general_debt_basket".to_string(),
            limit: 100_000_000.0,
        },
    ];

    for variant in variants {
        let rt = roundtrip(&variant);
        assert_eq!(variant, rt, "Failed for variant: {:?}", variant);
    }
}

#[test]
fn covenant_consequence_all_variants_roundtrip() {
    let variants = vec![
        CovenantConsequence::Default,
        CovenantConsequence::RateIncrease { bp_increase: 150.0 },
        CovenantConsequence::CashSweep {
            sweep_percentage: 0.75,
        },
        CovenantConsequence::BlockDistributions,
        CovenantConsequence::RequireCollateral {
            description: "Pledge additional real estate".to_string(),
        },
        CovenantConsequence::AccelerateMaturity {
            new_maturity: date(2025, 6, 30),
        },
    ];

    for variant in variants {
        let rt = roundtrip(&variant);
        assert_eq!(variant, rt, "Failed for variant: {:?}", variant);
    }
}

#[test]
fn springing_condition_roundtrip() {
    let condition = SpringingCondition {
        metric_id: CovenantMetricId::from("revolver_utilization"),
        test: ThresholdTest::Minimum(0.35),
    };

    let rt = roundtrip(&condition);
    assert_eq!(condition, rt);
}

#[test]
fn covenant_roundtrip() {
    let covenant = Covenant::new(
        CovenantType::MaxDebtToEBITDA { threshold: 5.0 },
        Tenor::quarterly(),
    )
    .with_cure_period(Some(30))
    .with_consequence(CovenantConsequence::RateIncrease { bp_increase: 100.0 })
    .with_consequence(CovenantConsequence::BlockDistributions)
    .with_scope(CovenantScope::Maintenance)
    .with_springing_condition(SpringingCondition {
        metric_id: CovenantMetricId::from("utilization"),
        test: ThresholdTest::Minimum(0.5),
    });

    let rt = roundtrip(&covenant);
    assert_eq!(covenant, rt);
}

#[test]
fn covenant_spec_roundtrip() {
    let spec = CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MinInterestCoverage { threshold: 1.5 },
            Tenor::quarterly(),
        ),
        CovenantMetricId::from("interest_coverage"),
    );

    let rt = roundtrip(&spec);
    // Note: custom_evaluator is skipped during serialization
    assert_eq!(spec.covenant, rt.covenant);
    assert_eq!(spec.metric_id, rt.metric_id);
    assert!(rt.custom_evaluator.is_none());
}

#[test]
fn covenant_test_spec_roundtrip() {
    let test_spec = CovenantTestSpec {
        specs: vec![
            CovenantSpec::with_metric(
                Covenant::new(
                    CovenantType::MaxTotalLeverage { threshold: 5.0 },
                    Tenor::quarterly(),
                ),
                CovenantMetricId::from("total_leverage"),
            ),
            CovenantSpec::with_metric(
                Covenant::new(
                    CovenantType::MinInterestCoverage { threshold: 1.5 },
                    Tenor::quarterly(),
                ),
                CovenantMetricId::from("interest_coverage"),
            ),
        ],
        test_date: date(2025, 3, 31),
        reference_date: Some(date(2025, 1, 1)),
    };

    let rt = roundtrip(&test_spec);
    assert_eq!(test_spec.test_date, rt.test_date);
    assert_eq!(test_spec.reference_date, rt.reference_date);
    assert_eq!(test_spec.specs.len(), rt.specs.len());
}

#[test]
fn covenant_window_roundtrip() {
    let window = CovenantWindow {
        start: date(2025, 1, 1),
        end: date(2025, 6, 30),
        covenants: vec![CovenantSpec::with_metric(
            Covenant::new(
                CovenantType::Custom {
                    metric: "liquidity".to_string(),
                    test: ThresholdTest::Minimum(1.0),
                },
                Tenor::quarterly(),
            ),
            CovenantMetricId::from("liquidity"),
        )],
    };

    let rt = roundtrip(&window);
    assert_eq!(window.start, rt.start);
    assert_eq!(window.end, rt.end);
    assert_eq!(window.covenants.len(), rt.covenants.len());
}

#[test]
fn covenant_breach_roundtrip() {
    let breach = CovenantBreach {
        covenant_id: "max_debt_ebitda".to_string(),
        covenant_type: "Debt/EBITDA <= 5.00".to_string(),
        breach_date: date(2025, 3, 31),
        actual_value: Some(5.5),
        threshold: Some(5.0),
        cure_deadline: Some(date(2025, 4, 30)),
        is_cured: false,
        applied_consequences: vec![
            CovenantConsequence::RateIncrease { bp_increase: 100.0 },
            CovenantConsequence::BlockDistributions,
        ],
    };

    let rt = roundtrip(&breach);
    assert_eq!(breach, rt);
}

#[test]
fn covenant_engine_roundtrip() {
    let mut engine = CovenantEngine::new();

    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEBITDA { threshold: 5.0 },
            Tenor::quarterly(),
        )
        .with_consequence(CovenantConsequence::Default),
        CovenantMetricId::from("debt_to_ebitda"),
    ));

    engine.add_window(CovenantWindow {
        start: date(2025, 1, 1),
        end: date(2025, 12, 31),
        covenants: vec![CovenantSpec::with_metric(
            Covenant::new(
                CovenantType::MinInterestCoverage { threshold: 1.25 },
                Tenor::quarterly(),
            ),
            CovenantMetricId::from("interest_coverage"),
        )],
    });

    engine.breach_history.push(CovenantBreach {
        covenant_id: "max_debt_ebitda".to_string(),
        covenant_type: "Test Breach".to_string(),
        breach_date: date(2025, 2, 28),
        actual_value: Some(6.0),
        threshold: Some(5.0),
        cure_deadline: None,
        is_cured: true,
        applied_consequences: vec![],
    });

    let rt = roundtrip(&engine);
    // Note: custom_metrics is skipped during serialization
    assert_eq!(engine.specs.len(), rt.specs.len());
    assert_eq!(engine.windows.len(), rt.windows.len());
    assert_eq!(engine.breach_history.len(), rt.breach_history.len());
    assert_eq!(engine.breach_history[0], rt.breach_history[0]);
    assert!(rt.custom_metrics.is_empty());
}

#[test]
fn validate_engine_json_rejects_unknown_top_level_and_nested_fields() {
    let top_level_typo = serde_json::json!({
        "specs": [],
        "breach_history": [],
        "windows": [],
        "waviers": []
    })
    .to_string();
    assert!(validate_covenant_engine_json(&top_level_typo).is_err());

    let waiver_typo = serde_json::json!({
        "specs": [],
        "breach_history": [],
        "windows": [],
        "waivers": [{
            "covenant_id": "max_debt_ebitda",
            "effective_date": "2025-01-01",
            "expiry_dat": "2025-03-31",
            "amended_threshold": null,
            "description": "typo should fail"
        }]
    })
    .to_string();
    assert!(validate_covenant_engine_json(&waiver_typo).is_err());
}

#[test]
fn validate_report_json_rejects_unknown_fields() {
    let report = serde_json::json!({
        "covenant_type": "Debt/EBITDA <= 5.00x",
        "passed": true,
        "surprise": true
    })
    .to_string();
    assert!(validate_covenant_report_json(&report).is_err());
}

#[test]
fn consequence_application_roundtrip() {
    let application = ConsequenceApplication {
        consequence_type: "Rate Increase".to_string(),
        applied_date: date(2025, 5, 1),
        details: "Rate increased by 150 bps".to_string(),
    };

    let rt = roundtrip(&application);
    assert_eq!(application, rt);
}

// ============================================================================
// forward.rs
// ============================================================================

#[test]
fn bound_kind_roundtrip() {
    let at_most = BoundKind::AtMost;
    let at_least = BoundKind::AtLeast;

    assert_eq!(at_most, roundtrip(&at_most));
    assert_eq!(at_least, roundtrip(&at_least));
}

#[test]
fn covenant_forecast_config_roundtrip() {
    let config = CovenantForecastConfig {
        stochastic: true,
        num_paths: 10_000,
        volatility: Some(0.25),
        random_seed: Some(42),
        antithetic: true,
        reference_date: Some(date(2025, 1, 1)),
        breach_probability_threshold: 0.05,
    };

    let rt = roundtrip(&config);
    assert_eq!(config, rt);
}

#[test]
fn covenant_forecast_config_default_roundtrip() {
    let config = CovenantForecastConfig::default();
    let rt = roundtrip(&config);
    assert_eq!(config, rt);
}

#[test]
fn covenant_forecast_roundtrip() {
    let forecast = CovenantForecast {
        covenant_id: "Debt/EBITDA <= 5.00x".to_string(),
        covenant_description: "Debt/EBITDA <= 5.00x".to_string(),
        comparator: BoundKind::AtMost,
        test_dates: vec![date(2025, 3, 31), date(2025, 6, 30), date(2025, 9, 30)],
        projected_values: vec![Some(4.2), Some(4.5), Some(4.8)],
        thresholds: vec![5.0, 5.0, 5.0],
        headroom: vec![Some(0.16), Some(0.10), Some(0.04)],
        breach_probability: vec![0.0, 0.0, 0.0],
        breach_probability_stderr: vec![0.0, 0.0, 0.0],
        first_breach_date: None,
        min_headroom_date: Some(date(2025, 9, 30)),
        min_headroom_value: Some(0.04),
    };

    let rt = roundtrip(&forecast);
    assert_eq!(forecast, rt);
}

#[test]
fn covenant_forecast_with_breach_roundtrip() {
    let forecast = CovenantForecast {
        covenant_id: "Interest Coverage >= 1.50x".to_string(),
        covenant_description: "Interest Coverage >= 1.50x".to_string(),
        comparator: BoundKind::AtLeast,
        test_dates: vec![date(2025, 3, 31), date(2025, 6, 30)],
        projected_values: vec![Some(1.6), Some(1.3)],
        thresholds: vec![1.5, 1.5],
        headroom: vec![Some(0.067), Some(-0.133)],
        breach_probability: vec![0.05, 0.75],
        breach_probability_stderr: vec![0.0, 0.0],
        first_breach_date: Some(date(2025, 6, 30)),
        min_headroom_date: Some(date(2025, 6, 30)),
        min_headroom_value: Some(-0.133),
    };

    let rt = roundtrip(&forecast);
    assert_eq!(forecast, rt);
}

#[test]
fn covenant_forecast_nullable_values_roundtrip_as_json_null() {
    let forecast = CovenantForecast {
        covenant_id: "max_debt_ebitda".to_string(),
        covenant_description: "Debt/EBITDA <= 4.00x".to_string(),
        comparator: BoundKind::AtMost,
        test_dates: vec![date(2025, 3, 31)],
        projected_values: vec![None],
        thresholds: vec![4.0],
        headroom: vec![None],
        breach_probability: vec![1.0],
        breach_probability_stderr: vec![0.0],
        first_breach_date: Some(date(2025, 3, 31)),
        min_headroom_date: None,
        min_headroom_value: None,
    };

    let json = serde_json::to_string(&forecast).expect("serialize forecast");
    assert!(json.contains("\"projected_values\":[null]"));
    assert!(json.contains("\"headroom\":[null]"));
    let restored: CovenantForecast = serde_json::from_str(&json).expect("round-trip forecast");
    assert_eq!(restored, forecast);
}

#[test]
fn future_breach_roundtrip() {
    let breach = FutureBreach {
        covenant_id: "Senior Leverage <= 3.00x".to_string(),
        covenant_description: "Senior Leverage <= 3.00x".to_string(),
        breach_date: date(2025, 9, 30),
        projected_value: Some(3.5),
        threshold: 3.0,
        headroom: Some(-0.167),
        breach_probability: 0.85,
    };

    let rt = roundtrip(&breach);
    assert_eq!(breach, rt);
}

// ============================================================================
// schedule.rs
// ============================================================================

#[test]
fn threshold_schedule_roundtrip() {
    let schedule = ThresholdSchedule::new(vec![
        (date(2025, 1, 1), 5.0),
        (date(2025, 7, 1), 4.75),
        (date(2026, 1, 1), 4.5),
        (date(2026, 7, 1), 4.25),
    ]);

    let rt = roundtrip(&schedule);
    assert_eq!(schedule, rt);
}

#[test]
fn threshold_schedule_empty_roundtrip() {
    let schedule = ThresholdSchedule::new(vec![]);
    let rt = roundtrip(&schedule);
    assert_eq!(schedule, rt);
}

#[test]
fn threshold_schedule_is_empty_helper() {
    let empty = ThresholdSchedule::new(vec![]);
    assert!(empty.is_empty());

    let populated = ThresholdSchedule::new(vec![(date(2025, 1, 1), 5.0)]);
    assert!(!populated.is_empty());
}

// ============================================================================
// Complex nested structures
// ============================================================================

#[test]
fn complex_covenant_package_roundtrip() {
    // Simulates a full credit agreement covenant package
    let mut engine = CovenantEngine::new();

    // Leverage covenant with step-downs implied via multiple specs
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEBITDA { threshold: 5.0 },
            Tenor::quarterly(),
        )
        .with_cure_period(Some(30))
        .with_consequence(CovenantConsequence::Default)
        .with_scope(CovenantScope::Maintenance),
        CovenantMetricId::from("debt_to_ebitda"),
    ));

    // Interest coverage covenant
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MinInterestCoverage { threshold: 1.5 },
            Tenor::quarterly(),
        )
        .with_consequence(CovenantConsequence::RateIncrease { bp_increase: 100.0 })
        .with_consequence(CovenantConsequence::BlockDistributions),
        CovenantMetricId::from("interest_coverage"),
    ));

    // Springing covenant based on revolver utilization
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxSeniorLeverage { threshold: 3.0 },
            Tenor::quarterly(),
        )
        .with_springing_condition(SpringingCondition {
            metric_id: CovenantMetricId::from("revolver_utilization"),
            test: ThresholdTest::Minimum(0.35),
        }),
        CovenantMetricId::from("senior_leverage"),
    ));

    // Negative covenant
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::Negative {
                restriction: "No additional secured debt without consent".to_string(),
            },
            Tenor::annual(),
        )
        .with_scope(CovenantScope::Incurrence),
        CovenantMetricId::from("negative_debt_incurrence"),
    ));

    // Basket covenant
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::Basket {
                name: "permitted_investments".to_string(),
                limit: 50_000_000.0,
            },
            Tenor::quarterly(),
        ),
        CovenantMetricId::from("permitted_investments"),
    ));

    // Add some breach history
    engine.breach_history.push(CovenantBreach {
        covenant_id: "max_debt_ebitda".to_string(),
        covenant_type: "Debt/EBITDA <= 5.00".to_string(),
        breach_date: date(2024, 12, 31),
        actual_value: Some(5.2),
        threshold: Some(5.0),
        cure_deadline: Some(date(2025, 1, 30)),
        is_cured: true,
        applied_consequences: vec![],
    });

    let rt = roundtrip(&engine);

    assert_eq!(engine.specs.len(), rt.specs.len());
    assert_eq!(engine.breach_history.len(), rt.breach_history.len());
    assert_eq!(engine.breach_history[0], rt.breach_history[0]);

    // Verify each spec roundtripped correctly
    for (original, restored) in engine.specs.iter().zip(rt.specs.iter()) {
        assert_eq!(original.covenant, restored.covenant);
        assert_eq!(original.metric_id, restored.metric_id);
    }
}

#[test]
fn json_format_stability() {
    // Verify that the JSON format is human-readable and stable
    let breach = CovenantBreach {
        covenant_id: "max_total_leverage".to_string(),
        covenant_type: "Max Leverage".to_string(),
        breach_date: date(2025, 6, 30),
        actual_value: Some(5.5),
        threshold: Some(5.0),
        cure_deadline: Some(date(2025, 7, 30)),
        is_cured: false,
        applied_consequences: vec![CovenantConsequence::RateIncrease { bp_increase: 50.0 }],
    };

    let json = serde_json::to_string_pretty(&breach).expect("serialize");

    // Verify key fields are present in expected format
    assert!(json.contains("\"covenant_type\": \"Max Leverage\""));
    assert!(json.contains("\"breach_date\""));
    assert!(json.contains("\"actual_value\": 5.5"));
    assert!(json.contains("\"threshold\": 5.0"));
    assert!(json.contains("\"is_cured\": false"));
    assert!(json.contains("\"RateIncrease\""));
    assert!(json.contains("\"bp_increase\": 50.0"));
}
