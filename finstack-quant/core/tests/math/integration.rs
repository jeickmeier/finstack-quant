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

/// Evaluation limit generous enough never to bind in these tests unless a test says so.
const MAX_EVALS: usize = 1_000_000;

#[test]
fn test_simpson_rule_polynomial() {
    // Test Simpson's rule on x² over [0, 1] = 1/3
    let f = |x: f64| x * x;
    let result = simpson_rule(|x| Ok(f(x)), 0.0, 1.0, 100).unwrap();
    assert!((result - 1.0 / 3.0).abs() < 1e-6);
}

#[test]
fn test_simpson_rule_sin() {
    // Test Simpson's rule on sin(x) over [0, π] = 2
    let f = |x: f64| x.sin();
    let result = simpson_rule(|x| Ok(f(x)), 0.0, PI, 100).unwrap();
    // Simpson with n=100 on sin(x): error ~ π/180 * (π/100)^4 ≈ 1.7e-9
    assert!((result - 2.0).abs() < 1e-7);
}

#[test]
fn test_adaptive_simpson_smooth_function() {
    // Test adaptive Simpson on e^(-x²) over [-2, 2]
    // For finite limits: ∫e^(-x²)dx from -a to a = √π · erf(a)
    // For a=2: √π · erf(2) ≈ 1.7641 (not √π ≈ 1.7725 which is for infinite limits)
    let f = |x: f64| (-x * x).exp();
    let result = adaptive_simpson(|x| Ok(f(x)), -2.0, 2.0, 1e-4, 20, MAX_EVALS).unwrap();
    let expected = PI.sqrt() * erf(2.0).expect("valid variate");
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
    let result = adaptive_simpson(|x| Ok(f(x)), 0.0, PI, 1e-4, 25, MAX_EVALS).unwrap();
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
    let result = quad.integrate(|x| Ok(f(x))).expect("finite integral");

    // Result should be positive (call option value component)
    assert!(result > 0.0);

    // Validate against analytical Black-Scholes (undiscounted, zero rate)
    // BS call = S·N(d1) - K·N(d2) where r=q=0
    let d1 = ((spot / strike).ln() + 0.5 * vol * vol * time) / (vol * time.sqrt());
    let d2 = d1 - vol * time.sqrt();
    let bs_price = spot * norm_cdf(d1).expect("valid normal variate")
        - strike * norm_cdf(d2).expect("valid normal variate");

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

    let simpson = simpson_rule(|x| Ok(f(x)), 0.0, 2.0, 100).unwrap();
    let adaptive = adaptive_simpson(|x| Ok(f(x)), 0.0, 2.0, 1e-6, 20, MAX_EVALS).unwrap();

    // Simpson should be most accurate for polynomials
    assert!((simpson - exact).abs() < 1e-8);
    // Adaptive should be very accurate
    assert!((adaptive - exact).abs() < 1e-10);
}

#[test]
fn test_integration_error_cases() {
    let f = |x: f64| x;

    // Simpson with odd number of intervals should fail
    assert!(simpson_rule(|x| Ok(f(x)), 0.0, 1.0, 99).is_err());

    // Zero intervals should fail
    assert!(simpson_rule(|x| Ok(f(x)), 0.0, 1.0, 0).is_err());
}

#[test]
fn test_financial_yield_curve_integration() {
    // Test integration relevant to yield curve construction
    // Integrate forward rate over time to get zero rate
    let f = |t: f64| 0.03 + 0.01 * t; // Simple linear forward rate

    // Integrate from 0 to 5 years
    let integrated_rate = simpson_rule(|x| Ok(f(x)), 0.0, 5.0, 1000).unwrap() / 5.0_f64;

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
            let actual = quad
                .integrate(|x| Ok(x.powi(power)))
                .expect("finite integral");
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

    let result = adaptive_simpson(|x| Ok(f(x)), 0.0, 5.0, 1e-6, 20, MAX_EVALS).unwrap();
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
    let result = adaptive_simpson(|x| Ok(f(x)), 0.0, 2.0, 0.1, 5, MAX_EVALS).unwrap();
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
    let result = gauss_legendre_integrate_adaptive(|x| Ok(f(x)), 0.0, 5.0, 2, 0.0, 0, MAX_EVALS);

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
    let result = adaptive_simpson(|x| Ok(f(x)), 0.0, 1.0, 0.0, 0, MAX_EVALS);

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
    let result = adaptive_simpson(|x| Ok(f(x)), 0.0, 1.0, 1e-8, 50, MAX_EVALS).unwrap();
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
    let result1 = adaptive_simpson(|x| Ok(f(x)), 0.0, 1.0, 1e-2, 20, MAX_EVALS).unwrap();
    let result2 = adaptive_simpson(|x| Ok(f(x)), 0.0, 1.0, 1e-6, 20, MAX_EVALS).unwrap();
    let result3 = adaptive_simpson(|x| Ok(f(x)), 0.0, 1.0, 1e-10, 20, MAX_EVALS).unwrap();

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
    let result = simpson_rule(|x| Ok(f(x)), 5.0, 5.0, 100).unwrap();
    assert!(result.abs() < 1e-12);

    // Negative interval (b < a)
    let result = simpson_rule(|x| Ok(f(x)), 5.0, 0.0, 100).unwrap();
    assert!((result + 12.5).abs() < 1e-3); // Should be negative of [0,5]
}

#[test]
fn test_gauss_legendre_composite() {
    // x³ on [0,2] = 4
    let f = |x: f64| x * x * x;

    // Single panel
    let result1 = gauss_legendre_integrate_composite(|x| Ok(f(x)), 0.0, 2.0, 4, 1).unwrap();

    // Multiple panels
    let result10 = gauss_legendre_integrate_composite(|x| Ok(f(x)), 0.0, 2.0, 4, 10).unwrap();

    let exact = 4.0;

    // Both should be accurate
    assert!((result1 - exact).abs() < 1e-10);
    assert!((result10 - exact).abs() < 1e-10);
}

#[test]
fn test_gauss_legendre_composite_zero_panels() {
    let f = |x: f64| x;

    assert!(gauss_legendre_integrate_composite(|x| Ok(f(x)), 0.0, 1.0, 4, 0).is_err());
}

#[test]
fn test_gauss_legendre_adaptive() {
    // Smooth function
    let f = |x: f64| x.exp();

    let result =
        gauss_legendre_integrate_adaptive(|x| Ok(f(x)), 0.0, 1.0, 4, 1e-8, 20, MAX_EVALS).unwrap();

    let exact = 1.0_f64.exp() - 1.0;
    assert!((result - exact).abs() < 1e-6);
}

#[test]
fn test_gauss_legendre_adaptive_oscillatory() {
    // Oscillatory function
    let f = |x: f64| (10.0 * x).sin();

    let result =
        gauss_legendre_integrate_adaptive(|x| Ok(f(x)), 0.0, PI, 8, 1e-6, 25, MAX_EVALS).unwrap();

    let exact = (1.0 - (10.0 * PI).cos()) / 10.0;
    assert!((result - exact).abs() < 1e-3);
}

#[test]
fn test_gauss_legendre_adaptive_max_depth() {
    let f = |x: f64| x * x;

    // Even with max_depth=0, should still return result
    let result =
        gauss_legendre_integrate_adaptive(|x| Ok(f(x)), 0.0, 1.0, 4, 1e-10, 0, MAX_EVALS).unwrap();

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

    let result = adaptive_simpson(|x| Ok(f(x)), -PI, PI, 1e-4, 20, MAX_EVALS).unwrap();
    // Sinc function integral is well-known
    assert!(result.is_finite());
    assert!(result > 0.0);
}

#[test]
fn test_convergence_behavior() {
    let f = |x: f64| x.exp();
    let exact = 1.0_f64.exp() - 1.0;

    // Test convergence as we increase intervals
    let result10 = simpson_rule(|x| Ok(f(x)), 0.0, 1.0, 10).unwrap();
    let result100 = simpson_rule(|x| Ok(f(x)), 0.0, 1.0, 100).unwrap();
    let result1000 = simpson_rule(|x| Ok(f(x)), 0.0, 1.0, 1000).unwrap();

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
    let result = quad
        .integrate(|x| Ok(x * x - 1.0))
        .expect("finite integral");
    assert!(
        result.abs() < 1e-10,
        "E[X^2-1] should be 0; compensated sum got {result}"
    );
}

#[test]
fn production_credit_audit_simpson_uses_total_error_budget() {
    let boundary = 0.123_456_7;
    let integral = adaptive_simpson(
        |x| Ok(if x >= boundary { 1.0 } else { 0.0 }),
        0.0,
        1.0,
        1e-6,
        20,
        MAX_EVALS,
    )
    .expect("localized discontinuity fits the global error budget");
    assert!((integral - (1.0 - boundary)).abs() <= 1e-6);
    assert!(
        adaptive_simpson(
            |x| Ok(if x >= boundary { 1.0 } else { 0.0 }),
            0.0,
            1.0,
            1e-12,
            2,
            MAX_EVALS,
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
                Ok(if swapped {
                    (small, large)
                } else {
                    (large, small)
                })
            },
            0.0,
            1.0,
            [1e-12, 1e-12],
            24,
            MAX_EVALS,
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
fn adaptive_simpson_pair_gates_each_budget_in_its_own_units() {
    use finstack_quant_core::{Error, InputError};

    // Component one is exact for Simpson; component two is exp(x). A budget
    // that is generous for the first and tight for the second must be met by
    // the second on its own, and a spare first budget must not rescue it.
    let pair = |x: f64| Ok((1.0e6 * x, x.exp()));
    let (first, second) = adaptive_simpson_pair(pair, 0.0, 1.0, [1.0, 1e-12], 24, MAX_EVALS)
        .expect("each component meets its own budget");
    assert!((first - 5.0e5).abs() < 1e-6);
    assert!((second - (1.0_f64.exp() - 1.0)).abs() < 2e-12);

    let Err(Error::Input(InputError::SolverConvergenceFailed { reason, .. })) =
        adaptive_simpson_pair(pair, 0.0, 1.0, [1.0e9, 0.0], 0, MAX_EVALS)
    else {
        panic!("a spare first budget must not rescue the second component");
    };
    assert!(reason.contains("component 2"), "{reason}");
}

#[test]
fn adaptive_simpson_pair_refines_distinct_localized_components() {
    // Both peaks have analytic arctangent integrals and need refinement in
    // different parts of the domain. The second also has a smaller scale.
    let (first, second) = adaptive_simpson_pair(
        |x| {
            Ok((
                1.0 / (1.0 + (40.0 * (x - 0.2)).powi(2)),
                1e-3 / (1.0 + (80.0 * (x - 0.8)).powi(2)),
            ))
        },
        0.0,
        1.0,
        [1e-11, 1e-11],
        24,
        MAX_EVALS,
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
    let f = |x: f64| Ok((if x >= boundary { 1.0 } else { 0.0 }, x * x * x));
    let (step, cube) = adaptive_simpson_pair(f, 0.0, 1.0, [1e-6, 1e-6], 20, MAX_EVALS)
        .expect("a depth-limited leaf can fit the global budget");
    assert!((step - (1.0 - boundary)).abs() <= 1e-6);
    assert!((cube - 0.25).abs() <= 1e-12);
    assert!(matches!(
        adaptive_simpson_pair(f, 0.0, 1.0, [1e-12, 1e-12], 2, MAX_EVALS),
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
                Ok(if failing_component == 1 {
                    (x.powi(5), 1.0)
                } else {
                    (1.0, x.powi(5))
                })
            },
            0.0,
            1.0,
            [0.0, 0.0],
            0,
            MAX_EVALS,
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
        // Actual work: the five samples of the root comparison, the last of
        // them at the right quarter point. Never a configured limit.
        assert_eq!(iterations, 5);
        assert!(residual.is_finite() && residual > 0.0);
        assert_eq!(last_x, 0.75);
        assert!(reason.contains(&format!("component {failing_component}")));
    }

    let both = adaptive_simpson_pair(
        |x| Ok((x.powi(5), x.powi(6))),
        0.0,
        1.0,
        [0.0, 0.0],
        0,
        MAX_EVALS,
    );
    let Err(Error::Input(InputError::SolverConvergenceFailed { reason, .. })) = both else {
        panic!("both unconverged components must fail: {both:?}");
    };
    assert!(reason.contains("component 1"));
}

#[test]
fn adaptive_simpson_pair_rejects_non_finite_components_and_arithmetic() {
    use finstack_quant_core::{Error, InputError};

    for (values, component) in [
        ((f64::NAN, 1.0), "component 1"),
        ((1.0, f64::NAN), "component 2"),
        ((f64::INFINITY, 1.0), "component 1"),
        ((1.0, f64::NEG_INFINITY), "component 2"),
        // Both invalid at one sample: the first in tuple order is named.
        ((f64::NAN, f64::NAN), "component 1"),
    ] {
        let Err(Error::Input(InputError::SolverConvergenceFailed {
            iterations,
            last_x,
            reason,
            ..
        })) = adaptive_simpson_pair(|_| Ok(values), 0.0, 1.0, [1e-8, 1e-8], 20, MAX_EVALS)
        else {
            panic!("non-finite sample must fail: {values:?}");
        };
        assert_eq!(iterations, 1, "the first sample already fails");
        assert_eq!(last_x, 0.0);
        assert!(reason.contains(component), "{reason}");
    }

    // A representable integral of very large samples is not lost to `4 * f`.
    let (unit, large) = adaptive_simpson_pair(
        |_| Ok((1.0, f64::MAX / 2.0)),
        0.0,
        1.0,
        [1e-8, 0.0],
        20,
        MAX_EVALS,
    )
    .expect("the weighted average of representable samples is representable");
    assert_eq!(unit, 1.0);
    assert!((large / (f64::MAX / 2.0) - 1.0).abs() < 1e-15);

    // An integral that is not representable fails instead of returning infinity.
    assert!(matches!(
        adaptive_simpson_pair(
            |_| Ok((1.0, f64::MAX)),
            0.0,
            2.0,
            [1e-8, 0.0],
            20,
            MAX_EVALS
        ),
        Err(Error::Input(InputError::SolverConvergenceFailed { .. }))
    ));
}

#[test]
fn adaptive_simpson_pair_preserves_interval_and_budget_validation() {
    use finstack_quant_core::Error;

    for (a, b, tol) in [
        (f64::NAN, 1.0, [1e-8, 1e-8]),
        (0.0, f64::INFINITY, [1e-8, 1e-8]),
        (0.0, 1.0, [-1e-8, 1e-8]),
        (0.0, 1.0, [1e-8, f64::NAN]),
        (0.0, 1.0, [f64::INFINITY, 1e-8]),
    ] {
        assert!(matches!(
            adaptive_simpson_pair(|x| Ok((x, x * x)), a, b, tol, 20, MAX_EVALS),
            Err(Error::Validation(_))
        ));
    }
    assert!(matches!(
        adaptive_simpson_pair(|x| Ok((x, x * x)), 0.0, 1.0, [1e-8, 1e-8], 20, 0),
        Err(Error::Validation(_))
    ));

    let reversed =
        adaptive_simpson_pair(|x| Ok((1.0, x * x * x)), 1.0, 0.0, [0.0, 0.0], 0, MAX_EVALS)
            .expect("exact signed polynomial integrals");
    assert_eq!(reversed, (-1.0, -0.25));
}

#[test]
fn adaptive_simpson_zero_width_validates_controls_and_makes_no_callback() {
    use finstack_quant_core::Error;
    use std::cell::Cell;

    let calls = Cell::new(0_usize);
    let counted = |x: f64| {
        calls.set(calls.get() + 1);
        Ok(x)
    };
    let zero = adaptive_simpson(counted, 2.0, 2.0, 0.0, 0, 1).expect("zero-width identity");
    assert_eq!(zero.to_bits(), 0.0_f64.to_bits());
    let zero_pair = adaptive_simpson_pair(|x| Ok((counted(x)?, x)), 2.0, 2.0, [0.0, 0.0], 0, 1)
        .expect("zero-width pair identity");
    assert_eq!(zero_pair, (0.0, 0.0));
    let fixed = simpson_rule(counted, 5.0, 5.0, 2).expect("zero-width fixed rule");
    assert_eq!(fixed.to_bits(), 0.0_f64.to_bits());
    assert_eq!(calls.get(), 0, "a zero-width interval makes no callback");

    // Controls are admitted before the zero-width identity.
    assert!(matches!(
        adaptive_simpson(counted, 2.0, 2.0, -1.0, 0, 1),
        Err(Error::Validation(_))
    ));
    assert!(matches!(
        adaptive_simpson(counted, 2.0, 2.0, 0.0, 0, 0),
        Err(Error::Validation(_))
    ));
    assert!(matches!(
        simpson_rule(counted, 5.0, 5.0, 3),
        Err(Error::Validation(_))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn adaptive_simpson_reversed_bounds_negate_the_same_samples() {
    use std::cell::RefCell;

    let forward_nodes = RefCell::new(Vec::new());
    let forward = adaptive_simpson(
        |x| {
            forward_nodes.borrow_mut().push(x);
            Ok(x.exp())
        },
        0.0,
        1.0,
        1e-10,
        20,
        MAX_EVALS,
    )
    .expect("forward");
    let reversed_nodes = RefCell::new(Vec::new());
    let reversed = adaptive_simpson(
        |x| {
            reversed_nodes.borrow_mut().push(x);
            Ok(x.exp())
        },
        1.0,
        0.0,
        1e-10,
        20,
        MAX_EVALS,
    )
    .expect("reversed");
    assert_eq!(reversed.to_bits(), (-forward).to_bits());
    assert_eq!(*forward_nodes.borrow(), *reversed_nodes.borrow());

    let fixed_forward = simpson_rule(|x| Ok(x.exp()), 0.0, 1.0, 10).expect("fixed forward");
    let fixed_reversed = simpson_rule(|x| Ok(x.exp()), 1.0, 0.0, 10).expect("fixed reversed");
    assert_eq!(fixed_reversed.to_bits(), (-fixed_forward).to_bits());
}

#[test]
fn adaptive_simpson_stops_at_the_first_callback_error_and_keeps_its_category() {
    use finstack_quant_core::Error;
    use std::cell::Cell;

    let calls = Cell::new(0_usize);
    let failing = |x: f64| {
        calls.set(calls.get() + 1);
        if calls.get() == 4 {
            Err(Error::Validation(format!("caller rejected {x}")))
        } else {
            Ok(x.exp())
        }
    };
    let Err(Error::Validation(message)) = adaptive_simpson(failing, 0.0, 1.0, 0.0, 20, MAX_EVALS)
    else {
        panic!("the callback's own error category must survive");
    };
    assert!(message.contains("caller rejected"));
    assert_eq!(calls.get(), 4, "no callback after the first error");

    calls.set(0);
    assert!(matches!(
        simpson_rule(failing, 0.0, 1.0, 10),
        Err(Error::Validation(_))
    ));
    assert_eq!(calls.get(), 4);
}

#[test]
fn adaptive_simpson_depth_zero_returns_the_refined_value_and_proxy() {
    use finstack_quant_core::{Error, InputError};

    // x^4 on [0, 1]: S1 = 5/24, S2 = 77/384, estimator |S2 - S1| / 15 = 1/1920.
    let quartic = |x: f64| Ok(x.powi(4));
    let value = adaptive_simpson(quartic, 0.0, 1.0, 1.0 / 1920.0, 0, 5)
        .expect("the proxy equals the budget, and equality passes");
    assert!((value - 77.0 / 384.0).abs() < 1e-15);

    let Err(Error::Input(InputError::SolverConvergenceFailed { residual, .. })) =
        adaptive_simpson(quartic, 0.0, 1.0, 1.0 / 1921.0, 0, 5)
    else {
        panic!("a budget just below the proxy must fail at depth zero");
    };
    assert!((residual - 1.0 / 1920.0).abs() < 1e-15);
}

#[test]
fn adaptive_simpson_enforces_the_evaluation_limit_exactly() {
    use finstack_quant_core::{Error, InputError};
    use std::cell::Cell;

    let calls = Cell::new(0_usize);
    let counted = |x: f64| {
        calls.set(calls.get() + 1);
        Ok(x.exp())
    };

    // Four evaluations cannot produce the root comparison: no callback at all.
    assert!(matches!(
        adaptive_simpson(counted, 0.0, 1.0, 1.0, 0, 4),
        Err(Error::Input(InputError::SolverConvergenceFailed {
            iterations: 0,
            ..
        }))
    ));
    assert_eq!(calls.get(), 0);

    // Five run the root comparison.
    adaptive_simpson(counted, 0.0, 1.0, 1.0, 0, 5).expect("root comparison fits five samples");
    assert_eq!(calls.get(), 5);

    // A limit reached mid-run fails with the actual count and no partial value.
    for limit in [5, 6, 7, 20, 101] {
        calls.set(0);
        let result = adaptive_simpson(counted, 0.0, 1.0, 0.0, 40, limit);
        let Err(Error::Input(InputError::SolverConvergenceFailed {
            iterations, reason, ..
        })) = result
        else {
            panic!("zero tolerance cannot be met within {limit} evaluations: {result:?}");
        };
        assert!(calls.get() <= limit, "never exceed the limit");
        assert_eq!(iterations, calls.get(), "report actual work");
        assert!(reason.contains("evaluation limit"), "{reason}");
    }

    // One paired call counts once.
    calls.set(0);
    adaptive_simpson_pair(|x| Ok((counted(x)?, x)), 0.0, 1.0, [1.0, 1.0], 0, 5)
        .expect("a pair root comparison also fits five calls");
    assert_eq!(calls.get(), 5);
}

#[test]
fn adaptive_simpson_splits_a_budget_without_losing_the_remainder() {
    // With eta the smallest positive double, tol = 3 eta splits into
    // left = 2 eta (3 eta / 2 rounds to even) and right = eta: the shares sum
    // to the original budget. The run must stay well defined at that scale.
    let eta = f64::from_bits(1);
    let tol = 3.0 * eta;
    assert_eq!(tol / 2.0, 2.0 * eta);
    assert_eq!(tol - tol / 2.0, eta);
    let value = adaptive_simpson(|x| Ok(x * x * x), 0.0, 1.0, tol, 10, MAX_EVALS)
        .expect("cubics are exact, so any nonnegative budget is met");
    assert_eq!(value, 0.25);
}

#[test]
fn adaptive_simpson_pair_shares_callback_work_and_preserves_node_order() {
    use std::cell::{Cell, RefCell};

    let calls = RefCell::new(Vec::new());
    let exact = adaptive_simpson_pair(
        |x| {
            calls.borrow_mut().push(x);
            Ok((x * x, x * x * x))
        },
        0.0,
        1.0,
        [1e-12, 1e-12],
        0,
        MAX_EVALS,
    )
    .expect("exact polynomial pair");
    assert_eq!(*calls.borrow(), [0.0, 1.0, 0.5, 0.25, 0.75]);
    assert!((exact.0 - 1.0 / 3.0).abs() < 1e-12);
    assert!((exact.1 - 0.25).abs() < 1e-12);

    // Left panel before right: after the root's five samples, the left half
    // is refined completely before the right half is touched.
    let order = RefCell::new(Vec::new());
    adaptive_simpson(
        |x| {
            order.borrow_mut().push(x);
            Ok(x.powi(5))
        },
        0.0,
        1.0,
        0.0,
        1,
        MAX_EVALS,
    )
    .expect_err("zero tolerance is not met at depth one");
    assert_eq!(
        *order.borrow(),
        [0.0, 1.0, 0.5, 0.25, 0.75, 0.125, 0.375, 0.625, 0.875]
    );

    let pair_calls = Cell::new(0);
    let pair = adaptive_simpson_pair(
        |x| {
            pair_calls.set(pair_calls.get() + 1);
            Ok((x.exp(), x.exp()))
        },
        0.0,
        1.0,
        [1e-10, 1e-10],
        20,
        MAX_EVALS,
    )
    .expect("identical smooth components");
    let scalar_calls = Cell::new(0);
    let scalar_integrand = |x: f64| {
        scalar_calls.set(scalar_calls.get() + 1);
        Ok(x.exp())
    };
    let first =
        adaptive_simpson(scalar_integrand, 0.0, 1.0, 1e-10, 20, MAX_EVALS).expect("first scalar");
    let second =
        adaptive_simpson(scalar_integrand, 0.0, 1.0, 1e-10, 20, MAX_EVALS).expect("second scalar");
    assert_eq!(pair.0.to_bits(), first.to_bits());
    assert_eq!(pair.1.to_bits(), second.to_bits());
    assert_eq!(pair_calls.get() * 2, scalar_calls.get());
}

#[test]
fn gauss_legendre_adaptive_counts_work_and_reuses_child_estimates() {
    use finstack_quant_core::{Error, InputError};
    use std::cell::Cell;

    for order in [2_usize, 4, 8, 16] {
        let calls = Cell::new(0_usize);
        let counted = |x: f64| {
            calls.set(calls.get() + 1);
            Ok(x.exp())
        };

        // One evaluation short of the root comparison: no callback at all.
        assert!(matches!(
            gauss_legendre_integrate_adaptive(counted, 0.0, 1.0, order, 1.0, 0, 3 * order - 1),
            Err(Error::Input(InputError::SolverConvergenceFailed {
                iterations: 0,
                ..
            }))
        ));
        assert_eq!(calls.get(), 0);

        // Exactly 3 * order runs the root comparison.
        gauss_legendre_integrate_adaptive(counted, 0.0, 1.0, order, 1.0, 0, 3 * order)
            .expect("root comparison");
        assert_eq!(calls.get(), 3 * order);

        // One bisection of the root costs 2 * order per child and nothing for
        // the children's one-panel values, which the parent already computed.
        // A cusp keeps the root estimate nonzero at every order.
        calls.set(0);
        let cusp = |x: f64| {
            calls.set(calls.get() + 1);
            Ok((x - 0.3).abs().sqrt())
        };
        let _ = gauss_legendre_integrate_adaptive(cusp, 0.0, 1.0, order, 0.0, 1, MAX_EVALS);
        assert_eq!(calls.get(), 3 * order + 2 * (2 * order));

        // A limit reached mid-run reports the actual count and never exceeds it.
        calls.set(0);
        let limit = 3 * order + 1;
        let Err(Error::Input(InputError::SolverConvergenceFailed {
            iterations, reason, ..
        })) = gauss_legendre_integrate_adaptive(cusp, 0.0, 1.0, order, 0.0, 30, limit)
        else {
            panic!("zero tolerance cannot be met within {limit} evaluations");
        };
        assert!(calls.get() <= limit);
        assert_eq!(iterations, calls.get());
        assert!(reason.contains("evaluation limit"), "{reason}");
    }
}

#[test]
fn gauss_legendre_rules_share_orientation_and_zero_width_semantics() {
    use finstack_quant_core::Error;
    use std::cell::Cell;

    let calls = Cell::new(0_usize);
    let counted = |x: f64| {
        calls.set(calls.get() + 1);
        Ok(x.exp())
    };
    let adaptive = gauss_legendre_integrate_adaptive(counted, 2.0, 2.0, 8, 0.0, 0, 1)
        .expect("zero-width adaptive");
    let composite =
        gauss_legendre_integrate_composite(counted, 2.0, 2.0, 8, 3).expect("zero-width composite");
    assert_eq!(adaptive.to_bits(), 0.0_f64.to_bits());
    assert_eq!(composite.to_bits(), 0.0_f64.to_bits());
    assert_eq!(calls.get(), 0, "a zero-width interval makes no callback");

    // Order and counts stay explicit even on a zero interval.
    assert!(matches!(
        gauss_legendre_integrate_adaptive(counted, 2.0, 2.0, 3, 0.0, 0, 1),
        Err(Error::Validation(_))
    ));
    assert!(matches!(
        gauss_legendre_integrate_composite(counted, 2.0, 2.0, 8, 0),
        Err(Error::Validation(_))
    ));
    assert_eq!(calls.get(), 0);

    let forward =
        gauss_legendre_integrate_adaptive(counted, 0.0, 1.0, 8, 1e-12, 20, MAX_EVALS).expect("fwd");
    let reversed =
        gauss_legendre_integrate_adaptive(counted, 1.0, 0.0, 8, 1e-12, 20, MAX_EVALS).expect("rev");
    assert_eq!(reversed.to_bits(), (-forward).to_bits());
    let forward = gauss_legendre_integrate_composite(counted, 0.0, 1.0, 8, 3).expect("fwd");
    let reversed = gauss_legendre_integrate_composite(counted, 1.0, 0.0, 8, 3).expect("rev");
    assert_eq!(reversed.to_bits(), (-forward).to_bits());
    assert!((forward - (1.0_f64.exp() - 1.0)).abs() < 1e-14);
}

#[test]
fn fixed_rules_stop_at_the_first_callback_error_and_reject_non_finite_samples() {
    use finstack_quant_core::{Error, InputError};
    use std::cell::Cell;

    let calls = Cell::new(0_usize);
    let failing = |x: f64| {
        calls.set(calls.get() + 1);
        if calls.get() == 3 {
            Err(Error::Validation(format!("caller rejected {x}")))
        } else {
            Ok(x)
        }
    };
    let quad = GaussHermiteQuadrature::new(10).unwrap();
    assert!(matches!(quad.integrate(failing), Err(Error::Validation(_))));
    assert_eq!(calls.get(), 3);
    calls.set(0);
    assert!(matches!(
        gauss_legendre_integrate_composite(failing, 0.0, 1.0, 4, 2),
        Err(Error::Validation(_))
    ));
    assert_eq!(calls.get(), 3);
    calls.set(0);
    assert!(matches!(
        gauss_legendre_integrate_adaptive(failing, 0.0, 1.0, 4, 1e-8, 5, MAX_EVALS),
        Err(Error::Validation(_))
    ));
    assert_eq!(calls.get(), 3);

    for bad in [f64::NAN, f64::INFINITY] {
        assert!(matches!(
            quad.integrate(|_| Ok(bad)),
            Err(Error::Input(InputError::SolverConvergenceFailed {
                iterations: 1,
                ..
            }))
        ));
        assert!(matches!(
            gauss_legendre_integrate_composite(|_| Ok(bad), 0.0, 1.0, 4, 2),
            Err(Error::Input(InputError::SolverConvergenceFailed {
                iterations: 1,
                ..
            }))
        ));
    }
}

#[test]
fn gauss_hermite_returns_normal_moments_and_does_not_overflow_a_representable_mean() {
    for order in [5_usize, 7, 10, 15, 20] {
        let quad = GaussHermiteQuadrature::new(order).unwrap();
        for (power, expected) in [(0, 1.0), (1, 0.0), (2, 1.0), (4, 3.0)] {
            let actual = quad.integrate(|z| Ok(z.powi(power))).unwrap();
            assert!(
                (actual - expected).abs() < 1e-12,
                "order {order}, E[Z^{power}] = {actual}"
            );
        }
        // The unnormalized weights sum to sqrt(pi) > 1, so a constant above
        // f64::MAX / sqrt(pi) would overflow that sum although its expectation
        // is representable.
        let constant = 0.75 * f64::MAX;
        let large = quad.integrate(|_| Ok(constant)).unwrap();
        assert!((large / constant - 1.0).abs() < 1e-14);
    }
}

#[test]
fn fixed_rules_reject_invalid_samples_and_stop_at_the_first_callback_error() {
    let hermite = GaussHermiteQuadrature::new(7).unwrap();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(hermite.integrate(|_| Ok(bad)).is_err());
        assert!(simpson_rule(|_| Ok(bad), 0.0, 1.0, 4).is_err());
        assert!(gauss_legendre_integrate_composite(|_| Ok(bad), 0.0, 1.0, 4, 2).is_err());
        assert!(simpson_rule(|_| panic!("invalid bounds must fail first"), bad, 1.0, 4).is_err());
    }
    let marker = finstack_quant_core::Error::Validation("callback sentinel".into());
    let mut calls = 0;
    let mut fail = |_| {
        calls += 1;
        Err(marker.clone())
    };
    assert_eq!(hermite.integrate(&mut fail), Err(marker.clone()));
    assert_eq!(simpson_rule(&mut fail, 5.0, 2.0, 4), Err(marker.clone()));
    assert_eq!(
        gauss_legendre_integrate_composite(&mut fail, 5.0, 2.0, 4, 2),
        Err(marker)
    );
    assert_eq!(calls, 3);
    assert_eq!(simpson_rule(|_| unreachable!(), 1.0, 1.0, 4).unwrap(), 0.0);
    assert_eq!(
        gauss_legendre_integrate_composite(|_| unreachable!(), 1.0, 1.0, 4, 2).unwrap(),
        0.0
    );
}

#[test]
fn fixed_simpson_keeps_representable_integrals_when_a_weight_is_out_of_range() {
    let extreme = f64::MAX;
    let v = simpson_rule(|_| Ok(0.25), -extreme, extreme, 2).unwrap();
    assert!((v / (extreme * 0.5) - 1.0).abs() < 4.0 * f64::EPSILON);
    let step = f64::from_bits(1);
    let subnormal = simpson_rule(|_| Ok(3.0), 0.0, 2.0 * step, 2).unwrap();
    assert_eq!(subnormal.to_bits(), 6);
}
