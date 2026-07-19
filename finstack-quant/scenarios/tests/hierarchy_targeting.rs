//! Tests for hierarchy-targeted scenario operations.

use finstack_quant_core::money::Money;
use finstack_quant_scenarios::{OperationSpec, ScenarioSpec};

/// Existing direct-targeted JSON must still deserialize correctly.
#[test]
fn existing_direct_target_json_round_trips() {
    let json = r#"{
        "id": "test",
        "operations": [
            {
                "kind": "curve_parallel_bp",
                "curve_kind": "discount",
                "curve_id": "USD-OIS",
                "bp": 50.0
            }
        ]
    }"#;
    let spec: ScenarioSpec = serde_json::from_str(json).unwrap();
    assert_eq!(spec.operations.len(), 1);
    match &spec.operations[0] {
        OperationSpec::CurveParallelBp { curve_id, bp, .. } => {
            assert_eq!(curve_id.as_str(), "USD-OIS");
            assert!((bp - 50.0).abs() < f64::EPSILON);
        }
        other => panic!("Expected CurveParallelBp, got: {:?}", other),
    }
}

/// JSON with explicit `resolution_mode` deserializes correctly.
#[test]
fn resolution_mode_field_deserializes_from_json() {
    let json = r#"{
        "id": "test",
        "operations": [],
        "resolution_mode": "cumulative"
    }"#;
    let spec: ScenarioSpec = serde_json::from_str(json).unwrap();
    use finstack_quant_core::market_data::hierarchy::ResolutionMode;
    assert_eq!(spec.resolution_mode, ResolutionMode::Cumulative);
}

/// JSON without `resolution_mode` defaults to `MostSpecificWins`.
#[test]
fn resolution_mode_defaults_to_most_specific_wins() {
    let json = r#"{"id": "test", "operations": []}"#;
    let spec: ScenarioSpec = serde_json::from_str(json).unwrap();
    use finstack_quant_core::market_data::hierarchy::ResolutionMode;
    assert_eq!(spec.resolution_mode, ResolutionMode::MostSpecificWins);
}

/// Hierarchy-targeted `HierarchyCurveParallelBp` resolves to one bump per matched curve.
#[test]
fn hierarchy_curve_parallel_bp_resolves_to_individual_bumps() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::hierarchy::HierarchyTarget;
    use finstack_quant_core::market_data::hierarchy::MarketDataHierarchy;
    use finstack_quant_core::market_data::hierarchy::ResolutionMode;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_scenarios::{CurveKind, ExecutionContext, ScenarioEngine};
    use finstack_quant_statements::FinancialModelSpec;
    use time::macros::date;

    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .curve_ids(&["USD-OIS"])
        .add_node("Rates/USD/Treasury")
        .curve_ids(&["USD-TSY"])
        .build()
        .unwrap();

    let base = date!(2025 - 01 - 01);
    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.78)])
        .build()
        .unwrap();
    let tsy = DiscountCurve::builder("USD-TSY")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(ois).insert(tsy);
    market.set_hierarchy(h);

    let mut model = FinancialModelSpec::new("test", vec![]);

    // Use hierarchy-targeted operation targeting the "Rates/USD" subtree,
    // which resolves to both USD-OIS and USD-TSY.
    let scenario = ScenarioSpec {
        id: "hierarchy_bump".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::HierarchyCurveParallelBp {
            curve_kind: CurveKind::Discount,
            target: HierarchyTarget {
                path: vec!["Rates".into(), "USD".into()],
                tag_filter: None,
            },
            bp: 50.0,
            discount_curve_id: None,
        }],
        priority: 0,
        resolution_mode: ResolutionMode::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    // Should have applied 2 bumps (one per resolved curve: USD-OIS, USD-TSY)
    assert_eq!(
        report.operations_applied, 2,
        "Expected 2 operations applied (one per resolved curve), got {}. Warnings: {:?}",
        report.operations_applied, report.warnings
    );
}

/// Cumulative mode stacks all matching shocks from every hierarchy level.
///
/// Two operations target overlapping subtrees:
///   - +50bp on all of "Credit" (matches JPM-5Y and OXY-5Y)
///   - +100bp on "Credit/US/HY" (matches OXY-5Y only)
///
/// With Cumulative mode the engine expands both operations independently, so
/// OXY-5Y receives two bumps and JPM-5Y receives one. Total: 3 operations applied.
#[test]
fn cumulative_mode_stacks_shocks_down_tree() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::hierarchy::HierarchyTarget;
    use finstack_quant_core::market_data::hierarchy::MarketDataHierarchy;
    use finstack_quant_core::market_data::hierarchy::ResolutionMode;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_scenarios::engine::ScenarioMarketTarget;
    use finstack_quant_scenarios::{CurveKind, ExecutionContext, ScenarioEngine};
    use finstack_quant_statements::FinancialModelSpec;
    use time::macros::date;

    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US/IG/Financials")
        .curve_ids(&["JPM-5Y"])
        .add_node("Credit/US/HY/Energy")
        .curve_ids(&["OXY-5Y"])
        .build()
        .unwrap();

    let base = date!(2025 - 01 - 01);
    let jpm = DiscountCurve::builder("JPM-5Y")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 0.90)])
        .build()
        .unwrap();
    let oxy = DiscountCurve::builder("OXY-5Y")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(jpm).insert(oxy);
    market.set_hierarchy(h);

    let mut model = FinancialModelSpec::new("test", vec![]);

    // Op1: +50bp on all Credit (matches JPM-5Y and OXY-5Y → 2 expansions)
    // Op2: +100bp on Credit/US/HY (matches OXY-5Y only → 1 expansion)
    // Cumulative: all 3 expansions applied → operations_applied == 3
    let scenario = ScenarioSpec {
        id: "cumulative_test".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::HierarchyCurveParallelBp {
                curve_kind: CurveKind::Discount,
                target: HierarchyTarget {
                    path: vec!["Credit".into()],
                    tag_filter: None,
                },
                bp: 50.0,
                discount_curve_id: None,
            },
            OperationSpec::HierarchyCurveParallelBp {
                curve_kind: CurveKind::Discount,
                target: HierarchyTarget {
                    path: vec!["Credit".into(), "US".into(), "HY".into()],
                    tag_filter: None,
                },
                bp: 100.0,
                discount_curve_id: None,
            },
        ],
        priority: 0,
        resolution_mode: ResolutionMode::Cumulative,
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    // Credit-wide op: JPM-5Y + OXY-5Y = 2 bumps
    // HY-specific op: OXY-5Y = 1 bump
    // Cumulative total: 3 operations applied
    assert_eq!(
        report.operations_applied, 3,
        "Expected 3 operations (2 from Credit-wide + 1 from HY-specific), got {}. Warnings: {:?}",
        report.operations_applied, report.warnings
    );
    assert_eq!(report.changes.market_targets.len(), 2);
    assert!(report
        .changes
        .market_targets
        .contains(&ScenarioMarketTarget::Curve {
            curve_kind: CurveKind::Discount,
            curve_id: "JPM-5Y".into(),
        }));
    assert!(report
        .changes
        .market_targets
        .contains(&ScenarioMarketTarget::Curve {
            curve_kind: CurveKind::Discount,
            curve_id: "OXY-5Y".into(),
        }));
}

/// MostSpecificWins keeps only the deepest matching shock per curve.
///
/// Two operations target overlapping subtrees:
///   - +50bp on all of "Credit" (path depth 1, matches JPM-5Y and OXY-5Y)
///   - +100bp on "Credit/US/HY" (path depth 3, matches OXY-5Y only)
///
/// With MostSpecificWins the engine deduplicates by curve ID keeping the deepest
/// match. OXY-5Y is matched by depth-3 and depth-1; depth-3 wins. JPM-5Y is
/// matched only by depth-1. Total: 2 operations applied (one per curve).
#[test]
fn most_specific_wins_keeps_only_deepest_shock() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::hierarchy::HierarchyTarget;
    use finstack_quant_core::market_data::hierarchy::MarketDataHierarchy;
    use finstack_quant_core::market_data::hierarchy::ResolutionMode;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_scenarios::{CurveKind, ExecutionContext, ScenarioEngine};
    use finstack_quant_statements::FinancialModelSpec;
    use time::macros::date;

    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US/IG/Financials")
        .curve_ids(&["JPM-5Y"])
        .add_node("Credit/US/HY/Energy")
        .curve_ids(&["OXY-5Y"])
        .build()
        .unwrap();

    let base = date!(2025 - 01 - 01);
    let jpm = DiscountCurve::builder("JPM-5Y")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 0.90)])
        .build()
        .unwrap();
    let oxy = DiscountCurve::builder("OXY-5Y")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(jpm).insert(oxy);
    market.set_hierarchy(h);

    let mut model = FinancialModelSpec::new("test", vec![]);

    // Op1 path depth 1 (Credit):     JPM-5Y → depth 1, OXY-5Y → depth 1
    // Op2 path depth 3 (Credit/US/HY): OXY-5Y → depth 3
    // MostSpecificWins: max depth per curve_id:
    //   JPM-5Y → 1, OXY-5Y → 3
    // Kept: JPM-5Y from Op1 (depth 1 == max 1), OXY-5Y from Op2 (depth 3 == max 3)
    // Dropped: OXY-5Y from Op1 (depth 1 < max 3)
    // operations_applied == 2
    let scenario = ScenarioSpec {
        id: "msw_test".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::HierarchyCurveParallelBp {
                curve_kind: CurveKind::Discount,
                target: HierarchyTarget {
                    path: vec!["Credit".into()],
                    tag_filter: None,
                },
                bp: 50.0,
                discount_curve_id: None,
            },
            OperationSpec::HierarchyCurveParallelBp {
                curve_kind: CurveKind::Discount,
                target: HierarchyTarget {
                    path: vec!["Credit".into(), "US".into(), "HY".into()],
                    tag_filter: None,
                },
                bp: 100.0,
                discount_curve_id: None,
            },
        ],
        priority: 0,
        resolution_mode: ResolutionMode::MostSpecificWins,
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    // JPM-5Y: only matched at depth 1 → +50bp applied
    // OXY-5Y: depth-3 match wins over depth-1 match → +100bp applied
    // Total: 2 operations (one per curve, deepest match only)
    assert_eq!(
        report.operations_applied, 2,
        "Expected 2 operations (one per curve, deepest match wins), got {}. Warnings: {:?}",
        report.operations_applied, report.warnings
    );
}

/// Engine smoke test: adding `resolution_mode` to `ScenarioSpec` does not break
/// the existing engine flow for a direct curve-targeted operation.
#[test]
fn engine_works_with_resolution_mode_field() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::hierarchy::MarketDataHierarchy;
    use finstack_quant_core::market_data::hierarchy::ResolutionMode;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_scenarios::{CurveKind, ExecutionContext, ScenarioEngine};
    use finstack_quant_statements::FinancialModelSpec;
    use time::macros::date;

    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .curve_ids(&["USD-OIS"])
        .add_node("Rates/USD/Treasury")
        .curve_ids(&["USD-TSY"])
        .build()
        .unwrap();

    let base = date!(2025 - 01 - 01);
    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.78)])
        .build()
        .unwrap();
    let tsy = DiscountCurve::builder("USD-TSY")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(ois).insert(tsy);
    market.set_hierarchy(h);

    let mut model = FinancialModelSpec::new("test", vec![]);

    // Bump USD-OIS directly with resolution_mode explicitly set
    let scenario = ScenarioSpec {
        id: "test_hierarchy".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD-OIS".into(),
            discount_curve_id: None,
            bp: 50.0,
        }],
        priority: 0,
        resolution_mode: ResolutionMode::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert!(
        report.operations_applied >= 1,
        "Expected at least 1 operation applied, got {}",
        report.operations_applied
    );
}

/// `HierarchyCurveParallelBp` with a `TagFilter` survives a JSON round-trip intact.
///
/// Verifies that:
/// - The operation serializes with the correct `"kind": "hierarchy_curve_parallel_bp"` tag.
/// - The `target.path`, `curve_kind`, and `bp` fields are preserved exactly.
/// - The optional `tag_filter` with a `TagPredicate::Equals` predicate round-trips correctly.
#[test]
fn hierarchy_operation_json_round_trip() {
    use finstack_quant_core::market_data::hierarchy::HierarchyTarget;
    use finstack_quant_core::market_data::hierarchy::{TagFilter, TagPredicate};
    use finstack_quant_scenarios::{CurveKind, OperationSpec};

    let op = OperationSpec::HierarchyCurveParallelBp {
        curve_kind: CurveKind::Discount,
        target: HierarchyTarget {
            path: vec!["Credit".into(), "US".into(), "IG".into()],
            tag_filter: Some(TagFilter {
                predicates: vec![TagPredicate::Equals {
                    key: "sector".into(),
                    value: "Financials".into(),
                }],
            }),
        },
        bp: 50.0,
        discount_curve_id: None,
    };

    let json = serde_json::to_string_pretty(&op).unwrap();

    // The OperationSpec enum uses #[serde(tag = "kind", rename_all = "snake_case")]
    // so the JSON must contain the snake_case variant name as the "kind" field.
    assert!(
        json.contains("hierarchy_curve_parallel_bp"),
        "JSON must contain the snake_case kind tag; got: {}",
        json
    );

    let deserialized: OperationSpec = serde_json::from_str(&json).unwrap();

    match deserialized {
        OperationSpec::HierarchyCurveParallelBp {
            curve_kind,
            target,
            bp,
            ..
        } => {
            assert_eq!(curve_kind, CurveKind::Discount);
            assert_eq!(
                target.path,
                vec!["Credit".to_string(), "US".to_string(), "IG".to_string()]
            );
            let filter = target
                .tag_filter
                .as_ref()
                .expect("tag_filter should be Some");
            assert_eq!(filter.predicates.len(), 1);
            match &filter.predicates[0] {
                TagPredicate::Equals { key, value } => {
                    assert_eq!(key, "sector");
                    assert_eq!(value, "Financials");
                }
                other => panic!("Expected Equals predicate, got: {:?}", other),
            }
            assert!(
                (bp - 50.0).abs() < f64::EPSILON,
                "bp should be exactly 50.0, got {}",
                bp
            );
        }
        other => panic!("Expected HierarchyCurveParallelBp, got: {:?}", other),
    }
}

/// A full `ScenarioSpec` with `resolution_mode: Cumulative` round-trips through JSON.
///
/// Verifies that:
/// - The serialized JSON contains the string `"cumulative"` (snake_case serde output).
/// - Deserialization restores `resolution_mode` to `ResolutionMode::Cumulative`.
#[test]
fn scenario_with_resolution_mode_json_round_trip() {
    use finstack_quant_core::market_data::hierarchy::HierarchyTarget;
    use finstack_quant_core::market_data::hierarchy::ResolutionMode;
    use finstack_quant_scenarios::{CurveKind, OperationSpec, ScenarioSpec};

    let scenario = ScenarioSpec {
        id: "hierarchy_test".into(),
        name: Some("Hierarchy Test".into()),
        description: None,
        operations: vec![OperationSpec::HierarchyCurveParallelBp {
            curve_kind: CurveKind::Discount,
            target: HierarchyTarget {
                path: vec!["Rates".into()],
                tag_filter: None,
            },
            bp: 25.0,
            discount_curve_id: None,
        }],
        priority: 0,
        resolution_mode: ResolutionMode::Cumulative,
    };

    let json = serde_json::to_string_pretty(&scenario).unwrap();

    // ResolutionMode uses #[serde(rename_all = "snake_case")] so Cumulative → "cumulative"
    assert!(
        json.contains("cumulative"),
        "JSON must contain \"cumulative\" for ResolutionMode::Cumulative; got: {}",
        json
    );

    let deserialized: ScenarioSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(
        deserialized.resolution_mode,
        ResolutionMode::Cumulative,
        "resolution_mode should deserialize back to Cumulative"
    );
    assert_eq!(deserialized.id, "hierarchy_test");
    assert_eq!(deserialized.operations.len(), 1);
}

#[test]
fn compose_preserves_resolution_mode_when_inputs_agree() {
    use finstack_quant_core::market_data::hierarchy::ResolutionMode;
    use finstack_quant_scenarios::ScenarioEngine;

    let s1 = ScenarioSpec {
        id: "one".into(),
        name: None,
        description: None,
        operations: vec![],
        priority: 0,
        resolution_mode: ResolutionMode::Cumulative,
    };
    let s2 = ScenarioSpec {
        id: "two".into(),
        name: None,
        description: None,
        operations: vec![],
        priority: 1,
        resolution_mode: ResolutionMode::Cumulative,
    };

    let composed = ScenarioEngine::new()
        .try_compose(vec![s1, s2])
        .expect("compose should succeed");
    assert_eq!(composed.resolution_mode, ResolutionMode::Cumulative);
}

#[test]
fn compose_with_mixed_resolution_modes_defaults_to_cumulative() {
    use finstack_quant_core::market_data::hierarchy::ResolutionMode;
    use finstack_quant_scenarios::ScenarioEngine;

    let most_specific = ScenarioSpec {
        id: "one".into(),
        name: None,
        description: None,
        operations: vec![],
        priority: 0,
        resolution_mode: ResolutionMode::MostSpecificWins,
    };
    let cumulative = ScenarioSpec {
        id: "two".into(),
        name: None,
        description: None,
        operations: vec![],
        priority: 1,
        resolution_mode: ResolutionMode::Cumulative,
    };

    let composed = ScenarioEngine::new()
        .try_compose(vec![most_specific, cumulative])
        .expect("compose should succeed");
    assert_eq!(composed.resolution_mode, ResolutionMode::Cumulative);
}

#[test]
fn most_specific_wins_uses_matched_node_depth_not_target_path_depth() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::hierarchy::{
        HierarchyTarget, MarketDataHierarchy, ResolutionMode, TagFilter, TagPredicate,
    };
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_scenarios::{CurveKind, ExecutionContext, ScenarioEngine};
    use finstack_quant_statements::FinancialModelSpec;
    use time::macros::date;

    let hierarchy = MarketDataHierarchy::builder()
        .add_node("Credit/US/IG/Financials")
        .tag("sector", "financials")
        .curve_ids(&["JPM-5Y"])
        .build()
        .unwrap();

    let base = date!(2025 - 01 - 01);
    let curve = DiscountCurve::builder("JPM-5Y")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 0.90)])
        .build()
        .unwrap();

    let mut hierarchy_market = MarketContext::new().insert(curve.clone());
    hierarchy_market.set_hierarchy(hierarchy);
    let mut direct_market = MarketContext::new().insert(curve);

    let hierarchy_scenario = ScenarioSpec {
        id: "hier-depth".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::HierarchyCurveParallelBp {
                curve_kind: CurveKind::Discount,
                target: HierarchyTarget {
                    path: vec!["Credit".into()],
                    tag_filter: Some(TagFilter {
                        predicates: vec![TagPredicate::Equals {
                            key: "sector".into(),
                            value: "financials".into(),
                        }],
                    }),
                },
                bp: 100.0,
                discount_curve_id: None,
            },
            OperationSpec::HierarchyCurveParallelBp {
                curve_kind: CurveKind::Discount,
                target: HierarchyTarget {
                    path: vec!["Credit".into(), "US".into()],
                    tag_filter: None,
                },
                bp: 50.0,
                discount_curve_id: None,
            },
        ],
        priority: 0,
        resolution_mode: ResolutionMode::MostSpecificWins,
    };

    let direct_scenario = ScenarioSpec {
        id: "direct".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "JPM-5Y".into(),
            discount_curve_id: None,
            bp: 100.0,
        }],
        priority: 0,
        resolution_mode: ResolutionMode::MostSpecificWins,
    };

    let engine = ScenarioEngine::new();
    let mut hierarchy_model = FinancialModelSpec::new("test", vec![]);
    let mut direct_model = FinancialModelSpec::new("test", vec![]);

    let mut hierarchy_ctx = ExecutionContext {
        market: &mut hierarchy_market,
        model: Some(&mut hierarchy_model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };
    let mut direct_ctx = ExecutionContext {
        market: &mut direct_market,
        model: Some(&mut direct_model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };

    engine
        .apply(&hierarchy_scenario, &mut hierarchy_ctx)
        .unwrap();
    engine.apply(&direct_scenario, &mut direct_ctx).unwrap();

    let hierarchy_df = hierarchy_market.get_discount("JPM-5Y").unwrap().df(5.0);
    let direct_df = direct_market.get_discount("JPM-5Y").unwrap().df(5.0);
    assert!(
        (hierarchy_df - direct_df).abs() < 1e-12,
        "hierarchy-targeted MostSpecificWins should match the direct +100bp outcome; got hierarchy_df={hierarchy_df}, direct_df={direct_df}"
    );
}

#[test]
fn most_specific_wins_deduplicates_per_operation_family_not_raw_identifier() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::hierarchy::{
        HierarchyTarget, MarketDataHierarchy, ResolutionMode,
    };
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_scenarios::{ExecutionContext, ScenarioEngine, VolSurfaceKind};
    use finstack_quant_statements::FinancialModelSpec;
    use time::macros::date;

    let hierarchy = MarketDataHierarchy::builder()
        .add_node("Equity/US/LargeCap")
        .curve_ids(&["SPX"])
        .build()
        .unwrap();

    let surface = VolSurface::builder("SPX")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0])
        .row(&[0.20, 0.20])
        .row(&[0.20, 0.20])
        .build()
        .unwrap();

    let mut hierarchy_market = MarketContext::new()
        .insert_surface(surface.clone())
        .insert_price("SPX", MarketScalar::Price(Money::new(100.0, Currency::USD)));
    hierarchy_market.set_hierarchy(hierarchy);

    let mut direct_market = MarketContext::new()
        .insert_surface(surface)
        .insert_price("SPX", MarketScalar::Price(Money::new(100.0, Currency::USD)));

    let hierarchy_scenario = ScenarioSpec {
        id: "family-collision".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::HierarchyVolSurfaceParallelPct {
                surface_kind: VolSurfaceKind::Equity,
                target: HierarchyTarget {
                    path: vec!["Equity".into()],
                    tag_filter: None,
                },
                pct: 10.0,
            },
            OperationSpec::HierarchyEquityPricePct {
                target: HierarchyTarget {
                    path: vec!["Equity".into(), "US".into()],
                    tag_filter: None,
                },
                pct: -5.0,
            },
        ],
        priority: 0,
        resolution_mode: ResolutionMode::MostSpecificWins,
    };

    let direct_scenario = ScenarioSpec {
        id: "direct".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::VolSurfaceParallelPct {
                surface_kind: VolSurfaceKind::Equity,
                surface_id: "SPX".into(),
                pct: 10.0,
            },
            OperationSpec::EquityPricePct {
                ids: vec!["SPX".into()],
                pct: -5.0,
            },
        ],
        priority: 0,
        resolution_mode: ResolutionMode::MostSpecificWins,
    };

    let engine = ScenarioEngine::new();
    let base = date!(2025 - 01 - 01);
    let mut hierarchy_model = FinancialModelSpec::new("test", vec![]);
    let mut direct_model = FinancialModelSpec::new("test", vec![]);

    let mut hierarchy_ctx = ExecutionContext {
        market: &mut hierarchy_market,
        model: Some(&mut hierarchy_model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };
    let mut direct_ctx = ExecutionContext {
        market: &mut direct_market,
        model: Some(&mut direct_model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };

    engine
        .apply(&hierarchy_scenario, &mut hierarchy_ctx)
        .unwrap();
    engine.apply(&direct_scenario, &mut direct_ctx).unwrap();

    let hierarchy_spot = match hierarchy_market.get_price("SPX").unwrap() {
        MarketScalar::Price(price) => price.amount(),
        other @ MarketScalar::Unitless(_) => panic!("expected SPX price, got {other:?}"),
    };
    let direct_spot = match direct_market.get_price("SPX").unwrap() {
        MarketScalar::Price(price) => price.amount(),
        other @ MarketScalar::Unitless(_) => panic!("expected SPX price, got {other:?}"),
    };

    let hierarchy_vol = hierarchy_market
        .get_surface("SPX")
        .unwrap()
        .value_clamped(1.0, 100.0);
    let direct_vol = direct_market
        .get_surface("SPX")
        .unwrap()
        .value_clamped(1.0, 100.0);

    assert!((hierarchy_spot - direct_spot).abs() < 1e-12);
    assert!((hierarchy_vol - direct_vol).abs() < 1e-12);
}
