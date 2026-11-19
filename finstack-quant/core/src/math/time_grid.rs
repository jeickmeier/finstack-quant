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
//! ```
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

use crate::math::interp::exact_f64_eq;

use crate::Result;
use crate::dates::{Date, DayCount, DayCountContext};

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

/// Validation error for time-grid construction, keeping the historical prefix.
fn invalid(message: impl Into<String>) -> crate::Error {
    crate::Error::Validation(format!("Invalid time grid: {}", message.into()))
}

/// Interior coordinate of the canonical uniform grid. The fraction is in [0, 1].
fn uniform_knot(i: usize, num_steps: usize, t_max: f64) -> f64 {
    t_max * (i as f64 / num_steps as f64)
}

impl TimeGrid {
    /// Create a uniform time grid from 0 to T with N steps.
    ///
    /// # Arguments
    ///
    /// * `t_max` - Finite nonnegative horizon in years.
    /// * `num_steps` - Base interval count, zero exactly when the horizon is zero.
    ///
    /// # Example
    ///
    /// ```
    /// // 1 year with 252 trading days
    /// use finstack_quant_core::math::time_grid::TimeGrid;
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let grid = TimeGrid::uniform(1.0, 252)?;
    /// # let _ = grid;
    /// # Ok(())
    /// # }
    /// ```
    pub fn uniform(t_max: f64, num_steps: usize) -> Result<Self> {
        if t_max == 0.0 && num_steps == 0 {
            return Self::from_times(vec![0.0]);
        }
        if !t_max.is_finite() || t_max <= 0.0 {
            return Err(invalid(format!(
                "t_max must be finite and positive, got {t_max}"
            )));
        }
        if num_steps as u64 >= 1_u64 << 53 {
            return Err(invalid(
                "uniform interval count exceeds exact binary64 index range",
            ));
        }
        if num_steps == 0 {
            return Err(invalid("num_steps must be greater than zero"));
        }

        let max_f64_capacity = isize::MAX as usize / std::mem::size_of::<f64>();
        let times_capacity = num_steps
            .checked_add(1)
            .filter(|&capacity| capacity <= max_f64_capacity)
            .ok_or_else(|| {
                invalid(format!(
                    "uniform grid step count {num_steps} exceeds vector capacity"
                ))
            })?;

        let mut times = Vec::new();
        times.try_reserve_exact(times_capacity).map_err(|error| {
            crate::Error::Internal(format!(
                "could not reserve {times_capacity} uniform time knots: {error}"
            ))
        })?;
        times.push(0.0);
        times.extend((1..num_steps).map(|i| uniform_knot(i, num_steps, t_max)));
        times.push(t_max);
        if times.windows(2).any(|w| w[0] >= w[1]) {
            return Err(crate::Error::Internal(format!(
                "cannot resolve {num_steps} distinct time steps on [0, {t_max}]"
            )));
        }
        Self::from_times(times)
    }

    /// Create a uniform base grid on `[0, t_max]` and merge in `required_times` exactly.
    ///
    /// Distinct represented event times are retained, including adjacent floats.
    ///
    /// # Arguments
    ///
    /// * `t_max` - Finite nonnegative horizon in years.
    /// * `num_steps` - Base interval count, zero exactly when the horizon is zero.
    /// * `required_times` - Contractual event coordinates in years on the closed interval `[0, t_max]`.
    pub fn uniform_with_required_times(
        t_max: f64,
        num_steps: usize,
        required_times: &[f64],
    ) -> Result<Self> {
        if required_times
            .iter()
            .any(|time| !(0.0..=t_max).contains(time))
        {
            return Err(invalid(
                "required event times must be finite and within the horizon",
            ));
        }
        let base = Self::uniform(t_max, num_steps)?;
        if required_times.is_empty() {
            return Ok(base);
        }
        let mut times = base.times;
        times
            .len()
            .checked_add(required_times.len())
            .filter(|&n| std::alloc::Layout::array::<f64>(n).is_ok())
            .ok_or_else(|| invalid("merged event grid exceeds vector capacity"))?;
        times
            .try_reserve_exact(required_times.len())
            .map_err(|error| {
                crate::Error::Internal(format!("could not reserve merged event grid: {error}"))
            })?;
        times.extend_from_slice(required_times);
        times.sort_unstable_by(f64::total_cmp);
        times.dedup();
        Self::from_times(times)
    }

    /// Find the exact index of an existing event coordinate.
    ///
    /// # Arguments
    ///
    /// * `time` - Model time in years, on the same clock as the grid knots.
    pub fn get_index(&self, time: f64) -> Result<usize> {
        if !(0.0..=self.t_max).contains(&time) {
            return Err(invalid(format!(
                "event time {time} outside [0, {}]",
                self.t_max
            )));
        }
        let time = if time == 0.0 { 0.0 } else { time };
        self.times
            .binary_search_by(|knot| knot.total_cmp(&time))
            .map_err(|_| invalid(format!("event time {time} is absent from the grid")))
    }

    /// Create a custom time grid from explicit time points.
    ///
    /// # Arguments
    ///
    /// * `times` - Monotonically increasing time points (must start at 0)
    ///
    /// # Example
    ///
    /// ```
    /// // Custom grid with more steps near expiry
    /// use finstack_quant_core::math::time_grid::TimeGrid;
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let times = vec![0.0, 0.5, 0.75, 0.9, 1.0];
    /// let grid = TimeGrid::from_times(times)?;
    /// # let _ = grid;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_times(mut times: Vec<f64>) -> Result<Self> {
        if times.is_empty() {
            return Err(invalid("times must not be empty"));
        }
        if !times[0].is_finite() {
            return Err(invalid("times[0] must be finite"));
        }
        if times[0] != 0.0 {
            return Err(invalid(format!(
                "times must start at 0.0, got {}",
                times[0]
            )));
        }

        times[0] = 0.0;
        for i in 1..times.len() {
            if !times[i].is_finite() {
                return Err(invalid(format!("times[{i}] must be finite")));
            }
            if times[i] <= times[i - 1] {
                return Err(crate::InputError::NonMonotonicKnots.into());
            }
        }

        let dts_capacity = times.len() - 1;
        if dts_capacity as u64 >= 1_u64 << 53 {
            return Err(invalid("interval count exceeds exact binary64 index range"));
        }
        let mut dts = Vec::new();
        dts.try_reserve_exact(dts_capacity).map_err(|error| {
            crate::Error::Internal(format!(
                "could not reserve {dts_capacity} custom time steps: {error}"
            ))
        })?;
        for i in 0..times.len() - 1 {
            let dt = times[i + 1] - times[i];
            if !dt.is_finite() {
                return Err(invalid(format!("time step {i} is not finite")));
            }
            dts.push(dt);
        }

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

    /// Whether the stored knots exactly match the canonical uniform grid.
    pub fn is_uniform(&self) -> bool {
        let count = self.num_steps();
        (1..count).all(|i| exact_f64_eq(self.times[i], uniform_knot(i, count, self.t_max)))
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
/// * `day_count` - Day-count convention used to convert dates to year fractions.
/// * `ctx` - Supplemental calendar or reference-period data required by `day_count`.
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
    day_count: DayCount,
    ctx: DayCountContext<'_>,
) -> crate::Result<usize> {
    let ttm = day_count.year_fraction(base_date, maturity_date, ctx)?;
    if ttm <= 0.0 || steps == 0 {
        return Ok(0);
    }
    let t_event = day_count
        .year_fraction(base_date, event_date, ctx)?
        .clamp(0.0, ttm);
    let step_index = ((t_event / ttm) * steps as f64).round() as usize;
    Ok(step_index.min(steps))
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
        let grid =
            TimeGrid::uniform_with_required_times(1.0, 4, &[0.75, 0.5, 0.50000000001, 1.0, 0.0])
                .expect("merged grid should succeed");

        assert_eq!(grid.times(), &[0.0, 0.25, 0.5, 0.50000000001, 0.75, 1.0]);
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
                matches!(result, Ok(Err(crate::Error::Validation(_)))),
                "num_steps={num_steps} must return an error without panicking"
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
    fn required_times_must_be_in_the_closed_horizon() {
        for bad in [-0.01, f64::NAN, f64::INFINITY, 1.0_f64.next_up()] {
            assert!(TimeGrid::uniform_with_required_times(1.0, 4, &[bad]).is_err());
        }
    }

    #[test]
    fn uniform_with_required_times_pins_terminal_knot_exactly() {
        // 11 * (0.1 / 11) is one ULP greater than 0.1 on IEEE-754 f64.
        let t_max = 0.1;
        let grid =
            TimeGrid::uniform_with_required_times(t_max, 11, &[]).expect("grid should succeed");

        assert_eq!(grid.t_max().to_bits(), t_max.to_bits());
        assert_eq!(
            grid.times().last().expect("terminal").to_bits(),
            t_max.to_bits()
        );
        assert!(grid.times().iter().all(|&time| time <= t_max));
    }

    #[test]
    fn uniform_and_event_grids_validate_the_base_before_merging() {
        for (horizon, n) in [
            (1.0, 0),
            (0.0, 1),
            (1.0, usize::MAX),
            (f64::from_bits(1), 3),
        ] {
            assert!(TimeGrid::uniform_with_required_times(horizon, n, &[0.0]).is_err());
        }
        let origin = TimeGrid::uniform_with_required_times(-0.0, 0, &[-0.0]).unwrap();
        assert_eq!(origin.times()[0].to_bits(), 0);
        assert_eq!(origin.get_index(-0.0).unwrap(), 0);
        assert!(origin.get_index(f64::NAN).is_err());
        let tiny = f64::from_bits(1);
        let grid = TimeGrid::uniform_with_required_times(1.0, 3, &[tiny, tiny.next_up()]).unwrap();
        assert_eq!(grid.get_index(tiny).unwrap(), 1);
        assert_eq!(grid.get_index(tiny.next_up()).unwrap(), 2);
        for (pair, &dt) in grid.times().windows(2).zip(grid.dts()) {
            assert_eq!(dt, pair[1] - pair[0]);
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
    fn map_date_to_step_propagates_missing_day_count_context() {
        let base = date!(2024 - 01 - 01);
        let maturity = date!(2025 - 01 - 01);

        assert!(
            map_date_to_step(
                base,
                date!(2024 - 07 - 01),
                maturity,
                12,
                DayCount::ActActIsma,
                DayCountContext::default(),
            )
            .is_err()
        );
        assert!(
            map_date_to_step(
                base,
                date!(2024 - 07 - 01),
                maturity,
                12,
                DayCount::Bus252,
                DayCountContext::default(),
            )
            .is_err()
        );
    }

    #[test]
    fn map_date_to_step_uses_provided_day_count_context() {
        use crate::dates::{Tenor, WEEKENDS_ONLY};

        let base = date!(2024 - 01 - 01);
        let event = date!(2024 - 07 - 01);
        let maturity = date!(2025 - 01 - 01);
        let isma = DayCountContext {
            frequency: Some(Tenor::annual()),
            coupon_period: Some((base, maturity)),
            ..Default::default()
        };
        assert_eq!(
            map_date_to_step(base, event, maturity, 12, DayCount::ActActIsma, isma)
                .expect("ISMA context"),
            6
        );

        let bus252 = DayCountContext {
            calendar: Some(&WEEKENDS_ONLY),
            ..Default::default()
        };
        let step = map_date_to_step(base, event, maturity, 12, DayCount::Bus252, bus252)
            .expect("Bus252 context");
        assert!((5..=7).contains(&step));
    }
}
