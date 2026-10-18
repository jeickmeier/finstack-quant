//! Unit conversion constants shared across the workspace.
//!
//! One definition of a basis point and of the decimal-to-percent factor, so
//! no crate carries its own `0.0001`.

/// One basis point as a decimal rate (`0.0001`).
pub const ONE_BASIS_POINT: f64 = 0.0001;

/// Basis points in one unit of decimal rate (`10_000`).
pub const BASIS_POINTS_PER_UNIT: f64 = 10_000.0;

/// Multiplier from a decimal fraction to a percentage (`100`).
pub const DECIMAL_TO_PERCENT: f64 = 100.0;
