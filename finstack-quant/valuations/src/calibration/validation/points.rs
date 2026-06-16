//! Shared validation grids (static points) to avoid repeated allocations.

/// Standard year-fraction points for checking discount curve monotonicity.
pub(crate) const DF_MONO_POINTS: &[f64] = &[
    0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0, 30.0, 40.0, 50.0,
];

/// Standard year-fraction points for checking discount curve no-arbitrage conditions.
///
/// Used for forward-rate sanity checks between adjacent tenors.
pub(crate) const DF_ARBI_POINTS: &[f64] = &[
    0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0, 30.0, 40.0, 50.0,
];

/// Standard year-fraction points for checking discount factor bounds.
pub(crate) const DF_BOUNDS_POINTS: &[f64] =
    &[0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 40.0, 50.0];

/// Standard year-fraction points for checking forward curve arbitrage.
pub(crate) const FWD_ARBI_POINTS: &[f64] = &[0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];

/// Standard year-fraction points for checking forward rate bounds.
pub(crate) const FWD_BOUNDS_POINTS: &[f64] = &[0.25, 0.5, 1.0, 2.0, 5.0, 10.0];

/// Standard year-fraction points for checking hazard rate arbitrage.
pub(crate) const HAZARD_ARBI_POINTS: &[f64] = &[0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];

/// Standard year-fraction points for checking survival probability monotonicity.
pub(crate) const HAZARD_MONO_POINTS: &[f64] = &[0.0, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];

/// Standard year-fraction points for checking survival probability bounds.
pub(crate) const HAZARD_BOUNDS_POINTS: &[f64] = &[0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];

/// Standard year-fraction points for checking inflation curve arbitrage.
pub(crate) const INFL_ARBI_POINTS: &[f64] = &[0.0, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0, 20.0];

/// Standard year-fraction points for checking inflation monotonicity.
pub(crate) const INFL_MONO_POINTS: &[f64] = &[1.0, 2.0, 3.0, 5.0, 10.0];

/// Standard year-fraction points for checking inflation expectation bounds.
pub(crate) const INFL_BOUNDS_POINTS: &[f64] = &[1.0, 2.0, 5.0, 10.0, 20.0, 30.0];
