//! Math module integration tests.
//!
//! This test suite verifies mathematical correctness for:
//! - Interpolation (linear, log-linear, cubic spline, etc.)
//! - Root-finding algorithms (Brent, Newton)
//! - Numerical integration (quadrature)
//! - Statistics (mean, variance, realized volatility)
//! - Compensated summation
//! - Correlation-matrix validation and nearest-correlation repair
//!
//! # Test Organization
//!
//! - `common`: Shared test helpers (approx_eq, standard test data)
//! - `interp`: Interpolation tests (all types, extrapolation, derivatives)
//! - `solver`: Root-finding tests (Brent, Newton)
//! - `correlation`: Correlation-matrix validation and repair tests
//! - `integration`: Numerical quadrature tests
//! - `stats`: Statistics tests (mean, variance, realized vol)
//! - `summation`: Compensated summation tests

#[path = "math/common.rs"]
mod common;

#[path = "math/correlation.rs"]
mod correlation;

#[path = "math/integration.rs"]
mod integration;

#[path = "math/interp.rs"]
mod interp;

#[path = "math/solver.rs"]
mod solver;

#[path = "math/stats.rs"]
mod stats;

#[path = "math/summation.rs"]
mod summation;
