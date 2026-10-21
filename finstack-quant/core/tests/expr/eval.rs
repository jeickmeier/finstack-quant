//! Tests for expression evaluator infrastructure.
//!
//! This module tests the core evaluation mechanics:
//! - CompiledExpr construction and evaluation
//! - Column and literal evaluation
//! - Binary operations and conditionals
//! - Metadata handling in results
//! - DAG planning integration
//!
//! Function-specific behavior tests are in functions.rs.

use finstack_quant_core::expr::{BinOp, CompiledExpr, Expr, Function, SimpleContext, UnaryOp};

fn create_test_data() -> (SimpleContext, Vec<Vec<f64>>) {
    let ctx = SimpleContext::new(["x", "y"]).expect("unique columns");
    let data = vec![
        vec![1.0, 2.0, 3.0, 4.0, 5.0],      // x column
        vec![10.0, 20.0, 30.0, 40.0, 50.0], // y column
    ];
    (ctx, data)
}

// Basic Evaluation: Column and Literal

#[test]
fn column_evaluation() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let col_expr = CompiledExpr::new(Expr::column("x")).expect("elementwise expression compiles");
    let result = col_expr.eval(&ctx, &cols).unwrap();
    assert_eq!(result, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn literal_evaluation() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let lit_expr = CompiledExpr::new(Expr::literal(42.0)).expect("elementwise expression compiles");
    let result = lit_expr.eval(&ctx, &cols).unwrap();
    assert_eq!(result, vec![42.0, 42.0, 42.0, 42.0, 42.0]);
}

#[test]
fn literal_zero_length() {
    let ctx = SimpleContext::new(["empty"]).expect("unique columns");
    let empty_data = [Vec::<f64>::new()];
    let cols: Vec<&[f64]> = empty_data.iter().map(|v| v.as_slice()).collect();

    let lit_expr = CompiledExpr::new(Expr::literal(5.0)).expect("elementwise expression compiles");
    let result = lit_expr.eval(&ctx, &cols).unwrap();
    assert!(result.is_empty());
}

// Binary Operations

#[test]
fn binop_add() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let add = CompiledExpr::new(Expr::bin_op(
        BinOp::Add,
        Expr::column("x"),
        Expr::literal(10.0),
    ))
    .expect("elementwise expression compiles");
    let result = add.eval(&ctx, &cols).unwrap();
    assert_eq!(result, vec![11.0, 12.0, 13.0, 14.0, 15.0]);
}

#[test]
fn binop_sub() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let sub = CompiledExpr::new(Expr::bin_op(
        BinOp::Sub,
        Expr::column("y"),
        Expr::column("x"),
    ))
    .expect("elementwise expression compiles");
    let result = sub.eval(&ctx, &cols).unwrap();
    assert_eq!(result, vec![9.0, 18.0, 27.0, 36.0, 45.0]);
}

#[test]
fn binop_mul() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let mul = CompiledExpr::new(Expr::bin_op(
        BinOp::Mul,
        Expr::column("x"),
        Expr::literal(2.0),
    ))
    .expect("elementwise expression compiles");
    let result = mul.eval(&ctx, &cols).unwrap();
    assert_eq!(result, vec![2.0, 4.0, 6.0, 8.0, 10.0]);
}

#[test]
fn binop_div() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let div = CompiledExpr::new(Expr::bin_op(
        BinOp::Div,
        Expr::column("y"),
        Expr::column("x"),
    ))
    .expect("elementwise expression compiles");
    let result = div.eval(&ctx, &cols).unwrap();
    assert_eq!(result, vec![10.0, 10.0, 10.0, 10.0, 10.0]);
}

#[test]
fn binop_division_by_zero_returns_nan() {
    let ctx = SimpleContext::new(["num", "den"]).expect("unique columns");
    let num = vec![1.0, -2.0, 0.0, 4.0];
    let den = vec![0.0, 0.0, 0.0, 2.0];
    let cols: Vec<&[f64]> = vec![num.as_slice(), den.as_slice()];

    let div = CompiledExpr::new(Expr::bin_op(
        BinOp::Div,
        Expr::column("num"),
        Expr::column("den"),
    ))
    .expect("elementwise expression compiles");
    let result = div.eval(&ctx, &cols).unwrap();

    assert!(result[0].is_nan());
    assert!(result[1].is_nan());
    assert!(result[2].is_nan());
    assert_eq!(result[3], 2.0);
}

#[test]
fn binop_comparisons() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    // Greater than
    let gt = CompiledExpr::new(Expr::bin_op(
        BinOp::Gt,
        Expr::column("x"),
        Expr::literal(3.0),
    ))
    .expect("elementwise expression compiles");
    let result = gt.eval(&ctx, &cols).unwrap();
    assert_eq!(result, vec![0.0, 0.0, 0.0, 1.0, 1.0]); // 0 = false, 1 = true

    // Less than
    let lt = CompiledExpr::new(Expr::bin_op(
        BinOp::Lt,
        Expr::column("x"),
        Expr::literal(3.0),
    ))
    .expect("elementwise expression compiles");
    let result = lt.eval(&ctx, &cols).unwrap();
    assert_eq!(result, vec![1.0, 1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn binop_extended_comparisons_logic_and_modulo() {
    let ctx = SimpleContext::new(["lhs", "rhs"]).expect("unique columns");
    let lhs = vec![4.0, 5.0, 5.0, 0.0];
    let rhs = vec![2.0, 5.0, 6.0, 1.0];
    let cols: Vec<&[f64]> = vec![lhs.as_slice(), rhs.as_slice()];

    let cases = [
        (BinOp::Mod, vec![0.0, 0.0, 5.0, 0.0]),
        (BinOp::Eq, vec![0.0, 1.0, 0.0, 0.0]),
        (BinOp::Ne, vec![1.0, 0.0, 1.0, 1.0]),
        (BinOp::Le, vec![0.0, 1.0, 1.0, 1.0]),
        (BinOp::Ge, vec![1.0, 1.0, 0.0, 0.0]),
        (BinOp::And, vec![1.0, 1.0, 1.0, 0.0]),
        (BinOp::Or, vec![1.0, 1.0, 1.0, 1.0]),
    ];

    for (op, expected) in cases {
        let expr = CompiledExpr::new(Expr::bin_op(op, Expr::column("lhs"), Expr::column("rhs")))
            .expect("elementwise expression compiles");
        let result = expr.eval(&ctx, &cols).unwrap();
        assert_eq!(result, expected, "{op:?}");
    }
}

#[test]
fn unary_not_maps_zero_to_true_and_non_zero_to_false() {
    let ctx = SimpleContext::new(["flag"]).expect("unique columns");
    let flag = vec![0.0, 1.0, -2.0, f64::NAN];
    let cols: Vec<&[f64]> = vec![flag.as_slice()];

    let expr = CompiledExpr::new(Expr::unary_op(UnaryOp::Not, Expr::column("flag")))
        .expect("elementwise expression compiles");
    let result = expr.eval(&ctx, &cols).unwrap();

    assert_eq!(result, vec![1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn cs_ref_is_rejected_at_compile_time() {
    let err = CompiledExpr::new(Expr::cs_ref("debt", "total"))
        .expect_err("capital-structure references need the statements evaluator");
    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
}

#[test]
fn new_accepts_elementwise_functions_and_rejects_statements_functions() {
    let scalar = Expr::call(
        Function::Abs,
        vec![Expr::bin_op(
            BinOp::Sub,
            Expr::column("x"),
            Expr::literal(2.0),
        )],
    );
    assert!(CompiledExpr::new(scalar).is_ok());

    for fun in [
        Function::Ttm,
        Function::RollingMean,
        Function::Lag,
        Function::Std,
    ] {
        let statements_layer = Expr::call(fun, vec![Expr::column("x"), Expr::literal(4.0)]);
        assert!(
            CompiledExpr::new(statements_layer).is_err(),
            "{fun} must be rejected"
        );
    }
}

// Conditional Expressions

#[test]
fn if_then_else_evaluation() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    // if x > y then x - y else y - x
    let cond = Expr::bin_op(BinOp::Gt, Expr::column("x"), Expr::column("y"));
    let then_expr = Expr::bin_op(BinOp::Sub, Expr::column("x"), Expr::column("y"));
    let else_expr = Expr::bin_op(BinOp::Sub, Expr::column("y"), Expr::column("x"));
    let expr = Expr::if_then_else(cond, then_expr, else_expr);

    let compiled = CompiledExpr::new(expr).expect("elementwise expression compiles");
    let out = compiled.eval(&ctx, &cols).unwrap();

    // x: [1, 2, 3, 4, 5], y: [10, 20, 30, 40, 50]
    // x > y is always false, so we get y - x
    assert_eq!(out, vec![9.0, 18.0, 27.0, 36.0, 45.0]);
}

#[test]
fn if_then_else_mixed_condition() {
    let ctx = SimpleContext::new(["x", "y"]).expect("unique columns");
    let x = vec![1.0, 2.0, 3.0, 4.0];
    let y = vec![2.0, 1.0, 0.0, -1.0];
    let cols: Vec<&[f64]> = vec![x.as_slice(), y.as_slice()];

    // if x > y then x - y else y - x
    let cond = Expr::bin_op(BinOp::Gt, Expr::column("x"), Expr::column("y"));
    let then_expr = Expr::bin_op(BinOp::Sub, Expr::column("x"), Expr::column("y"));
    let else_expr = Expr::bin_op(BinOp::Sub, Expr::column("y"), Expr::column("x"));
    let expr = Expr::if_then_else(cond, then_expr, else_expr);

    let compiled = CompiledExpr::new(expr).expect("elementwise expression compiles");
    let out = compiled.eval(&ctx, &cols).unwrap();

    // x > y: [false, true, true, true]
    // Results: [2-1=1, 2-1=1, 3-0=3, 4-(-1)=5]
    assert_eq!(out, vec![1.0, 1.0, 3.0, 5.0]);
}

// Metadata and Results

// Edge Cases

#[test]
fn empty_data_column() {
    let ctx = SimpleContext::new(["empty"]).expect("unique columns");
    let empty_data = [Vec::<f64>::new()];
    let cols: Vec<&[f64]> = empty_data.iter().map(|v| v.as_slice()).collect();

    let expr = CompiledExpr::new(Expr::column("empty")).expect("elementwise expression compiles");
    let result = expr.eval(&ctx, &cols).unwrap();
    assert!(result.is_empty());
}

#[test]
fn empty_data_function() {
    let ctx = SimpleContext::new(["empty"]).expect("unique columns");
    let empty_data = [Vec::<f64>::new()];
    let cols: Vec<&[f64]> = empty_data.iter().map(|v| v.as_slice()).collect();

    let expr = CompiledExpr::new(Expr::call(Function::Abs, vec![Expr::column("empty")]))
        .expect("elementwise expression compiles");
    let result = expr.eval(&ctx, &cols).unwrap();
    assert!(result.is_empty());
}

#[test]
fn single_element_data() {
    let ctx = SimpleContext::new(["single"]).expect("unique columns");
    let data = [vec![42.0]];
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let expr = CompiledExpr::new(Expr::column("single")).expect("elementwise expression compiles");
    let result = expr.eval(&ctx, &cols).unwrap();
    assert_eq!(result, vec![42.0]);
}

// Complex Expression Evaluation

#[test]
fn nested_function_calls() {
    let ctx = SimpleContext::new(["x"]).expect("unique columns");
    let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let cols: Vec<&[f64]> = vec![x.as_slice()];

    // sqrt(abs(x - 3))
    let shifted = Expr::bin_op(BinOp::Sub, Expr::column("x"), Expr::literal(3.0));
    let abs = Expr::call(Function::Abs, vec![shifted]);
    let sqrt = Expr::call(Function::Sqrt, vec![abs]);

    let compiled = CompiledExpr::new(sqrt).expect("elementwise expression compiles");
    let result = compiled.eval(&ctx, &cols).unwrap();

    // abs(x - 3) = [2, 1, 0, 1, 2]
    assert_eq!(result.len(), 5);
    assert_eq!(result[0], 2f64.sqrt());
    assert_eq!(result[1], 1.0);
    assert_eq!(result[2], 0.0);
    assert_eq!(result[3], 1.0);
    assert_eq!(result[4], 2f64.sqrt());
}

#[test]
fn binop_with_function_result() {
    let ctx = SimpleContext::new(["x"]).expect("unique columns");
    let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let cols: Vec<&[f64]> = vec![x.as_slice()];

    // x + abs(x)
    let abs = Expr::call(Function::Abs, vec![Expr::column("x")]);
    let add = Expr::bin_op(BinOp::Add, Expr::column("x"), abs);

    let compiled = CompiledExpr::new(add).expect("elementwise expression compiles");
    let result = compiled.eval(&ctx, &cols).unwrap();

    assert_eq!(result, vec![2.0, 4.0, 6.0, 8.0, 10.0]);
}

#[test]
fn missing_column_is_an_error() {
    let ctx = SimpleContext::new(["x"]).expect("unique columns");
    let x = vec![1.0, 2.0, 3.0];
    let cols: Vec<&[f64]> = vec![x.as_slice()];

    let compiled =
        CompiledExpr::new(Expr::column("missing")).expect("elementwise expression compiles");
    let err = compiled
        .eval(&ctx, &cols)
        .expect_err("missing columns should fail closed");

    assert!(matches!(err, finstack_quant_core::Error::Input(_)));
}

#[test]
fn unsupported_financial_function_is_an_error() {
    let ctx = SimpleContext::new(["x"]).expect("unique columns");
    let x = vec![1.0, 2.0, 3.0];
    let cols: Vec<&[f64]> = vec![x.as_slice()];

    let err = CompiledExpr::new(Expr::call(Function::GrowthRate, vec![Expr::column("x")]))
        .expect_err("core expr compilation should reject statements-layer functions");
    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
    let _ = (ctx, cols);
}
