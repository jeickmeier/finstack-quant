use super::*;
use crate::spec::{OperationSpec, TimeRollMode};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::hierarchy::{
    HierarchyTarget, MarketDataHierarchy, ResolutionMode,
};
use finstack_quant_statements::FinancialModelSpec;
use time::macros::date;

#[test]
fn compose_rejects_two_time_rolls() {
    let s1 = ScenarioSpec {
        id: "roll_6m".into(),
        name: Some("Roll 6M".into()),
        description: None,
        operations: vec![OperationSpec::TimeRollForward {
            period: "6M".into(),
            apply_shocks: true,
            roll_mode: TimeRollMode::default(),
        }],
        priority: 1,
        resolution_mode: ResolutionMode::Cumulative,
        hazard_bump_mode: Default::default(),
    };
    let s2 = ScenarioSpec {
        id: "roll_1y".into(),
        name: Some("Roll 1Y".into()),
        description: None,
        operations: vec![OperationSpec::TimeRollForward {
            period: "1Y".into(),
            apply_shocks: true,
            roll_mode: TimeRollMode::default(),
        }],
        priority: 2,
        resolution_mode: ResolutionMode::Cumulative,
        hazard_bump_mode: Default::default(),
    };

    let err = ScenarioSpec::compose(vec![s1, s2])
        .expect_err("duplicate TimeRollForward must error at compose time");
    let msg = format!("{err}");
    assert!(msg.contains("TimeRollForward"));
}

#[test]
fn compose_preserves_source_ids_and_names() {
    let scenarios = vec![
        ScenarioSpec {
            id: "rates_up".into(),
            name: Some("Rates Up".into()),
            description: None,
            operations: vec![OperationSpec::StmtForecastPercent {
                node_id: "Revenue".into(),
                pct: 1.0,
            }],
            priority: 2,
            resolution_mode: ResolutionMode::MostSpecificWins,
            hazard_bump_mode: Default::default(),
        },
        ScenarioSpec {
            id: "credit_down".into(),
            name: None,
            description: None,
            operations: vec![OperationSpec::StmtForecastPercent {
                node_id: "Expenses".into(),
                pct: -1.0,
            }],
            priority: 1,
            resolution_mode: ResolutionMode::Cumulative,
            hazard_bump_mode: Default::default(),
        },
    ];

    let strict = ScenarioSpec::compose(scenarios).expect("valid compose");

    assert_eq!(strict.id.as_str(), "credit_down+rates_up");
    assert_eq!(strict.name.as_deref(), Some("credit_down + Rates Up"));
    assert_eq!(strict.operations.len(), 2);
    assert_eq!(strict.resolution_mode, ResolutionMode::Cumulative);
    assert_eq!(strict.hazard_bump_mode, crate::HazardBumpMode::SolveToPar);
}

#[test]
fn compose_keeps_agreed_first_order_hazard_mode() {
    let s1 = ScenarioSpec {
        id: "a".into(),
        name: None,
        description: None,
        operations: Vec::new(),
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: crate::HazardBumpMode::FirstOrderShift,
    };
    let s2 = ScenarioSpec {
        id: "b".into(),
        name: None,
        description: None,
        operations: Vec::new(),
        priority: 1,
        resolution_mode: Default::default(),
        hazard_bump_mode: crate::HazardBumpMode::FirstOrderShift,
    };
    let composed = ScenarioSpec::compose(vec![s1, s2]).expect("compose");
    assert_eq!(
        composed.hazard_bump_mode,
        crate::HazardBumpMode::FirstOrderShift
    );
}

#[test]
fn compose_falls_back_to_solve_to_par_when_modes_disagree() {
    let s1 = ScenarioSpec {
        id: "a".into(),
        name: None,
        description: None,
        operations: Vec::new(),
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: crate::HazardBumpMode::FirstOrderShift,
    };
    let s2 = ScenarioSpec {
        id: "b".into(),
        name: None,
        description: None,
        operations: Vec::new(),
        priority: 1,
        resolution_mode: Default::default(),
        hazard_bump_mode: crate::HazardBumpMode::SolveToPar,
    };
    let error =
        ScenarioSpec::compose(vec![s1, s2]).expect_err("mixed hazard bump modes must be rejected");
    let message = error.to_string();
    assert!(message.contains("a"), "unexpected error: {message}");
    assert!(message.contains("b"), "unexpected error: {message}");
    assert!(
        message.contains("first_order_shift"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("solve_to_par"),
        "unexpected error: {message}"
    );
}

#[test]
fn apply_rejects_hierarchy_op_without_hierarchy() {
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);
    let scenario = ScenarioSpec {
        id: "h_no_attach".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::HierarchyEquityPricePct {
            target: HierarchyTarget {
                path: vec!["equities".into(), "us".into()],
                tag_filter: None,
            },
            pct: -10.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: date!(2025 - 01 - 01),
    };
    let err = engine
        .apply(&scenario, &mut ctx)
        .expect_err("hierarchy op without hierarchy must error");
    assert!(err.to_string().contains("hierarchy"));
}

#[test]
fn apply_emits_warning_when_hierarchy_target_matches_no_curves() {
    // Empty hierarchy attached, but the target path has no curves.
    let hierarchy = MarketDataHierarchy::default();
    let mut market = MarketContext::new();
    market.set_hierarchy(hierarchy);
    let mut model = FinancialModelSpec::new("test", vec![]);
    let scenario = ScenarioSpec {
        id: "h_empty".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::HierarchyEquityPricePct {
            target: HierarchyTarget {
                path: vec!["equities".into(), "us".into()],
                tag_filter: None,
            },
            pct: -10.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: date!(2025 - 01 - 01),
    };
    let report = engine
        .apply(&scenario, &mut ctx)
        .expect("apply should succeed");

    assert_eq!(report.operations_applied, 0);
    assert_eq!(report.expanded_operations, 0);
    assert!(
        report.warnings.iter().any(|w| matches!(
            w,
            Warning::HierarchyNoMatch { op_kind, .. } if op_kind == "HierarchyEquityPricePct"
        )),
        "expected HierarchyNoMatch warning, got {:?}",
        report.warnings
    );
}

#[test]
fn market_only_context_applies_without_statement_model() {
    let mut market = MarketContext::new();
    let scenario = ScenarioSpec {
        id: "market_only_roll".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::TimeRollForward {
            period: "1D".into(),
            apply_shocks: false,
            roll_mode: TimeRollMode::CalendarDays,
        }],
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: date!(2025 - 01 - 01),
    };

    let report = engine
        .apply(&scenario, &mut ctx)
        .expect("market-only scenario should not require a statement model");

    assert_eq!(report.operations_applied, 1);
    assert!(report.changes.as_of_changed);
    assert!(report.changes.all_dirty);
    assert_eq!(ctx.as_of, date!(2025 - 01 - 02));
}

#[test]
fn application_report_requires_change_manifest() {
    let incomplete_json = r#"{
        "operations_applied": 0,
        "user_operations": 0,
        "expanded_operations": 0,
        "warnings": [],
        "meta": null
    }"#;
    let error = serde_json::from_str::<ApplicationReport>(incomplete_json)
        .expect_err("changes is required by the canonical report contract");
    assert!(error.to_string().contains("changes"));

    let report = ApplicationReport {
        operations_applied: 0,
        user_operations: 0,
        expanded_operations: 0,
        changes: ScenarioChangeManifest::default(),
        applied_shocks: Vec::new(),
        warnings: vec![],
        meta: None,
        time_roll: None,
    };
    let encoded = serde_json::to_value(&report).expect("report should serialize");
    assert_eq!(encoded["changes"]["market_targets"], serde_json::json!([]));
    assert_eq!(encoded["changes"]["all_dirty"], serde_json::json!(false));
    // An empty shock list is omitted, and a report written before the field
    // existed still deserializes.
    assert!(encoded.get("applied_shocks").is_none());
    let reparsed: ApplicationReport =
        serde_json::from_value(encoded).expect("report without applied_shocks parses");
    assert!(reparsed.applied_shocks.is_empty());
}

/// Every market shock is reported with its size and unit, in application
/// order, and reconciles with the change manifest.
#[test]
fn application_report_records_the_size_of_each_market_shock() {
    use crate::spec::{CurveKind, TenorMatchMode};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};

    let as_of = date!(2025 - 01 - 01);
    let knots = [(0.0, 1.0), (1.0, 0.97), (5.0, 0.85), (10.0, 0.70)];
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(knots)
        .build()
        .expect("discount curve should build");
    let provider = std::sync::Arc::new(SimpleFxProvider::new());
    provider
        .set_quote(Currency::EUR, Currency::USD, 1.10)
        .expect("quote should set");
    let mut market = MarketContext::new()
        .insert(curve)
        .insert_fx(FxMatrix::new(provider))
        .insert_price("AAPL", MarketScalar::Unitless(200.0));

    let scenario = ScenarioSpec {
        id: "sized".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD-OIS".into(),
                discount_curve_id: None,
                bp: 25.0,
            },
            OperationSpec::EquityPricePct {
                ids: vec!["AAPL".into()],
                pct: -10.0,
            },
            OperationSpec::MarketFxPct {
                base: Currency::EUR,
                quote: Currency::USD,
                pct: 5.0,
            },
            OperationSpec::CurveNodeBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD-OIS".into(),
                discount_curve_id: None,
                nodes: vec![("3Y".into(), 10.0)],
                match_mode: TenorMatchMode::Interpolate,
            },
        ],
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };
    let report = ScenarioEngine::new()
        .apply(&scenario, &mut ctx)
        .expect("scenario should apply");

    let curve_target = ScenarioMarketTarget::Curve {
        curve_kind: CurveKind::Discount,
        curve_id: "USD-OIS".into(),
    };
    let market = |target: &ScenarioMarketTarget| AppliedShockTarget::Market {
        target: target.clone(),
    };
    let shocks = &report.applied_shocks;
    assert_eq!(shocks.len(), report.operations_applied);

    assert_eq!(
        shocks[0],
        AppliedShock {
            applies_to: market(&curve_target),
            shock: ShockMagnitude::Uniform {
                value: 25.0,
                unit: ShockUnit::Bp
            },
            level_change: None,
        }
    );

    let equity = &shocks[1];
    assert_eq!(
        equity.applies_to,
        market(&ScenarioMarketTarget::EquityPrice {
            spot_id: "AAPL".into()
        })
    );
    assert_eq!(
        equity.shock,
        ShockMagnitude::Uniform {
            value: -10.0,
            unit: ShockUnit::Percent
        }
    );
    let level = equity.level_change.expect("scalar levels are recorded");
    assert_eq!(level.before, 200.0);
    assert!((level.after - level.before * (1.0 + -10.0 / 100.0)).abs() < 1e-12);
    let MarketScalar::Unitless(stored) = ctx.market.get_price("AAPL").expect("price") else {
        panic!("AAPL stays unitless");
    };
    assert_eq!(level.after, *stored);

    assert_eq!(
        shocks[2],
        AppliedShock {
            applies_to: market(&ScenarioMarketTarget::Fx {
                base: Currency::EUR,
                quote: Currency::USD
            }),
            shock: ShockMagnitude::Uniform {
                value: 5.0,
                unit: ShockUnit::Percent
            },
            level_change: None,
        }
    );

    // The off-knot 3Y node is delivered as key-rate bumps on the curve's own
    // knots: one entry per bump, each naming its knot.
    let key_rates = &shocks[3..];
    assert!(!key_rates.is_empty());
    for shock in key_rates {
        assert_eq!(shock.applies_to, market(&curve_target));
        let ShockMagnitude::KeyRate {
            time_years,
            value,
            unit,
        } = shock.shock
        else {
            panic!("node shock on a discount curve is a key-rate bump: {shock:?}");
        };
        assert_eq!(unit, ShockUnit::Bp);
        assert!(value.is_finite() && value != 0.0);
        assert!(knots.iter().any(|(knot, _)| *knot == time_years));
    }

    // Every manifest target has at least one sized entry, and vice versa.
    for target in &report.changes.market_targets {
        assert!(shocks.iter().any(|s| s.applies_to == market(target)));
    }
    for shock in shocks {
        let AppliedShockTarget::Market { target } = &shock.applies_to else {
            panic!("no instrument shock was requested");
        };
        assert!(report.changes.market_targets.contains(target));
    }

    let json = serde_json::to_value(&report).expect("report serializes");
    assert_eq!(
        json["applied_shocks"][0],
        serde_json::json!({
            "applies_to": {
                "scope": "market",
                "target": {"kind": "curve", "curve_kind": "discount", "curve_id": "USD-OIS"}
            },
            "shock": {"kind": "uniform", "value": 25.0, "unit": "bp"}
        })
    );
    let reparsed: ApplicationReport = serde_json::from_value(json).expect("report round-trips");
    assert_eq!(&reparsed.applied_shocks, shocks);
}

#[test]
fn statement_operation_without_model_errors_clearly() {
    let mut market = MarketContext::new();
    let scenario = ScenarioSpec {
        id: "missing_model".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::StmtForecastPercent {
            node_id: "Revenue".into(),
            pct: -10.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: date!(2025 - 01 - 01),
    };

    let err = engine
        .apply(&scenario, &mut ctx)
        .expect_err("statement operation should require a statement model");

    assert!(matches!(
        err,
        crate::error::Error::MissingStatementModel { .. }
    ));
}

fn single_op_spec(id: &str, operation: OperationSpec) -> ScenarioSpec {
    ScenarioSpec {
        id: id.into(),
        name: None,
        description: None,
        operations: vec![operation],
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    }
}

#[test]
fn apply_rejects_instrument_scoped_operations_without_inventory() {
    let operations = [
        OperationSpec::InstrumentPricePctByType {
            instrument_types: vec![finstack_quant_valuations::pricer::InstrumentType::Bond],
            pct: -5.0,
        },
        OperationSpec::AssetCorrelationPts { delta_pts: 0.02 },
    ];
    for operation in operations {
        let mut market = MarketContext::new();
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: date!(2025 - 01 - 01),
        };
        let error = ScenarioEngine::new()
            .apply(&single_op_spec("needs_inventory", operation), &mut ctx)
            .expect_err("instrument-scoped operation without an inventory must fail");
        assert!(
            matches!(&error, crate::error::Error::Validation(message)
                if message.contains("no instruments were supplied")),
            "unexpected error: {error}"
        );
    }
}

#[test]
fn apply_reports_spec_errors_before_the_missing_inventory() {
    let mut market = MarketContext::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: date!(2025 - 01 - 01),
    };
    let spec = single_op_spec("", OperationSpec::AssetCorrelationPts { delta_pts: 0.02 });
    let error = ScenarioEngine::new()
        .apply(&spec, &mut ctx)
        .expect_err("blank id must fail");
    assert!(
        error.to_string().contains("Scenario ID cannot be empty"),
        "{error}"
    );
}

#[test]
fn apply_accepts_instrument_scoped_operations_with_an_empty_inventory() {
    let mut market = MarketContext::new();
    let mut inventory = Vec::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: Some(&mut inventory),
        rate_bindings: None,
        calendar: None,
        as_of: date!(2025 - 01 - 01),
    };
    let report = ScenarioEngine::new()
        .apply(
            &single_op_spec(
                "empty",
                OperationSpec::AssetCorrelationPts { delta_pts: 0.02 },
            ),
            &mut ctx,
        )
        .expect("an empty inventory is an inventory");
    assert_eq!(report.operations_applied, 0);
}

#[test]
fn compose_rejects_invalid_inputs_naming_the_scenario() {
    let blank = single_op_spec(
        "",
        OperationSpec::StmtForecastPercent {
            node_id: "Revenue".into(),
            pct: 1.0,
        },
    );
    let valid = single_op_spec(
        "a",
        OperationSpec::StmtForecastPercent {
            node_id: "Revenue".into(),
            pct: 1.0,
        },
    );
    let error = ScenarioSpec::compose(vec![valid.clone(), blank])
        .expect_err("a blank-id input must be rejected")
        .to_string();
    assert!(
        error.contains("Cannot compose scenario ''")
            && error.contains("Scenario ID cannot be empty"),
        "{error}"
    );

    let nan = single_op_spec(
        "nan",
        OperationSpec::StmtForecastPercent {
            node_id: "Revenue".into(),
            pct: f64::NAN,
        },
    );
    let error = ScenarioSpec::compose(vec![valid, nan])
        .expect_err("a non-finite operation must be rejected")
        .to_string();
    assert!(error.contains("Cannot compose scenario 'nan'"), "{error}");
}

#[test]
fn correlation_shocks_update_inventory_and_report_clamping() {
    use finstack_quant_models::credit::pool::CorrelationStructure;
    use finstack_quant_valuations::instruments::{
        fixed_income::structured_credit::StructuredCredit, Instrument,
    };
    for (operation, expected_asset, expected_prepay, clamped) in [
        (
            OperationSpec::AssetCorrelationPts { delta_pts: 0.05 },
            0.25,
            -0.30,
            false,
        ),
        (
            OperationSpec::AssetCorrelationPts { delta_pts: 0.90 },
            0.99,
            -0.30,
            true,
        ),
        (
            OperationSpec::PrepayDefaultCorrelationPts { delta_pts: 0.10 },
            0.20,
            -0.20,
            false,
        ),
        (
            OperationSpec::PrepayDefaultCorrelationPts { delta_pts: -0.90 },
            0.20,
            -0.99,
            true,
        ),
    ] {
        let mut instrument = StructuredCredit::example().expect("example");
        instrument.credit_model.correlation_structure =
            Some(CorrelationStructure::flat(0.20, -0.30).expect("valid correlations"));
        let mut inventory: Vec<Box<dyn Instrument>> = vec![Box::new(instrument)];
        let mut market = MarketContext::new();
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: Some(&mut inventory),
            rate_bindings: None,
            calendar: None,
            as_of: date!(2025 - 01 - 01),
        };
        let (OperationSpec::AssetCorrelationPts { delta_pts }
        | OperationSpec::PrepayDefaultCorrelationPts { delta_pts }) = operation
        else {
            panic!("fixture lists correlation operations only");
        };
        let report = ScenarioEngine::default()
            .apply(&single_op_spec("correlation", operation), &mut ctx)
            .expect("apply");
        assert_eq!(report.operations_applied, 1);
        assert_eq!(report.changes.changed_instrument_indices, vec![0]);
        assert_eq!(
            report.applied_shocks,
            vec![AppliedShock {
                applies_to: AppliedShockTarget::Instruments { indices: vec![0] },
                shock: ShockMagnitude::Uniform {
                    value: delta_pts,
                    unit: ShockUnit::Absolute
                },
                level_change: None,
            }]
        );
        assert_eq!(
            report
                .warnings
                .iter()
                .any(|w| matches!(w, Warning::CorrelationClamped { .. })),
            clamped
        );
        let instrument = inventory[0]
            .as_any()
            .downcast_ref::<StructuredCredit>()
            .expect("structured credit");
        let correlations = instrument
            .credit_model
            .correlation_structure
            .as_ref()
            .expect("correlations");
        assert!((correlations.asset_correlation() - expected_asset).abs() < 1e-12);
        assert!((correlations.prepay_default_correlation() - expected_prepay).abs() < 1e-12);
    }
}
