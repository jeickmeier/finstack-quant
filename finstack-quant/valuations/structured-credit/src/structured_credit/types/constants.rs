//! Constants for structured credit instruments.
//!
//! This module contains the basis-point conversion and the solver tolerances
//! shared across structured credit modeling.

/// Divisor converting basis points to a decimal rate (10,000.0).
pub const BASIS_POINTS_DIVISOR: f64 = 10_000.0;

// These tolerances are calibrated to market conventions for structured credit.
// Reference: Bloomberg BVAL, Intex, and industry-standard pricing systems.

/// Solver tolerance for Z-spread calculations (decimal spread).
///
/// The price objective is relative to the dirty settlement target. A tight
/// decimal-spread tolerance preserves clean/dirty round trips across notionals.
pub const Z_SPREAD_SOLVER_TOLERANCE: f64 = 1e-12;

/// Solver tolerance for YTM calculations (decimal yield).
///
/// Market standard: 0.1 bp = 1e-6 in decimal terms.
/// YTM is quoted in bp to 1 decimal place.
pub const YTM_SOLVER_TOLERANCE: f64 = 1e-6;

/// Initial bracket size for Z-spread solver (decimal spread).
///
/// ±500 bp is sufficient for most structured credit instruments.
/// Extreme distressed securities may require wider brackets.
pub const Z_SPREAD_INITIAL_BRACKET: f64 = 0.05; // ±500 bp
