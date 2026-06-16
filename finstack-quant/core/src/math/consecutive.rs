//! Consecutive streak counter for return series analysis.
//!
//! Used by the aggregation module to compute the longest win and loss streaks
//! from period-level return sequences.

/// Count maximum consecutive runs where `predicate` holds.
///
/// Scans `values` left-to-right and returns the length of the longest
/// unbroken streak of elements for which `predicate` returns `true`.
/// Typical usage: longest run of positive returns (wins) or negative
/// returns (losses) in a period-aggregated return series.
///
/// # Arguments
///
/// * `values` - Slice of return values to scan.
/// * `predicate` - Closure returning `true` for elements that count as a "hit".
///
/// # Returns
///
/// Length of the longest consecutive streak. Returns `0` if `values` is
/// empty or no element satisfies the predicate.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::consecutive::count_consecutive;
///
/// // One losing period followed by three wins → longest streak is 3.
/// let returns = [-0.01, 0.02, 0.03, 0.04];
/// assert_eq!(count_consecutive(&returns, |v| v > 0.0), 3);
///
/// // Empty slice always returns 0.
/// assert_eq!(count_consecutive(&[], |v: f64| v > 0.0), 0);
/// ```
pub fn count_consecutive<F: Fn(f64) -> bool>(values: &[f64], predicate: F) -> usize {
    let mut max_streak = 0usize;
    let mut current = 0usize;
    for &v in values {
        if predicate(v) {
            current += 1;
            if current > max_streak {
                max_streak = current;
            }
        } else {
            current = 0;
        }
    }
    max_streak
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert_eq!(count_consecutive(&[], |v| v > 0.0), 0);
    }

    #[test]
    fn all_positive() {
        let data = [0.01, 0.02, 0.03];
        assert_eq!(count_consecutive(&data, |v| v > 0.0), 3);
    }

    #[test]
    fn streak_at_end() {
        let data = [-0.01, 0.02, 0.03, 0.04];
        assert_eq!(count_consecutive(&data, |v| v > 0.0), 3);
    }

    #[test]
    fn no_match() {
        let data = [-0.01, -0.02, -0.03];
        assert_eq!(count_consecutive(&data, |v| v > 0.0), 0);
    }

    #[test]
    fn alternating() {
        let data = [0.01, -0.01, 0.01, -0.01];
        assert_eq!(count_consecutive(&data, |v| v > 0.0), 1);
    }
}
