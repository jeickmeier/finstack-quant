//! Constants for structured credit instruments.
//!
//! This module contains the unit conversions, solver tolerances and
//! simulation thresholds shared across structured credit modeling.

use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry_or_panic;

/// Average days per year for structured credit day count calculations (ACT/365.25).
///
/// Re-exported from `finstack_quant_core::dates::AVERAGE_DAYS_PER_YEAR`.
/// Uses 365.25 to account for leap years over multi-year amortisation horizons.
pub use finstack_quant_core::dates::AVERAGE_DAYS_PER_YEAR;

/// Number of calendar months in one year (12).
pub const MONTHS_PER_YEAR: i32 = 12;

/// Number of quarterly payment periods in one year (4.0).
pub const QUARTERLY_PERIODS_PER_YEAR: f64 = 4.0;

/// Divisor converting basis points to a decimal rate (10,000.0).
pub const BASIS_POINTS_DIVISOR: f64 = 10_000.0;

/// Multiplier converting a decimal rate to a percentage (100.0).
pub const PERCENTAGE_MULTIPLIER: f64 = 100.0;

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

/// Lower bound for prepayment rates expressed as a decimal fraction.
pub const MIN_PREPAYMENT_RATE: f64 = 0.0;

/// AssetPool balance threshold (in base currency units) below which cashflow generation stops.
///
/// For example, for a USD-denominated pool, this means stop when balance < $100.
/// This prevents unnecessary computation for immaterial remaining balances.
pub fn pool_balance_cleanup_threshold() -> f64 {
    embedded_registry_or_panic().pool_balance_cleanup_threshold()
}
