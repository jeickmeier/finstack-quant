//! Tests for expression evaluator infrastructure.
//!
//! This module tests the core evaluation mechanics:
//! - CompiledExpr construction and evaluation
//! - Column and literal evaluation
//! - Binary operations and conditionals
//! - Metadata handling in results
//! - DAG planning integration
//! - Cache configuration
//!
//! Function-specific behavior tests are in functions.rs.

use finstack_core::config::{results_meta, FinstackConfig};
use finstack_core::expr::{BinOp, CompiledExpr, EvalOpts, Expr, Function, SimpleContext, UnaryOp};

fn create_test_data() -> (SimpleContext, Vec<Vec<f64>>) {
    let ctx = SimpleContext::new(["x", "y"]).expect("unique columns");
    let data = vec![
        vec![1.0, 2.0, 3.0, 4.0, 5.0],      // x column
        vec![10.0, 20.0, 30.0, 40.0, 50.0], // y column
    ];
    (ctx, data)
}

// =============================================================================
// Basic Evaluation: Column and Literal
// =============================================================================

#[test]
fn column_evaluation() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let col_expr = CompiledExpr::new(Expr::column("x"));
    let result = col_expr
        .eval(&ctx, &cols, EvalOpts::default())
        .unwrap()
        .values;
    assert_eq!(result, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn literal_evaluation() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let lit_expr = CompiledExpr::new(Expr::literal(42.0));
    let result = lit_expr
        .eval(&ctx, &cols, EvalOpts::default())
        .unwrap()
        .values;
    assert_eq!(result, vec![42.0, 42.0, 42.0, 42.0, 42.0]);
}

#[test]
fn literal_zero_length() {
    let ctx = SimpleContext::new(["empty"]).expect("unique columns");
    let empty_data = [Vec::<f64>::new()];
    let cols: Vec<&[f64]> = empty_data.iter().map(|v| v.as_slice()).collect();

    let lit_expr = CompiledExpr::new(Expr::literal(5.0));
    let result = lit_expr
        .eval(&ctx, &cols, EvalOpts::default())
        .unwrap()
        .values;
    assert!(result.is_empty());
}

// =============================================================================
// Binary Operations
// =============================================================================

#[test]
fn binop_add() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let add = CompiledExpr::new(Expr::bin_op(
        BinOp::Add,
        Expr::column("x"),
        Expr::literal(10.0),
    ));
    let result = add.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
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
    ));
    let result = sub.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
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
    ));
    let result = mul.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
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
    ));
    let result = div.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
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
    ));
    let result = div.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;

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
    ));
    let result = gt.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
    assert_eq!(result, vec![0.0, 0.0, 0.0, 1.0, 1.0]); // 0 = false, 1 = true

    // Less than
    let lt = CompiledExpr::new(Expr::bin_op(
        BinOp::Lt,
        Expr::column("x"),
        Expr::literal(3.0),
    ));
    let result = lt.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
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
        let expr = CompiledExpr::new(Expr::bin_op(op, Expr::column("lhs"), Expr::column("rhs")));
        let result = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
        assert_eq!(result, expected, "{op:?}");
    }
}

#[test]
fn unary_not_maps_zero_to_true_and_non_zero_to_false() {
    let ctx = SimpleContext::new(["flag"]).expect("unique columns");
    let flag = vec![0.0, 1.0, -2.0, f64::NAN];
    let cols: Vec<&[f64]> = vec![flag.as_slice()];

    let expr = CompiledExpr::new(Expr::unary_op(UnaryOp::Not, Expr::column("flag")));
    let result = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;

    assert_eq!(result, vec![1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn cs_ref_eval_returns_validation_error() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let expr = CompiledExpr::new(Expr::cs_ref("debt", "total"));
    let result = expr.eval(&ctx, &cols, EvalOpts::default());

    assert!(result.is_err());
}

#[test]
fn try_new_scalar_accepts_scalar_functions_and_rejects_statements_functions() {
    let scalar = Expr::call(
        Function::Abs,
        vec![Expr::bin_op(
            BinOp::Sub,
            Expr::column("x"),
            Expr::literal(2.0),
        )],
    );
    assert!(CompiledExpr::try_new_scalar(scalar).is_ok());

    let statements_layer = Expr::call(Function::Ttm, vec![Expr::column("x"), Expr::literal(4.0)]);
    assert!(CompiledExpr::try_new_scalar(statements_layer).is_err());
}

// =============================================================================
// Conditional Expressions
// =============================================================================

#[test]
fn if_then_else_evaluation() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    // if x > y then x - y else y - x
    let cond = Expr::bin_op(BinOp::Gt, Expr::column("x"), Expr::column("y"));
    let then_expr = Expr::bin_op(BinOp::Sub, Expr::column("x"), Expr::column("y"));
    let else_expr = Expr::bin_op(BinOp::Sub, Expr::column("y"), Expr::column("x"));
    let expr = Expr::if_then_else(cond, then_expr, else_expr);

    let compiled = CompiledExpr::new(expr);
    let out = compiled
        .eval(&ctx, &cols, EvalOpts::default())
        .unwrap()
        .values;

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

    let compiled = CompiledExpr::new(expr);
    let out = compiled
        .eval(&ctx, &cols, EvalOpts::default())
        .unwrap()
        .values;

    // x > y: [false, true, true, true]
    // Results: [2-1=1, 2-1=1, 3-0=3, 4-(-1)=5]
    assert_eq!(out, vec![1.0, 1.0, 3.0, 5.0]);
}

// =============================================================================
// Metadata and Results
// =============================================================================

#[test]
fn evaluation_result_metadata() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let expr = CompiledExpr::new(Expr::column("x"));
    let result = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap();

    assert_eq!(result.values, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    assert_eq!(format!("{:?}", result.metadata.numeric_mode), "F64");
}

// =============================================================================
// DAG Planning Integration
// =============================================================================

#[test]
fn with_planning_produces_same_result() {
    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let expr = Expr::call(
        Function::RollingMean,
        vec![Expr::column("x"), Expr::literal(2.0)],
    );

    // Without planning
    let without_planning = CompiledExpr::new(expr.clone());
    let result_no_plan = without_planning
        .eval(&ctx, &cols, EvalOpts::default())
        .unwrap()
        .values;

    // With planning
    let meta = results_meta(&FinstackConfig::default());
    let with_planning = CompiledExpr::with_planning(expr, meta).unwrap();
    let result_with_plan = with_planning
        .eval(&ctx, &cols, EvalOpts::default())
        .unwrap()
        .values;

    // Should produce identical results
    assert_eq!(result_no_plan.len(), result_with_plan.len());
    for (a, b) in result_no_plan.iter().zip(result_with_plan.iter()) {
        if a.is_nan() {
            assert!(b.is_nan());
        } else {
            assert!((a - b).abs() < 1e-15);
        }
    }
}

#[test]
fn with_cache_configuration() {
    let ctx = SimpleContext::new(["x"]).expect("unique columns");
    let x = vec![1.0, 2.0, 3.0, 4.0];
    let cols: Vec<&[f64]> = vec![x.as_slice()];

    let expr = Expr::call(
        Function::RollingSum,
        vec![Expr::column("x"), Expr::literal(2.0)],
    );

    let meta = results_meta(&FinstackConfig::default());
    let plain = CompiledExpr::with_planning(expr.clone(), meta.clone()).unwrap();
    let compiled = CompiledExpr::with_planning(expr, meta)
        .unwrap()
        .with_cache(1);
    assert!(!compiled.has_cache());

    let mut opts = EvalOpts::default();
    opts.cache_budget_mb = Some(1);
    opts.max_arena_bytes = 1_073_741_824;

    let result = compiled.eval(&ctx, &cols, opts).unwrap().values;
    let expected = plain.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;

    for (actual, expected) in result.iter().zip(expected.iter()) {
        if expected.is_nan() {
            assert!(actual.is_nan());
        } else {
            assert!((actual - expected).abs() < 1e-12);
        }
    }

    assert!(result[0].is_nan());
    assert!((result[1] - 3.0).abs() < 1e-12);
    assert!((result[2] - 5.0).abs() < 1e-12);
    assert!((result[3] - 7.0).abs() < 1e-12);
}

#[test]
fn repeated_eval_on_different_same_length_inputs_returns_fresh_results() {
    // Regression for the stale cross-eval cache (
    // ): the persistent cache
    // keyed on (dag_node_id, len) with no input fingerprint, so re-evaluating
    // the same CompiledExpr on different same-length data returned the FIRST
    // dataset's values. The shared rolling_std sub-expression below was a
    // cache node under the old strategy.
    let ctx = SimpleContext::new(["x"]).expect("unique columns");
    let rolling = Expr::call(
        Function::RollingStd,
        vec![Expr::column("x"), Expr::literal(2.0)],
    );
    let expr = Expr::bin_op(BinOp::Add, rolling.clone(), rolling);

    let meta = results_meta(&FinstackConfig::default());
    let compiled = CompiledExpr::with_planning(expr.clone(), meta.clone())
        .unwrap()
        .with_cache(8);

    let a = vec![1.0, 2.0, 3.0, 4.0];
    let cols_a: Vec<&[f64]> = vec![a.as_slice()];
    let first = compiled
        .eval(&ctx, &cols_a, EvalOpts::default())
        .unwrap()
        .values;

    let b = vec![10.0, 40.0, 90.0, 160.0];
    let cols_b: Vec<&[f64]> = vec![b.as_slice()];
    let second = compiled
        .eval(&ctx, &cols_b, EvalOpts::default())
        .unwrap()
        .values;

    // Expected: result for dataset b computed by a fresh evaluator.
    let fresh = CompiledExpr::with_planning(expr, meta)
        .unwrap()
        .eval(&ctx, &cols_b, EvalOpts::default())
        .unwrap()
        .values;

    assert_eq!(second.len(), 4);
    assert!(second[0].is_nan());
    for i in 1..4 {
        assert!(
            (second[i] - fresh[i]).abs() < 1e-12,
            "second eval [{}]: {} != fresh {}",
            i,
            second[i],
            fresh[i]
        );
        // Sanity: dataset b results must differ from dataset a results.
        assert!(
            (second[i] - first[i]).abs() > 1.0,
            "second eval [{}] returned stale first-dataset value {}",
            i,
            second[i]
        );
    }
}

#[test]
fn eval_stamps_metadata_from_planning_meta() {
    // Regression: eval() previously stamped results_meta(&FinstackConfig::default()),
    // ignoring the caller's meta passed to with_planning
    // ( "Major — expression engine").
    use finstack_core::config::RoundingMode;

    let (ctx, data) = create_test_data();
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let mut meta = results_meta(&FinstackConfig::default());
    meta.rounding.mode = RoundingMode::AwayFromZero;

    let expr = Expr::bin_op(BinOp::Add, Expr::column("x"), Expr::literal(1.0));
    let compiled = CompiledExpr::with_planning(expr, meta).unwrap();
    let result = compiled.eval(&ctx, &cols, EvalOpts::default()).unwrap();

    assert_eq!(result.values, vec![2.0, 3.0, 4.0, 5.0, 6.0]);
    assert_eq!(result.metadata.rounding.mode, RoundingMode::AwayFromZero);
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn empty_data_column() {
    let ctx = SimpleContext::new(["empty"]).expect("unique columns");
    let empty_data = [Vec::<f64>::new()];
    let cols: Vec<&[f64]> = empty_data.iter().map(|v| v.as_slice()).collect();

    let expr = CompiledExpr::new(Expr::column("empty"));
    let result = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
    assert!(result.is_empty());
}

#[test]
fn empty_data_function() {
    let ctx = SimpleContext::new(["empty"]).expect("unique columns");
    let empty_data = [Vec::<f64>::new()];
    let cols: Vec<&[f64]> = empty_data.iter().map(|v| v.as_slice()).collect();

    let expr = CompiledExpr::new(Expr::call(
        Function::RollingMean,
        vec![Expr::column("empty"), Expr::literal(2.0)],
    ));
    let result = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
    assert!(result.is_empty());
}

#[test]
fn single_element_data() {
    let ctx = SimpleContext::new(["single"]).expect("unique columns");
    let data = [vec![42.0]];
    let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

    let expr = CompiledExpr::new(Expr::column("single"));
    let result = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
    assert_eq!(result, vec![42.0]);
}

// =============================================================================
// Complex Expression Evaluation
// =============================================================================

#[test]
fn nested_function_calls() {
    let ctx = SimpleContext::new(["x"]).expect("unique columns");
    let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let cols: Vec<&[f64]> = vec![x.as_slice()];

    // rolling_mean(diff(x, 1), 2)
    let diff = Expr::call(Function::Diff, vec![Expr::column("x"), Expr::literal(1.0)]);
    let rolling_mean = Expr::call(Function::RollingMean, vec![diff, Expr::literal(2.0)]);

    let compiled = CompiledExpr::new(rolling_mean);
    let result = compiled
        .eval(&ctx, &cols, EvalOpts::default())
        .unwrap()
        .values;

    // diff(x, 1) = [NaN, 1, 1, 1, 1]
    // rolling_mean(..., 2) = [NaN, NaN, 1, 1, 1]
    assert_eq!(result.len(), 5);
    assert!(result[0].is_nan());
    assert!(result[1].is_nan());
    assert_eq!(result[2], 1.0);
    assert_eq!(result[3], 1.0);
    assert_eq!(result[4], 1.0);
}

#[test]
fn binop_with_function_result() {
    let ctx = SimpleContext::new(["x"]).expect("unique columns");
    let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let cols: Vec<&[f64]> = vec![x.as_slice()];

    // x + cumsum(x)
    let cumsum = Expr::call(Function::CumSum, vec![Expr::column("x")]);
    let add = Expr::bin_op(BinOp::Add, Expr::column("x"), cumsum);

    let compiled = CompiledExpr::new(add);
    let result = compiled
        .eval(&ctx, &cols, EvalOpts::default())
        .unwrap()
        .values;

    // cumsum(x) = [1, 3, 6, 10, 15]
    // x + cumsum(x) = [2, 5, 9, 14, 20]
    assert_eq!(result, vec![2.0, 5.0, 9.0, 14.0, 20.0]);
}

#[test]
fn missing_column_is_an_error() {
    let ctx = SimpleContext::new(["x"]).expect("unique columns");
    let x = vec![1.0, 2.0, 3.0];
    let cols: Vec<&[f64]> = vec![x.as_slice()];

    let compiled = CompiledExpr::new(Expr::column("missing"));
    let err = compiled
        .eval(&ctx, &cols, EvalOpts::default())
        .expect_err("missing columns should fail closed");

    assert!(matches!(err, finstack_core::Error::Input(_)));
}

#[test]
fn unsupported_financial_function_is_an_error() {
    let ctx = SimpleContext::new(["x"]).expect("unique columns");
    let x = vec![1.0, 2.0, 3.0];
    let cols: Vec<&[f64]> = vec![x.as_slice()];

    let compiled = CompiledExpr::new(Expr::call(Function::GrowthRate, vec![Expr::column("x")]));
    let err = compiled
        .eval(&ctx, &cols, EvalOpts::default())
        .expect_err("core expr evaluation should reject statements-layer functions");

    assert!(matches!(err, finstack_core::Error::Validation(_)));
}
