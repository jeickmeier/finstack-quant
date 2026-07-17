//! Shared defaults and calculations used by term-structure implementations.
//!
/// Calculate triangular weight for key-rate DV01.
///
/// Returns a weight in [0, 1] that peaks at `target`. The shape is:
///
/// - **Interior bucket** (`prev = Some(p)`, `next = Some(n)`): a full
///   triangle — 0 at `prev`, linear rise to 1 at `target`, linear fall to
///   0 at `next`.
/// - **First (left-wing) bucket** (`prev = None`, `next = Some(n)`): flat
///   1.0 for `t ≤ target`, then linear fall to 0 at `next`.
/// - **Last (right-wing) bucket** (`prev = Some(p)`, `next = None`):
///   linear rise from 0 at `prev` to 1 at `target`, then flat 1.0 for
///   `t > target`.
/// - **Sole bucket** (`prev = None`, `next = None`): a constant 1.0
///   everywhere. Equivalent to a flat parallel shift.
///
/// This function defines the weight based on the **bucket grid** (not the
/// curve knots). When the wings are configured as `None`, the weights of
/// the full bucket set sum to 1.0 at any time t covered by any bucket —
/// the invariant that makes `Σ bucketed_dv01 = parallel_dv01`.
///
/// # Arguments
/// * `t` - The time at which to calculate the weight
/// * `prev` - Previous bucket time (`None` for the first / left-wing bucket)
/// * `target` - Target bucket time (peak of the triangle)
/// * `next` - Next bucket time (`None` for the last / right-wing bucket)
///
/// # Returns
/// Weight in [0, 1] representing the contribution of this bucket to the rate at time t.
#[inline]
pub(crate) fn triangular_weight(t: f64, prev: Option<f64>, target: f64, next: Option<f64>) -> f64 {
    match prev {
        None if t <= target => return 1.0,
        Some(p) if t <= p => return 0.0,
        Some(p) if t <= target => {
            let denom = (target - p).max(1e-10);
            return (t - p) / denom;
        }
        _ => {}
    }

    match next {
        None => 1.0,
        Some(n) => {
            if t < n {
                let denom = (n - target).max(1e-10);
                (n - t) / denom
            } else {
                0.0
            }
        }
    }
}

/// Validate a triangular key-rate bucket grid before applying it.
///
/// Every finite bound must satisfy `prev < target < next`, and any provided
/// bound must be finite: a non-finite neighbour (e.g. `Some(f64::INFINITY)`
/// as a "no right neighbour" sentinel) would make [`triangular_weight`]
/// compute `∞/∞ = NaN` for knots beyond `target` and corrupt the curve.
/// Wing buckets must be expressed with `None`, not infinite sentinels.
///
/// Mirrors the validation on the copy-path bump constructors
/// (`with_triangular_key_rate_bump_neighbors`) so the in-place and copy bump
/// paths reject the same malformed grids.
#[inline]
pub(crate) fn validate_triangular_bucket_grid(
    prev: Option<f64>,
    target: f64,
    next: Option<f64>,
) -> crate::Result<()> {
    if !target.is_finite() {
        return Err(crate::error::InputError::Invalid.into());
    }
    if let Some(p) = prev {
        if !p.is_finite() || p >= target {
            return Err(crate::error::InputError::Invalid.into());
        }
    }
    if let Some(n) = next {
        if !n.is_finite() || target >= n {
            return Err(crate::error::InputError::Invalid.into());
        }
    }
    Ok(())
}

/// Helper to shift knot times backward by `dt` and filter out expired points (t <= 0).
///
/// Used by `roll_forward` implementations in discount and forward curves.
#[inline]
pub(crate) fn roll_knots(knots: &[f64], values: &[f64], dt: f64) -> Vec<(f64, f64)> {
    knots
        .iter()
        .zip(values.iter())
        .filter_map(|(&t, &v)| {
            let new_t = t - dt;
            if new_t > 0.0 {
                Some((new_t, v))
            } else {
                None
            }
        })
        .collect()
}

/// Apply an additive parallel bump to a slice of (t, value) knots.
///
/// Each value is clamped to zero from below: `max(0, v + bump)`.
/// Returns the bumped knots as a new `Vec`.
#[inline]
pub(crate) fn bump_knots_parallel(knots: &[f64], values: &[f64], bump: f64) -> Vec<(f64, f64)> {
    knots
        .iter()
        .zip(values.iter())
        .map(|(&t, &v)| (t, (v + bump).max(0.0)))
        .collect()
}

/// Apply a multiplicative percentage bump to a slice of (t, value) knots.
///
/// Each value is scaled by `1 + pct` and clamped to zero from below.
#[inline]
pub(crate) fn bump_knots_percentage(knots: &[f64], values: &[f64], pct: f64) -> Vec<(f64, f64)> {
    let factor = 1.0 + pct;
    knots
        .iter()
        .zip(values.iter())
        .map(|(&t, &v)| (t, (v * factor).max(0.0)))
        .collect()
}

/// Apply a triangular key-rate bump to a slice of (t, value) knots.
///
/// Each knot receives a weight in `[0, 1]` based on its proximity to
/// `target_bucket`. `prev_bucket = None` / `next_bucket = None` make the
/// bucket a half-triangle (flat wing) on that side. Spot (t=0) is
/// typically excluded by the caller.
#[inline]
pub(crate) fn bump_knots_triangular(
    knots: &[f64],
    values: &[f64],
    prev_bucket: Option<f64>,
    target_bucket: f64,
    next_bucket: Option<f64>,
    bump: f64,
) -> Vec<(f64, f64)> {
    knots
        .iter()
        .zip(values.iter())
        .map(|(&t, &v)| {
            let w = triangular_weight(t, prev_bucket, target_bucket, next_bucket);
            (t, (v + bump * w).max(0.0))
        })
        .collect()
}

/// Validate that all values in a knot slice are non-negative.
///
/// Returns a descriptive error that includes the tenor and value for quick diagnosis.
pub(crate) fn validate_non_negative_knots(
    knots: &[f64],
    values: &[f64],
    value_label: &str,
) -> crate::Result<()> {
    for (i, (&t, &v)) in knots.iter().zip(values.iter()).enumerate() {
        if v < 0.0 {
            return Err(crate::Error::Validation(format!(
                "{value_label} must be non-negative at t={t:.6}: value={v:.8} (index {i})"
            )));
        }
    }
    Ok(())
}

/// Infer the spot value from a knot set when the first knot is at t≈0.
///
/// Returns `Some(v)` when the first knot is at t=0 (within 1e-14), otherwise `None`.
#[inline]
pub(crate) fn infer_spot_from_knots(knots: &[f64], values: &[f64]) -> Option<f64> {
    knots
        .first()
        .filter(|&&t| t.abs() <= 1e-14)
        .map(|_| values[0])
}

/// Validate that a value is within the unit range `[0.0, 1.0]`.
///
/// Returns an error with a descriptive message if the value is out of range.
/// Used by hazard curve recovery rates, base correlation values, etc.
#[inline]
pub(crate) fn validate_unit_range(value: f64, field_name: &str) -> crate::Result<()> {
    if !(0.0..=1.0).contains(&value) {
        return Err(crate::error::InputError::Invalid.into());
    }
    let _ = field_name; // used in error context if needed in the future
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::triangular_weight;

    #[test]
    fn first_bucket_half_triangle_is_flat_to_the_left() {
        let target = 0.25;
        let next = Some(0.5);
        for &t in &[0.0_f64, 1e-12, 1e-6, 0.01, 0.10, 0.25] {
            let w = triangular_weight(t, None, target, next);
            assert!(
                (w - 1.0).abs() < 1e-12,
                "first-bucket weight at t={t} must be 1.0, got {w}"
            );
        }
        let w_mid = triangular_weight(0.375, None, target, next);
        assert!(
            (w_mid - 0.5).abs() < 1e-12,
            "first-bucket falling-edge weight at midpoint should be 0.5, got {w_mid}"
        );
        assert_eq!(triangular_weight(0.6, None, target, next), 0.0);
    }

    #[test]
    fn last_bucket_half_triangle_is_flat_to_the_right() {
        let prev = Some(20.0);
        let target = 30.0;
        for &t in &[30.0_f64, 31.0, 45.0, 100.0, 1e6] {
            let w = triangular_weight(t, prev, target, None);
            assert!(
                (w - 1.0).abs() < 1e-12,
                "last-bucket weight at t={t} must be 1.0, got {w}"
            );
        }
        let w_mid = triangular_weight(25.0, prev, target, None);
        assert!(
            (w_mid - 0.5).abs() < 1e-12,
            "last-bucket rising-edge weight at midpoint should be 0.5, got {w_mid}"
        );
        assert_eq!(triangular_weight(15.0, prev, target, None), 0.0);
    }

    #[test]
    fn interior_bucket_full_triangle_is_unchanged() {
        let prev = Some(3.0);
        let target = 5.0;
        let next = Some(7.0);
        assert_eq!(triangular_weight(3.0, prev, target, next), 0.0);
        assert!((triangular_weight(4.0, prev, target, next) - 0.5).abs() < 1e-12);
        assert!((triangular_weight(5.0, prev, target, next) - 1.0).abs() < 1e-12);
        assert!((triangular_weight(6.0, prev, target, next) - 0.5).abs() < 1e-12);
        assert_eq!(triangular_weight(7.0, prev, target, next), 0.0);
    }

    #[test]
    fn full_bucket_set_partitions_unity_across_curve() {
        let bucket_times = [0.25_f64, 1.0, 5.0];
        let last = bucket_times.len() - 1;
        for &t in &[
            0.0_f64, 1e-9, 0.01, 0.10, 0.25, 0.50, 1.00, 2.00, 4.00, 5.00, 10.00, 1e6,
        ] {
            let mut sum = 0.0_f64;
            for (i, &target) in bucket_times.iter().enumerate() {
                let prev = if i == 0 {
                    None
                } else {
                    Some(bucket_times[i - 1])
                };
                let next = if i == last {
                    None
                } else {
                    Some(bucket_times[i + 1])
                };
                sum += triangular_weight(t, prev, target, next);
            }
            assert!(
                (sum - 1.0).abs() < 1e-12,
                "bucket weights at t={t} must sum to 1.0, got {sum}"
            );
        }
    }
}
