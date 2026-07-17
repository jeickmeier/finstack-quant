//! Covenant threshold schedules and interpolation.
//!
//! [`ThresholdSchedule`] stores a piecewise-constant mapping from dates to
//! threshold values, sorted ascending. The effective threshold for a test
//! date is the last entry with date <= test date.

use finstack_quant_core::dates::Date;
use serde::{Deserialize, Serialize};

/// Piecewise-constant threshold schedule for covenants.
///
/// Entries are stored sorted by date ascending. The effective threshold for a
/// test date is the last entry with date <= test_date. If no entry applies,
/// `threshold_for_date` returns `None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThresholdSchedule(Vec<(Date, f64)>);

impl ThresholdSchedule {
    /// Create a new threshold schedule, sorting entries by date.
    pub fn new(mut entries: Vec<(Date, f64)>) -> Self {
        entries.sort_by_key(|(d, _)| *d);
        Self(entries)
    }

    /// Create a validated threshold schedule, sorting entries by date.
    ///
    /// A schedule is piecewise constant: each entry takes effect on its date
    /// and remains in force until a later entry. Use this constructor for an
    /// externally supplied schedule; unlike [`new`](Self::new), it rejects
    /// non-finite thresholds and duplicate effective dates.
    ///
    /// # Errors
    ///
    /// Returns a validation error when a threshold is `NaN` or infinite, or
    /// when two entries have the same effective date. The entries are sorted
    /// before validation, so input ordering does not affect the result.
    pub fn try_new(entries: Vec<(Date, f64)>) -> finstack_quant_core::Result<Self> {
        let schedule = Self::new(entries);
        schedule.validate()?;
        Ok(schedule)
    }

    /// Check if the threshold schedule is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Number of threshold entries.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Read-only access to the sorted schedule entries.
    pub fn entries(&self) -> &[(Date, f64)] {
        &self.0
    }

    /// Consume the schedule and return the sorted entries.
    pub fn into_inner(self) -> Vec<(Date, f64)> {
        self.0
    }

    pub(crate) fn validate(&self) -> finstack_quant_core::Result<()> {
        for (_, value) in &self.0 {
            if !value.is_finite() {
                return Err(finstack_quant_core::Error::Validation(
                    "threshold schedule values must be finite".to_string(),
                ));
            }
        }
        for pair in self.0.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "threshold schedule contains duplicate date {}",
                    pair[0].0
                )));
            }
        }
        Ok(())
    }
}

/// Resolve threshold for a given test date from a piecewise-constant schedule.
///
/// # Arguments
///
/// * `schedule` - Effective-date threshold schedule sorted in ascending date
///   order; an empty schedule returns `None`.
/// * `test_date` - Covenant test date for which the latest threshold effective
///   on or before that date is required.
pub fn threshold_for_date(schedule: &ThresholdSchedule, test_date: Date) -> Option<f64> {
    if schedule.0.is_empty() {
        return None;
    }
    debug_assert!(
        schedule.0.windows(2).all(|w| w[0].0 <= w[1].0),
        "ThresholdSchedule entries must be sorted by date ascending"
    );
    let mut last: Option<f64> = None;
    for (d, v) in &schedule.0 {
        if *d <= test_date {
            last = Some(*v);
        } else {
            break;
        }
    }
    last
}
