//! Tests for hierarchy-targeted scenario operations.

use finstack_quant_core::market_data::bumps::MarketDataTarget;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_scenarios::{OperationSpec, ScenarioSpec};

/// A tag filter that matches both a parent node and one of its children
/// reaches the child's targets once per selector at the child's matched depth
/// and retains stable preorder placement order (`BANK-5Y`, then `OXY-5Y`).
#[test]
fn tag_filter_matching_parent_and_child_shocks_each_curve_once() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::hierarchy::{
        HierarchyTarget, MarketDataHierarchy, TagFilter, TagPredicate,
    };
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_scenarios::ScenarioMarketTarget;
    use finstack_quant_scenarios::{
        AppliedShockTarget, CurveKind, ExecutionContext, ScenarioEngine,
    };
    use time::macros::date;

    let hierarchy = MarketDataHierarchy::builder()
        .add_node("Credit/US")
        .tag("region", "us")
        .add_node("Credit/US/Banks")
        .tag("region", "us")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("BANK-5Y")))
        .add_node("Credit/US/Energy")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("OXY-5Y")))
        .build()
        .expect("hierarchy should build");
    let base = date!(2025 - 01 - 01);
    let curve = |id: &str| {
        DiscountCurve::builder(id)
            .base_date(base)
            .knots([(0.0, 1.0), (5.0, 0.90)])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("curve should build")
    };
    let mut market = MarketContext::new()
        .try_insert(curve("BANK-5Y"))
        .expect("insert market curve")
        .try_insert(curve("OXY-5Y"))
        .expect("insert market curve");
    market.set_hierarchy(hierarchy);

    let scenario = ScenarioSpec {
        id: "tagged".into(),
        operations: vec![OperationSpec::HierarchyCurveParallelBp {
            curve_kind: CurveKind::Discount,
            target: HierarchyTarget {
                path: vec!["Credit".into()],
                tag_filter: Some(TagFilter {
                    predicates: vec![TagPredicate::Equals {
                        key: "region".into(),
                        value: "us".into(),
                    }],
                }),
            },
            bp: 50.0,
            discount_curve_id: None,
        }],
        ..Default::default()
    };
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };
    let report = ScenarioEngine::new()
        .apply(&scenario, &mut ctx)
        .expect("scenario should apply");

    let shocked: Vec<&str> = report
        .applied_shocks
        .iter()
        .map(|shock| match &shock.applies_to {
            AppliedShockTarget::Market {
                target: ScenarioMarketTarget::Curve { curve_id, .. },
            } => curve_id.as_str(),
            other => panic!("unexpected shock target {other:?}"),
        })
        .collect();
    assert_eq!(shocked, vec!["BANK-5Y", "OXY-5Y"]);
    assert_eq!(report.expanded_operations, 2);
}

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
    use finstack_quant_statements::types::FinancialModelSpec;
    use time::macros::date;

    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
        .add_node("Rates/USD/Treasury")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-TSY")))
        .build()
        .unwrap();

    let base = date!(2025 - 01 - 01);
    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.78)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let tsy = DiscountCurve::builder("USD-TSY")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let mut market = MarketContext::new()
        .try_insert(ois)
        .expect("insert market curve")
        .try_insert(tsy)
        .expect("insert market curve");
    market.set_hierarchy(h);

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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
    use finstack_quant_scenarios::ScenarioMarketTarget;
    use finstack_quant_scenarios::{CurveKind, ExecutionContext, ScenarioEngine};
    use finstack_quant_statements::types::FinancialModelSpec;
    use time::macros::date;

    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US/IG/Financials")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("JPM-5Y")))
        .add_node("Credit/US/HY/Energy")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("OXY-5Y")))
        .build()
        .unwrap();

    let base = date!(2025 - 01 - 01);
    let jpm = DiscountCurve::builder("JPM-5Y")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 0.90)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let oxy = DiscountCurve::builder("OXY-5Y")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let mut market = MarketContext::new()
        .try_insert(jpm)
        .expect("insert market curve")
        .try_insert(oxy)
        .expect("insert market curve");
    market.set_hierarchy(h);

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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
    assert!(
        report
            .changes
            .market_targets
            .contains(&ScenarioMarketTarget::Curve {
                curve_kind: CurveKind::Discount,
                curve_id: "JPM-5Y".into(),
            })
    );
    assert!(
        report
            .changes
            .market_targets
            .contains(&ScenarioMarketTarget::Curve {
                curve_kind: CurveKind::Discount,
                curve_id: "OXY-5Y".into(),
            })
    );
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
    use finstack_quant_statements::types::FinancialModelSpec;
    use time::macros::date;

    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US/IG/Financials")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("JPM-5Y")))
        .add_node("Credit/US/HY/Energy")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("OXY-5Y")))
        .build()
        .unwrap();

    let base = date!(2025 - 01 - 01);
    let jpm = DiscountCurve::builder("JPM-5Y")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 0.90)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let oxy = DiscountCurve::builder("OXY-5Y")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let mut market = MarketContext::new()
        .try_insert(jpm)
        .expect("insert market curve")
        .try_insert(oxy)
        .expect("insert market curve");
    market.set_hierarchy(h);

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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
    use finstack_quant_statements::types::FinancialModelSpec;
    use time::macros::date;

    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
        .add_node("Rates/USD/Treasury")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-TSY")))
        .build()
        .unwrap();

    let base = date!(2025 - 01 - 01);
    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.78)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let tsy = DiscountCurve::builder("USD-TSY")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let mut market = MarketContext::new()
        .try_insert(ois)
        .expect("insert market curve")
        .try_insert(tsy)
        .expect("insert market curve");
    market.set_hierarchy(h);

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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

    let composed = ScenarioSpec::compose(vec![s1, s2]).expect("compose should succeed");
    assert_eq!(composed.resolution_mode, ResolutionMode::Cumulative);
}

#[test]
fn compose_rejects_mixed_resolution_modes() {
    use finstack_quant_core::market_data::hierarchy::ResolutionMode;

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

    let err = ScenarioSpec::compose(vec![most_specific, cumulative])
        .expect_err("compose with mixed resolution modes must fail");
    assert!(err.to_string().contains("resolution_mode"), "{err}");
}

#[test]
fn most_specific_wins_uses_matched_node_depth_not_target_path_depth() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::hierarchy::{
        HierarchyTarget, MarketDataHierarchy, ResolutionMode, TagFilter, TagPredicate,
    };
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_scenarios::{CurveKind, ExecutionContext, ScenarioEngine};
    use finstack_quant_statements::types::FinancialModelSpec;
    use time::macros::date;

    let hierarchy = MarketDataHierarchy::builder()
        .add_node("Credit/US/IG/Financials")
        .tag("sector", "financials")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("JPM-5Y")))
        .build()
        .unwrap();

    let base = date!(2025 - 01 - 01);
    let curve = DiscountCurve::builder("JPM-5Y")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 0.90)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let mut hierarchy_market = MarketContext::new()
        .try_insert(curve.clone())
        .expect("insert market curve");
    hierarchy_market.set_hierarchy(hierarchy);
    let mut direct_market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");

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
    let mut hierarchy_model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
    let mut direct_model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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

    let hierarchy_df = hierarchy_market
        .get_discount("JPM-5Y")
        .unwrap()
        .df(5.0)
        .unwrap();
    let direct_df = direct_market
        .get_discount("JPM-5Y")
        .unwrap()
        .df(5.0)
        .unwrap();
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
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    };
    use finstack_quant_scenarios::{ExecutionContext, ScenarioEngine};
    use finstack_quant_statements::types::FinancialModelSpec;
    use time::macros::date;

    let hierarchy = MarketDataHierarchy::builder()
        .add_node("Equity/US/LargeCap")
        .targets([
            MarketDataTarget::VolSurface(CurveId::from("SPX")),
            MarketDataTarget::PriceScalar(CurveId::from("SPX")),
        ])
        .build()
        .unwrap();

    let surface = VolSurface::builder("SPX")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 100.0])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.20, 0.20])
        .row(&[0.20, 0.20])
        .build()
        .unwrap();

    let mut hierarchy_market = MarketContext::new()
        .insert_surface(surface.clone())
        .insert_price(
            "SPX",
            MarketScalar::price(Money::new(100.0, Currency::USD).expect("valid money fixture")),
        );
    hierarchy_market.set_hierarchy(hierarchy);

    let mut direct_market = MarketContext::new().insert_surface(surface).insert_price(
        "SPX",
        MarketScalar::price(Money::new(100.0, Currency::USD).expect("valid money fixture")),
    );

    let hierarchy_scenario = ScenarioSpec {
        id: "family-collision".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::HierarchyVolSurfaceParallelPct {
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
                vol_surface_id: "SPX".into(),
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
    let mut hierarchy_model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
    let mut direct_model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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

    let hierarchy_spot = hierarchy_market
        .get_price("SPX")
        .unwrap()
        .get_price()
        .expect("expected SPX price")
        .amount();
    let direct_spot = direct_market
        .get_price("SPX")
        .unwrap()
        .get_price()
        .expect("expected SPX price")
        .amount();

    let hierarchy_surface = hierarchy_market.get_surface("SPX").unwrap();
    let hierarchy_vol =
        finstack_quant_models::volatility::get_surface_vol_clamped(&hierarchy_surface, 1.0, 100.0);
    let direct_surface = direct_market.get_surface("SPX").unwrap();
    let direct_vol =
        finstack_quant_models::volatility::get_surface_vol_clamped(&direct_surface, 1.0, 100.0);

    assert!((hierarchy_spot - direct_spot).abs() < 1e-12);
    assert!((hierarchy_vol - direct_vol).abs() < 1e-12);
}

#[test]
fn mixed_family_subtree_projects_declared_family_and_errors_on_zero_compatible_targets() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::hierarchy::{HierarchyTarget, MarketDataHierarchy};
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_scenarios::{CurveKind, ExecutionContext, ScenarioEngine};
    use time::macros::date;

    let base = date!(2025 - 01 - 01);
    let hierarchy = MarketDataHierarchy::builder()
        .add_node("Rates/USD")
        .targets([
            MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")),
            MarketDataTarget::ForwardCurve(CurveId::from("USD-SOFR")),
        ])
        .build()
        .unwrap();

    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let sofr = ForwardCurve::flat(
        "USD-SOFR",
        base,
        0.03,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .unwrap();
    let orig_ois_df = ois.df(5.0).unwrap();
    let orig_sofr = sofr.df(2.0).unwrap();

    let mut market = MarketContext::new()
        .try_insert(ois)
        .expect("insert market curve")
        .try_insert(sofr)
        .expect("insert market curve");
    market.set_hierarchy(hierarchy);

    // Discount bump projects out the ForwardCurve without error.
    let ok_spec = ScenarioSpec {
        id: "disc-only".into(),
        operations: vec![OperationSpec::HierarchyCurveParallelBp {
            curve_kind: CurveKind::Discount,
            target: HierarchyTarget::unfiltered(vec!["Rates".into(), "USD".into()]).unwrap(),
            bp: 25.0,
            discount_curve_id: None,
        }],
        ..Default::default()
    };
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };
    let report = ScenarioEngine::new().apply(&ok_spec, &mut ctx).unwrap();
    assert_eq!(report.expanded_operations, 1);
    assert!(market.get_discount("USD-OIS").unwrap().df(5.0).unwrap() < orig_ois_df);
    assert!((market.get_forward("USD-SOFR").unwrap().df(2.0).unwrap() - orig_sofr).abs() < 1e-12);

    // Zero-sized shock targeting a family absent from the subtree still fails explicitly.
    let zero_mismatch_spec = ScenarioSpec {
        id: "cds-missing".into(),
        operations: vec![OperationSpec::HierarchyCurveParallelBp {
            curve_kind: CurveKind::ParCDS,
            target: HierarchyTarget::unfiltered(vec!["Rates".into(), "USD".into()]).unwrap(),
            bp: 0.0,
            discount_curve_id: None,
        }],
        ..Default::default()
    };
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };
    assert!(
        ScenarioEngine::new()
            .apply(&zero_mismatch_spec, &mut ctx)
            .is_err()
    );
}

#[test]
fn hierarchy_execution_rolls_back_market_and_as_of_on_missing_or_wrong_kind_active_target() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::hierarchy::{HierarchyTarget, MarketDataHierarchy};
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_scenarios::{CurveKind, ExecutionContext, ScenarioEngine, TimeRollMode};
    use time::macros::date;

    let base = date!(2025 - 01 - 01);
    let hierarchy = MarketDataHierarchy::builder()
        .add_node("Rates/USD")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
        .add_node("Equity/Missing")
        .target(MarketDataTarget::PriceScalar(CurveId::from("NO-SUCH-EQ")))
        .add_node("Equity/WrongKind")
        .target(MarketDataTarget::UnitlessScalar(CurveId::from("SPX")))
        .build()
        .unwrap();

    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let orig_df = ois.df(5.0).unwrap();

    let mut market = MarketContext::new()
        .try_insert(ois)
        .expect("insert market curve")
        .insert_price(
            "SPX",
            MarketScalar::price(Money::new(100.0, Currency::USD).expect("valid money")),
        );
    market.set_hierarchy(hierarchy);

    // Prior TimeRollForward + USD-OIS bump must not survive failure on later active equity target.
    let failing_missing = ScenarioSpec {
        id: "atomic-rollback".into(),
        operations: vec![
            OperationSpec::TimeRollForward {
                period: "1M".into(),
                apply_shocks: true,
                roll_mode: TimeRollMode::BusinessDays,
            },
            OperationSpec::HierarchyCurveParallelBp {
                curve_kind: CurveKind::Discount,
                target: HierarchyTarget::unfiltered(vec!["Rates".into(), "USD".into()]).unwrap(),
                bp: 50.0,
                discount_curve_id: None,
            },
            OperationSpec::HierarchyEquityPricePct {
                target: HierarchyTarget::unfiltered(vec!["Equity".into(), "Missing".into()])
                    .unwrap(),
                pct: -5.0,
            },
        ],
        ..Default::default()
    };
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };
    assert!(
        ScenarioEngine::new()
            .apply(&failing_missing, &mut ctx)
            .is_err()
    );
    assert_eq!(
        ctx.as_of, base,
        "failed scenario must not advance ctx.as_of"
    );
    assert!(
        (ctx.market.get_discount("USD-OIS").unwrap().df(5.0).unwrap() - orig_df).abs() < 1e-12,
        "failed scenario must restore base market"
    );

    // UnitlessScalar declaration against a MarketScalar::Price entry must also fail cleanly.
    let wrong_scalar_subkind = ScenarioSpec {
        id: "wrong-subkind".into(),
        operations: vec![OperationSpec::HierarchyEquityPricePct {
            target: HierarchyTarget::unfiltered(vec!["Equity".into(), "WrongKind".into()]).unwrap(),
            pct: -5.0,
        }],
        ..Default::default()
    };
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };
    assert!(
        ScenarioEngine::new()
            .apply(&wrong_scalar_subkind, &mut ctx)
            .is_err()
    );
}
