use super::adaptive::*;
use super::gauss_hermite::*;
use super::gauss_legendre::*;
use crate::{Error, InputError};

#[test]
fn test_gauss_legendre_basic() {
    // x² on [0,1] = 1/3
    let f = |x: f64| x * x;

    // Test different orders
    let result2 =
        gauss_legendre_integrate_composite(|x| Ok(f(x)), 0.0, 1.0, 2, 1).expect("supported order");
    let result4 =
        gauss_legendre_integrate_composite(|x| Ok(f(x)), 0.0, 1.0, 4, 1).expect("supported order");
    let result8 =
        gauss_legendre_integrate_composite(|x| Ok(f(x)), 0.0, 1.0, 8, 1).expect("supported order");

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
    assert!(gauss_legendre_integrate_composite(|x| Ok(f(x)), 0.0, f64::INFINITY, 4, 1).is_err());
    assert!(
        gauss_legendre_integrate_composite(|x| Ok(f(x)), f64::NEG_INFINITY, 0.0, 4, 1).is_err()
    );

    // NaN bounds
    assert!(gauss_legendre_integrate_composite(|x| Ok(f(x)), 0.0, f64::NAN, 4, 1).is_err());
}
#[test]
fn test_gauss_legendre_equal_bounds() {
    let f = |x: f64| x * x;

    // Equal bounds should give 0
    let result =
        gauss_legendre_integrate_composite(|x| Ok(f(x)), 5.0, 5.0, 4, 1).expect("supported order");
    assert!(result.abs() < 1e-12);
}
#[test]
fn test_gauss_legendre_invalid_order() {
    let f = |x: f64| x;

    // Unsupported order
    assert!(gauss_legendre_integrate_composite(|x| Ok(f(x)), 0.0, 1.0, 3, 1).is_err());
    assert!(gauss_legendre_integrate_composite(|x| Ok(f(x)), 0.0, 1.0, 7, 1).is_err());
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
    let result = gauss_legendre_integrate_composite(|_| Ok(1.0), a, b, 4, 1).expect("valid");
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
    let result = gauss_legendre_integrate_composite(|_| Ok(1.0), a, b, 4, 1).expect("valid");
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

#[test]
fn test_gauss_hermite_new_returns_result() {
    // Valid orders should succeed
    assert!(GaussHermiteQuadrature::new(5).is_ok());
    assert!(GaussHermiteQuadrature::new(7).is_ok());
    assert!(GaussHermiteQuadrature::new(10).is_ok());
    assert!(GaussHermiteQuadrature::new(15).is_ok());
    assert!(GaussHermiteQuadrature::new(20).is_ok());

    // Invalid orders should return Err with helpful message
    let err = GaussHermiteQuadrature::new(3);
    assert!(err.is_err());
    let msg = err
        .expect_err("Expected error for unsupported order")
        .to_string();
    assert!(msg.contains("3"), "Error should mention the invalid order");
    assert!(
        msg.contains("Supported orders") || msg.contains("5, 7, 10, 15, 20"),
        "Error should list supported orders"
    );

    assert!(GaussHermiteQuadrature::new(42).is_err());
    assert!(GaussHermiteQuadrature::new(100).is_err());
}

#[test]
fn gauss_legendre_adaptive_shares_unused_budget_only_within_total_tolerance() {
    // The right leaf misses its half-budget (3.25e-6), but the summed
    // terminal error estimate (about 6.29e-6) fits the global budget.
    let tolerance = 6.5e-6;
    let value =
        gauss_legendre_integrate_adaptive(|x| Ok(x.sin()), 0.0, 1.0, 2, tolerance, 1, 100_000)
            .expect("summed leaf error is within total tolerance");
    let exact = 1.0 - 1.0_f64.cos();

    assert!((value - exact).abs() <= tolerance);
    assert!(
        gauss_legendre_integrate_adaptive(|x| Ok(x.sin()), 0.0, 1.0, 2, 4.7e-6, 1, 100_000)
            .is_err()
    );
}

#[test]
fn gauss_legendre_adaptive_enforces_global_polynomial_error_budget() {
    let tolerance = 2e-10;
    let result =
        gauss_legendre_integrate_adaptive(|x| Ok(x.powi(4)), 0.0, 1.0, 2, tolerance, 5, 100_000);
    match result {
        Err(Error::Input(InputError::SolverConvergenceFailed { residual, .. })) => {
            assert!(residual > tolerance);
        }
        other => panic!("expected an error for the combined leaf budget, got {other:?}"),
    }

    for (a, b, exact) in [(0.0, 1.0, 0.2), (1.0, 0.0, -0.2)] {
        let value =
            gauss_legendre_integrate_adaptive(|x| Ok(x.powi(4)), a, b, 2, tolerance, 8, 100_000)
                .expect("sufficient refinement meets the global budget");
        assert!((value - exact).abs() <= tolerance);
    }
}

#[test]
fn gauss_legendre_adaptive_rejects_invalid_budget_and_non_finite_values() {
    for tolerance in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(gauss_legendre_integrate_adaptive(Ok, 0.0, 1.0, 2, tolerance, 5, 100_000).is_err());
    }
    assert!(
        gauss_legendre_integrate_adaptive(|_| Ok(f64::NAN), 0.0, 1.0, 2, 1e-8, 5, 100_000).is_err()
    );
    assert!(gauss_legendre_integrate_adaptive(Ok, 0.0, 0.0, 3, 1e-8, 5, 100_000).is_err());
}

#[test]
fn adaptive_quadrature_fails_closed_on_midpoint_collapse_and_handles_extreme_bounds() {
    // Non-polynomial cusp with tol = 0.0 and large max_depth hits ULP midpoint
    // collapse before `error_estimate <= 0.0`; must fail as a geometric
    // computation error, well inside the evaluation limit.
    let cusp = |x: f64| (x - 0.3).abs().sqrt();
    let scalar = adaptive_simpson(|x| Ok(cusp(x)), 0.0, 1.0, 0.0, 200, 1_000_000)
        .expect_err("midpoint collapse must fail");
    assert!(
        scalar.to_string().contains("cannot be bisected"),
        "{scalar}"
    );
    let pair = adaptive_simpson_pair(|x| Ok((cusp(x), x)), 0.0, 1.0, [0.0, 0.0], 200, 1_000_000)
        .expect_err("midpoint collapse must fail");
    assert!(pair.to_string().contains("cannot be bisected"), "{pair}");
    assert!(
        gauss_legendre_integrate_adaptive(|x| Ok(cusp(x)), 0.0, 1.0, 4, 0.0, 200, 1_000_000)
            .is_err()
    );

    // Same-sign large bounds where `(a + b) / 2.0` would overflow `f64::MAX`
    let a = 1.0e308_f64;
    let b = 1.0e308_f64 + 1.0e295_f64;
    let res =
        adaptive_simpson(|_| Ok(1.0), a, b, 1.0e285, 10, 100).expect("overflow-safe midpoint");
    assert!((res - (b - a)).abs() <= 1.0e285);
}
