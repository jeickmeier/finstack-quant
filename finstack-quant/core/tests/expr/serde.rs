//! Tests for expression engine serialization/deserialization.
//!
//! This module tests serde roundtrip for:
//! - Expression AST nodes (Column, Literal, Call, BinOp, IfThenElse)
//! - Function enum variants
//! - Context types (SimpleContext)

use finstack_quant_core::expr::{Expr, ExprNode, Function, SimpleContext};

#[test]
fn test_expr_ast_serde_roundtrip() {
    // Test basic expression AST serialization
    let expr = Expr::call(
        Function::RollingMean,
        vec![Expr::column("x"), Expr::literal(3.0)],
    );

    let json = serde_json::to_string(&expr).expect("Failed to serialize Expr");
    let deserialized: Expr = serde_json::from_str(&json).expect("Failed to deserialize Expr");

    assert_eq!(expr, deserialized);
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
        Function::Pow,
        Function::Round,
        Function::Floor,
        Function::Ceil,
        Function::Ln,
        Function::Exp,
        Function::Log10,
        Function::Sqrt,
        Function::Clamp,
        Function::IsMissing,
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
fn test_expr_rejects_unknown_fields() {
    let json = r#"{"id":null,"node":{"Column":"x"},"bogus":1}"#;
    let result: Result<Expr, _> = serde_json::from_str(json);
    assert!(result.is_err(), "unknown Expr fields must be rejected");
}

#[test]
fn test_expr_node_rejects_unknown_variant_fields() {
    // Struct variants of ExprNode are strict too.
    let json = r#"{"CsRef":{"component":"debt","instrument_or_total":"total","bogus":1}}"#;
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

#[test]
fn obsolete_caller_id_is_rejected() {
    let expr = Expr::column("x");
    let mut state = serde_json::to_value(&expr).unwrap();
    assert!(state.get("id").is_none());
    state["id"] = serde_json::json!(42);
    assert!(serde_json::from_value::<Expr>(state).is_err());
}
