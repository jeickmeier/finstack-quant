//! Default Monte Carlo discretization counts shared across equity MC pricers.

/// Default Monte Carlo path count when no instrument override is supplied.
pub(crate) const DEFAULT_MC_PATHS: usize = 100_000;

/// Default time-grid resolution (steps per year) for daily-discretised
/// path-dependent pricers.
pub(crate) const DEFAULT_STEPS_PER_YEAR: f64 = 252.0;

/// Default Monte Carlo step count for rough-volatility pricers
/// (rough Heston, rough Bergomi).
pub(crate) const DEFAULT_ROUGH_VOL_STEPS: usize = 100;
