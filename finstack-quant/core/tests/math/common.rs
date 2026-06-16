//! Shared test helpers for math module tests.

/// Approximate equality check for floating point values.
#[inline]
pub(crate) fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol.max(1e-15)
}

/// Standard test knots for interpolation tests.
pub(crate) fn standard_knots() -> Box<[f64]> {
    vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice()
}

/// Standard test discount factors for interpolation tests.
pub(crate) fn standard_dfs() -> Box<[f64]> {
    vec![1.0, 0.95, 0.9, 0.85].into_boxed_slice()
}

/// Two-point test knots for minimal interpolation tests.
pub(crate) fn two_point_knots() -> Box<[f64]> {
    vec![0.0, 1.0].into_boxed_slice()
}

/// Two-point test discount factors for minimal interpolation tests.
pub(crate) fn two_point_dfs() -> Box<[f64]> {
    vec![1.0, 0.95].into_boxed_slice()
}
