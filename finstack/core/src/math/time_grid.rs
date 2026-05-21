//! Time grids for Monte Carlo simulation.
//!
//! Provides uniform and custom time grids with validation.
//!
//! # Time Convention
//!
//! Time grids operate on **year fractions** (f64), not calendar dates.
//! The MC engine is agnostic to day-count conventions.
//!
//! ## Design Philosophy
//!
//! - **MC Layer**: Pure mathematical time (this module)
//! - **Instrument Layer**: Converts dates → year fractions using `finstack_core::dates`
//!
//! ## Usage
//!
//! ```rust
//! # use finstack_core::Result;
//! # fn main() -> Result<()> {
//! use finstack_core::math::time_grid::TimeGrid;
//!
//! // Uniform grid: 1 year with 252 trading days
//! let grid = TimeGrid::uniform(1.0, 252)?;
//!
//! // Custom grid with irregular periods
//! let times = vec![0.0, 0.25, 0.5, 0.75, 1.0]; // Quarterly
//! let grid = TimeGrid::from_times(times)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Converting from Dates
//!
//! Use `finstack_core::dates` to convert calendar dates to year fractions:
//!
//! ```ignore
//! use finstack_core::dates::{DayCount, DayCountContext};
//! use finstack_core::math::time_grid::TimeGrid;
//! use time::macros::date;
//!
//! # fn main() -> finstack_core::Result<()> {
//! let start = date!(2024-01-15);
//! let end = date!(2025-01-15);
//!
//! // Apply day-count convention
//! let time = DayCount::Act365F.year_fraction(start, end, DayCountContext::default())?;
//!
//! // Create time grid
//! let grid = TimeGrid::uniform(time, 252)?;
//! # let _ = grid;
//! # Ok(())
//! # }
//! ```
//!
//! See the Monte Carlo conventions doc in `finstack-valuations` for detailed guidelines.

use crate::dates::{Date, DayCount, DayCountContext};
use crate::Result;
use thiserror::Error;

/// Time grid for Monte Carlo simulation.
///
/// Defines the discretization points in time from t=0 to t=T.
#[derive(Clone, Debug)]
pub struct TimeGrid {
    /// Time points in years (monotonically increasing)
    times: Vec<f64>,
    /// Time steps (`dt[i] = times[i+1] - times[i]`).
    dts: Vec<f64>,
    /// Maximum time (cached from times.last())
    t_max: f64,
}

/// Error type for time grid construction and validation
#[derive(Debug, Error)]
#[error("Invalid time grid: {0}")]
pub struct TimeGridError(String);

impl TimeGrid {
    /// Create a uniform time grid from 0 to T with N steps.
    ///
    /// # Arguments
    ///
    /// * `t_max` - Final time in years (must be > 0)
    /// * `num_steps` - Number of time steps (must be > 0)
    ///
    /// # Example
    ///
    /// ```ignore
    /// // 1 year with 252 trading days
    /// use finstack_core::math::time_grid::TimeGrid;
    /// # fn main() -> finstack_core::Result<()> {
    /// let grid = TimeGrid::uniform(1.0, 252)?;
    /// # let _ = grid;
    /// # Ok(())
    /// # }
    /// ```
    pub fn uniform(t_max: f64, num_steps: usize) -> Result<Self> {
        if t_max <= 0.0 {
            return Err(crate::error::InputError::Invalid.into());
        }
        if num_steps == 0 {
            return Err(crate::error::InputError::Invalid.into());
        }

        let dt = t_max / num_steps as f64;
        let mut times = Vec::with_capacity(num_steps + 1);
        let mut dts = Vec::with_capacity(num_steps);

        times.push(0.0);
        for i in 1..=num_steps {
            times.push(i as f64 * dt);
            dts.push(dt);
        }

        Ok(Self { t_max, times, dts })
    }

    /// Create a uniform base grid on `[0, t_max]` and merge in `required_times` exactly.
    ///
    /// Steps are chosen as `round(t_max * steps_per_year)`, floored to at least
    /// `min_steps`, matching [`Self::uniform`] spacing. Any finite `required_time` in
    /// `(0, t_max]` is inserted, the combined knot list is sorted and near-duplicates
    /// removed, then [`Self::from_times`] validates the result (so the final grid may
    /// be **non-uniform** if extra event times split intervals).
    ///
    /// # Arguments
    ///
    /// * `t_max` - Horizon in years (`> 0`).
    /// * `steps_per_year` - Target density for the underlying uniform spacing (`> 0`).
    /// * `min_steps` - Minimum number of uniform steps before merging events.
    /// * `required_times` - Extra knot times (e.g. barrier monitoring, cashflow dates).
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error`] if inputs are invalid or the merged grid fails
    /// [`Self::from_times`] validation.
    pub fn uniform_with_required_times(
        t_max: f64,
        steps_per_year: f64,
        min_steps: usize,
        required_times: &[f64],
    ) -> Result<Self> {
        if !steps_per_year.is_finite() || steps_per_year <= 0.0 {
            return Err(crate::error::InputError::Invalid.into());
        }

        let num_steps = ((t_max * steps_per_year).round() as usize).max(min_steps);
        let mut times = Vec::with_capacity(num_steps + required_times.len() + 1);
        times.push(0.0);

        let dt = t_max / num_steps as f64;
        for i in 1..=num_steps {
            times.push(i as f64 * dt);
        }

        for &required_time in required_times {
            if required_time.is_finite() && required_time > 1e-10 && required_time <= t_max {
                times.push(required_time);
            }
        }

        times.sort_by(|a, b| a.total_cmp(b));
        times.dedup_by(|a, b| (*a - *b).abs() < 1e-10);

        Self::from_times(times)
    }

    /// Create a custom time grid from explicit time points.
    ///
    /// # Arguments
    ///
    /// * `times` - Monotonically increasing time points (must start at 0)
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Custom grid with more steps near expiry
    /// use finstack_core::math::time_grid::TimeGrid;
    /// # fn main() -> finstack_core::Result<()> {
    /// let times = vec![0.0, 0.5, 0.75, 0.9, 1.0];
    /// let grid = TimeGrid::from_times(times)?;
    /// # let _ = grid;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_times(times: Vec<f64>) -> Result<Self> {
        if times.is_empty() {
            return Err(crate::error::InputError::Invalid.into());
        }
        if !times[0].is_finite() {
            return Err(crate::error::InputError::Invalid.into());
        }
        if times[0] != 0.0 {
            return Err(crate::error::InputError::Invalid.into());
        }

        // Validate monotonicity and check for duplicate/near-duplicate times
        const MIN_DT_THRESHOLD: f64 = 1e-12;
        for i in 1..times.len() {
            if !times[i].is_finite() {
                return Err(crate::error::InputError::Invalid.into());
            }
            if times[i] <= times[i - 1] {
                return Err(crate::error::InputError::NonMonotonicKnots.into());
            }
            // Check for duplicate or near-duplicate time points
            if (times[i] - times[i - 1]).abs() < MIN_DT_THRESHOLD {
                return Err(crate::error::InputError::Invalid.into());
            }
        }

        // Compute time steps
        let mut dts = Vec::with_capacity(times.len() - 1);
        for i in 0..times.len() - 1 {
            let dt = times[i + 1] - times[i];
            if !dt.is_finite() {
                return Err(crate::error::InputError::Invalid.into());
            }
            dts.push(dt);
        }

        // Check for minimum dt to prevent numerical issues
        const MIN_DT: f64 = 1e-10;
        if let Some(&min_dt) = dts.iter().min_by(|a, b| a.total_cmp(b)) {
            if min_dt < MIN_DT {
                return Err(crate::error::InputError::Invalid.into());
            }
        }

        // Store t_max from the last time point (guaranteed to exist after validation)
        let t_max = times.last().copied().unwrap_or(0.0);
        Ok(Self { times, dts, t_max })
    }

    /// Number of time steps.
    pub fn num_steps(&self) -> usize {
        self.dts.len()
    }

    /// Total time span.
    pub fn t_max(&self) -> f64 {
        self.t_max
    }

    /// Get time at step i.
    pub fn time(&self, step: usize) -> f64 {
        self.times[step]
    }

    /// Get time step size at step `i` (`dt[i] = t[i+1] - t[i]`).
    pub fn dt(&self, step: usize) -> f64 {
        self.dts[step]
    }

    /// Get all time points.
    pub fn times(&self) -> &[f64] {
        &self.times
    }

    /// Get all time steps.
    pub fn dts(&self) -> &[f64] {
        &self.dts
    }

    /// Check if grid is uniform (all dts equal within tolerance).
    pub fn is_uniform(&self) -> bool {
        if self.dts.is_empty() {
            return true;
        }
        let first_dt = self.dts[0];
        let tol = 1e-10;
        self.dts.iter().all(|&dt| (dt - first_dt).abs() < tol)
    }
}

/// Map Bermudan exercise dates (as year fractions relative to maturity) to step indices.
pub fn map_exercise_dates_to_steps(
    exercise_dates: &[f64],
    total_time: f64,
    steps: usize,
) -> Vec<usize> {
    let mut out = Vec::new();
    if total_time <= 0.0 || steps == 0 {
        return out;
    }
    for &ex_time in exercise_dates {
        let ratio = if total_time != 0.0 {
            ex_time / total_time
        } else {
            0.0
        };
        let step = (ratio * steps as f64).round() as usize;
        if step <= steps {
            out.push(step);
        }
    }
    out
}

/// Map a calendar date to a step index using a day-count convention.
pub fn map_date_to_step(
    base_date: Date,
    event_date: Date,
    maturity_date: Date,
    steps: usize,
    dc: DayCount,
) -> usize {
    let ttm = dc
        .year_fraction(base_date, maturity_date, DayCountContext::default())
        .unwrap_or(0.0);
    if ttm <= 0.0 || steps == 0 {
        return 0;
    }
    let t_event = dc
        .year_fraction(base_date, event_date, DayCountContext::default())
        .unwrap_or(0.0)
        .clamp(0.0, ttm);
    let step_index = ((t_event / ttm) * steps as f64).round() as usize;
    step_index.min(steps)
}

/// Map multiple calendar dates to step indices.
pub fn map_dates_to_steps(
    base_date: Date,
    dates: &[Date],
    maturity_date: Date,
    steps: usize,
    dc: DayCount,
) -> Vec<usize> {
    dates
        .iter()
        .map(|&d| map_date_to_step(base_date, d, maturity_date, steps, dc))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniform_grid() {
        let grid =
            TimeGrid::uniform(1.0, 100).expect("Uniform grid creation should succeed in test");
        assert_eq!(grid.num_steps(), 100);
        assert_eq!(grid.t_max(), 1.0);
        assert!(grid.is_uniform());
        assert_eq!(grid.dt(0), 0.01);
        assert_eq!(grid.time(0), 0.0);
        assert_eq!(grid.time(100), 1.0);
    }

    #[test]
    fn test_custom_grid() {
        let times = vec![0.0, 0.1, 0.5, 1.0];
        let grid = TimeGrid::from_times(times).expect("TimeGrid creation should succeed in test");
        assert_eq!(grid.num_steps(), 3);
        assert_eq!(grid.t_max(), 1.0);
        assert!(!grid.is_uniform());
        assert_eq!(grid.dt(0), 0.1);
        assert_eq!(grid.dt(1), 0.4);
        assert_eq!(grid.dt(2), 0.5);
    }

    #[test]
    fn test_invalid_grids() {
        // Zero t_max
        assert!(TimeGrid::uniform(0.0, 100).is_err());
        // Zero steps
        assert!(TimeGrid::uniform(1.0, 0).is_err());
        // Empty times
        assert!(TimeGrid::from_times(vec![]).is_err());
        // Doesn't start at 0
        assert!(TimeGrid::from_times(vec![0.1, 0.5, 1.0]).is_err());
        // Non-monotonic
        assert!(TimeGrid::from_times(vec![0.0, 0.5, 0.3, 1.0]).is_err());
    }

    #[test]
    fn test_uniform_with_required_times_merges_and_dedups_events() {
        let grid = TimeGrid::uniform_with_required_times(
            1.0,
            4.0,
            2,
            &[0.75, 0.5, 0.50000000001, 1.0, 0.0],
        )
        .expect("merged grid should succeed");

        assert_eq!(grid.times(), &[0.0, 0.25, 0.5, 0.75, 1.0]);
    }
}
