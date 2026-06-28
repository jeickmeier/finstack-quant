use finstack_quant_core::math::{
    erf, gauss_legendre_integrate, gauss_legendre_integrate_adaptive,
    gauss_legendre_integrate_composite,
    integration::{adaptive_simpson, simpson_rule, trapezoidal_rule},
    norm_cdf, GaussHermiteQuadrature,
};
use std::f64::consts::PI;

#[test]
fn test_simpson_rule_polynomial() {
    // Test Simpson's rule on x² over [0, 1] = 1/3
    let f = |x: f64| x * x;
    let result = simpson_rule(f, 0.0, 1.0, 100).unwrap();
    assert!((result - 1.0 / 3.0).abs() < 1e-6);
}

#[test]
fn test_simpson_rule_sin() {
    // Test Simpson's rule on sin(x) over [0, π] = 2
    let f = |x: f64| x.sin();
    let result = simpson_rule(f, 0.0, PI, 100).unwrap();
    // Simpson with n=100 on sin(x): error ~ π/180 * (π/100)^4 ≈ 1.7e-9
    assert!((result - 2.0).abs() < 1e-7);
}

#[test]
fn test_trapezoidal_rule_linear() {
    // Test trapezoidal rule on linear function x over [0, 2] = 2
    let f = |x: f64| x;
    let result = trapezoidal_rule(f, 0.0, 2.0, 100).unwrap();
    assert!((result - 2.0).abs() < 1e-6);
}

#[test]
fn test_trapezoidal_rule_quadratic() {
    // Test trapezoidal rule on x² over [0, 1] = 1/3
    let f = |x: f64| x * x;
    let result = trapezoidal_rule(f, 0.0, 1.0, 1000).unwrap();
    assert!((result - 1.0 / 3.0).abs() < 1e-3); // Less accurate than Simpson
}

#[test]
fn test_adaptive_simpson_smooth_function() {
    // Test adaptive Simpson on e^(-x²) over [-2, 2]
    // For finite limits: ∫e^(-x²)dx from -a to a = √π · erf(a)
    // For a=2: √π · erf(2) ≈ 1.7641 (not √π ≈ 1.7725 which is for infinite limits)
    let f = |x: f64| (-x * x).exp();
    let result = adaptive_simpson(f, -2.0, 2.0, 1e-4, 20).unwrap();
    let expected = PI.sqrt() * erf(2.0);
    assert!(
        (result - expected).abs() < 1e-3,
        "Adaptive Simpson {} vs expected {}",
        result,
        expected
    );
}

#[test]
fn test_adaptive_simpson_oscillatory() {
    // Test on oscillatory function sin(10x) over [0, π]
    // Exact integral = (1 - cos(10π))/10 = 2/10 = 0.2
    let f = |x: f64| (10.0 * x).sin();
    let result = adaptive_simpson(f, 0.0, PI, 1e-4, 25).unwrap();
    let expected = (1.0 - (10.0 * PI).cos()) / 10.0;
    assert!((result - expected).abs() < 1e-2);
}

#[test]
fn test_financial_application_option_payoff() {
    // Test integration of option payoff max(S - K, 0) under lognormal
    // This would be used in Monte Carlo option pricing
    let strike = 100.0_f64;
    let spot = 100.0_f64;
    let vol = 0.2_f64;
    let time = 1.0_f64;

    // Simplified Black-Scholes integrand (just the payoff part)
    let f = |z: f64| {
        // z is standard normal, transform to stock price
        let log_s = spot.ln() + (-0.5 * vol * vol * time) + vol * time.sqrt() * z;
        let s = log_s.exp();
        (s - strike).max(0.0)
    };

    // Use Gauss-Hermite for integration over normal distribution
    let quad = GaussHermiteQuadrature::new(10).expect("valid order");
    let result = quad.integrate(f);

    // Result should be positive (call option value component)
    assert!(result > 0.0);

    // Validate against analytical Black-Scholes (undiscounted, zero rate)
    // BS call = S·N(d1) - K·N(d2) where r=q=0
    let d1 = ((spot / strike).ln() + 0.5 * vol * vol * time) / (vol * time.sqrt());
    let d2 = d1 - vol * time.sqrt();
    let bs_price = spot * norm_cdf(d1) - strike * norm_cdf(d2);

    // Gauss-Hermite should be within 0.5 of Black-Scholes for this ATM option.
    // Option payoffs have a kink so GH is less accurate than for smooth functions,
    // but 10-point GH should still achieve well under 10% relative error.
    assert!(
        (result - bs_price).abs() < 0.5,
        "GH integral {} vs Black-Scholes {} (diff {})",
        result,
        bs_price,
        (result - bs_price).abs()
    );
}

#[test]
fn test_integration_methods_comparison() {
    // Compare different methods on same function: x³ over [0, 2] = 4
    let f = |x: f64| x * x * x;
    let exact = 4.0;

    let simpson = simpson_rule(f, 0.0, 2.0, 100).unwrap();
    let trapezoidal = trapezoidal_rule(f, 0.0, 2.0, 100).unwrap();
    let adaptive = adaptive_simpson(f, 0.0, 2.0, 1e-6, 20).unwrap();

    // Simpson should be most accurate for polynomials
    assert!((simpson - exact).abs() < 1e-8);
    // Trapezoidal less accurate
    assert!((trapezoidal - exact).abs() < 1e-2);
    // Adaptive should be very accurate
    assert!((adaptive - exact).abs() < 1e-10);
}

#[test]
fn test_integration_error_cases() {
    let f = |x: f64| x;

    // Simpson with odd number of intervals should fail
    assert!(simpson_rule(f, 0.0, 1.0, 99).is_err());

    // Zero intervals should fail
    assert!(simpson_rule(f, 0.0, 1.0, 0).is_err());
    assert!(trapezoidal_rule(f, 0.0, 1.0, 0).is_err());
}

#[test]
fn test_financial_yield_curve_integration() {
    // Test integration relevant to yield curve construction
    // Integrate forward rate over time to get zero rate
    let f = |t: f64| 0.03 + 0.01 * t; // Simple linear forward rate

    // Integrate from 0 to 5 years
    let integrated_rate = simpson_rule(f, 0.0, 5.0, 1000).unwrap() / 5.0_f64;

    // Should be around 5.5% (average of 3% to 8%)
    assert!((integrated_rate - 0.055).abs() < 1e-3);
}

// ==========================================
// Additional integration behavior
// ==========================================

#[test]
fn test_gauss_hermite_adaptive_low_tolerance() {
    let quad = GaussHermiteQuadrature::new(5).expect("valid order");
    let f = |x: f64| x * x;

    let result = quad.integrate_adaptive(f, 1e-10);
    // x² over standard normal should be 1.0
    assert!(
        (result - 1.0).abs() < 1e-8,
        "E[X^2] should be 1.0, got {}",
        result
    );
}

#[test]
fn test_gauss_hermite_adaptive_high_tolerance() {
    let quad = GaussHermiteQuadrature::new(7).expect("valid order");
    let f = |x: f64| x * x * x * x; // x^4

    let result = quad.integrate_adaptive(f, 1e-2);
    // x^4 over standard normal should be 3.0
    assert!(
        (result - 3.0).abs() < 1e-6,
        "E[X^4] should be 3.0, got {}",
        result
    );
}

#[test]
fn test_gauss_hermite_adaptive_order_10_no_refinement() {
    // Order 10 shouldn't refine (it's already the highest)
    let quad = GaussHermiteQuadrature::new(10).expect("valid order");
    let f = |x: f64| x * x;

    let base = quad.integrate(f);
    let adaptive = quad.integrate_adaptive(f, 1e-10);

    // Should return the same result (no refinement)
    assert!((base - adaptive).abs() < 1e-12);
}

#[test]
fn test_gauss_hermite_constant_function() {
    let quad = GaussHermiteQuadrature::new(7).expect("valid order");

    // Integrate constant function
    let result = quad.integrate(|_x| 5.0);

    // Should equal 5.0 (constant * 1.0)
    assert!((result - 5.0).abs() < 1e-6);
}

#[test]
fn test_gauss_hermite_linear_function() {
    let quad = GaussHermiteQuadrature::new(7).expect("valid order");

    // Integrate odd function x over symmetric domain
    let result = quad.integrate(|x| x);

    // Should be 0 (odd function)
    assert!(result.abs() < 1e-12);
}

#[test]
fn test_gauss_hermite_high_order_polynomial() {
    let quad = GaussHermiteQuadrature::new(10).expect("valid order");

    // x^6 over standard normal = 15 (formula: (2n-1)!! for x^(2n))
    let result = quad.integrate(|x| x.powi(6));
    assert!(
        (result - 15.0).abs() < 1e-6,
        "E[X^6] should be 15.0, got {}",
        result
    );
}

#[test]
fn test_adaptive_simpson_constant_function() {
    let f = |_x: f64| 10.0;

    let result = adaptive_simpson(f, 0.0, 5.0, 1e-6, 20).unwrap();
    // Integral of 10 from 0 to 5 = 50; constant functions integrate exactly
    assert!((result - 50.0).abs() < 1e-12);
}

#[test]
fn test_adaptive_simpson_discontinuous_function() {
    // Step function — discontinuity at x=1 is pathological for adaptive quadrature.
    // The algorithm recurses deeply into the jump until it hits max_depth.
    // Use a loose tolerance and modest max_depth so that sub-intervals away from the
    // jump converge quickly (error < tol) while the interval straddling x=1 also
    // exhausts recursion without triggering an error (error near the jump will be
    // small in absolute terms once the sub-interval is tiny).
    let f = |x: f64| if x < 1.0 { 0.0 } else { 1.0 };
    // max_depth=5 with tol=0.1 converges everywhere except near x=1, where
    // the sub-interval contribution is negligible.
    let result = adaptive_simpson(f, 0.0, 2.0, 0.1, 5).unwrap();
    assert!(
        (result - 1.0).abs() < 0.5,
        "result {result} too far from 1.0"
    );
}

#[test]
fn test_gauss_legendre_adaptive_max_depth_returns_convergence_error() {
    use finstack_quant_core::{Error, InputError};

    // tol=0.0 cannot be met for a non-polynomial integrand; with max_depth=0
    // the routine must error (matching adaptive_simpson) instead of silently
    // returning the unconverged estimate.
    let f = |x: f64| (10.0 * x).sin() / (1.0 + x * x);
    let result = gauss_legendre_integrate_adaptive(f, 0.0, 5.0, 2, 0.0, 0);

    assert!(
        result.is_err(),
        "expected convergence error but got Ok({:?})",
        result.ok()
    );
    assert!(matches!(
        result.unwrap_err(),
        Error::Input(InputError::SolverConvergenceFailed { .. })
    ));
}

#[test]
fn test_adaptive_simpson_max_depth_returns_convergence_error() {
    use finstack_quant_core::{Error, InputError};

    // x^5 has a 5th-order term; Simpson's rule is only exact for polynomials of
    // degree ≤ 3, so the error estimate is strictly > 0 on [0, 1].
    // tol=0.0 can therefore never be satisfied: with max_depth=0 the convergence
    // check fires on the first call.
    let f = |x: f64| x.powi(5);
    let result = adaptive_simpson(f, 0.0, 1.0, 0.0, 0);

    assert!(
        result.is_err(),
        "expected convergence error but got Ok({:?})",
        result.ok()
    );
    let err = result.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Input(InputError::SolverConvergenceFailed { .. })
        ),
        "expected SolverConvergenceFailed, got: {:?}",
        err
    );
    // The reason string should name adaptive_simpson and max_depth.
    if let Error::Input(InputError::SolverConvergenceFailed { reason, .. }) = &err {
        assert!(
            reason.contains("adaptive_simpson") || reason.contains("max_depth"),
            "reason should mention adaptive_simpson or max_depth, got: {reason}"
        );
    }
}

#[test]
fn test_adaptive_simpson_smooth_function_still_converges() {
    // Smoke test: normal smooth function must still return accurate Ok result.
    let f = |x: f64| x.exp();
    let result = adaptive_simpson(f, 0.0, 1.0, 1e-8, 50).unwrap();
    let exact = 1.0_f64.exp() - 1.0;
    assert!(
        (result - exact).abs() < 1e-6,
        "expected accurate integral, got {result}"
    );
}

#[test]
fn test_adaptive_simpson_various_tolerances() {
    let f = |x: f64| x.exp();

    // Test with different tolerances
    let result1 = adaptive_simpson(f, 0.0, 1.0, 1e-2, 20).unwrap();
    let result2 = adaptive_simpson(f, 0.0, 1.0, 1e-6, 20).unwrap();
    let result3 = adaptive_simpson(f, 0.0, 1.0, 1e-10, 20).unwrap();

    let exact = 1.0_f64.exp() - 1.0; // e - 1

    // Tighter tolerance should give better accuracy
    assert!((result1 - exact).abs() < 0.1);
    assert!((result2 - exact).abs() < 1e-4);
    assert!((result3 - exact).abs() < 1e-8);
}

#[test]
fn test_simpson_rule_edge_cases() {
    let f = |x: f64| x;

    // Zero-width interval
    let result = simpson_rule(f, 5.0, 5.0, 100).unwrap();
    assert!(result.abs() < 1e-12);

    // Negative interval (b < a)
    let result = simpson_rule(f, 5.0, 0.0, 100).unwrap();
    assert!((result + 12.5).abs() < 1e-3); // Should be negative of [0,5]
}

#[test]
fn test_trapezoidal_rule_edge_cases() {
    let f = |x: f64| x * x;

    // Very small interval
    let result = trapezoidal_rule(f, 0.0, 0.001, 10).unwrap();
    assert!(result.abs() < 1e-6);

    // Large interval with few points
    let result = trapezoidal_rule(f, 0.0, 100.0, 10).unwrap();
    let exact = 100.0_f64.powi(3) / 3.0;
    // Should be less accurate but still reasonable
    assert!((result - exact).abs() / exact < 0.1); // Within 10%
}

#[test]
fn test_gauss_legendre_basic() {
    // x² on [0,1] = 1/3
    let f = |x: f64| x * x;

    // Test different orders
    let result2 = gauss_legendre_integrate(f, 0.0, 1.0, 2).unwrap();
    let result4 = gauss_legendre_integrate(f, 0.0, 1.0, 4).unwrap();
    let result8 = gauss_legendre_integrate(f, 0.0, 1.0, 8).unwrap();

    let exact = 1.0 / 3.0;

    // All should be accurate for polynomial
    assert!((result2 - exact).abs() < 1e-10);
    assert!((result4 - exact).abs() < 1e-10);
    assert!((result8 - exact).abs() < 1e-10);
}

#[test]
fn test_gauss_legendre_invalid_bounds() {
    let f = |x: f64| x;

    // Infinite bounds
    assert!(gauss_legendre_integrate(f, 0.0, f64::INFINITY, 4).is_err());
    assert!(gauss_legendre_integrate(f, f64::NEG_INFINITY, 0.0, 4).is_err());

    // NaN bounds
    assert!(gauss_legendre_integrate(f, 0.0, f64::NAN, 4).is_err());
}

#[test]
fn test_gauss_legendre_equal_bounds() {
    let f = |x: f64| x * x;

    // Equal bounds should give 0
    let result = gauss_legendre_integrate(f, 5.0, 5.0, 4).unwrap();
    assert!(result.abs() < 1e-12);
}

#[test]
fn test_gauss_legendre_invalid_order() {
    let f = |x: f64| x;

    // Unsupported order
    assert!(gauss_legendre_integrate(f, 0.0, 1.0, 3).is_err());
    assert!(gauss_legendre_integrate(f, 0.0, 1.0, 7).is_err());
}

#[test]
fn test_gauss_legendre_composite() {
    // x³ on [0,2] = 4
    let f = |x: f64| x * x * x;

    // Single panel
    let result1 = gauss_legendre_integrate_composite(f, 0.0, 2.0, 4, 1).unwrap();

    // Multiple panels
    let result10 = gauss_legendre_integrate_composite(f, 0.0, 2.0, 4, 10).unwrap();

    let exact = 4.0;

    // Both should be accurate
    assert!((result1 - exact).abs() < 1e-10);
    assert!((result10 - exact).abs() < 1e-10);
}

#[test]
fn test_gauss_legendre_composite_zero_panels() {
    let f = |x: f64| x;

    assert!(gauss_legendre_integrate_composite(f, 0.0, 1.0, 4, 0).is_err());
}

#[test]
fn test_gauss_legendre_adaptive() {
    // Smooth function
    let f = |x: f64| x.exp();

    let result = gauss_legendre_integrate_adaptive(f, 0.0, 1.0, 4, 1e-8, 20).unwrap();

    let exact = 1.0_f64.exp() - 1.0;
    assert!((result - exact).abs() < 1e-6);
}

#[test]
fn test_gauss_legendre_adaptive_oscillatory() {
    // Oscillatory function
    let f = |x: f64| (10.0 * x).sin();

    let result = gauss_legendre_integrate_adaptive(f, 0.0, PI, 8, 1e-6, 25).unwrap();

    let exact = (1.0 - (10.0 * PI).cos()) / 10.0;
    assert!((result - exact).abs() < 1e-3);
}

#[test]
fn test_gauss_legendre_adaptive_max_depth() {
    let f = |x: f64| x * x;

    // Even with max_depth=0, should still return result
    let result = gauss_legendre_integrate_adaptive(f, 0.0, 1.0, 4, 1e-10, 0).unwrap();

    // Should be reasonably accurate
    assert!((result - 1.0 / 3.0).abs() < 1e-6);
}

#[test]
fn test_integration_numerical_stability() {
    // Test with function that could cause numerical issues
    let f = |x: f64| {
        if x.abs() < 1e-10 {
            1.0 // Avoid division by zero
        } else {
            x.sin() / x // sinc function
        }
    };

    let result = adaptive_simpson(f, -PI, PI, 1e-4, 20).unwrap();
    // Sinc function integral is well-known
    assert!(result.is_finite());
    assert!(result > 0.0);
}

#[test]
fn test_convergence_behavior() {
    let f = |x: f64| x.exp();
    let exact = 1.0_f64.exp() - 1.0;

    // Test convergence as we increase intervals
    let result10 = simpson_rule(f, 0.0, 1.0, 10).unwrap();
    let result100 = simpson_rule(f, 0.0, 1.0, 100).unwrap();
    let result1000 = simpson_rule(f, 0.0, 1.0, 1000).unwrap();

    // Error should decrease
    let err10 = (result10 - exact).abs();
    let err100 = (result100 - exact).abs();
    let err1000 = (result1000 - exact).abs();

    assert!(err100 < err10);
    assert!(err1000 < err100);
}

#[test]
fn test_gauss_hermite_serde() {
    let quad5 = GaussHermiteQuadrature::new(5).expect("valid order");
    let quad7 = GaussHermiteQuadrature::new(7).expect("valid order");
    let quad10 = GaussHermiteQuadrature::new(10).expect("valid order");

    // Serialize
    let json5 = serde_json::to_string(&quad5).unwrap();
    let json7 = serde_json::to_string(&quad7).unwrap();
    let json10 = serde_json::to_string(&quad10).unwrap();

    // Deserialize
    let deser5: GaussHermiteQuadrature = serde_json::from_str(&json5).unwrap();
    let deser7: GaussHermiteQuadrature = serde_json::from_str(&json7).unwrap();
    let deser10: GaussHermiteQuadrature = serde_json::from_str(&json10).unwrap();

    // Check they work the same
    let f = |x: f64| x * x;
    assert!((quad5.integrate(f) - deser5.integrate(f)).abs() < 1e-12);
    assert!((quad7.integrate(f) - deser7.integrate(f)).abs() < 1e-12);
    assert!((quad10.integrate(f) - deser10.integrate(f)).abs() < 1e-12);
}

#[test]
fn test_gauss_hermite_serde_invalid_order() {
    let json = r#"{"order":99}"#;
    let result: Result<GaussHermiteQuadrature, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

// ── H1 regression: compensated accumulation for order-20 cancellation-prone integrand ──
//
// Integrand f(x) = x^2 - 1 has E[f(X)] = 0 (exact). With tiny weights (2.2e-13)
// and moderate function values the naive sum accumulates rounding error; Neumaier
// compensated summation should keep |result| < 1e-10.
#[test]
fn test_gauss_hermite_order20_cancellation_accuracy() {
    let quad = GaussHermiteQuadrature::new(20).expect("order 20 valid");
    // E[X^2 - 1] = 1 - 1 = 0
    let result = quad.integrate(|x| x * x - 1.0);
    assert!(
        result.abs() < 1e-10,
        "E[X^2-1] should be 0; compensated sum got {result}"
    );
}

// ── H3 regression: large-bounds/small-width midpoint safety ──
//
// With a = 1e14 and b = 1e14 + 1 the naive midpoint (a+b)/2 has catastrophic
// cancellation on 64-bit floats; a + 0.5*(b-a) is stable. Integrand is
// constant 1, so the exact answer is 1.0.
#[test]
fn test_gauss_legendre_large_bounds_small_width_midpoint() {
    let a = 1.0e14_f64;
    let b = a + 1.0;
    let result = gauss_legendre_integrate(|_| 1.0, a, b, 4).expect("valid");
    assert!(
        (result - 1.0).abs() < 1e-6,
        "Integral of 1 over [1e14, 1e14+1] should be 1.0, got {result}"
    );
}

// ── H2 guard: tiny-but-nonzero interval must not collapse to zero ──
//
// The `a == b` shortcut is intentional for exact zero-width intervals.
// Confirm a very small but nonzero interval is still integrated.
#[test]
fn test_gauss_legendre_tiny_nonzero_interval_not_collapsed() {
    // Interval width = 1e-12; integrand = 1; exact answer ≈ 1e-12
    let a = 1.0_f64;
    let b = 1.0 + 1.0e-12;
    let result = gauss_legendre_integrate(|_| 1.0, a, b, 4).expect("valid");
    // Must be positive — interval must not be treated as zero-width.
    assert!(
        result > 0.0,
        "Tiny nonzero interval must not be treated as zero-width; got {result}"
    );
    // Result should match (b - a) to within floating-point precision of that subtraction.
    let width = b - a;
    assert!(
        (result - width).abs() / width < 1.0e-10,
        "Expected ~{width}, got {result}"
    );
}
