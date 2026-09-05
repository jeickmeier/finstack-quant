//! Small numerical helpers shared by scenario adapters.

/// Convert basis-point integers to absolute fractions (e.g., `300 bp` → `0.03`).
#[inline]
pub(crate) fn bp_to_fractions(bp: &[i32]) -> Vec<f64> {
    bp.iter().map(|bp| f64::from(*bp) / 10_000.0).collect()
}

/// Interpolation weights and flat-extrapolation metadata for a curve-node shock.
#[derive(Debug, Clone)]
pub(crate) struct InterpolationResult {
    pub(crate) weights: Vec<(usize, f64)>,
    pub(crate) is_extrapolation: bool,
    pub(crate) extrapolation_distance: Option<f64>,
}

/// Split a target time across its nearest knots and report flat extrapolation.
pub(crate) fn calculate_interpolation_weights(target: f64, knots: &[f64]) -> InterpolationResult {
    if knots.is_empty() {
        return InterpolationResult {
            weights: vec![],
            is_extrapolation: false,
            extrapolation_distance: None,
        };
    }

    let max_knot = knots[knots.len() - 1];
    let min_knot = knots[0];

    let (is_extrapolation, extrapolation_distance) = if target > max_knot + 1e-10 {
        (true, Some(target - max_knot))
    } else if target < min_knot - 1e-10 {
        (true, Some(min_knot - target))
    } else {
        (false, None)
    };

    let pos = knots
        .iter()
        .position(|&t| t >= target)
        .unwrap_or(knots.len() - 1);

    let weights = if pos == 0 {
        vec![(0, 1.0)]
    } else if target > max_knot {
        vec![(knots.len() - 1, 1.0)]
    } else {
        let i0 = pos - 1;
        let i1 = pos;
        let t0 = knots[i0];
        let t1 = knots[i1];

        if (t1 - t0).abs() < 1e-12 {
            vec![(i0, 0.5), (i1, 0.5)]
        } else {
            let w1 = (target - t0) / (t1 - t0);
            let w0 = 1.0 - w1;
            vec![(i0, w0), (i1, w1)]
        }
    };

    InterpolationResult {
        weights,
        is_extrapolation,
        extrapolation_distance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bps_to_fractions() {
        let out = bp_to_fractions(&[300, 700]);
        assert!((out[0] - 0.03).abs() < 1e-9);
        assert!((out[1] - 0.07).abs() < 1e-9);
        assert!(bp_to_fractions(&[]).is_empty());
    }

    #[test]
    fn interpolation_weights_preserve_math_and_extrapolation_metadata() {
        let interpolated = calculate_interpolation_weights(3.0, &[1.0, 2.0, 5.0, 10.0]);
        assert_eq!(interpolated.weights[0].0, 1);
        assert!((interpolated.weights[0].1 - 2.0 / 3.0).abs() < 1e-12);
        assert_eq!(interpolated.weights[1].0, 2);
        assert!((interpolated.weights[1].1 - 1.0 / 3.0).abs() < 1e-12);
        assert!(!interpolated.is_extrapolation);
        assert_eq!(interpolated.extrapolation_distance, None);

        let extrapolated = calculate_interpolation_weights(15.0, &[1.0, 2.0, 5.0, 10.0]);
        assert_eq!(extrapolated.weights, vec![(3, 1.0)]);
        assert!(extrapolated.is_extrapolation);
        assert_eq!(extrapolated.extrapolation_distance, Some(5.0));
    }
}
