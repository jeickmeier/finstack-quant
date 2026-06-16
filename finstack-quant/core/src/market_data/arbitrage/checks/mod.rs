//! Arbitrage check trait and individual check implementations.
//!
//! Each check is a stateless, composable unit that inspects a volatility
//! surface for a specific class of arbitrage violation. Checks return
//! `Vec<ArbitrageViolation>` (empty means pass) and never mutate input.

pub mod butterfly;
pub mod calendar_spread;
pub mod local_vol_density;
pub mod svi;

pub use butterfly::ButterflyCheck;
pub use calendar_spread::CalendarSpreadCheck;
pub use local_vol_density::LocalVolDensityCheck;
pub use svi::SviArbitrageCheck;

use super::types::{ArbitrageSeverity, ArbitrageViolation};
use crate::market_data::surfaces::VolSurface;

/// A composable arbitrage check that can be run against a volatility surface.
///
/// Implementations are pure functions: they inspect the surface and return
/// violations without mutating the input.
pub trait ArbitrageCheck: Send + Sync {
    /// Human-readable name of this check (e.g., "Butterfly", "Calendar Spread").
    fn name(&self) -> &str;

    /// Run this check against the given surface and return all violations found.
    ///
    /// An empty Vec means this check passes.
    fn check(&self, surface: &VolSurface) -> Vec<ArbitrageViolation>;
}

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
