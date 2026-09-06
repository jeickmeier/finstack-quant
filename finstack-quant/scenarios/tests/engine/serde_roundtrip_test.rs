//! Tests for JSON serialization stability.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::{ContractError, LoadLimits};
use finstack_quant_scenarios::engine::ScenarioMarketTarget;
use finstack_quant_scenarios::{
    CurveKind, OperationSpec, ScenarioEnvelope, ScenarioSpec, TenorMatchMode, TimeRollMode,
};
use indexmap::IndexMap;

#[test]
fn test_scenario_json_roundtrip() {
    let scenario = ScenarioSpec {
        id: "test_scenario".into(),
        name: Some("Test Scenario".into()),
        description: Some("For JSON testing".into()),
        operations: vec![
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD_SOFR".into(),
                discount_curve_id: None,
                bp: 50.0,
            },
            OperationSpec::EquityPricePct {
                ids: vec!["SPY".into()],
                pct: -10.0,
            },
            OperationSpec::MarketFxPct {
                base: Currency::EUR,
                quote: Currency::USD,
                pct: 5.0,
            },
        ],
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&scenario).unwrap();
    println!("Serialized scenario:\n{}", json);

    // Deserialize back
    let deserialized: ScenarioSpec = serde_json::from_str(&json).unwrap();

    // Verify equality
    assert_eq!(scenario.id, deserialized.id);
    assert_eq!(scenario.name, deserialized.name);
    assert_eq!(scenario.operations.len(), deserialized.operations.len());
    assert_eq!(scenario.priority, deserialized.priority);
}

#[test]
fn scenario_envelope_strict_loader_enforces_schema_and_semantics() {
    let scenario = ScenarioSpec {
        id: "strict".into(),
        name: None,
        description: None,
        operations: Vec::new(),
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };
    let envelope = ScenarioEnvelope::new(scenario);
    let bytes = serde_json::to_vec(&envelope).expect("serialize envelope");
    let (loaded, report) =
        ScenarioEnvelope::from_slice_strict(&bytes, &LoadLimits::default()).expect("valid");
    assert_eq!(loaded.id, "strict");
    assert!(report.diagnostics.is_empty());

    let bare = serde_json::json!({
        "id": "bare",
        "operations": [],
        "priority": 0,
        "resolution_mode": "most_specific_wins"
    });
    let error = ScenarioEnvelope::from_slice_strict(
        &serde_json::to_vec(&bare).expect("serialize bare"),
        &LoadLimits::default(),
    )
    .expect_err("missing schema must fail");
    let ContractError::Report(report) = error else {
        panic!("expected structured report");
    };
    assert_eq!(report.diagnostics[0].code, "contract/version-missing");

    for schema in [
        "finstack_quant.scenario/0",
        "finstack_quant.scenario/2",
        "finstack_quant.scenario/not-a-version",
    ] {
        let value = serde_json::json!({
            "schema": schema,
            "scenario": {
                "id": "strict",
                "operations": [],
                "priority": 0,
                "resolution_mode": "most_specific_wins"
            }
        });
        assert!(
            ScenarioEnvelope::from_slice_strict(
                &serde_json::to_vec(&value).expect("fixture"),
                &LoadLimits::default(),
            )
            .is_err(),
            "{schema} must fail"
        );
    }

    let invalid = serde_json::json!({
        "schema": "finstack_quant.scenario/1",
        "scenario": {
            "id": "",
            "operations": [],
            "priority": 0,
            "resolution_mode": "most_specific_wins"
        }
    });
    assert!(
        ScenarioEnvelope::from_slice_strict(
            &serde_json::to_vec(&invalid).expect("fixture"),
            &LoadLimits::default(),
        )
        .is_err(),
        "semantic validation must run"
    );
}

#[test]
fn test_all_operation_types_serialize() {
    let operations = vec![
        OperationSpec::MarketFxPct {
            base: Currency::EUR,
            quote: Currency::USD,
            pct: 5.0,
        },
        OperationSpec::EquityPricePct {
            ids: vec!["SPY".into()],
            pct: -10.0,
        },
        OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD_SOFR".into(),
            discount_curve_id: None,
            bp: 50.0,
        },
        OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Forward,
            curve_id: "USD_LIBOR".into(),
            discount_curve_id: None,
            nodes: vec![("1Y".into(), 25.0), ("5Y".into(), -10.0)],
            match_mode: TenorMatchMode::Interpolate,
        },
        OperationSpec::BaseCorrParallelPts {
            surface_id: "CDX".into(),
            points: 0.05,
        },
        OperationSpec::VolSurfaceParallelPct {
            vol_surface_id: "SPX".into(),
            pct: 20.0,
        },
        OperationSpec::StmtForecastPercent {
            node_id: "Revenue".into(),
            pct: -5.0,
        },
        OperationSpec::StmtForecastAssign {
            node_id: "Cost".into(),
            value: 100_000.0,
        },
    ];

    let scenario = ScenarioSpec {
        id: "all_ops".into(),
        name: None,
        description: None,
        operations,
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };

    // Roundtrip
    let json = serde_json::to_string(&scenario).unwrap();
    let deserialized: ScenarioSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(scenario.operations.len(), deserialized.operations.len());
}

#[test]
fn test_reject_unknown_fields() {
    let json = r#"{
        "id": "test",
        "operations": [],
        "priority": 0,
        "unknown_field": "should_fail"
    }"#;

    let result = serde_json::from_str::<ScenarioSpec>(json);
    assert!(result.is_err(), "Should reject unknown fields");
}

#[test]
fn test_attribute_selector_serde() {
    let mut attrs = IndexMap::new();
    attrs.insert("sector".into(), "Energy".into());
    attrs.insert("rating".into(), "BBB".into());

    let op = OperationSpec::InstrumentPricePctByAttr { attrs, pct: -5.0 };

    let scenario = ScenarioSpec {
        id: "attr_test".into(),
        name: None,
        description: None,
        operations: vec![op],
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };

    let json = serde_json::to_string_pretty(&scenario).unwrap();
    let deserialized: ScenarioSpec = serde_json::from_str(&json).unwrap();

    match &deserialized.operations[0] {
        OperationSpec::InstrumentPricePctByAttr { attrs, pct } => {
            assert_eq!(attrs.len(), 2);
            assert_eq!(attrs.get("sector").unwrap(), "Energy");
            assert_eq!(*pct, -5.0);
        }
        _ => panic!("Wrong operation type"),
    }
}

#[test]
fn test_time_roll_forward_default_apply_shocks() {
    let op = OperationSpec::TimeRollForward {
        period: "1M".into(),
        apply_shocks: true,
        roll_mode: TimeRollMode::BusinessDays,
    };

    let json = serde_json::to_string(&op).unwrap();
    let deserialized: OperationSpec = serde_json::from_str(&json).unwrap();

    match deserialized {
        OperationSpec::TimeRollForward {
            period,
            apply_shocks,
            roll_mode,
        } => {
            assert_eq!(period, "1M");
            assert!(apply_shocks);
            assert_eq!(roll_mode, TimeRollMode::BusinessDays);
        }
        _ => panic!("Wrong operation type"),
    }
}

#[test]
fn test_time_roll_forward_apply_shocks_false() {
    let op = OperationSpec::TimeRollForward {
        period: "1W".into(),
        apply_shocks: false,
        roll_mode: TimeRollMode::BusinessDays,
    };

    let json = serde_json::to_string(&op).unwrap();
    let deserialized: OperationSpec = serde_json::from_str(&json).unwrap();

    match deserialized {
        OperationSpec::TimeRollForward {
            period,
            apply_shocks,
            roll_mode,
        } => {
            assert_eq!(period, "1W");
            assert!(!apply_shocks);
            assert_eq!(roll_mode, TimeRollMode::BusinessDays);
        }
        _ => panic!("Wrong operation type"),
    }
}

#[test]
fn test_instrument_type_operations_serde() {
    use finstack_quant_valuations::pricer::InstrumentType;

    let ops = vec![
        OperationSpec::InstrumentPricePctByType {
            instrument_types: vec![InstrumentType::Bond, InstrumentType::Cds],
            pct: -5.0,
        },
        OperationSpec::InstrumentSpreadBpByType {
            instrument_types: vec![InstrumentType::TermLoan],
            bp: 100.0,
        },
    ];

    let scenario = ScenarioSpec {
        id: "inst_types".into(),
        name: None,
        description: None,
        operations: ops,
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };

    let json = serde_json::to_string_pretty(&scenario).unwrap();
    let deserialized: ScenarioSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.operations.len(), 2);
}

#[test]
fn test_tenor_match_mode_default() {
    let op = OperationSpec::CurveNodeBp {
        curve_kind: CurveKind::Discount,
        curve_id: "USD_SOFR".into(),
        discount_curve_id: None,
        nodes: vec![("5Y".into(), 50.0)],
        match_mode: TenorMatchMode::Interpolate,
    };

    let json = serde_json::to_string(&op).unwrap();
    let deserialized: OperationSpec = serde_json::from_str(&json).unwrap();

    match deserialized {
        OperationSpec::CurveNodeBp { match_mode, .. } => {
            assert_eq!(match_mode, TenorMatchMode::Interpolate);
        }
        _ => panic!("Wrong operation type"),
    }
}

#[test]
fn test_optional_fields_serialize() {
    let scenario = ScenarioSpec {
        id: "test".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::BaseCorrBucketPts {
                surface_id: "CDX".into(),
                detachment_bp: None,
                points: 0.05,
            },
            OperationSpec::VolSurfaceBucketPct {
                vol_surface_id: "SPX".into(),
                tenors: None,
                strikes: Some(vec![100.0, 110.0]),
                pct: 10.0,
            },
        ],
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };

    let json = serde_json::to_string_pretty(&scenario).unwrap();
    let deserialized: ScenarioSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.operations.len(), 2);
}

#[test]
fn base_corr_bucket_rejects_removed_maturities_field() {
    let legacy = serde_json::json!({
        "kind": "base_corr_bucket_pts",
        "surface_id": "CDX",
        "detachment_bp": [300, 700],
        "maturities": ["5Y"],
        "points": 0.01,
    });

    assert!(serde_json::from_value::<OperationSpec>(legacy).is_err());
}

#[test]
fn vol_surface_contracts_reject_removed_surface_kind() {
    let legacy_operations = [
        serde_json::json!({
            "kind": "vol_surface_parallel_pct",
            "surface_kind": "equity",
            "vol_surface_id": "SPX",
            "pct": 10.0,
        }),
        serde_json::json!({
            "kind": "vol_surface_bucket_pct",
            "surface_kind": "credit",
            "vol_surface_id": "CDX-VOL",
            "tenors": ["5Y"],
            "strikes": null,
            "pct": 10.0,
        }),
        serde_json::json!({
            "kind": "hierarchy_vol_surface_parallel_pct",
            "surface_kind": "swaption",
            "target": {"path": ["Rates"]},
            "pct": 10.0,
        }),
    ];

    for legacy in legacy_operations {
        let mut canonical = legacy.clone();
        canonical
            .as_object_mut()
            .expect("operation JSON object")
            .remove("surface_kind");
        assert!(serde_json::from_value::<OperationSpec>(canonical).is_ok());
        assert!(serde_json::from_value::<OperationSpec>(legacy).is_err());
    }

    let legacy_target = serde_json::json!({
        "kind": "vol_surface",
        "surface_kind": "equity",
        "vol_surface_id": "SPX",
    });
    let mut canonical_target = legacy_target.clone();
    canonical_target
        .as_object_mut()
        .expect("market target JSON object")
        .remove("surface_kind");
    assert!(serde_json::from_value::<ScenarioMarketTarget>(canonical_target).is_ok());
    assert!(serde_json::from_value::<ScenarioMarketTarget>(legacy_target).is_err());
}

#[test]
fn test_scenario_with_metadata() {
    let scenario = ScenarioSpec {
        id: "full_metadata".into(),
        name: Some("Full Scenario Name".into()),
        description: Some("This is a comprehensive test scenario".into()),
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["SPY".into()],
            pct: -10.0,
        }],
        priority: 5,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };

    let json = serde_json::to_string_pretty(&scenario).unwrap();
    let deserialized: ScenarioSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, "full_metadata");
    assert_eq!(deserialized.name, Some("Full Scenario Name".into()));
    assert_eq!(
        deserialized.description,
        Some("This is a comprehensive test scenario".into())
    );
    assert_eq!(deserialized.priority, 5);
}
