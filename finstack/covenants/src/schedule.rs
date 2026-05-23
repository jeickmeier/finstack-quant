use finstack_core::dates::Date;
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
}

/// Resolve threshold for a given test date from a piecewise-constant schedule.
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
