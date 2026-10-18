//! Tests for the elementwise expression functions evaluated by core.
//!
//! Windowed, cumulative, positional and reducing functions are evaluated by
//! the `statements` crate; `CompiledExpr::new` rejects them (see eval.rs).

use finstack_quant_core::expr::{BinOp, CompiledExpr, Expr, Function, SimpleContext};

fn to_slice_refs(data: &[Vec<f64>]) -> Vec<&[f64]> {
    data.iter().map(|v| v.as_slice()).collect()
}

mod elementwise_math {
    use super::*;

    fn eval_call(
        func: Function,
        args: Vec<Expr>,
        cols: &[&[f64]],
        ctx: &SimpleContext,
    ) -> Vec<f64> {
        CompiledExpr::new(Expr::call(func, args))
            .expect("elementwise expression compiles")
            .eval(ctx, cols)
            .expect("element-wise function should evaluate")
            .values
    }

    #[test]
    fn binary_op_missing_tail_yields_nan() {
        let ctx = SimpleContext::new(["lhs", "rhs"]).expect("unique columns");
        let lhs = vec![1.0, 2.0, 3.0, 4.0];
        let rhs = vec![10.0, 20.0];
        let cols = vec![lhs.as_slice(), rhs.as_slice()];

        let expr = CompiledExpr::new(Expr::bin_op(
            BinOp::Add,
            Expr::column("lhs"),
            Expr::column("rhs"),
        ))
        .expect("elementwise expression compiles");
        let result = expr.eval(&ctx, &cols).unwrap().values;

        assert_eq!(result[0], 11.0);
        assert_eq!(result[1], 22.0);
        assert!(result[2].is_nan());
        assert!(result[3].is_nan());
    }

    #[test]
    fn abs_and_sign_apply_elementwise_and_preserve_nan() {
        let ctx = SimpleContext::new(["v"]).expect("unique columns");
        let v = vec![-3.0, 0.0, 4.0, f64::NAN];
        let cols: Vec<&[f64]> = vec![v.as_slice()];

        let abs = CompiledExpr::new(Expr::call(Function::Abs, vec![Expr::column("v")]))
            .expect("elementwise expression compiles");
        let abs_result = abs.eval(&ctx, &cols).unwrap().values;
        assert_eq!(&abs_result[..3], &[3.0, 0.0, 4.0]);
        assert!(abs_result[3].is_nan());

        let sign = CompiledExpr::new(Expr::call(Function::Sign, vec![Expr::column("v")]))
            .expect("elementwise expression compiles");
        let sign_result = sign.eval(&ctx, &cols).unwrap().values;
        assert_eq!(&sign_result[..3], &[-1.0, 0.0, 1.0]);
        assert!(sign_result[3].is_nan());
    }

    #[test]
    fn pow_basic_and_ieee_edge_cases() {
        let ctx = SimpleContext::new(["base"]).expect("unique columns");
        let data = [vec![2.0, 9.0, -1.0, 0.0]];
        let cols = to_slice_refs(&data);

        let result = eval_call(
            Function::Pow,
            vec![Expr::column("base"), Expr::literal(0.5)],
            &cols,
            &ctx,
        );
        assert!((result[0] - 2.0_f64.sqrt()).abs() < 1e-12);
        assert!((result[1] - 3.0).abs() < 1e-12);
        assert!(result[2].is_nan()); // (-1)^0.5
        assert_eq!(result[3], 0.0);

        let result = eval_call(
            Function::Pow,
            vec![Expr::column("base"), Expr::literal(-1.0)],
            &cols,
            &ctx,
        );
        assert!((result[0] - 0.5).abs() < 1e-12);
        assert!(result[3].is_infinite() && result[3] > 0.0); // 0^-1
    }

    #[test]
    fn round_default_digits_ties_away_from_zero() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let data = [vec![2.5, -2.5, 1.4, -1.4, f64::NAN]];
        let cols = to_slice_refs(&data);

        let result = eval_call(Function::Round, vec![Expr::column("x")], &cols, &ctx);
        assert_eq!(result[0], 3.0);
        assert_eq!(result[1], -3.0);
        assert_eq!(result[2], 1.0);
        assert_eq!(result[3], -1.0);
        assert!(result[4].is_nan());
    }

    #[test]
    fn round_positive_and_negative_digits() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let data = [vec![2.34567, 1234.0, -1250.0]];
        let cols = to_slice_refs(&data);

        let result = eval_call(
            Function::Round,
            vec![Expr::column("x"), Expr::literal(2.0)],
            &cols,
            &ctx,
        );
        assert!((result[0] - 2.35).abs() < 1e-12);

        let result = eval_call(
            Function::Round,
            vec![Expr::column("x"), Expr::literal(-2.0)],
            &cols,
            &ctx,
        );
        assert_eq!(result[1], 1200.0);
        assert_eq!(result[2], -1300.0); // half away from zero at the hundreds
    }

    #[test]
    fn round_fractional_digits_yields_nan() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let data = [vec![1.0, 2.0]];
        let cols = to_slice_refs(&data);

        let result = eval_call(
            Function::Round,
            vec![Expr::column("x"), Expr::literal(1.5)],
            &cols,
            &ctx,
        );
        assert!(result.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn floor_and_ceil() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let data = [vec![1.5, -1.5, 2.0, f64::NAN]];
        let cols = to_slice_refs(&data);

        let result = eval_call(Function::Floor, vec![Expr::column("x")], &cols, &ctx);
        assert_eq!(result[0], 1.0);
        assert_eq!(result[1], -2.0);
        assert_eq!(result[2], 2.0);
        assert!(result[3].is_nan());

        let result = eval_call(Function::Ceil, vec![Expr::column("x")], &cols, &ctx);
        assert_eq!(result[0], 2.0);
        assert_eq!(result[1], -1.0);
        assert_eq!(result[2], 2.0);
        assert!(result[3].is_nan());
    }

    #[test]
    fn ln_exp_log10_sqrt_ieee_semantics() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let data = [vec![1.0, std::f64::consts::E, 0.0, -1.0]];
        let cols = to_slice_refs(&data);

        let result = eval_call(Function::Ln, vec![Expr::column("x")], &cols, &ctx);
        assert_eq!(result[0], 0.0);
        assert!((result[1] - 1.0).abs() < 1e-12);
        assert!(result[2].is_infinite() && result[2] < 0.0); // ln(0) = -inf
        assert!(result[3].is_nan()); // ln(-1)

        let result = eval_call(Function::Exp, vec![Expr::column("x")], &cols, &ctx);
        assert!((result[0] - std::f64::consts::E).abs() < 1e-12);
        assert_eq!(result[2], 1.0);

        let data10 = [vec![100.0, 1.0, 0.0, -10.0]];
        let cols10 = to_slice_refs(&data10);
        let result = eval_call(Function::Log10, vec![Expr::column("x")], &cols10, &ctx);
        assert!((result[0] - 2.0).abs() < 1e-12);
        assert_eq!(result[1], 0.0);
        assert!(result[2].is_infinite() && result[2] < 0.0);
        assert!(result[3].is_nan());

        let data_sq = [vec![4.0, 0.0, -4.0, 2.0]];
        let cols_sq = to_slice_refs(&data_sq);
        let result = eval_call(Function::Sqrt, vec![Expr::column("x")], &cols_sq, &ctx);
        assert_eq!(result[0], 2.0);
        assert_eq!(result[1], 0.0);
        assert!(result[2].is_nan());
        assert!((result[3] - 2.0_f64.sqrt()).abs() < 1e-15);
    }

    #[test]
    fn clamp_range_nan_and_inverted_bounds() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let data = [vec![-5.0, 5.0, 15.0, f64::NAN]];
        let cols = to_slice_refs(&data);

        let result = eval_call(
            Function::Clamp,
            vec![Expr::column("x"), Expr::literal(0.0), Expr::literal(10.0)],
            &cols,
            &ctx,
        );
        assert_eq!(result[0], 0.0);
        assert_eq!(result[1], 5.0);
        assert_eq!(result[2], 10.0);
        assert!(result[3].is_nan());

        // Inverted range must return NaN, never panic.
        let result = eval_call(
            Function::Clamp,
            vec![Expr::column("x"), Expr::literal(10.0), Expr::literal(0.0)],
            &cols,
            &ctx,
        );
        assert!(result.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn is_missing_flags_non_finite_only() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let data = [vec![1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0]];
        let cols = to_slice_refs(&data);

        let result = eval_call(Function::IsMissing, vec![Expr::column("x")], &cols, &ctx);
        assert_eq!(result, vec![0.0, 1.0, 1.0, 1.0, 0.0]);
    }

    #[test]
    fn pow_composes_with_arithmetic() {
        // 1 / (1 + r)^n discount-factor shape: pow feeding a division.
        let ctx = SimpleContext::new(["r"]).expect("unique columns");
        let data = [vec![0.05, 0.10]];
        let cols = to_slice_refs(&data);

        let one_plus_r = Expr::bin_op(BinOp::Add, Expr::literal(1.0), Expr::column("r"));
        let denom = Expr::call(Function::Pow, vec![one_plus_r, Expr::literal(3.0)]);
        let expr = CompiledExpr::new(Expr::bin_op(BinOp::Div, Expr::literal(1.0), denom))
            .expect("elementwise expression compiles");
        let result = expr.eval(&ctx, &cols).unwrap().values;

        assert!((result[0] - 1.0 / 1.05_f64.powi(3)).abs() < 1e-12);
        assert!((result[1] - 1.0 / 1.10_f64.powi(3)).abs() < 1e-12);
    }
}
