//! Swaption metrics configuration constants.
//! Centralizes scaling parameters to avoid magic numbers in calculators.

/// Vega scale for 1% volatility change.
pub(crate) const VOL_PCT_SCALE: f64 = 100.0;
