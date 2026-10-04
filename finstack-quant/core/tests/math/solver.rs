//! Root-finding solver tests.
//!
//! This module consolidates all solver tests including:
//! - Brent solver tests
//! - Serialization tests

use finstack_quant_core::math::solver::BrentSolver;

// Brent Solver Tests

mod brent {
    use super::*;

    #[test]
    fn finds_root_simple_quadratic() {
        // f(x) = x^2 - 2 ⇒ root = sqrt(2)
        let f = |x: f64| x * x - 2.0;
        let solver = BrentSolver::new().tolerance(1e-12);
        let r = solver.solve(f, 1.5).unwrap();

        assert!(
            f(r).abs() < 1e-11,
            "f(root) = {} exceeds tolerance",
            f(r).abs()
        );
        assert!((r - 2.0_f64.sqrt()).abs() < 1e-10);
    }

    #[test]
    fn handles_cubic() {
        // f(x)=x^3 - x, roots at -1, 0, 1 ⇒ 1
        let f = |x: f64| x * x * x - x;
        let solver = BrentSolver::new().tolerance(1e-12);
        let r = solver.solve(f, 0.85).unwrap();

        assert!(
            f(r).abs() < 1e-11,
            "f(root) = {} exceeds tolerance",
            f(r).abs()
        );
        assert!((r - 1.0).abs() < 1e-10);
    }

    #[test]
    fn simple_quadratic() {
        let f = |x: f64| x * x - 4.0; // root at x = 2
        let solver = BrentSolver::new().tolerance(1e-12);

        let root = solver.solve(f, 1.8).unwrap();

        assert!(
            f(root).abs() < 1e-11,
            "f(root) = {} exceeds tolerance",
            f(root).abs()
        );
        assert!((root - 2.0).abs() < 1e-10);
    }

    #[test]
    fn with_distant_guess() {
        // Case where initial guess is far from root
        let f = |x: f64| x * x * x - x - 2.0; // Cubic with root near 1.5
        let solver = BrentSolver::new().tolerance(1e-12);

        // Bad initial guess that would cause Newton to diverge
        let root = solver.solve(f, 100.0).unwrap();

        assert!(
            f(root).abs() < 1e-11,
            "f(root) = {} exceeds tolerance",
            f(root).abs()
        );
    }

    #[test]
    fn bond_yield() {
        // Financial application: yield-to-maturity type calculation
        let target_price = 95.0;
        let coupon = 5.0;
        let face_value = 100.0;
        let periods = 5.0;

        let f = |y: f64| {
            if y.abs() < 1e-10 {
                return coupon * periods + face_value - target_price;
            }
            let discount_factor = 1.0 / (1.0 + y);
            let annuity_pv = coupon * (1.0 - discount_factor.powf(periods)) / y;
            let principal_pv = face_value * discount_factor.powf(periods);
            annuity_pv + principal_pv - target_price
        };

        let solver = BrentSolver::new().tolerance(1e-10);
        let yield_result = solver.solve(f, 0.06).unwrap();

        // 5% coupon, $95 price, 5-period bond YTM ≈ 6.20%
        assert!(yield_result > 0.061 && yield_result < 0.063);
        assert!(
            f(yield_result).abs() < 1e-9,
            "f(yield) = {} exceeds tolerance",
            f(yield_result).abs()
        );
    }

    #[test]
    fn sqrt_function() {
        // Pathological case where derivative is problematic
        let f = |x: f64| (x - 1.5).signum() * (x - 1.5).abs().powf(0.5);
        let solver = BrentSolver::new().tolerance(1e-6);

        let root = solver.solve(f, 2.0).unwrap();
        assert!(
            f(root).abs() < 1e-5,
            "f(root) = {} exceeds tolerance",
            f(root).abs()
        );
        assert!((root - 1.5).abs() < 1e-5);
    }
}

// Solver Error Diagnostics Tests

mod error_diagnostics {
    use super::*;
    use finstack_quant_core::InputError;

    #[test]
    fn brent_no_bracket_found_error() {
        // Function with no roots
        let f = |x: f64| x * x + 1.0;
        let solver = BrentSolver::new().tolerance(1e-12);

        let result = solver.solve(f, 0.0);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_msg = format!("{}", err);

        // Error should contain diagnostic information about the bracket search
        assert!(
            err_msg.contains("sign") || err_msg.contains("bracket") || err_msg.contains("f("),
            "Error should explain bracket failure: {}",
            err_msg
        );
    }

    #[test]
    fn solver_convergence_failed_error_variant() {
        // Verify that InputError::SolverConvergenceFailed exists and can be constructed
        let err = InputError::SolverConvergenceFailed {
            iterations: 50,
            residual: 1e-5,
            last_x: 0.123,
            reason: "max iterations reached".to_string(),
        };

        let err_msg = format!("{}", err);
        assert!(err_msg.contains("50"));
        assert!(err_msg.contains("iterations"));
    }
}

mod serde_tests {
    use finstack_quant_core::math::integration::GaussHermiteQuadrature;
    use finstack_quant_core::math::solver::BrentSolver;

    #[test]
    fn brent_solver_roundtrip() {
        let solver = BrentSolver::new()
            .tolerance(1e-8)
            .initial_bracket_size(Some(0.5));

        let json = serde_json::to_string(&solver).unwrap();
        let deserialized: BrentSolver = serde_json::from_str(&json).unwrap();

        assert_eq!(solver.tolerance, deserialized.tolerance);
        assert_eq!(solver.max_iterations, deserialized.max_iterations);
        assert_eq!(solver.bracket_expansion, deserialized.bracket_expansion);
        assert_eq!(
            solver.initial_bracket_size,
            deserialized.initial_bracket_size
        );
    }

    #[test]
    fn gauss_hermite_new_supports_all_orders() {
        // Test all supported orders
        for order in [5, 7, 10, 15, 20] {
            let quad = GaussHermiteQuadrature::new(order);
            assert!(quad.is_ok(), "Order {} should be supported", order);
            assert_eq!(quad.unwrap().get_points().len(), order);
        }

        // Test unsupported orders
        for order in [1, 3, 8, 12, 25, 32] {
            let quad = GaussHermiteQuadrature::new(order);
            assert!(quad.is_err(), "Order {} should not be supported", order);
        }
    }

    #[test]
    fn gauss_hermite_higher_order_accuracy() {
        // Test that higher orders give better accuracy for E[X^4] = 3 (standard normal)
        let f = |x: f64| x.powi(4);

        let quad10 = GaussHermiteQuadrature::new(10).expect("valid order");
        let quad15 = GaussHermiteQuadrature::new(15).expect("valid order");
        let quad20 = GaussHermiteQuadrature::new(20).expect("valid order");

        let result10 = quad10.integrate(f);
        let result15 = quad15.integrate(f);
        let result20 = quad20.integrate(f);

        let expected = 3.0;

        // Higher orders should be more accurate (or at least as accurate)
        assert!(
            (result15 - expected).abs() <= (result10 - expected).abs() + 1e-10,
            "Order 15 ({}) should be at least as accurate as order 10 ({})",
            result15,
            result10
        );
        assert!(
            (result20 - expected).abs() <= (result15 - expected).abs() + 1e-10,
            "Order 20 ({}) should be at least as accurate as order 15 ({})",
            result20,
            result15
        );
    }
}
