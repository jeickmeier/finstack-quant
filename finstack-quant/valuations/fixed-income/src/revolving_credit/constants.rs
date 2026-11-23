//! Numerical constants and model-clock conventions for revolving credit facilities.

// Numerical Constants
// Centralized thresholds for numerical stability and consistency across the module.

/// Tolerance for interpolation equality checks.
/// Used when comparing time points or interpolation boundaries.
pub const INTERPOLATION_TOLERANCE: f64 = 1e-10;

/// Minimum spread value for CIR process stability.
/// Ensures spreads don't go to exactly zero, which would cause numerical issues.
pub const MIN_CIR_SPREAD: f64 = 1e-8;

/// Maximum allowed recovery rate (inclusive).
pub const MAX_RECOVERY_RATE: f64 = 1.0;

/// Day count of the Monte Carlo model clock.
///
/// Factor paths (`ThreeFactorPathData::time_points`), the pathwise bank
/// account and pathwise survival integrate on ACT/365F years, the clock the
/// `finstack_quant_models` rate and credit processes are calibrated on. Interest
/// and fee accrual keep the facility's own `day_count`; only the simulation
/// time axis is fixed here, so an ACT/360 facility no longer over-accrues its
/// Hull-White numeraire by the 365/360 ratio.
pub const MC_CLOCK_DAY_COUNT: finstack_quant_core::dates::DayCount =
    finstack_quant_core::dates::DayCount::Act365F;
