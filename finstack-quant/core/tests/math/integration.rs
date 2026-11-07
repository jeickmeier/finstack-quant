//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::math::{
    integration::{
        GaussHermiteQuadrature, adaptive_simpson, adaptive_simpson_pair,
        gauss_legendre_integrate_adaptive, gauss_legendre_integrate_composite, simpson_rule,
    },
    special_functions::{erf, norm_cdf},
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
    let adaptive = adaptive_simpson(f, 0.0, 2.0, 1e-6, 20).unwrap();

    // Simpson should be most accurate for polynomials
    assert!((simpson - exact).abs() < 1e-8);
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

// Additional integration behavior

#[test]
fn gauss_hermite_supported_orders_match_standard_normal_moments() {
    let moments = [(0, 1.0), (1, 0.0), (2, 1.0), (4, 3.0), (6, 15.0)];

    for order in [5, 7, 10, 15, 20] {
        let quad = GaussHermiteQuadrature::new(order).expect("supported order");
        for (power, expected) in moments {
            let actual = quad.integrate(|x| x.powi(power));
            assert!(
                (actual - expected).abs() <= 1e-12,
                "order {order}, E[X^{power}]: got {actual:.16e}, expected {expected:.16e}"
            );
        }
    }
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

#[test]
fn production_credit_audit_simpson_uses_total_error_budget() {
    let boundary = 0.123_456_7;
    let integral = adaptive_simpson(
        |x| if x >= boundary { 1.0 } else { 0.0 },
        0.0,
        1.0,
        1e-6,
        20,
    )
    .expect("localized discontinuity fits the global error budget");
    assert!((integral - (1.0 - boundary)).abs() <= 1e-6);
    assert!(
        adaptive_simpson(
            |x| if x >= boundary { 1.0 } else { 0.0 },
            0.0,
            1.0,
            1e-12,
            2,
        )
        .is_err()
    );
}

#[test]
fn adaptive_simpson_pair_checks_unequal_scales_independently() {
    // The large, exactly integrated component must not hide quadrature error
    // in the small component, regardless of tuple order.
    let large = 1_048_576.0;
    let small_exact = 1e-3 * (1.0_f64.exp() - 1.0);
    for swapped in [false, true] {
        let (first, second) = adaptive_simpson_pair(
            |x| {
                let small = 1e-3 * x.exp();
                if swapped {
                    (small, large)
                } else {
                    (large, small)
                }
            },
            0.0,
            1.0,
            1e-12,
            24,
        )
        .expect("each independent absolute budget converges");
        let (large_actual, small_actual) = if swapped {
            (second, first)
        } else {
            (first, second)
        };
        assert!((large_actual - large).abs() < 1e-9);
        assert!((small_actual - small_exact).abs() < 2e-12);
    }
}

#[test]
fn adaptive_simpson_pair_refines_distinct_localized_components() {
    // Both peaks have analytic arctangent integrals and need refinement in
    // different parts of the domain. The second also has a smaller scale.
    let (first, second) = adaptive_simpson_pair(
        |x| {
            (
                1.0 / (1.0 + (40.0 * (x - 0.2)).powi(2)),
                1e-3 / (1.0 + (80.0 * (x - 0.8)).powi(2)),
            )
        },
        0.0,
        1.0,
        1e-11,
        24,
    )
    .expect("both localized integrals converge");
    let first_exact = (32.0_f64.atan() - (-8.0_f64).atan()) / 40.0;
    let second_exact = 1e-3 * (16.0_f64.atan() - (-64.0_f64).atan()) / 80.0;
    assert!((first - first_exact).abs() < 2e-11);
    assert!((second - second_exact).abs() < 2e-11);
}

#[test]
fn adaptive_simpson_pair_preserves_total_error_budget_at_depth_limit() {
    use finstack_quant_core::{Error, InputError};

    let boundary = 0.123_456_7;
    let f = |x: f64| (if x >= boundary { 1.0 } else { 0.0 }, x * x * x);
    let (step, cube) = adaptive_simpson_pair(f, 0.0, 1.0, 1e-6, 20)
        .expect("a depth-limited leaf can fit the global budget");
    assert!((step - (1.0 - boundary)).abs() <= 1e-6);
    assert!((cube - 0.25).abs() <= 1e-12);
    assert!(matches!(
        adaptive_simpson_pair(f, 0.0, 1.0, 1e-12, 2),
        Err(Error::Input(InputError::SolverConvergenceFailed { .. }))
    ));
}

#[test]
fn adaptive_simpson_pair_reports_the_failing_component() {
    use finstack_quant_core::{Error, InputError};

    // Simpson integrates the constant exactly, but x^5 cannot meet zero
    // tolerance at depth zero. Either tuple position must cause failure.
    for failing_component in [1, 2] {
        let result = adaptive_simpson_pair(
            |x| {
                if failing_component == 1 {
                    (x.powi(5), 1.0)
                } else {
                    (1.0, x.powi(5))
                }
            },
            0.0,
            1.0,
            0.0,
            0,
        );
        let Err(Error::Input(InputError::SolverConvergenceFailed {
            iterations,
            residual,
            last_x,
            reason,
        })) = result
        else {
            panic!("expected component {failing_component} convergence failure: {result:?}");
        };
        assert_eq!(iterations, 0);
        assert!(residual.is_finite() && residual > 0.0);
        assert_eq!(last_x, 0.5);
        assert!(reason.contains(&format!("component {failing_component}")));
    }

    let both = adaptive_simpson_pair(|x| (x.powi(5), x.powi(6)), 0.0, 1.0, 0.0, 0);
    let Err(Error::Input(InputError::SolverConvergenceFailed { reason, .. })) = both else {
        panic!("both unconverged components must fail: {both:?}");
    };
    assert!(reason.contains("component 1"));
}

#[test]
fn adaptive_simpson_pair_rejects_non_finite_components_and_arithmetic() {
    use finstack_quant_core::{Error, InputError};

    for values in [
        (f64::NAN, 1.0),
        (1.0, f64::NAN),
        (f64::INFINITY, 1.0),
        (1.0, f64::NEG_INFINITY),
        // Finite callback values whose Simpson weighted sum overflows must
        // not become a successful non-finite estimate.
        (1.0, f64::MAX / 2.0),
    ] {
        assert!(matches!(
            adaptive_simpson_pair(|_| values, 0.0, 1.0, 1e-8, 20),
            Err(Error::Input(InputError::SolverConvergenceFailed { .. }))
        ));
    }
}

#[test]
fn adaptive_simpson_pair_preserves_interval_and_budget_validation() {
    use finstack_quant_core::Error;

    for (a, b, tol) in [
        (f64::NAN, 1.0, 1e-8),
        (0.0, f64::INFINITY, 1e-8),
        (0.0, 1.0, -1e-8),
        (0.0, 1.0, f64::NAN),
        (0.0, 1.0, f64::INFINITY),
    ] {
        assert!(matches!(
            adaptive_simpson_pair(|x| (x, x * x), a, b, tol, 20),
            Err(Error::Validation(_))
        ));
    }

    let zero = adaptive_simpson_pair(|x| (x, x * x), 2.0, 2.0, 0.0, 0)
        .expect("finite zero-width integrals");
    assert_eq!(zero, (0.0, 0.0));
    let reversed = adaptive_simpson_pair(|x| (1.0, x * x * x), 1.0, 0.0, 0.0, 0)
        .expect("exact signed polynomial integrals");
    assert_eq!(reversed, (-1.0, -0.25));
}

#[test]
fn adaptive_simpson_pair_shares_callback_work_and_preserves_node_order() {
    use std::cell::{Cell, RefCell};

    let calls = RefCell::new(Vec::new());
    let exact = adaptive_simpson_pair(
        |x| {
            calls.borrow_mut().push(x);
            (x * x, x * x * x)
        },
        0.0,
        1.0,
        1e-12,
        0,
    )
    .expect("exact polynomial pair");
    assert_eq!(*calls.borrow(), [0.0, 1.0, 0.5, 0.25, 0.75]);
    assert!((exact.0 - 1.0 / 3.0).abs() < 1e-12);
    assert!((exact.1 - 0.25).abs() < 1e-12);

    let pair_calls = Cell::new(0);
    let pair = adaptive_simpson_pair(
        |x| {
            pair_calls.set(pair_calls.get() + 1);
            (x.exp(), x.exp())
        },
        0.0,
        1.0,
        1e-10,
        20,
    )
    .expect("identical smooth components");
    let scalar_calls = Cell::new(0);
    let scalar_integrand = |x: f64| {
        scalar_calls.set(scalar_calls.get() + 1);
        x.exp()
    };
    let first = adaptive_simpson(scalar_integrand, 0.0, 1.0, 1e-10, 20).expect("first scalar");
    let second = adaptive_simpson(scalar_integrand, 0.0, 1.0, 1e-10, 20).expect("second scalar");
    assert_eq!(pair.0.to_bits(), first.to_bits());
    assert_eq!(pair.1.to_bits(), second.to_bits());
    assert_eq!(pair_calls.get() * 2, scalar_calls.get());
}
