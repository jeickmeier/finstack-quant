//! Tests for expression engine functions.
//!
//! This module tests individual function behaviors organized by category:
//! - Shift operations (Lag, Lead, Shift, Diff, PctChange)
//! - Cumulative operations (CumSum, CumProd, CumMin, CumMax)
//! - Rolling window operations (RollingMean, RollingSum, RollingStd, etc.)
//! - Exponentially weighted operations (EwmMean, EwmStd, EwmVar)
//! - Statistical operations (Std, Var, Median, Rank, Quantile)

use finstack_core::expr::{BinOp, CompiledExpr, EvalOpts, Expr, Function, SimpleContext};

/// Standard test data: values 1-10 and indices 0-9.
fn standard_test_data() -> (SimpleContext, Vec<Vec<f64>>) {
    let ctx = SimpleContext::new(["values", "index"]).expect("unique columns");
    let data = vec![
        vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0], // values
        vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],  // index
    ];
    (ctx, data)
}

/// Small test data for simpler tests.
fn small_test_data() -> (SimpleContext, Vec<Vec<f64>>) {
    let ctx = SimpleContext::new(["x", "y"]).expect("unique columns");
    let data = vec![
        vec![1.0, 2.0, 3.0, 4.0, 5.0],      // x
        vec![10.0, 20.0, 30.0, 40.0, 50.0], // y
    ];
    (ctx, data)
}

fn to_slice_refs(data: &[Vec<f64>]) -> Vec<&[f64]> {
    data.iter().map(|v| v.as_slice()).collect()
}

// =============================================================================
// Shift Operations: Lag, Lead, Shift, Diff, PctChange
// =============================================================================

mod shift_operations {
    use super::*;

    #[test]
    fn lag_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let lag_expr = CompiledExpr::new(Expr::call(
            Function::Lag,
            vec![Expr::column("x"), Expr::literal(1.0)],
        ));
        let result = lag_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(result[0].is_nan()); // First value should be NaN
        assert_eq!(result[1], 1.0); // Second value should be first input
        assert_eq!(result[2], 2.0);
        assert_eq!(result[3], 3.0);
        assert_eq!(result[4], 4.0);
    }

    #[test]
    fn lag_multiple_periods() {
        let (ctx, data) = standard_test_data();
        let cols = to_slice_refs(&data);

        let lag_expr = CompiledExpr::new(Expr::call(
            Function::Lag,
            vec![Expr::column("values"), Expr::literal(2.0)],
        ));
        let result = lag_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert!((result[2] - 1.0).abs() < 1e-10);
        assert!((result[3] - 2.0).abs() < 1e-10);
    }

    #[test]
    fn lead_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let lead_expr = CompiledExpr::new(Expr::call(
            Function::Lead,
            vec![Expr::column("x"), Expr::literal(1.0)],
        ));
        let result = lead_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result[0], 2.0);
        assert_eq!(result[1], 3.0);
        assert_eq!(result[2], 4.0);
        assert_eq!(result[3], 5.0);
        assert!(result[4].is_nan()); // Last value should be NaN
    }

    #[test]
    fn lead_multiple_periods() {
        let (ctx, data) = standard_test_data();
        let cols = to_slice_refs(&data);

        let lead_expr = CompiledExpr::new(Expr::call(
            Function::Lead,
            vec![Expr::column("values"), Expr::literal(2.0)],
        ));
        let result = lead_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!((result[0] - 3.0).abs() < 1e-10);
        assert!((result[7] - 10.0).abs() < 1e-10);
        assert!(result[8].is_nan());
        assert!(result[9].is_nan());
    }

    #[test]
    fn shift_positive_equals_lag() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![10.0, 20.0, 30.0, 40.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let lag_expr = Expr::call(Function::Lag, vec![Expr::column("x"), Expr::literal(1.0)]);
        let shift_pos = Expr::call(Function::Shift, vec![Expr::column("x"), Expr::literal(1.0)]);

        let lag_out = CompiledExpr::new(lag_expr)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;
        let shift_out = CompiledExpr::new(shift_pos)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        for i in 0..4 {
            if lag_out[i].is_nan() {
                assert!(shift_out[i].is_nan());
            } else {
                assert_eq!(lag_out[i], shift_out[i]);
            }
        }
    }

    #[test]
    fn shift_negative_equals_lead() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![10.0, 20.0, 30.0, 40.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let lead_expr = Expr::call(Function::Lead, vec![Expr::column("x"), Expr::literal(1.0)]);
        let shift_neg = Expr::call(
            Function::Shift,
            vec![Expr::column("x"), Expr::literal(-1.0)],
        );

        let lead_out = CompiledExpr::new(lead_expr)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;
        let shift_out = CompiledExpr::new(shift_neg)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        for i in 0..4 {
            if lead_out[i].is_nan() {
                assert!(shift_out[i].is_nan());
            } else {
                assert_eq!(lead_out[i], shift_out[i]);
            }
        }
    }

    #[test]
    fn diff_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let diff_expr = CompiledExpr::new(Expr::call(
            Function::Diff,
            vec![Expr::column("x"), Expr::literal(1.0)],
        ));
        let result = diff_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(result[0].is_nan());
        assert_eq!(result[1], 1.0); // 2 - 1 = 1
        assert_eq!(result[2], 1.0); // 3 - 2 = 1
        assert_eq!(result[3], 1.0); // 4 - 3 = 1
        assert_eq!(result[4], 1.0); // 5 - 4 = 1
    }

    #[test]
    fn diff_multiple_periods() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![1.0, 2.0, 4.0, 8.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let diff2 = Expr::call(Function::Diff, vec![Expr::column("x"), Expr::literal(2.0)]);
        let result = CompiledExpr::new(diff2)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert_eq!(result[2], 3.0); // 4 - 1 = 3
        assert_eq!(result[3], 6.0); // 8 - 2 = 6
    }

    #[test]
    fn pct_change_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let pct_expr = CompiledExpr::new(Expr::call(
            Function::PctChange,
            vec![Expr::column("x"), Expr::literal(1.0)],
        ));
        let result = pct_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(result[0].is_nan());
        assert_eq!(result[1], 1.0); // (2/1) - 1 = 1
        assert_eq!(result[2], 0.5); // (3/2) - 1 = 0.5
        assert!((result[3] - 1.0 / 3.0).abs() < 1e-10); // (4/3) - 1 = 1/3
    }

    #[test]
    fn pct_change_multiple_periods() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![1.0, 2.0, 4.0, 8.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let pct2 = Expr::call(
            Function::PctChange,
            vec![Expr::column("x"), Expr::literal(2.0)],
        );
        let result = CompiledExpr::new(pct2)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert!((result[2] - 3.0).abs() < 1e-12); // (4/1) - 1 = 3
        assert!((result[3] - 3.0).abs() < 1e-12); // (8/2) - 1 = 3
    }

    #[test]
    fn pct_change_distinguishes_zero_over_zero_from_nonzero_over_zero() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![0.0, 0.0, 2.0, 0.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let pct_expr = CompiledExpr::new(Expr::call(
            Function::PctChange,
            vec![Expr::column("x"), Expr::literal(1.0)],
        ));
        let result = pct_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(result[0].is_nan());
        assert_eq!(result[1], 0.0);
        assert!(result[2].is_infinite() && result[2].is_sign_positive());
        assert_eq!(result[3], -1.0);
    }
}

// =============================================================================
// Cumulative Operations: CumSum, CumProd, CumMin, CumMax
// =============================================================================

mod cumulative_operations {
    use super::*;

    #[test]
    fn cumsum_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let cumsum_expr = CompiledExpr::new(Expr::call(Function::CumSum, vec![Expr::column("x")]));
        let result = cumsum_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result, vec![1.0, 3.0, 6.0, 10.0, 15.0]); // 1, 1+2, 1+2+3, etc.
    }

    #[test]
    fn cumsum_longer_series() {
        let (ctx, data) = standard_test_data();
        let cols = to_slice_refs(&data);

        let cumsum_expr =
            CompiledExpr::new(Expr::call(Function::CumSum, vec![Expr::column("values")]));
        let result = cumsum_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        let expected = [1.0, 3.0, 6.0, 10.0, 15.0, 21.0, 28.0, 36.0, 45.0, 55.0];
        for (a, b) in result.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-10);
        }
    }

    #[test]
    fn cumprod_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let cumprod_expr =
            CompiledExpr::new(Expr::call(Function::CumProd, vec![Expr::column("x")]));
        let result = cumprod_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result, vec![1.0, 2.0, 6.0, 24.0, 120.0]); // 1, 1*2, 1*2*3, etc.
    }

    #[test]
    fn cumprod_longer_series() {
        let (ctx, data) = standard_test_data();
        let cols = to_slice_refs(&data);

        let cumprod_expr =
            CompiledExpr::new(Expr::call(Function::CumProd, vec![Expr::column("values")]));
        let result = cumprod_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        let expected = [
            1.0, 2.0, 6.0, 24.0, 120.0, 720.0, 5040.0, 40320.0, 362880.0, 3628800.0,
        ];
        for (a, b) in result.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-5); // Larger tolerance for large numbers
        }
    }

    #[test]
    fn cummin_basic() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![3.0, 1.0, 4.0, 2.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let cummin = CompiledExpr::new(Expr::call(Function::CumMin, vec![Expr::column("x")]));
        let result = cummin
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result, vec![3.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn cummin_increasing_series() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let cummin_expr = CompiledExpr::new(Expr::call(Function::CumMin, vec![Expr::column("x")]));
        let result = cummin_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        // Min so far - always 1.0 since series is increasing
        assert_eq!(result, vec![1.0, 1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn cummax_basic() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![3.0, 1.0, 4.0, 2.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let cummax = CompiledExpr::new(Expr::call(Function::CumMax, vec![Expr::column("x")]));
        let result = cummax
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result, vec![3.0, 3.0, 4.0, 4.0]);
    }

    #[test]
    fn cummax_increasing_series() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let cummax_expr = CompiledExpr::new(Expr::call(Function::CumMax, vec![Expr::column("x")]));
        let result = cummax_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        // Max so far - follows series since it's increasing
        assert_eq!(result, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }
}

// =============================================================================
// Rolling Window Operations
// =============================================================================

mod rolling_operations {
    use super::*;

    #[test]
    fn rolling_mean_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let rolling_mean_expr = CompiledExpr::new(Expr::call(
            Function::RollingMean,
            vec![Expr::column("x"), Expr::literal(3.0)],
        ));
        let result = rolling_mean_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert_eq!(result[2], 2.0); // (1+2+3)/3 = 2
        assert_eq!(result[3], 3.0); // (2+3+4)/3 = 3
        assert_eq!(result[4], 4.0); // (3+4+5)/3 = 4
    }

    #[test]
    fn rolling_mean_window_2() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let rmean = Expr::call(
            Function::RollingMean,
            vec![Expr::column("x"), Expr::literal(2.0)],
        );
        let rm = CompiledExpr::new(rmean)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(rm[0].is_nan());
        assert!((rm[1] - 1.5).abs() < 1e-12);
        assert!((rm[2] - 2.5).abs() < 1e-12);
        assert!((rm[3] - 3.5).abs() < 1e-12);
    }

    #[test]
    fn rolling_sum_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let rolling_sum_expr = CompiledExpr::new(Expr::call(
            Function::RollingSum,
            vec![Expr::column("x"), Expr::literal(2.0)],
        ));
        let result = rolling_sum_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(result[0].is_nan());
        assert_eq!(result[1], 3.0); // 1+2 = 3
        assert_eq!(result[2], 5.0); // 2+3 = 5
        assert_eq!(result[3], 7.0); // 3+4 = 7
        assert_eq!(result[4], 9.0); // 4+5 = 9
    }

    #[test]
    fn rolling_sum_window_3() {
        let (ctx, data) = standard_test_data();
        let cols = to_slice_refs(&data);

        let rolling_sum_expr = CompiledExpr::new(Expr::call(
            Function::RollingSum,
            vec![Expr::column("values"), Expr::literal(3.0)],
        ));
        let result = rolling_sum_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        let expected = [
            f64::NAN,
            f64::NAN,
            6.0,
            9.0,
            12.0,
            15.0,
            18.0,
            21.0,
            24.0,
            27.0,
        ];

        for (i, (a, b)) in result.iter().zip(expected.iter()).enumerate() {
            if a.is_nan() && b.is_nan() {
                continue;
            }
            assert!((a - b).abs() < 1e-10, "rolling_sum[{}]: {} != {}", i, a, b);
        }
    }

    #[test]
    fn rolling_std_basic() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let rstd = Expr::call(
            Function::RollingStd,
            vec![Expr::column("x"), Expr::literal(2.0)],
        );
        let rst = CompiledExpr::new(rstd)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(rst[0].is_nan());
        assert!((rst[1] - 0.5).abs() < 1e-12);
        assert!((rst[2] - 0.5).abs() < 1e-12);
        assert!((rst[3] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn rolling_var_basic() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let rvar = Expr::call(
            Function::RollingVar,
            vec![Expr::column("x"), Expr::literal(2.0)],
        );
        let rv = CompiledExpr::new(rvar)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(rv[0].is_nan());
        assert!((rv[1] - 0.25).abs() < 1e-12);
        assert!((rv[2] - 0.25).abs() < 1e-12);
        assert!((rv[3] - 0.25).abs() < 1e-12);
    }

    #[test]
    fn rolling_median_basic() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let rmed = Expr::call(
            Function::RollingMedian,
            vec![Expr::column("x"), Expr::literal(2.0)],
        );
        let rmd = CompiledExpr::new(rmed)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(rmd[0].is_nan());
        assert!((rmd[1] - 1.5).abs() < 1e-12);
        assert!((rmd[2] - 2.5).abs() < 1e-12);
        assert!((rmd[3] - 3.5).abs() < 1e-12);
    }

    #[test]
    fn rolling_min_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let rolling_min = CompiledExpr::new(Expr::call(
            Function::RollingMin,
            vec![Expr::column("x"), Expr::literal(2.0)],
        ));
        let result = rolling_min
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(result[0].is_nan());
        assert_eq!(result[1], 1.0);
        assert_eq!(result[2], 2.0);
        assert_eq!(result[3], 3.0);
    }

    #[test]
    fn rolling_max_basic() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let rmax = Expr::call(
            Function::RollingMax,
            vec![Expr::column("x"), Expr::literal(2.0)],
        );
        let rmx = CompiledExpr::new(rmax)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(rmx[0].is_nan());
        assert_eq!(rmx[1], 2.0);
        assert_eq!(rmx[2], 3.0);
        assert_eq!(rmx[3], 4.0);
    }

    #[test]
    fn rolling_count_basic() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let rcount = Expr::call(
            Function::RollingCount,
            vec![Expr::column("x"), Expr::literal(2.0)],
        );
        let rc = CompiledExpr::new(rcount)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(rc[0].is_nan());
        assert_eq!(rc[1], 2.0);
        assert_eq!(rc[2], 2.0);
        assert_eq!(rc[3], 2.0);
    }
}

// =============================================================================
// Exponentially Weighted Operations
// =============================================================================

mod ewm_operations {
    use super::*;

    #[test]
    fn ewm_mean_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let ewm_expr = CompiledExpr::new(Expr::call(
            Function::EwmMean,
            vec![Expr::column("x"), Expr::literal(0.5)],
        ));
        let result = ewm_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        // First value should be the input value
        assert_eq!(result[0], 1.0);

        // All values should be finite and reasonable
        assert_eq!(result.len(), 5);
        for val in result {
            assert!(val.is_finite());
            assert!(val > 0.0); // Should be positive given positive inputs
        }
    }

    #[test]
    fn ewm_mean_adjust_false() {
        let (ctx, data) = standard_test_data();
        let cols = to_slice_refs(&data);

        let ewm_expr = CompiledExpr::new(Expr::call(
            Function::EwmMean,
            vec![
                Expr::column("values"),
                Expr::literal(0.5),
                Expr::literal(0.0),
            ], // adjust=false
        ));
        let result = ewm_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        // Calculate expected EWM manually (adjust=false)
        let alpha = 0.5;
        let mut expected = Vec::with_capacity(10);
        let mut ewm = 1.0; // First value
        expected.push(ewm);

        for i in 1..10 {
            let value = (i + 1) as f64;
            ewm = alpha * value + (1.0 - alpha) * ewm;
            expected.push(ewm);
        }

        assert_eq!(result.len(), expected.len());
        for (i, (a, b)) in result.iter().zip(expected.iter()).enumerate() {
            assert!((a - b).abs() < 1e-10, "ewm_mean[{}]: {} != {}", i, a, b);
        }
    }

    #[test]
    fn ewm_mean_adjust_true_matches_weighted_definition() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let ewm_expr = CompiledExpr::new(Expr::call(
            Function::EwmMean,
            vec![Expr::column("x"), Expr::literal(0.5), Expr::literal(1.0)], // adjust=true
        ));
        let result = ewm_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        let expected = [1.0, 5.0 / 3.0, 17.0 / 7.0, 49.0 / 15.0, 129.0 / 31.0];
        for (i, (a, b)) in result.iter().zip(expected.iter()).enumerate() {
            assert!(
                (a - b).abs() < 1e-10,
                "ewm_mean_adjust_true[{}]: {} != {}",
                i,
                a,
                b
            );
        }
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
        ));
        let result = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;

        assert_eq!(result[0], 11.0);
        assert_eq!(result[1], 22.0);
        assert!(result[2].is_nan());
        assert!(result[3].is_nan());
    }

    #[test]
    fn ewm_std_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let ewm_std = CompiledExpr::new(Expr::call(
            Function::EwmStd,
            vec![
                Expr::column("x"),
                Expr::literal(0.5),
                Expr::literal(1.0), // adjust=true
            ],
        ));
        let result = ewm_std
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result.len(), 5);
        assert_eq!(result[0], 0.0); // First value has zero std
        assert!(result[1].is_finite());
    }

    #[test]
    fn ewm_var_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let ewm_var = CompiledExpr::new(Expr::call(
            Function::EwmVar,
            vec![
                Expr::column("x"),
                Expr::literal(0.5),
                Expr::literal(0.0), // adjust=false
            ],
        ));
        let result = ewm_var
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result.len(), 5);
        assert_eq!(result[0], 0.0);
        assert!(result[1] >= 0.0); // Variance should be non-negative
    }

    #[test]
    fn ewm_std_empty_input() {
        let ctx = SimpleContext::new(["x", "alpha"]).expect("unique columns");
        let x: Vec<f64> = vec![];
        let alpha_col = vec![];
        let cols: Vec<&[f64]> = vec![x.as_slice(), alpha_col.as_slice()];

        let std_expr = CompiledExpr::new(Expr::call(
            Function::EwmStd,
            vec![Expr::column("x"), Expr::column("alpha")],
        ));
        let result = std_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;
        assert!(result.is_empty());
    }

    #[test]
    fn ewm_var_empty_input() {
        let ctx = SimpleContext::new(["x", "alpha"]).expect("unique columns");
        let x: Vec<f64> = vec![];
        let alpha_col = vec![];
        let cols: Vec<&[f64]> = vec![x.as_slice(), alpha_col.as_slice()];

        let var_expr = CompiledExpr::new(Expr::call(
            Function::EwmVar,
            vec![Expr::column("x"), Expr::column("alpha")],
        ));
        let result = var_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;
        assert!(result.is_empty());
    }

    #[test]
    fn ewm_std_leading_nan_does_not_poison_state() {
        // Regression: ema/ema_sq previously seeded from base[0] unconditionally,
        // so a leading NaN poisoned the recursion and `.max(0.0)` converted the
        // NaN variance to 0.0 — ewm_std([NaN,1,5,9,2]) returned all zeros
        // ().
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![f64::NAN, 1.0, 5.0, 9.0, 2.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let std_expr = CompiledExpr::new(Expr::call(
            Function::EwmStd,
            vec![Expr::column("x"), Expr::literal(0.5), Expr::literal(1.0)],
        ));
        let result = std_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result.len(), 5);
        // Leading NaN stays NaN; seed point (first non-NaN obs) is 0.0.
        assert!(result[0].is_nan());
        assert_eq!(result[1], 0.0);
        // Post-seed values must be finite and strictly positive (the data varies).
        for (i, &v) in result.iter().enumerate().skip(2) {
            assert!(v.is_finite(), "ewm_std[{i}] should be finite, got {v}");
            assert!(v > 0.0, "ewm_std[{i}] should be positive, got {v}");
        }

        // The result must match evaluating the same series without the
        // leading NaN, shifted by one position.
        let x_tail = vec![1.0, 5.0, 9.0, 2.0];
        let cols_tail: Vec<&[f64]> = vec![x_tail.as_slice()];
        let tail_expr = CompiledExpr::new(Expr::call(
            Function::EwmStd,
            vec![Expr::column("x"), Expr::literal(0.5), Expr::literal(1.0)],
        ));
        let tail = tail_expr
            .eval(&ctx, &cols_tail, EvalOpts::default())
            .unwrap()
            .values;
        for i in 0..4 {
            assert!(
                (result[i + 1] - tail[i]).abs() < 1e-12,
                "ewm_std with leading NaN[{}]: {} != {}",
                i + 1,
                result[i + 1],
                tail[i]
            );
        }
    }

    #[test]
    fn ewm_var_all_nan_input_returns_all_nan() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![f64::NAN, f64::NAN, f64::NAN];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let var_expr = CompiledExpr::new(Expr::call(
            Function::EwmVar,
            vec![Expr::column("x"), Expr::literal(0.5)],
        ));
        let result = var_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn ewm_mean_leading_nan_does_not_poison_state() {
        // Regression: prev/weighted_sum previously seeded from base[0]
        // unconditionally, so a leading NaN made every subsequent output NaN
        // (). The seeding now
        // matches eval_ewm_variance_core: leading NaNs emit NaN, the first
        // non-NaN observation seeds the state and emits the input value, and
        // interior NaNs emit NaN without updating the state.
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![f64::NAN, 1.0, 5.0, 9.0, 2.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        for adjust in [0.0, 1.0] {
            let mean_expr = CompiledExpr::new(Expr::call(
                Function::EwmMean,
                vec![Expr::column("x"), Expr::literal(0.5), Expr::literal(adjust)],
            ));
            let result = mean_expr
                .eval(&ctx, &cols, EvalOpts::default())
                .unwrap()
                .values;

            assert_eq!(result.len(), 5);
            // Leading NaN stays NaN; seed point emits the input value.
            assert!(result[0].is_nan(), "adjust={adjust}: leading NaN lost");
            assert_eq!(result[1], 1.0, "adjust={adjust}: seed point");
            for (i, &v) in result.iter().enumerate().skip(2) {
                assert!(
                    v.is_finite(),
                    "adjust={adjust}: ewm_mean[{i}] should be finite, got {v}"
                );
            }

            // The result must match evaluating the same series without the
            // leading NaN, shifted by one position.
            let x_tail = vec![1.0, 5.0, 9.0, 2.0];
            let cols_tail: Vec<&[f64]> = vec![x_tail.as_slice()];
            let tail = mean_expr
                .eval(&ctx, &cols_tail, EvalOpts::default())
                .unwrap()
                .values;
            for i in 0..4 {
                assert!(
                    (result[i + 1] - tail[i]).abs() < 1e-12,
                    "adjust={adjust}: ewm_mean with leading NaN[{}]: {} != {}",
                    i + 1,
                    result[i + 1],
                    tail[i]
                );
            }
        }
    }

    #[test]
    fn ewm_mean_interior_nan_is_skipped_without_updating_state() {
        // Interior NaNs now match ewm_var/ewm_std: NaN is emitted and the
        // recursion state is unchanged (previously an interior NaN poisoned
        // every subsequent output).
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![1.0, f64::NAN, 5.0, 9.0];
        let x_dense = vec![1.0, 5.0, 9.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];
        let cols_dense: Vec<&[f64]> = vec![x_dense.as_slice()];

        let mean_expr = CompiledExpr::new(Expr::call(
            Function::EwmMean,
            vec![Expr::column("x"), Expr::literal(0.5)],
        ));
        let result = mean_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;
        let dense = mean_expr
            .eval(&ctx, &cols_dense, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result[0], 1.0);
        assert!(result[1].is_nan());
        assert!(
            (result[2] - dense[1]).abs() < 1e-12,
            "state must be unchanged across the interior NaN: {} != {}",
            result[2],
            dense[1]
        );
        assert!((result[3] - dense[2]).abs() < 1e-12);
    }

    #[test]
    fn ewm_mean_all_nan_input_returns_all_nan() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![f64::NAN, f64::NAN, f64::NAN];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let mean_expr = CompiledExpr::new(Expr::call(
            Function::EwmMean,
            vec![Expr::column("x"), Expr::literal(0.5)],
        ));
        let result = mean_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn ewm_std_var_consistency() {
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let cols: Vec<&[f64]> = vec![x.as_slice()];

        let alpha = Expr::literal(0.5);
        let adjust_true = Expr::literal(1.0);
        let std_expr = Expr::call(
            Function::EwmStd,
            vec![Expr::column("x"), alpha.clone(), adjust_true.clone()],
        );
        let var_expr = Expr::call(
            Function::EwmVar,
            vec![Expr::column("x"), alpha, adjust_true],
        );

        let std_vals = CompiledExpr::new(std_expr)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;
        let var_vals = CompiledExpr::new(var_expr)
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(std_vals.len(), var_vals.len());
        for i in 0..std_vals.len() {
            if std_vals[i].is_nan() || var_vals[i].is_nan() {
                continue;
            }
            // std^2 should equal var
            let diff = std_vals[i] * std_vals[i] - var_vals[i];
            assert!(
                diff.abs() < 1e-9,
                "index {}: std^2={}, var={}",
                i,
                std_vals[i] * std_vals[i],
                var_vals[i]
            );
            assert!(var_vals[i] >= 0.0);
        }
    }
}

// =============================================================================
// Statistical Operations: Std, Var, Median, Rank, Quantile
// =============================================================================

mod statistical_operations {
    use super::*;

    #[test]
    fn std_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let std_expr = CompiledExpr::new(Expr::call(Function::Std, vec![Expr::column("x")]));
        let result = std_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        // All values should be the same (standard deviation of the entire series)
        let expected_std = result[0];
        assert!(expected_std > 0.0);
        for val in &result {
            assert!((val - expected_std).abs() < 1e-10);
        }
    }

    #[test]
    fn std_longer_series() {
        let (ctx, data) = standard_test_data();
        let cols = to_slice_refs(&data);

        let std_expr = CompiledExpr::new(Expr::call(Function::Std, vec![Expr::column("values")]));
        let result = std_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        // Population std of [1..10] = sqrt(82.5/10) ≈ 2.8723
        let expected_std = (82.5_f64 / 10.0).sqrt();
        for r in &result {
            assert!(
                (r - expected_std).abs() < 1e-10,
                "std: {} != {}",
                r,
                expected_std
            );
        }
    }

    #[test]
    fn var_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let var_expr = CompiledExpr::new(Expr::call(Function::Var, vec![Expr::column("x")]));
        let result = var_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        let std_expr = CompiledExpr::new(Expr::call(Function::Std, vec![Expr::column("x")]));
        let std_result = std_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        // Variance should be std^2
        let expected_var = std_result[0] * std_result[0];
        for val in &result {
            assert!((val - expected_var).abs() < 1e-10);
        }
    }

    #[test]
    fn var_longer_series() {
        let (ctx, data) = standard_test_data();
        let cols = to_slice_refs(&data);

        let var_expr = CompiledExpr::new(Expr::call(Function::Var, vec![Expr::column("values")]));
        let result = var_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        // Population variance of [1..10] = 82.5/10 = 8.25
        let expected_var = 82.5_f64 / 10.0;
        for r in &result {
            assert!(
                (r - expected_var).abs() < 1e-10,
                "var: {} != {}",
                r,
                expected_var
            );
        }
    }

    #[test]
    fn median_basic() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let median_expr = CompiledExpr::new(Expr::call(Function::Median, vec![Expr::column("x")]));
        let result = median_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        // Median of [1,2,3,4,5] should be 3
        for val in &result {
            assert_eq!(*val, 3.0);
        }
    }

    #[test]
    fn median_longer_series() {
        let (ctx, data) = standard_test_data();
        let cols = to_slice_refs(&data);

        let median_expr =
            CompiledExpr::new(Expr::call(Function::Median, vec![Expr::column("values")]));
        let result = median_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        // Median of [1,2,3,4,5,6,7,8,9,10] = 5.5
        let expected_median = 5.5;
        for r in &result {
            assert!(
                (r - expected_median).abs() < 1e-10,
                "median: {} != {}",
                r,
                expected_median
            );
        }
    }

    #[test]
    fn rank_basic() {
        let ctx = SimpleContext::new(["v"]).expect("unique columns");
        let v = vec![3.0, 1.0, 2.0, 2.0];
        let cols: Vec<&[f64]> = vec![v.as_slice()];

        let rank = CompiledExpr::new(Expr::call(Function::Rank, vec![Expr::column("v")]));
        let result = rank.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;

        assert_eq!(result, vec![3.0, 1.0, 2.0, 2.0]);
    }

    #[test]
    fn rank_increasing_series() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let rank_expr = CompiledExpr::new(Expr::call(Function::Rank, vec![Expr::column("x")]));
        let result = rank_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn quantile_median() {
        let ctx = SimpleContext::new(["v"]).expect("unique columns");
        let v = vec![3.0, 1.0, 2.0, 2.0];
        let cols: Vec<&[f64]> = vec![v.as_slice()];

        let q50 = CompiledExpr::new(Expr::call(
            Function::Quantile,
            vec![Expr::column("v"), Expr::literal(0.5)],
        ));
        let result = q50.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;

        // Median of [1, 2, 2, 3] is 2.0
        for val in result {
            assert!((val - 2.0).abs() < 1e-12);
        }
    }

    #[test]
    fn quantile_y_column() {
        let (ctx, data) = small_test_data();
        let cols = to_slice_refs(&data);

        let quantile_expr = CompiledExpr::new(Expr::call(
            Function::Quantile,
            vec![Expr::column("y"), Expr::literal(0.5)],
        ));
        let result = quantile_expr
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        // Median of [10,20,30,40,50] = 30
        assert!(result.iter().all(|&v| (v - 30.0).abs() < 1e-12));
    }

    #[test]
    fn median_skips_nan() {
        // Unified reducer NaN policy: NaNs are excluded from the sample and
        // the count (same as quantile). Previously NaN sorted as the largest
        // value, so median([1,2,3,NaN]) wrongly returned 2.5.
        let ctx = SimpleContext::new(["v"]).expect("unique columns");
        let v = vec![1.0, 2.0, 3.0, f64::NAN];
        let cols: Vec<&[f64]> = vec![v.as_slice()];

        let median = CompiledExpr::new(Expr::call(Function::Median, vec![Expr::column("v")]));
        let result = median
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        // Median of the valid values [1,2,3] is 2.0
        assert!(result.iter().all(|&r| (r - 2.0).abs() < 1e-12));
    }

    #[test]
    fn median_all_nan_returns_nan() {
        let ctx = SimpleContext::new(["v"]).expect("unique columns");
        let v = vec![f64::NAN, f64::NAN, f64::NAN];
        let cols: Vec<&[f64]> = vec![v.as_slice()];

        let median = CompiledExpr::new(Expr::call(Function::Median, vec![Expr::column("v")]));
        let result = median
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert!(result.iter().all(|r| r.is_nan()));
    }

    #[test]
    fn rolling_median_skips_nan_in_window() {
        let ctx = SimpleContext::new(["v"]).expect("unique columns");
        let v = vec![1.0, f64::NAN, 3.0, 5.0];
        let cols: Vec<&[f64]> = vec![v.as_slice()];

        let rmed = CompiledExpr::new(Expr::call(
            Function::RollingMedian,
            vec![Expr::column("v"), Expr::literal(3.0)],
        ));
        let result = rmed.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;

        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        // Window [1, NaN, 3] → valid [1,3] → median 2.0
        assert!((result[2] - 2.0).abs() < 1e-12);
        // Window [NaN, 3, 5] → valid [3,5] → median 4.0
        assert!((result[3] - 4.0).abs() < 1e-12);
    }

    #[test]
    fn rolling_median_all_nan_window_returns_nan() {
        let ctx = SimpleContext::new(["v"]).expect("unique columns");
        let v = vec![f64::NAN, f64::NAN, 1.0];
        let cols: Vec<&[f64]> = vec![v.as_slice()];

        let rmed = CompiledExpr::new(Expr::call(
            Function::RollingMedian,
            vec![Expr::column("v"), Expr::literal(2.0)],
        ));
        let result = rmed.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;

        assert!(result[0].is_nan());
        assert!(result[1].is_nan()); // window all-NaN
        assert!((result[2] - 1.0).abs() < 1e-12); // window [NaN, 1] → 1.0
    }

    #[test]
    fn std_and_var_skip_nan() {
        // Global reducers skip NaN (aligned with median/quantile and the
        // statements-layer reducers).
        let ctx = SimpleContext::new(["v"]).expect("unique columns");
        let v = vec![1.0, f64::NAN, 2.0, 3.0, f64::NAN];
        let cols: Vec<&[f64]> = vec![v.as_slice()];

        let var = CompiledExpr::new(Expr::call(Function::Var, vec![Expr::column("v")]));
        let var_result = var.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
        // Population variance of [1,2,3] = 2/3
        let expected_var = 2.0 / 3.0;
        assert!(var_result.iter().all(|&r| (r - expected_var).abs() < 1e-12));

        let std = CompiledExpr::new(Expr::call(Function::Std, vec![Expr::column("v")]));
        let std_result = std.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
        assert!(std_result
            .iter()
            .all(|&r| (r - expected_var.sqrt()).abs() < 1e-12));
    }

    #[test]
    fn std_with_fewer_than_two_valid_values_returns_nan() {
        let ctx = SimpleContext::new(["v"]).expect("unique columns");
        let v = vec![f64::NAN, 7.0, f64::NAN];
        let cols: Vec<&[f64]> = vec![v.as_slice()];

        let std = CompiledExpr::new(Expr::call(Function::Std, vec![Expr::column("v")]));
        let result = std.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
        assert!(result.iter().all(|r| r.is_nan()));
    }

    #[test]
    fn rolling_var_is_stable_for_large_means() {
        // Two-pass per-window variance: the old E[x^2]-E[x]^2 fast path
        // catastrophically cancels for large means with tiny spread.
        let ctx = SimpleContext::new(["v"]).expect("unique columns");
        let base = 1.0e8;
        let v = vec![base + 1.0, base + 2.0, base + 3.0, base + 4.0];
        let cols: Vec<&[f64]> = vec![v.as_slice()];

        let rvar = CompiledExpr::new(Expr::call(
            Function::RollingVar,
            vec![Expr::column("v"), Expr::literal(2.0)],
        ));
        let result = rvar.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;

        assert!(result[0].is_nan());
        for r in &result[1..] {
            assert!(
                (r - 0.25).abs() < 1e-12,
                "rolling_var should be exactly 0.25, got {r}"
            );
        }
    }

    #[test]
    fn window_argument_must_be_constant_series() {
        // Passing a data column as the window/step parameter is a validation
        // error instead of silently using only the first element.
        let ctx = SimpleContext::new(["v", "w"]).expect("unique columns");
        let v = vec![1.0, 2.0, 3.0, 4.0];
        let w = vec![2.0, 3.0, 2.0, 2.0]; // non-constant
        let cols: Vec<&[f64]> = vec![v.as_slice(), w.as_slice()];

        let expr = CompiledExpr::new(Expr::call(
            Function::RollingMean,
            vec![Expr::column("v"), Expr::column("w")],
        ));
        let err = expr
            .eval(&ctx, &cols, EvalOpts::default())
            .expect_err("non-constant window argument should be rejected");
        assert!(
            err.to_string().contains("constant"),
            "unexpected error: {err}"
        );
    }
}

// =============================================================================
// Edge Cases and Determinism
// =============================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn empty_data() {
        let ctx = SimpleContext::new(["empty"]).expect("unique columns");
        let data = [vec![]];
        let cols = to_slice_refs(&data);

        let expr = CompiledExpr::new(Expr::call(
            Function::RollingMean,
            vec![Expr::column("empty"), Expr::literal(2.0)],
        ));
        let result = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn single_value() {
        let ctx = SimpleContext::new(["single"]).expect("unique columns");
        let data = [vec![42.0]];
        let cols = to_slice_refs(&data);

        let expr = CompiledExpr::new(Expr::call(Function::CumSum, vec![Expr::column("single")]));
        let result = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
        assert_eq!(result, vec![42.0]);
    }

    #[test]
    fn nan_handling_cumsum() {
        let ctx = SimpleContext::new(["nan_data"]).expect("unique columns");
        let data = [vec![1.0, f64::NAN, 3.0, f64::NAN, 5.0]];
        let cols = to_slice_refs(&data);

        let expr = CompiledExpr::new(Expr::call(Function::CumSum, vec![Expr::column("nan_data")]));
        let result = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;

        // NaN values are skipped (treated as identity element 0 for sum)
        assert_eq!(result.len(), 5);
        assert_eq!(result[0], 1.0);
        assert_eq!(result[1], 1.0);
        assert_eq!(result[2], 4.0);
        assert_eq!(result[3], 4.0);
        assert_eq!(result[4], 9.0);
    }

    #[test]
    fn determinism_across_runs() {
        let (ctx, data) = standard_test_data();
        let cols = to_slice_refs(&data);

        let expr = CompiledExpr::new(Expr::call(
            Function::RollingMean,
            vec![Expr::column("values"), Expr::literal(3.0)],
        ));

        let result1 = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
        let result2 = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;
        let result3 = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;

        assert_eq!(result1.len(), result2.len());
        assert_eq!(result2.len(), result3.len());

        for i in 0..result1.len() {
            if result1[i].is_nan() {
                assert!(result2[i].is_nan() && result3[i].is_nan());
            } else {
                assert!((result1[i] - result2[i]).abs() < 1e-15);
                assert!((result2[i] - result3[i]).abs() < 1e-15);
            }
        }
    }

    #[test]
    fn all_functions_produce_valid_output() {
        let (ctx, data) = standard_test_data();
        let cols = to_slice_refs(&data);

        let functions_to_test = vec![
            (
                Function::Lag,
                vec![Expr::column("values"), Expr::literal(1.0)],
            ),
            (
                Function::Lead,
                vec![Expr::column("values"), Expr::literal(1.0)],
            ),
            (
                Function::Diff,
                vec![Expr::column("values"), Expr::literal(1.0)],
            ),
            (Function::CumSum, vec![Expr::column("values")]),
            (Function::CumProd, vec![Expr::column("values")]),
            (
                Function::RollingMean,
                vec![Expr::column("values"), Expr::literal(3.0)],
            ),
        ];

        for (func, args) in functions_to_test {
            let expr = CompiledExpr::new(Expr::call(func, args));
            let result = expr.eval(&ctx, &cols, EvalOpts::default()).unwrap().values;

            // Ensure evaluation worked
            assert_eq!(result.len(), data[0].len());

            // Verify all results are finite or NaN where expected
            for val in &result {
                assert!(
                    val.is_finite() || val.is_nan(),
                    "Function {:?} produced invalid value",
                    func
                );
            }
        }
    }
}
