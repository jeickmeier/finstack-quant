//! Tests for expression AST construction and structural equality.
//!
//! This module tests:
//! - Expression builder methods (column, literal, call, if_then_else, etc.)
//! - Structural equality semantics (id-independent equality)
//! - Expression node types and their construction

use finstack_quant_core::expr::{BinOp, Expr, ExprNode, Function};
use std::collections::HashSet;

// =============================================================================
// Expression Builder Tests
// =============================================================================

#[test]
fn column_builder() {
    let col = Expr::column("price");
    match &col.node {
        ExprNode::Column(name) => assert_eq!(name, "price"),
        _ => panic!("Expected Column node"),
    }
}

#[test]
fn literal_builder() {
    let lit = Expr::literal(42.5);
    match &lit.node {
        ExprNode::Literal(val) => assert_eq!(*val, 42.5),
        _ => panic!("Expected Literal node"),
    }
}

#[test]
fn call_builder() {
    let call = Expr::call(Function::Lag, vec![Expr::column("x"), Expr::literal(1.0)]);
    match &call.node {
        ExprNode::Call(func, args) => {
            assert_eq!(*func, Function::Lag);
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected Call node"),
    }
}

#[test]
fn binop_builder() {
    let add = Expr::bin_op(BinOp::Add, Expr::literal(2.0), Expr::literal(3.0));
    match &add.node {
        ExprNode::BinOp { op, left, right } => {
            assert!(matches!(op, BinOp::Add));
            assert!(matches!(&left.node, ExprNode::Literal(2.0)));
            assert!(matches!(&right.node, ExprNode::Literal(3.0)));
        }
        _ => panic!("Expected BinOp node"),
    }
}

#[test]
fn if_then_else_builder() {
    let cond = Expr::bin_op(BinOp::Gt, Expr::column("x"), Expr::literal(0.0));
    let then_expr = Expr::literal(1.0);
    let else_expr = Expr::literal(-1.0);
    let ite = Expr::if_then_else(cond, then_expr, else_expr);

    match &ite.node {
        ExprNode::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => {
            assert!(matches!(&condition.node, ExprNode::BinOp { .. }));
            assert!(matches!(&then_expr.node, ExprNode::Literal(1.0)));
            assert!(matches!(&else_expr.node, ExprNode::Literal(-1.0)));
        }
        _ => panic!("Expected IfThenElse node"),
    }
}

#[test]
fn with_id_builder() {
    let expr = Expr::column("x").with_id(42);
    assert_eq!(expr.id, Some(42));
}

// =============================================================================
// Structural Equality Tests
// =============================================================================

#[test]
fn equality_ignores_id() {
    let a = Expr::column("x").with_id(1);
    let b = Expr::column("x").with_id(999);
    assert_eq!(a, b, "Expr equality must ignore id");
}

#[test]
fn equality_same_structure_different_ids() {
    let e1 = Expr::call(
        Function::RollingMean,
        vec![Expr::column("x"), Expr::literal(3.0)],
    )
    .with_id(1);
    let e2 = Expr::call(
        Function::RollingMean,
        vec![Expr::column("x"), Expr::literal(3.0)],
    )
    .with_id(999);

    assert_eq!(e1, e2, "Expr equality must ignore id");
}

#[test]
fn hash_ignores_id() {
    let e1 = Expr::call(
        Function::RollingMean,
        vec![Expr::column("x"), Expr::literal(3.0)],
    )
    .with_id(1);
    let e2 = Expr::call(
        Function::RollingMean,
        vec![Expr::column("x"), Expr::literal(3.0)],
    )
    .with_id(999);

    let mut set = HashSet::new();
    set.insert(e1);

    // Should be considered duplicate due to structural identity
    assert!(set.contains(&e2), "Hash lookup must ignore id");
    let inserted = set.insert(e2);
    assert!(
        !inserted,
        "Hash must ignore id so structural duplicates do not insert twice"
    );
}

#[test]
fn structural_identity_matches_identical_nodes() {
    let base_a = Expr::call(
        Function::RollingMean,
        vec![Expr::column("x"), Expr::literal(3.0)],
    );
    let base_b = Expr::call(
        Function::RollingMean,
        vec![Expr::column("x"), Expr::literal(3.0)],
    );
    assert_eq!(
        base_a, base_b,
        "Structural identity must match for identical nodes"
    );
}

#[test]
fn different_structure_not_equal() {
    let a = Expr::call(
        Function::RollingMean,
        vec![Expr::column("x"), Expr::literal(3.0)],
    );
    let b = Expr::call(
        Function::RollingSum,
        vec![Expr::column("x"), Expr::literal(3.0)],
    );
    assert_ne!(a, b, "Different functions should not be equal");

    let c = Expr::call(
        Function::RollingMean,
        vec![Expr::column("y"), Expr::literal(3.0)],
    );
    assert_ne!(a, c, "Different column names should not be equal");
}

// =============================================================================
// Complex Expression Tree Tests
// =============================================================================

#[test]
fn nested_expression_construction() {
    // Build: rolling_mean(diff(lag(price, 1), 1), 10)
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

    // Verify structure
    match &expr.node {
        ExprNode::Call(func, args) => {
            assert_eq!(*func, Function::RollingMean);
            assert_eq!(args.len(), 2);

            // Check nested Diff
            match &args[0].node {
                ExprNode::Call(inner_func, inner_args) => {
                    assert_eq!(*inner_func, Function::Diff);
                    assert_eq!(inner_args.len(), 2);

                    // Check nested Lag
                    match &inner_args[0].node {
                        ExprNode::Call(lag_func, lag_args) => {
                            assert_eq!(*lag_func, Function::Lag);
                            assert_eq!(lag_args.len(), 2);
                        }
                        _ => panic!("Expected Lag Call node"),
                    }
                }
                _ => panic!("Expected Diff Call node"),
            }
        }
        _ => panic!("Expected RollingMean Call node at root"),
    }
}

#[test]
fn conditional_expression_with_binops() {
    // Build: if x > y then x - y else y - x
    let cond = Expr::bin_op(BinOp::Gt, Expr::column("x"), Expr::column("y"));
    let then_expr = Expr::bin_op(BinOp::Sub, Expr::column("x"), Expr::column("y"));
    let else_expr = Expr::bin_op(BinOp::Sub, Expr::column("y"), Expr::column("x"));
    let expr = Expr::if_then_else(cond, then_expr, else_expr);

    match &expr.node {
        ExprNode::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => {
            assert!(matches!(
                &condition.node,
                ExprNode::BinOp { op: BinOp::Gt, .. }
            ));
            assert!(matches!(
                &then_expr.node,
                ExprNode::BinOp { op: BinOp::Sub, .. }
            ));
            assert!(matches!(
                &else_expr.node,
                ExprNode::BinOp { op: BinOp::Sub, .. }
            ));
        }
        _ => panic!("Expected IfThenElse node"),
    }
}
