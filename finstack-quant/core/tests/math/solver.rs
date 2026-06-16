//! Root-finding solver tests.
//!
//! This module consolidates all solver tests including:
//! - Brent solver tests
//! - Newton solver tests
//! - Serialization tests

use finstack_quant_core::math::solver::{BrentSolver, NewtonSolver, Solver};

// ============================================================================
// Brent Solver Tests
// ============================================================================

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

// ============================================================================
// Newton Solver Tests
// ============================================================================

mod newton {
    use super::*;

    #[test]
    fn finds_root_simple_quadratic() {
        // f(x) = x^2 - 2 ⇒ root = sqrt(2)
        let f = |x: f64| x * x - 2.0;
        let solver = NewtonSolver::new().tolerance(1e-12);
        let r = solver.solve(f, 1.5).unwrap();

        assert!(
            f(r).abs() < 1e-11,
            "f(root) = {} exceeds tolerance",
            f(r).abs()
        );
        assert!((r - 2.0_f64.sqrt()).abs() < 1e-10);
    }

    #[test]
    fn with_analytical_derivative() {
        // f(x) = x^2 - 2, f'(x) = 2x
        let f = |x: f64| x * x - 2.0;
        let df = |x: f64| 2.0 * x;
        let solver = NewtonSolver::new().tolerance(1e-12);
        let r = solver.solve_with_derivative(f, df, 1.5).unwrap();

        assert!(
            f(r).abs() < 1e-11,
            "f(root) = {} exceeds tolerance",
            f(r).abs()
        );
        assert!((r - 2.0_f64.sqrt()).abs() < 1e-10);
    }

    #[test]
    fn handles_cubic() {
        // f(x) = x^3 - x, roots at -1, 0, 1
        let f = |x: f64| x * x * x - x;
        let solver = NewtonSolver::new().tolerance(1e-12);
        let r = solver.solve(f, 0.85).unwrap();

        assert!(
            f(r).abs() < 1e-11,
            "f(root) = {} exceeds tolerance",
            f(r).abs()
        );
        assert!((r - 1.0).abs() < 1e-10);
    }

    #[test]
    fn with_analytical_derivative_cubic() {
        // f(x) = x^3 - 2x - 5, f'(x) = 3x^2 - 2
        let f = |x: f64| x.powi(3) - 2.0 * x - 5.0;
        let df = |x: f64| 3.0 * x.powi(2) - 2.0;
        let solver = NewtonSolver::new().tolerance(1e-12);
        let r = solver.solve_with_derivative(f, df, 2.0).unwrap();

        assert!(
            f(r).abs() < 1e-11,
            "f(root) = {} exceeds tolerance",
            f(r).abs()
        );
        // Root of x³ - 2x - 5 ≈ 2.09455132 (verified numerically)
        assert!((r - 2.09455132_f64).abs() < 1e-6);
    }

    #[test]
    fn bond_yield() {
        // Same financial application as Brent: yield-to-maturity
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

        let solver = NewtonSolver::new().tolerance(1e-10);
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
    fn transcendental_equation() {
        // f(x) = e^x - 3x, has root near x ≈ 1.05
        let f = |x: f64| x.exp() - 3.0 * x;
        let df = |x: f64| x.exp() - 3.0;
        let solver = NewtonSolver::new().tolerance(1e-12);
        let r = solver.solve_with_derivative(f, df, 1.0).unwrap();

        assert!(
            f(r).abs() < 1e-11,
            "f(root) = {} exceeds tolerance",
            f(r).abs()
        );
    }
}

// ============================================================================
// Serialization Tests
// ============================================================================

// ============================================================================
// Solver Error Diagnostics Tests
// ============================================================================

mod error_diagnostics {
    use super::*;
    use finstack_quant_core::math::solver::BracketHint;
    use finstack_quant_core::InputError;

    #[test]
    fn newton_error_contains_iteration_count() {
        // Function that doesn't converge - always returns 1.0
        let f = |_x: f64| 1.0;
        let solver = NewtonSolver::new().tolerance(1e-12).max_iterations(10);

        let result = solver.solve(f, 0.0);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_msg = format!("{}", err);

        // Error message should contain iteration count
        assert!(
            err_msg.contains("10") || err_msg.contains("iterations"),
            "Error should mention iterations: {}",
            err_msg
        );
    }

    #[test]
    fn newton_error_contains_residual() {
        // Function with no root in reasonable range
        let f = |x: f64| x * x + 1.0; // always positive
        let solver = NewtonSolver::new().tolerance(1e-12).max_iterations(5);

        let result = solver.solve(f, 1.0);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_msg = format!("{}", err);

        // Error message should contain diagnostic information
        assert!(
            err_msg.contains("residual") || err_msg.contains("e-") || err_msg.contains("e+"),
            "Error should contain residual info: {}",
            err_msg
        );
    }

    #[test]
    fn newton_derivative_too_small_error() {
        // Function with very flat region where derivative is essentially zero
        // f(x) = tanh(1000*(x-0.5)) has nearly zero derivative far from x=0.5
        let f = |x: f64| (1000.0 * (x - 0.5)).tanh() - 0.5;
        let solver = NewtonSolver::new()
            .tolerance(1e-12)
            .min_derivative(1e-10)
            .max_iterations(20);

        // Starting far from the root in the flat region
        let result = solver.solve(f, 10.0);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_msg = format!("{}", err);

        // Error should mention derivative or convergence failure
        assert!(
            err_msg.contains("derivative") || err_msg.contains("Solver failed"),
            "Error should mention derivative issue or solver failure: {}",
            err_msg
        );
    }

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
    fn bracket_hint_implied_vol() {
        let solver = BrentSolver::new().bracket_hint(BracketHint::ImpliedVol);

        // Initial bracket should be ±0.2
        assert_eq!(solver.initial_bracket_size, Some(0.2));
    }

    #[test]
    fn bracket_hint_rate() {
        let solver = BrentSolver::new().bracket_hint(BracketHint::Rate);

        assert_eq!(solver.initial_bracket_size, Some(0.02));
    }

    #[test]
    fn bracket_hint_ytm() {
        let solver = BrentSolver::new().bracket_hint(BracketHint::Ytm);

        assert_eq!(solver.initial_bracket_size, Some(0.02));
    }

    #[test]
    fn bracket_hint_spread() {
        let solver = BrentSolver::new().bracket_hint(BracketHint::Spread);

        assert_eq!(solver.initial_bracket_size, Some(0.005));
    }

    #[test]
    fn bracket_hint_custom() {
        let solver = BrentSolver::new().bracket_hint(BracketHint::Custom(0.5));

        assert_eq!(solver.initial_bracket_size, Some(0.5));
    }

    #[test]
    fn bracket_hint_improves_convergence() {
        // Implied vol scenario: price error function
        let target_price = 10.0;
        let f = |vol: f64| vol * 100.0 - target_price; // root at vol = 0.1

        // With implied vol hint (bracket ±0.2), should find root quickly
        let solver = BrentSolver::new()
            .bracket_hint(BracketHint::ImpliedVol)
            .tolerance(1e-10);

        let root = solver.solve(f, 0.2).unwrap();
        assert!((root - 0.1).abs() < 1e-9);
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
    use finstack_quant_core::math::solver::{BrentSolver, NewtonSolver};

    #[test]
    fn newton_solver_roundtrip() {
        let solver = NewtonSolver::new().tolerance(1e-10).max_iterations(100);

        let json = serde_json::to_string(&solver).unwrap();
        let deserialized: NewtonSolver = serde_json::from_str(&json).unwrap();

        assert_eq!(solver.tolerance, deserialized.tolerance);
        assert_eq!(solver.max_iterations, deserialized.max_iterations);
        assert_eq!(solver.fd_step, deserialized.fd_step);
    }

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
    fn gauss_hermite_quadrature_order_5() {
        let quad5 = GaussHermiteQuadrature::new(5).expect("valid order");
        let json = serde_json::to_string(&quad5).unwrap();
        let deserialized: GaussHermiteQuadrature = serde_json::from_str(&json).unwrap();
        assert_eq!(quad5.points.len(), deserialized.points.len());
        assert_eq!(quad5.weights.len(), deserialized.weights.len());
    }

    #[test]
    fn gauss_hermite_quadrature_order_7() {
        let quad7 = GaussHermiteQuadrature::new(7).expect("valid order");
        let json = serde_json::to_string(&quad7).unwrap();
        let deserialized: GaussHermiteQuadrature = serde_json::from_str(&json).unwrap();
        assert_eq!(quad7.points.len(), deserialized.points.len());
        assert_eq!(quad7.weights.len(), deserialized.weights.len());
    }

    #[test]
    fn gauss_hermite_quadrature_order_10() {
        let quad10 = GaussHermiteQuadrature::new(10).expect("valid order");
        let json = serde_json::to_string(&quad10).unwrap();
        let deserialized: GaussHermiteQuadrature = serde_json::from_str(&json).unwrap();
        assert_eq!(quad10.points.len(), deserialized.points.len());
        assert_eq!(quad10.weights.len(), deserialized.weights.len());

        let quad5_json =
            serde_json::to_string(&GaussHermiteQuadrature::new(5).expect("valid order")).unwrap();
        assert!(quad5_json.contains("\"order\":5"));
    }

    #[test]
    fn gauss_hermite_quadrature_order_15() {
        let quad15 = GaussHermiteQuadrature::new(15).expect("valid order");
        assert_eq!(quad15.points.len(), 15);
        assert_eq!(quad15.weights.len(), 15);

        let json = serde_json::to_string(&quad15).unwrap();
        let deserialized: GaussHermiteQuadrature = serde_json::from_str(&json).unwrap();
        assert_eq!(quad15.points.len(), deserialized.points.len());
        assert_eq!(quad15.weights.len(), deserialized.weights.len());
        assert!(json.contains("\"order\":15"));
    }

    #[test]
    fn gauss_hermite_quadrature_order_20() {
        let quad20 = GaussHermiteQuadrature::new(20).expect("valid order");
        assert_eq!(quad20.points.len(), 20);
        assert_eq!(quad20.weights.len(), 20);

        let json = serde_json::to_string(&quad20).unwrap();
        let deserialized: GaussHermiteQuadrature = serde_json::from_str(&json).unwrap();
        assert_eq!(quad20.points.len(), deserialized.points.len());
        assert_eq!(quad20.weights.len(), deserialized.weights.len());
        assert!(json.contains("\"order\":20"));
    }

    #[test]
    fn gauss_hermite_new_supports_all_orders() {
        // Test all supported orders
        for order in [5, 7, 10, 15, 20] {
            let quad = GaussHermiteQuadrature::new(order);
            assert!(quad.is_ok(), "Order {} should be supported", order);
            assert_eq!(quad.unwrap().points.len(), order);
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

    #[test]
    fn solver_configs_roundtrip() {
        let newton = NewtonSolver {
            tolerance: 1e-15,
            max_iterations: 200,
            fd_step: 1e-7,
            min_derivative: 1e-14,
            min_derivative_rel: 1e-6,
            residual_scale: 1.0,
        };

        let json = serde_json::to_string_pretty(&newton).unwrap();
        let newton2: NewtonSolver = serde_json::from_str(&json).unwrap();

        assert_eq!(newton.tolerance, newton2.tolerance);
        assert_eq!(newton.max_iterations, newton2.max_iterations);
        assert_eq!(newton.fd_step, newton2.fd_step);

        assert!(json.contains("\"tolerance\""));
        assert!(json.contains("\"max_iterations\""));
        assert!(json.contains("\"fd_step\""));
    }

    #[test]
    fn quadrature_functional_equivalence() {
        let quad_orig = GaussHermiteQuadrature::new(7).expect("valid order");
        let json = serde_json::to_string(&quad_orig).unwrap();
        let quad_deser: GaussHermiteQuadrature = serde_json::from_str(&json).unwrap();

        let f = |x: f64| x * x;
        let result_orig = quad_orig.integrate(f);
        let result_deser = quad_deser.integrate(f);

        assert!((result_orig - result_deser).abs() < 1e-15);
    }
}
