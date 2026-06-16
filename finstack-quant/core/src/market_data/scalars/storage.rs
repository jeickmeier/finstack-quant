//! Lightweight columnar storage for date-indexed time series.
//!
//! Provides a minimal, WASM-compatible replacement for Polars DataFrame
//! storage in ScalarTimeSeries. Stores dates as i32 (days since epoch) and
//! values as f64 in parallel sorted arrays for efficient binary search lookup.

use crate::{error::InputError, Result};

/// Lightweight columnar storage for date-indexed series.
///
/// Stores dates and values in parallel sorted arrays. Dates are stored as
/// i32 (days since Unix epoch) for compact representation and fast comparison.
#[derive(Clone, Debug)]
pub(super) struct TimeSeriesStorage {
    /// Dates as days since epoch (1970-01-01), sorted ascending, no duplicates
    dates: Box<[i32]>,
    /// Values aligned with dates
    values: Box<[f64]>,
}

impl TimeSeriesStorage {
    /// Create from observations, sorting and validating uniqueness.
    ///
    /// # Parameters
    /// - `observations`: unsorted list of (days_since_epoch, value) pairs
    ///
    /// # Errors
    /// - `InputError::TooFewPoints` if empty
    /// - `InputError::NonMonotonicKnots` if duplicate dates exist
    pub(super) fn new(mut observations: Vec<(i32, f64)>) -> Result<Self> {
        if observations.is_empty() {
            return Err(crate::Error::Input(InputError::TooFewPoints));
        }

        // Sort by date ascending
        observations.sort_by_key(|(date, _)| *date);

        // Check for duplicates
        for window in observations.windows(2) {
            if window[0].0 == window[1].0 {
                return Err(crate::Error::Input(InputError::NonMonotonicKnots));
            }
        }

        // Split into parallel arrays
        let (dates, values): (Vec<_>, Vec<_>) = observations.into_iter().unzip();

        Ok(Self {
            dates: dates.into_boxed_slice(),
            values: values.into_boxed_slice(),
        })
    }

    /// Binary search lookup for a specific day.
    ///
    /// Returns Ok(index) if found, Err(insertion_point) if not found.
    #[inline]
    #[cfg(test)]
    pub(super) fn binary_search(&self, query_day: i32) -> core::result::Result<usize, usize> {
        self.dates.binary_search(&query_day)
    }

    /// Get date at index.
    #[inline]
    #[cfg(test)]
    pub(super) fn date(&self, idx: usize) -> i32 {
        self.dates[idx]
    }

    /// Get value at index.
    #[inline]
    #[cfg(test)]
    pub(super) fn value(&self, idx: usize) -> f64 {
        self.values[idx]
    }

    /// Length of series.
    #[inline]
    pub(super) fn len(&self) -> usize {
        self.dates.len()
    }

    /// Check if empty.
    #[inline]
    pub(super) fn is_empty(&self) -> bool {
        self.dates.is_empty()
    }

    /// Date range (min, max) as days since epoch.
    #[inline]
    #[cfg(test)]
    pub(super) fn date_range(&self) -> (i32, i32) {
        if self.dates.is_empty() {
            (0, 0)
        } else {
            (self.dates[0], self.dates[self.dates.len() - 1])
        }
    }

    /// Get all dates as a slice.
    #[inline]
    pub(super) fn dates(&self) -> &[i32] {
        &self.dates
    }

    /// Get all values as a slice.
    #[inline]
    pub(super) fn values(&self) -> &[f64] {
        &self.values
    }

    /// Iterator over (date, value) pairs.
    pub(super) fn iter(&self) -> impl Iterator<Item = (i32, f64)> + '_ {
        self.dates
            .iter()
            .zip(self.values.iter())
            .map(|(&d, &v)| (d, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_creation_and_sorting() {
        // Create unsorted observations
        let obs = vec![(2, 20.0), (0, 10.0), (1, 15.0), (3, 25.0)];
        let storage =
            TimeSeriesStorage::new(obs).expect("TimeSeriesStorage creation should succeed in test");

        // Should be sorted
        assert_eq!(storage.len(), 4);
        assert_eq!(storage.date(0), 0);
        assert_eq!(storage.date(1), 1);
        assert_eq!(storage.date(2), 2);
        assert_eq!(storage.date(3), 3);
        assert_eq!(storage.value(0), 10.0);
        assert_eq!(storage.value(1), 15.0);
        assert_eq!(storage.value(2), 20.0);
        assert_eq!(storage.value(3), 25.0);
    }

    #[test]
    fn test_storage_duplicate_detection() {
        // Duplicate dates should error
        let obs = vec![(0, 10.0), (1, 15.0), (1, 20.0)];
        let result = TimeSeriesStorage::new(obs);
        assert!(result.is_err());
    }

    #[test]
    fn test_storage_binary_search() {
        let obs = vec![(0, 10.0), (2, 15.0), (4, 20.0)];
        let storage =
            TimeSeriesStorage::new(obs).expect("TimeSeriesStorage creation should succeed in test");

        // Exact match
        assert_eq!(storage.binary_search(2), Ok(1));
        assert_eq!(storage.binary_search(0), Ok(0));
        assert_eq!(storage.binary_search(4), Ok(2));

        // Not found - insertion point
        assert_eq!(storage.binary_search(1), Err(1)); // Would insert at index 1
        assert_eq!(storage.binary_search(3), Err(2)); // Would insert at index 2
        assert_eq!(storage.binary_search(5), Err(3)); // Would insert at index 3
    }

    #[test]
    fn test_storage_date_range() {
        let obs = vec![(5, 50.0), (1, 10.0), (10, 100.0)];
        let storage =
            TimeSeriesStorage::new(obs).expect("TimeSeriesStorage creation should succeed in test");

        let (min, max) = storage.date_range();
        assert_eq!(min, 1);
        assert_eq!(max, 10);
    }

    #[test]
    fn test_storage_iteration() {
        let obs = vec![(2, 20.0), (0, 10.0), (1, 15.0)];
        let storage =
            TimeSeriesStorage::new(obs).expect("TimeSeriesStorage creation should succeed in test");

        let collected: Vec<_> = storage.iter().collect();
        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0], (0, 10.0));
        assert_eq!(collected[1], (1, 15.0));
        assert_eq!(collected[2], (2, 20.0));
    }
}
