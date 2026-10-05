//! Individual arbitrage checks.
//!
//! Each check is a small config struct with a `check(&self, surface)` method
//! that inspects a volatility surface for a specific class of arbitrage
//! violation. Checks return `Vec<ArbitrageViolation>` (empty means pass) and
//! never mutate input.

pub mod butterfly;
pub mod calendar_spread;
pub mod local_vol_density;

pub use butterfly::ButterflyCheck;
pub use calendar_spread::CalendarSpreadCheck;
pub use local_vol_density::LocalVolDensityCheck;

use super::types::ArbitrageSeverity;

/// Classify violation magnitude into a severity bucket.
///
/// Thresholds are in total-variance units (sigma^2 * T).
pub(crate) fn classify_severity(
    magnitude: f64,
    minor_threshold: f64,
    major_threshold: f64,
    critical_threshold: f64,
) -> ArbitrageSeverity {
    if magnitude < minor_threshold {
        ArbitrageSeverity::Negligible
    } else if magnitude < major_threshold {
        ArbitrageSeverity::Minor
    } else if magnitude < critical_threshold {
        ArbitrageSeverity::Major
    } else {
        ArbitrageSeverity::Critical
    }
}
