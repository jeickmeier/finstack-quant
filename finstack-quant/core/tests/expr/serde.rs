//! Tests for expression engine serialization/deserialization.
//!
//! This module tests serde roundtrip for:
//! - Expression AST nodes (Column, Literal, Call, BinOp, IfThenElse)
//! - Function enum variants
//! - Evaluation types (CompiledExpr, EvalOpts, EvaluationResult)
//! - Context types (SimpleContext)

use finstack_quant_core::config::{
    NumericMode, ResultsMeta, RoundingContext, RoundingMode, ToleranceConfig,
};
use finstack_quant_core::expr::{
    CompiledExpr, EvalOpts, EvaluationResult, Expr, ExprNode, Function, SimpleContext,
};
use std::collections::BTreeMap;

#[test]
fn test_expr_ast_serde_roundtrip() {
    // Test basic expression AST serialization
    let expr = Expr::call(
        Function::RollingMean,
        vec![Expr::column("x"), Expr::literal(3.0)],
    )
    .with_id(42);

    let json = serde_json::to_string(&expr).expect("Failed to serialize Expr");
    let deserialized: Expr = serde_json::from_str(&json).expect("Failed to deserialize Expr");

    assert_eq!(expr.id, deserialized.id);
    match (&expr.node, &deserialized.node) {
        (ExprNode::Call(f1, args1), ExprNode::Call(f2, args2)) => {
            assert_eq!(f1, f2);
            assert_eq!(args1.len(), args2.len());
        }
        _ => panic!("Node type mismatch"),
    }
}

#[test]
fn test_expr_node_types_serde() {
    // Test Column node
    let col_node = ExprNode::Column("price".to_string());
    let json = serde_json::to_string(&col_node).expect("Failed to serialize Column");
    let deserialized: ExprNode = serde_json::from_str(&json).expect("Failed to deserialize Column");
    match deserialized {
        ExprNode::Column(name) => assert_eq!(name, "price"),
        _ => panic!("Expected Column node"),
    }

    // Test Literal node
    let lit_node = ExprNode::Literal(42.5);
    let json = serde_json::to_string(&lit_node).expect("Failed to serialize Literal");
    let deserialized: ExprNode =
        serde_json::from_str(&json).expect("Failed to deserialize Literal");
    match deserialized {
        ExprNode::Literal(val) => assert_eq!(val, 42.5),
        _ => panic!("Expected Literal node"),
    }

    // Test Call node
    let call_node = ExprNode::Call(
        Function::Lag,
        vec![Expr::column("value"), Expr::literal(1.0)],
    );
    let json = serde_json::to_string(&call_node).expect("Failed to serialize Call");
    let deserialized: ExprNode = serde_json::from_str(&json).expect("Failed to deserialize Call");
    match deserialized {
        ExprNode::Call(func, args) => {
            assert_eq!(func, Function::Lag);
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected Call node"),
    }
}

#[test]
fn test_function_enum_serde() {
    // Test all function variants
    let functions = vec![
        Function::Lag,
        Function::Lead,
        Function::Diff,
        Function::PctChange,
        Function::CumSum,
        Function::CumProd,
        Function::CumMin,
        Function::CumMax,
        Function::RollingMean,
        Function::RollingSum,
        Function::EwmMean,
        Function::Std,
        Function::Var,
        Function::Median,
        Function::RollingStd,
        Function::RollingVar,
        Function::RollingMedian,
        Function::Shift,
        Function::Rank,
        Function::Quantile,
        Function::RollingMin,
        Function::RollingMax,
        Function::RollingCount,
        Function::EwmStd,
        Function::EwmVar,
    ];

    for func in functions {
        let json = serde_json::to_string(&func)
            .unwrap_or_else(|_| panic!("Failed to serialize {:?}", func));
        let deserialized: Function = serde_json::from_str(&json)
            .unwrap_or_else(|_| panic!("Failed to deserialize {:?}", func));
        assert_eq!(func, deserialized);
    }
}

#[test]
fn test_evaluation_result_serde() {
    let result = EvaluationResult {
        values: vec![1.0, 2.0, 3.0, 4.0, 5.0],
        metadata: ResultsMeta {
            numeric_mode: NumericMode::F64,
            rounding: RoundingContext {
                mode: RoundingMode::Bankers,
                ingest_scale_by_ccy: BTreeMap::new(),
                output_scale_by_ccy: BTreeMap::new(),
                tolerances: ToleranceConfig::default(),
                version: 1,
            },
            fx_policy_applied: None,
            timestamp: None,
            version: None,
        },
    };

    let json = serde_json::to_string(&result).expect("Failed to serialize EvaluationResult");
    let deserialized: EvaluationResult =
        serde_json::from_str(&json).expect("Failed to deserialize EvaluationResult");

    assert_eq!(result.values, deserialized.values);
    assert_eq!(
        result.metadata.numeric_mode,
        deserialized.metadata.numeric_mode
    );
    assert_eq!(
        result.metadata.rounding.version,
        deserialized.metadata.rounding.version
    );
}

#[test]
fn test_eval_opts_serde() {
    let mut opts = EvalOpts::default();
    opts.cache_budget_mb = Some(256);
    opts.max_arena_bytes = 1_073_741_824;

    let json = serde_json::to_string(&opts).expect("Failed to serialize EvalOpts");

    // `plan` is #[serde(skip)]: the internal execution plan (DagNode topology,
    // CacheStrategy, etc.) is intentionally NOT part of the wire format, so a
    // deserialized EvalOpts can never inject an arbitrary plan that eval()
    // would execute instead of the compiled AST. This pins the field's
    // absence from the serialized form.
    assert!(
        !json.contains("\"plan\""),
        "EvalOpts.plan must not be serialized, got: {json}"
    );

    let deserialized: EvalOpts =
        serde_json::from_str(&json).expect("Failed to deserialize EvalOpts");

    assert_eq!(opts.cache_budget_mb, deserialized.cache_budget_mb);
    assert_eq!(opts.max_arena_bytes, deserialized.max_arena_bytes);
    assert!(!deserialized.has_plan());
}

#[test]
fn test_eval_opts_rejects_inbound_plan_field() {
    // EvalOpts is strict (deny_unknown_fields) and `plan` is serde-skipped:
    // payloads carrying a `plan` field (as serialized by older versions) must
    // be rejected rather than silently executing an injected plan.
    let json = r#"{"plan":null,"cache_budget_mb":null,"max_arena_bytes":1024}"#;
    let result: Result<EvalOpts, _> = serde_json::from_str(json);
    assert!(result.is_err(), "inbound `plan` field must be rejected");
}

#[test]
fn test_eval_opts_rejects_unknown_fields() {
    let json = r#"{"cache_budget_mb":null,"max_arena_bytes":1024,"bogus":1}"#;
    let result: Result<EvalOpts, _> = serde_json::from_str(json);
    assert!(result.is_err(), "unknown fields must be rejected");
}

#[test]
fn test_expr_rejects_unknown_fields() {
    let json = r#"{"id":null,"node":{"Column":"x"},"bogus":1}"#;
    let result: Result<Expr, _> = serde_json::from_str(json);
    assert!(result.is_err(), "unknown Expr fields must be rejected");
}

#[test]
fn test_expr_node_rejects_unknown_variant_fields() {
    // Struct variants of ExprNode are strict too.
    let json = r#"{"CSRef":{"component":"debt","instrument_or_total":"total","bogus":1}}"#;
    let result: Result<ExprNode, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "unknown fields inside ExprNode struct variants must be rejected"
    );
}

#[test]
fn test_simple_context_rejects_unknown_fields() {
    let json = r#"{"column_indices":{"x":0},"bogus":1}"#;
    let result: Result<SimpleContext, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "unknown SimpleContext fields must be rejected"
    );
}

#[test]
fn test_compiled_expr_serde() {
    // Create a compiled expression with a plan
    let meta = ResultsMeta {
        numeric_mode: NumericMode::F64,
        rounding: RoundingContext {
            mode: RoundingMode::Bankers,
            ingest_scale_by_ccy: BTreeMap::new(),
            output_scale_by_ccy: BTreeMap::new(),
            tolerances: ToleranceConfig::default(),
            version: 1,
        },
        fx_policy_applied: None,
        timestamp: None,
        version: None,
    };

    let expr = Expr::call(
        Function::RollingSum,
        vec![Expr::column("values"), Expr::literal(5.0)],
    );

    let compiled = CompiledExpr::with_planning(expr, meta).unwrap();

    let json = serde_json::to_string(&compiled).expect("Failed to serialize CompiledExpr");
    let deserialized: CompiledExpr =
        serde_json::from_str(&json).expect("Failed to deserialize CompiledExpr");

    // Verify AST is preserved
    assert_eq!(compiled.ast.id, deserialized.ast.id);

    // Verify plan is preserved if it existed
    assert_eq!(compiled.has_plan(), deserialized.has_plan());

    // Cache should be None after deserialization (it's skipped)
    assert!(!deserialized.has_cache());
}

#[test]
fn test_simple_context_serde() {
    let context = SimpleContext::new(vec!["price", "volume", "timestamp"]).expect("unique columns");

    let json = serde_json::to_string(&context).expect("Failed to serialize SimpleContext");
    let deserialized: SimpleContext =
        serde_json::from_str(&json).expect("Failed to deserialize SimpleContext");

    // Verify indices are preserved
    assert_eq!(context.index_of("price"), deserialized.index_of("price"));
    assert_eq!(context.index_of("volume"), deserialized.index_of("volume"));
    assert_eq!(
        context.index_of("timestamp"),
        deserialized.index_of("timestamp")
    );
    assert_eq!(
        context.index_of("unknown"),
        deserialized.index_of("unknown")
    );
}

// Note: CachedResult and CacheStats are internal types (pub(crate))
// and cannot be tested from external tests. Their serialization is
// tested indirectly through the public API types that use them.

#[test]
fn test_complex_expression_tree_serde() {
    // Build a complex expression tree
    let expr = Expr::call(
        Function::RollingMean,
        vec![
            Expr::call(
                Function::Diff,
                vec![
                    Expr::call(
                        Function::Lag,
                        vec![Expr::column("price"), Expr::literal(1.0)],
                    ),
                    Expr::literal(1.0),
                ],
            ),
            Expr::literal(10.0),
        ],
    );

    let json = serde_json::to_string(&expr).expect("Failed to serialize complex expression");
    let deserialized: Expr =
        serde_json::from_str(&json).expect("Failed to deserialize complex expression");

    // Verify structure is preserved
    match &deserialized.node {
        ExprNode::Call(func, args) => {
            assert_eq!(*func, Function::RollingMean);
            assert_eq!(args.len(), 2);

            // Check nested structure
            match &args[0].node {
                ExprNode::Call(inner_func, inner_args) => {
                    assert_eq!(*inner_func, Function::Diff);
                    assert_eq!(inner_args.len(), 2);
                }
                _ => panic!("Expected nested Call node"),
            }
        }
        _ => panic!("Expected Call node at root"),
    }
}
