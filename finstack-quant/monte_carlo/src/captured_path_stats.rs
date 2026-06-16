use crate::estimate::Estimate;
use crate::paths::SimulatedPath;

fn interpolated_percentile(sorted_values: &[f64], percentile: f64) -> f64 {
    let len = sorted_values.len();
    if len == 1 {
        return sorted_values[0];
    }

    let rank = percentile.clamp(0.0, 1.0) * (len - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    if lower == upper {
        sorted_values[lower]
    } else {
        let weight = rank - lower as f64;
        sorted_values[lower] * (1.0 - weight) + sorted_values[upper] * weight
    }
}

pub(crate) fn apply_captured_path_statistics(
    estimate: Estimate,
    paths: &[SimulatedPath],
) -> Estimate {
    if paths.is_empty() {
        return estimate;
    }

    let mut values: Vec<f64> = paths.iter().map(|path| path.final_value).collect();
    // The engine validates discounted payoffs in `validate_discounted_payoff`
    // before they reach captured paths, so non-finite values should not
    // appear here. `total_cmp` gives a deterministic NaN ordering as a
    // belt-and-braces guarantee in case a future caller bypasses the engine.
    debug_assert!(
        values.iter().all(|v| v.is_finite()),
        "captured-path final_value should be finite by engine invariant; got non-finite"
    );
    values.sort_by(|a, b| a.total_cmp(b));

    let len = values.len();
    let median = if len.is_multiple_of(2) {
        (values[len / 2 - 1] + values[len / 2]) / 2.0
    } else {
        values[len / 2]
    };
    let percentile_25 = interpolated_percentile(&values, 0.25);
    let percentile_75 = interpolated_percentile(&values, 0.75);
    let min = values[0];
    let max = values[len - 1];

    estimate
        .with_median(median)
        .with_percentiles(percentile_25, percentile_75)
        .with_range(min, max)
}

#[cfg(test)]
mod tests {
    use super::apply_captured_path_statistics;
    use crate::estimate::Estimate;
    use crate::paths::SimulatedPath;

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
