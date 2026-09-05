//! Covenant threshold schedules (piecewise-constant step-downs).
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
/// [`ThresholdSchedule::threshold_for`] returns `None`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThresholdSchedule(Vec<(Date, f64)>);

impl<'de> Deserialize<'de> for ThresholdSchedule {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let entries = Vec::<(Date, f64)>::deserialize(deserializer)?;
        Self::new(entries).map_err(serde::de::Error::custom)
    }
}

impl ThresholdSchedule {
    /// Create a validated threshold schedule, sorting entries by date.
    ///
    /// # Arguments
    ///
    /// * `entries` - Effective dates and finite threshold values. Dates may be
    ///   supplied in any order but must be unique.
    ///
    /// # Errors
    ///
    /// Returns a validation error when a threshold is `NaN` or infinite, or
    /// when two entries have the same effective date.
    pub fn new(mut entries: Vec<(Date, f64)>) -> finstack_quant_core::Result<Self> {
        entries.sort_by_key(|(d, _)| *d);
        for (_, value) in &entries {
            if !value.is_finite() {
                return Err(finstack_quant_core::Error::Validation(
                    "threshold schedule values must be finite".to_string(),
                ));
            }
        }
        for pair in entries.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "threshold schedule contains duplicate date {}",
                    pair[0].0
                )));
            }
        }
        Ok(Self(entries))
    }

    /// Effective-date / threshold entries in ascending date order.
    pub fn entries(&self) -> &[(Date, f64)] {
        &self.0
    }

    /// Threshold in force on `test_date`: the last entry whose effective date
    /// is on or before it, or `None` when no entry has taken effect yet.
    ///
    /// # Arguments
    ///
    /// * `test_date` - Covenant test date for which the latest threshold
    ///   effective on or before that date is required.
    pub fn threshold_for(&self, test_date: Date) -> Option<f64> {
        debug_assert!(
            self.0.windows(2).all(|w| w[0].0 <= w[1].0),
            "ThresholdSchedule entries must be sorted by date ascending"
        );
        let mut last = None;
        for (d, v) in &self.0 {
            if *d <= test_date {
                last = Some(*v);
            } else {
                break;
            }
        }
        last
    }
}
