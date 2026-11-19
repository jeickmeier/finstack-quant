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
            .eval(ctx, cols, cols[0].len())
            .expect("element-wise function should evaluate")
    }

    #[test]
    fn misaligned_columns_fail_before_evaluation() {
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
        assert!(expr.eval(&ctx, &cols, 4).is_err());
        assert!(expr.eval(&ctx, &cols, 2).is_err());
        assert!(expr.eval(&ctx, &cols[..1], 4).is_err());
    }

    #[test]
    fn abs_and_sign_apply_elementwise_and_preserve_nan() {
        let ctx = SimpleContext::new(["v"]).expect("unique columns");
        let v = vec![-3.0, 0.0, 4.0, f64::NAN];
        let cols: Vec<&[f64]> = vec![v.as_slice()];

        let abs = CompiledExpr::new(Expr::call(Function::Abs, vec![Expr::column("v")]))
            .expect("elementwise expression compiles");
        let abs_result = abs.eval(&ctx, &cols, cols[0].len()).unwrap();
        assert_eq!(&abs_result[..3], &[3.0, 0.0, 4.0]);
        assert!(abs_result[3].is_nan());

        let sign = CompiledExpr::new(Expr::call(Function::Sign, vec![Expr::column("v")]))
            .expect("elementwise expression compiles");
        let sign_result = sign.eval(&ctx, &cols, cols[0].len()).unwrap();
        assert_eq!(&sign_result[..3], &[-1.0, 0.0, 1.0]);
        assert!(sign_result[3].is_nan());
    }

    #[test]
    fn pow_handles_missing_before_identities_and_rejects_invalid_pairs() {
        let ctx = SimpleContext::new(["base", "exponent"]).unwrap();
        let expr = CompiledExpr::new(Expr::call(
            Function::Pow,
            vec![Expr::column("base"), Expr::column("exponent")],
        ))
        .unwrap();
        let output = expr
            .eval(&ctx, &[&[9.0, 0.0, f64::NAN], &[0.5, 0.0, 0.0]], 3)
            .unwrap();
        assert_eq!(&output[..2], &[3.0, 1.0]);
        assert!(output[2].is_nan());
        for (x, y) in [(-1.0, 0.5), (0.0, -1.0), (f64::MAX, 2.0)] {
            assert!(matches!(
                expr.eval(&ctx, &[&[x], &[y]], 1),
                Err(finstack_quant_core::Error::ExpressionEvaluation { row: 0, .. })
            ));
        }
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
    fn round_compile_checks_static_digits_and_arity() {
        for args in [
            vec![],
            vec![Expr::literal(0.0), Expr::literal(1.5)],
            vec![Expr::literal(0.0), Expr::column("digits")],
            vec![Expr::literal(0.0), Expr::literal(i32::MAX as f64 + 1.0)],
            vec![Expr::literal(0.0); 3],
        ] {
            assert!(CompiledExpr::new(Expr::call(Function::Round, args)).is_err());
        }
    }

    #[test]
    fn decimal_quantization_restores_once_with_signed_zero_and_overflow_checks() {
        use finstack_quant_core::math::round_half_away;
        let integer = 7.0 * 2.0_f64.powi(50) + 1.0;
        for (x, digits, expected) in [
            (2.675, 2, 2.67),
            (-0.125, 2, -0.13),
            (integer, 1, integer),
            (-1.0, i32::MIN, -0.0),
            (f64::MAX, -1, f64::MAX),
            (f64::from_bits(1), 323, 0.0),
            (f64::from_bits(1), 324, f64::from_bits(1)),
        ] {
            assert_eq!(
                round_half_away(x, digits).unwrap().to_bits(),
                expected.to_bits()
            );
        }
        assert!(round_half_away(f64::MAX, -308).is_err());
        assert!(round_half_away(f64::NAN, 0).is_err());
        for (lo, hi) in [(-1.0, 1.0), (0.0, 1.0), (0.0, 0.0)] {
            assert_eq!(
                finstack_quant_core::math::clamp(-0.0, lo, hi)
                    .unwrap()
                    .to_bits(),
                (-0.0_f64).to_bits()
            );
        }
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
    fn scalar_errors_identify_the_failing_row_and_preserve_missing_values() {
        let ctx = SimpleContext::new(["x"]).unwrap();
        for (fun, valid, expected, invalid) in [
            (Function::Ln, 1.0, 0.0, 0.0),
            (Function::Exp, -1000.0, 0.0, 1000.0),
            (Function::Log10, 100.0, 2.0, -1.0),
            (Function::Sqrt, 4.0, 2.0, -1.0),
        ] {
            let expr = CompiledExpr::new(Expr::call(fun, vec![Expr::column("x")])).unwrap();
            let result = expr.eval(&ctx, &[&[valid, f64::NAN]], 2).unwrap();
            assert_eq!(result[0], expected);
            assert!(result[1].is_nan());
            assert!(matches!(
                expr.eval(&ctx, &[&[valid, invalid]], 2),
                Err(finstack_quant_core::Error::ExpressionEvaluation { row: 1, .. })
            ));
        }
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

        let inverted = CompiledExpr::new(Expr::call(
            Function::Clamp,
            vec![Expr::column("x"), Expr::literal(10.0), Expr::literal(0.0)],
        ))
        .unwrap();
        assert!(inverted.eval(&ctx, &cols, 4).is_err());
        assert!(inverted.eval(&ctx, &[&[f64::NAN]], 1).unwrap()[0].is_nan());
    }

    #[test]
    fn is_missing_flags_nan() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let data = [vec![1.0, f64::NAN, 0.0]];
        let cols = to_slice_refs(&data);

        let result = eval_call(Function::IsMissing, vec![Expr::column("x")], &cols, &ctx);
        assert_eq!(result, vec![0.0, 1.0, 0.0]);
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
        let result = expr.eval(&ctx, &cols, cols[0].len()).unwrap();

        assert!((result[0] - 1.0 / 1.05_f64.powi(3)).abs() < 1e-12);
        assert!((result[1] - 1.0 / 1.10_f64.powi(3)).abs() < 1e-12);
    }
}
