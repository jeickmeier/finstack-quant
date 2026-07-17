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
//! - **Instrument Layer**: Converts dates → year fractions using `finstack_quant_core::dates`
//!
//! ## Usage
//!
//! ```rust
//! # use finstack_quant_core::Result;
//! # fn main() -> Result<()> {
//! use finstack_quant_core::math::time_grid::TimeGrid;
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
//! Use `finstack_quant_core::dates` to convert calendar dates to year fractions:
//!
//! ```ignore
//! use finstack_quant_core::dates::{DayCount, DayCountContext};
//! use finstack_quant_core::math::time_grid::TimeGrid;
//! use time::macros::date;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
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
//! See the Monte Carlo conventions doc in `finstack-quant-valuations` for detailed guidelines.

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
#[derive(Debug, Clone, PartialEq, Error, serde::Serialize, serde::Deserialize)]
#[error("Invalid time grid: {0}")]
pub struct TimeGridError(String);

impl TimeGridError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

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
    /// use finstack_quant_core::math::time_grid::TimeGrid;
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let grid = TimeGrid::uniform(1.0, 252)?;
    /// # let _ = grid;
    /// # Ok(())
    /// # }
    /// ```
    pub fn uniform(t_max: f64, num_steps: usize) -> Result<Self> {
        if !t_max.is_finite() || t_max <= 0.0 {
            return Err(crate::error::InputError::Invalid.into());
        }
        if num_steps == 0 {
            return Err(crate::error::InputError::Invalid.into());
        }

        let max_f64_capacity = isize::MAX as usize / std::mem::size_of::<f64>();
        let times_capacity = num_steps
            .checked_add(1)
            .filter(|&capacity| capacity <= max_f64_capacity)
            .ok_or_else(|| {
                TimeGridError::new(format!(
                    "uniform grid step count {num_steps} exceeds vector capacity"
                ))
            })?;
        if num_steps > max_f64_capacity {
            return Err(TimeGridError::new(format!(
                "uniform grid step count {num_steps} exceeds vector capacity"
            ))
            .into());
        }

        let mut times = Vec::new();
        times.try_reserve_exact(times_capacity).map_err(|error| {
            TimeGridError::new(format!(
                "could not reserve {times_capacity} uniform time knots: {error}"
            ))
        })?;
        let mut dts = Vec::new();
        dts.try_reserve_exact(num_steps).map_err(|error| {
            TimeGridError::new(format!(
                "could not reserve {num_steps} uniform time steps: {error}"
            ))
        })?;

        let dt = t_max / num_steps as f64;
        times.push(0.0);
        for i in 1..num_steps {
            times.push(i as f64 * dt);
            dts.push(dt);
        }
        // Pin the final knot to t_max exactly: `num_steps * (t_max/num_steps)`
        // can differ from t_max by 1 ulp, which would make
        // `grid.time(num_steps) != grid.t_max()` and disagree with
        // `from_times` (which derives t_max from the last knot).
        let last_interior = times[num_steps - 1];
        dts.push(t_max - last_interior);
        times.push(t_max);

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
    /// * `min_steps` - Minimum number of uniform steps before merging events (`>= 1`).
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
        if !t_max.is_finite() || t_max <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "uniform_with_required_times requires finite t_max > 0, got {t_max}"
            )));
        }
        if !steps_per_year.is_finite() || steps_per_year <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "uniform_with_required_times requires finite steps_per_year > 0, got \
                 {steps_per_year}"
            )));
        }
        if min_steps == 0 {
            return Err(crate::Error::Validation(
                "uniform_with_required_times requires min_steps >= 1".to_string(),
            ));
        }

        let requested_steps = t_max * steps_per_year;
        if !requested_steps.is_finite() {
            return Err(crate::Error::Validation(
                "uniform_with_required_times step count overflowed".to_string(),
            ));
        }
        let rounded_steps = requested_steps.round();
        let max_f64_capacity = isize::MAX as usize / std::mem::size_of::<f64>();
        // `Vec` allocations are bounded by `isize::MAX` bytes; checking the
        // f64-element capacity before the float-to-usize cast also avoids
        // Rust's saturating float-cast behavior turning an oversized request
        // into `usize::MAX`.
        if rounded_steps >= max_f64_capacity as f64 {
            return Err(TimeGridError::new(format!(
                "uniform_with_required_times step count {rounded_steps} exceeds capacity"
            ))
            .into());
        }
        let num_steps = (rounded_steps as usize).max(min_steps);
        let capacity = num_steps
            .checked_add(required_times.len())
            .and_then(|value| value.checked_add(1))
            .filter(|&value| value <= max_f64_capacity)
            .ok_or_else(|| {
                TimeGridError::new("uniform_with_required_times merged grid capacity overflowed")
            })?;
        let mut times = Vec::new();
        times.try_reserve_exact(capacity).map_err(|error| {
            TimeGridError::new(format!(
                "uniform_with_required_times could not reserve {capacity} knots: {error}"
            ))
        })?;
        times.push(0.0);

        let dt = t_max / num_steps as f64;
        for i in 1..num_steps {
            times.push(i as f64 * dt);
        }
        times.push(t_max);

        for &required_time in required_times {
            // Accept times a float-noise tolerance past t_max (day-count year
            // fractions routinely land 1 ulp beyond) by snapping them to the
            // terminal knot instead of silently dropping the event.
            if required_time.is_finite() && required_time > 1e-10 {
                if required_time <= t_max {
                    times.push(required_time);
                } else if required_time - t_max < 1e-10 {
                    times.push(t_max);
                }
            }
        }

        times.sort_by(|a, b| a.total_cmp(b));
        times.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        if let Some(last) = times.last_mut() {
            *last = t_max;
        }

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
    /// use finstack_quant_core::math::time_grid::TimeGrid;
    /// # fn main() -> finstack_quant_core::Result<()> {
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
        let dts_capacity = times.len() - 1;
        let mut dts = Vec::new();
        dts.try_reserve_exact(dts_capacity).map_err(|error| {
            TimeGridError::new(format!(
                "could not reserve {dts_capacity} custom time steps: {error}"
            ))
        })?;
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
///
/// Tree/lattice pricers cannot insert knots, so each date is **rounded to the
/// nearest step** (up to half a step of displacement on coarse grids — use a
/// finer grid or an exact-knot [`TimeGrid`] when that bias matters).
///
/// The output is sorted and de-duplicated: two dates that round to the same
/// step yield one exercise opportunity, not a double-processed step. Dates up
/// to one float-noise tolerance beyond maturity are clamped to the terminal
/// step; dates materially beyond maturity are dropped (they cannot be
/// represented on the grid).
///
/// # Arguments
///
/// * `exercise_dates` - Exercise times in years from the valuation date.
/// * `total_time` - Maturity time in years represented by the lattice.
/// * `steps` - Number of uniform lattice intervals between zero and maturity.
pub fn map_exercise_dates_to_steps(
    exercise_dates: &[f64],
    total_time: f64,
    steps: usize,
) -> Vec<usize> {
    let mut out = Vec::new();
    if total_time <= 0.0 || steps == 0 {
        return out;
    }
    // Day-count year fractions can land a hair past maturity; treat anything
    // within half a step of the terminal as the terminal exercise rather than
    // silently dropping the right.
    let max_ratio = 1.0 + 0.5 / steps as f64;
    for &ex_time in exercise_dates {
        let ratio = ex_time / total_time;
        if !(0.0..=max_ratio).contains(&ratio) {
            continue;
        }
        let step = (ratio * steps as f64).round() as usize;
        out.push(step.min(steps));
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Map a calendar date to a step index using a day-count convention and context.
///
/// # Arguments
///
/// * `base_date` - Valuation date that defines time zero for the lattice.
/// * `event_date` - Calendar date to map to the nearest lattice step.
/// * `maturity_date` - Terminal lattice date that defines the full time span.
/// * `steps` - Number of uniform lattice intervals between base and maturity.
/// * `dc` - Day-count convention used to convert dates to year fractions.
/// * `ctx` - Supplemental calendar or reference-period data required by `dc`.
///
/// # Errors
///
/// Propagates day-count errors, including missing calendars for `Bus252` and
/// missing coupon frequencies for `ActActIsma`.
pub fn map_date_to_step(
    base_date: Date,
    event_date: Date,
    maturity_date: Date,
    steps: usize,
    dc: DayCount,
    ctx: DayCountContext<'_>,
) -> crate::Result<usize> {
    let ttm = dc.year_fraction(base_date, maturity_date, ctx)?;
    if ttm <= 0.0 || steps == 0 {
        return Ok(0);
    }
    let t_event = dc
        .year_fraction(base_date, event_date, ctx)?
        .clamp(0.0, ttm);
    let step_index = ((t_event / ttm) * steps as f64).round() as usize;
    Ok(step_index.min(steps))
}

/// Map multiple calendar dates to step indices.
///
/// # Arguments
///
/// * `base_date` - Valuation date that defines time zero for the lattice.
/// * `dates` - Calendar dates to map; output positions preserve this order.
/// * `maturity_date` - Terminal lattice date that defines the full time span.
/// * `steps` - Number of uniform lattice intervals between base and maturity.
/// * `dc` - Day-count convention used to convert dates to year fractions.
/// * `ctx` - Supplemental calendar or reference-period data required by `dc`.
///
/// # Errors
///
/// Returns the first day-count error encountered while mapping the dates.
pub fn map_dates_to_steps(
    base_date: Date,
    dates: &[Date],
    maturity_date: Date,
    steps: usize,
    dc: DayCount,
    ctx: DayCountContext<'_>,
) -> crate::Result<Vec<usize>> {
    dates
        .iter()
        .map(|&d| map_date_to_step(base_date, d, maturity_date, steps, dc, ctx))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

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

    #[test]
    fn test_uniform_final_knot_is_exactly_t_max() {
        // `n * (t_max/n)` can differ from t_max by 1 ulp; the final knot must
        // be pinned so `time(num_steps) == t_max()` for every constructor.
        for &(t_max, n) in &[(0.7_f64, 7usize), (1.0, 252), (2.5, 3), (0.123456, 11)] {
            let grid = TimeGrid::uniform(t_max, n).expect("grid");
            assert_eq!(
                grid.time(n).to_bits(),
                t_max.to_bits(),
                "t_max = {t_max}, n = {n}"
            );
        }
    }

    #[test]
    fn uniform_rejects_oversized_step_counts_without_allocating() {
        for num_steps in [usize::MAX, isize::MAX as usize] {
            let result = std::panic::catch_unwind(|| TimeGrid::uniform(1.0, num_steps));
            assert!(
                matches!(result, Ok(Err(crate::Error::TimeGrid(_)))),
                "num_steps={num_steps} must return TimeGridError without panicking"
            );
        }
    }

    #[test]
    fn test_uniform_rejects_non_finite_horizons() {
        for t_max in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(TimeGrid::uniform(t_max, 4).is_err());
        }
    }

    #[test]
    fn test_uniform_with_required_times_snaps_float_noise_past_t_max() {
        // A required time 1 ulp past maturity (typical day-count noise) must
        // snap to the terminal knot, not be silently dropped.
        let just_past = 1.0 + f64::EPSILON;
        let grid = TimeGrid::uniform_with_required_times(1.0, 4.0, 2, &[just_past])
            .expect("merged grid should succeed");
        assert_eq!(grid.t_max(), 1.0);
        assert_eq!(grid.times().last(), Some(&1.0));
    }

    #[test]
    fn uniform_with_required_times_pins_terminal_knot_exactly() {
        // 11 * (0.1 / 11) is one ULP greater than 0.1 on IEEE-754 f64.
        let t_max = 0.1;
        let grid = TimeGrid::uniform_with_required_times(t_max, 110.0, 1, &[])
            .expect("grid should succeed");

        assert_eq!(grid.t_max().to_bits(), t_max.to_bits());
        assert_eq!(
            grid.times().last().expect("terminal").to_bits(),
            t_max.to_bits()
        );
        assert!(grid.times().iter().all(|&time| time <= t_max));
    }

    #[test]
    fn uniform_with_required_times_rejects_zero_min_steps() {
        assert!(TimeGrid::uniform_with_required_times(1.0, 4.0, 0, &[]).is_err());
    }

    #[test]
    fn uniform_with_required_times_rejects_invalid_horizon_and_density() {
        for t_max in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let result = std::panic::catch_unwind(|| {
                TimeGrid::uniform_with_required_times(t_max, 4.0, 1, &[])
            });
            assert!(
                matches!(result, Ok(Err(_))),
                "invalid t_max={t_max} must return Err without panicking"
            );
        }
        for steps_per_year in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(TimeGrid::uniform_with_required_times(1.0, steps_per_year, 1, &[]).is_err());
        }
    }

    #[test]
    fn uniform_with_required_times_rejects_step_overflow_without_allocating() {
        for result in [
            std::panic::catch_unwind(|| {
                TimeGrid::uniform_with_required_times(f64::MAX, 2.0, 1, &[])
            }),
            std::panic::catch_unwind(|| {
                TimeGrid::uniform_with_required_times(1.0, 1.0, usize::MAX, &[])
            }),
        ] {
            assert!(
                matches!(result, Ok(Err(_))),
                "overflowing step count must return Err without allocation panic"
            );
        }
    }

    #[test]
    fn test_map_exercise_dates_sorted_deduped_and_clamped() {
        // Two dates rounding to the same step collapse to ONE exercise
        // opportunity; unsorted input comes out sorted; a date within half a
        // step past maturity clamps to the terminal step; a date materially
        // beyond maturity is dropped.
        let steps = map_exercise_dates_to_steps(&[0.74, 0.26, 0.24, 1.04, 2.0], 1.0, 4);
        assert_eq!(steps, vec![1, 3, 4]);
    }

    #[test]
    fn test_map_exercise_dates_rejects_degenerate_inputs() {
        assert!(map_exercise_dates_to_steps(&[0.5], 0.0, 4).is_empty());
        assert!(map_exercise_dates_to_steps(&[0.5], 1.0, 0).is_empty());
        assert!(map_exercise_dates_to_steps(&[-0.5, f64::NAN], 1.0, 4).is_empty());
    }

    #[test]
    fn test_map_date_to_step_clamps_and_handles_degenerate_grid() {
        let base = date!(2024 - 01 - 01);
        let mid = date!(2024 - 07 - 01);
        let maturity = date!(2025 - 01 - 01);
        let after_maturity = date!(2026 - 01 - 01);

        let ctx = DayCountContext::default();
        let mid_step =
            map_date_to_step(base, mid, maturity, 12, DayCount::Act365F, ctx).expect("map date");
        assert_eq!(mid_step, 6);
        assert_eq!(
            map_date_to_step(base, after_maturity, maturity, 12, DayCount::Act365F, ctx)
                .expect("map date"),
            12
        );
        assert_eq!(
            map_date_to_step(base, mid, base, 12, DayCount::Act365F, ctx).expect("map date"),
            0
        );
        assert_eq!(
            map_date_to_step(base, mid, maturity, 0, DayCount::Act365F, ctx).expect("map date"),
            0
        );
    }

    #[test]
    fn test_map_dates_to_steps_preserves_input_order() {
        let base = date!(2024 - 01 - 01);
        let maturity = date!(2025 - 01 - 01);
        let steps = map_dates_to_steps(
            base,
            &[date!(2025 - 01 - 01), date!(2024 - 01 - 01)],
            maturity,
            4,
            DayCount::Act365F,
            DayCountContext::default(),
        )
        .expect("map dates");

        assert_eq!(steps, vec![4, 0]);
    }

    #[test]
    fn map_date_to_step_propagates_missing_day_count_context() {
        let base = date!(2024 - 01 - 01);
        let maturity = date!(2025 - 01 - 01);

        assert!(map_date_to_step(
            base,
            date!(2024 - 07 - 01),
            maturity,
            12,
            DayCount::ActActIsma,
            DayCountContext::default(),
        )
        .is_err());
        assert!(map_date_to_step(
            base,
            date!(2024 - 07 - 01),
            maturity,
            12,
            DayCount::Bus252,
            DayCountContext::default(),
        )
        .is_err());
    }

    #[test]
    fn map_date_to_step_uses_provided_day_count_context() {
        use crate::dates::{Calendar, Tenor};

        let base = date!(2024 - 01 - 01);
        let event = date!(2024 - 07 - 01);
        let maturity = date!(2025 - 01 - 01);
        let isma = DayCountContext {
            frequency: Some(Tenor::semi_annual()),
            ..Default::default()
        };
        assert_eq!(
            map_date_to_step(base, event, maturity, 12, DayCount::ActActIsma, isma)
                .expect("ISMA context"),
            6
        );

        let calendar = Calendar::new("test", "Test", false, &[]);
        let bus252 = DayCountContext {
            calendar: Some(&calendar),
            ..Default::default()
        };
        let step = map_date_to_step(base, event, maturity, 12, DayCount::Bus252, bus252)
            .expect("Bus252 context");
        assert!((5..=7).contains(&step));
    }
}
