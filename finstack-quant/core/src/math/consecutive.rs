//! Consecutive streak counter for return series analysis.
//!
//! Used by the aggregation module to compute the longest win and loss streaks
//! from period-level return sequences.

/// Count maximum consecutive runs where `predicate` holds.
fn count_consecutive<F: Fn(f64) -> bool>(values: &[f64], predicate: F) -> usize {
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

/// Count the longest unbroken run of strictly positive values.
///
/// Canonical definition of a "winning streak" shared across Rust and host
/// bindings.
///
/// # Arguments
///
/// * `values` - Slice of return values to scan.
///
/// # Returns
///
/// Length of the longest streak of strictly positive elements. Returns `0`
/// if `values` is empty or contains no positive element.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::consecutive::longest_positive_run;
///
/// // One losing period followed by three wins → longest streak is 3.
/// let returns = [-0.01, 0.02, 0.03, 0.04];
/// assert_eq!(longest_positive_run(&returns), 3);
///
/// assert_eq!(longest_positive_run(&[]), 0);
/// ```
pub fn longest_positive_run(values: &[f64]) -> usize {
    count_consecutive(values, |v| v > 0.0)
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

    #[test]
    fn longest_positive_run_matches_generic_predicate() {
        let data = [-0.01, 0.02, 0.03, 0.04, -0.02, 0.05];
        assert_eq!(longest_positive_run(&data), 3);
        assert_eq!(
            longest_positive_run(&data),
            count_consecutive(&data, |v| v > 0.0)
        );
        assert_eq!(longest_positive_run(&[]), 0);
        assert_eq!(longest_positive_run(&[-0.1, -0.2]), 0);
    }
}
