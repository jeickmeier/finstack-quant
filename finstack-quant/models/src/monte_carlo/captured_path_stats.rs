//! Online statistics computed from paths captured during Monte Carlo execution.
//!
use crate::monte_carlo::estimate::Estimate;
use crate::monte_carlo::paths::SimulatedPath;
use finstack_quant_core::math::stats::quantile_sorted;

pub(crate) fn apply_captured_path_statistics(
    estimate: Estimate,
    paths: &[SimulatedPath],
) -> Estimate {
    if paths.is_empty() {
        return estimate;
    }

    let mut values: Vec<f64> = paths.iter().map(|path| path.final_value).collect();
    debug_assert!(
        values.iter().all(|v| v.is_finite()),
        "captured-path final_value should be finite by engine invariant; got non-finite"
    );
    values.sort_by(|a, b| a.total_cmp(b));

    let median = quantile_sorted(&values, 0.5);
    let percentile_25 = quantile_sorted(&values, 0.25);
    let percentile_75 = quantile_sorted(&values, 0.75);
    let min = values[0];
    let max = values[values.len() - 1];

    estimate
        .with_median(median)
        .with_percentiles(percentile_25, percentile_75)
        .with_range(min, max)
}

#[cfg(test)]
mod tests {
    use super::apply_captured_path_statistics;
    use crate::monte_carlo::estimate::Estimate;
    use crate::monte_carlo::paths::SimulatedPath;

    fn captured_path(path_id: usize, final_value: f64) -> SimulatedPath {
        let mut path = SimulatedPath::new(path_id);
        path.set_final_value(final_value);
        path
    }

    #[test]
    fn test_apply_captured_path_statistics_uses_expected_percentiles() {
        let estimate = Estimate::new(0.0, 0.0, (0.0, 0.0), 5);
        let paths = vec![
            captured_path(0, 0.625),
            captured_path(1, 0.125),
            captured_path(2, 0.375),
            captured_path(3, 0.5),
            captured_path(4, 0.25),
        ];

        let estimate = apply_captured_path_statistics(estimate, &paths);

        assert_eq!(estimate.median, Some(0.375));
        assert_eq!(estimate.percentile_25, Some(0.25));
        assert_eq!(estimate.percentile_75, Some(0.5));
        assert_eq!(estimate.min, Some(0.125));
        assert_eq!(estimate.max, Some(0.625));
    }

    #[test]
    fn test_apply_captured_path_statistics_interpolates_even_sample_percentiles() {
        let estimate = Estimate::new(0.0, 0.0, (0.0, 0.0), 4);
        let paths = vec![
            captured_path(0, 4.0),
            captured_path(1, 1.0),
            captured_path(2, 3.0),
            captured_path(3, 2.0),
        ];

        let estimate = apply_captured_path_statistics(estimate, &paths);

        assert_eq!(estimate.median, Some(2.5));
        assert_eq!(estimate.percentile_25, Some(1.75));
        assert_eq!(estimate.percentile_75, Some(3.25));
    }
}
