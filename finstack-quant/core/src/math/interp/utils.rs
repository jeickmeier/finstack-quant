//! Shared validation and search utilities for interpolators.
//!
//! Provides common helpers for knot validation, segment location, and
//! monotonicity checking used across all interpolation implementations.

use super::types::ExtrapolationPolicy;
use crate::InputError;

/// Extension rule applied outside the knot range.
///
/// [`check_extrapolation`] resolves [`ExtrapolationPolicy::None`] to `NaN`
/// before any rule runs, so the rules only ever see the two flat extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Extension {
    /// Hold the boundary value constant.
    FlatZero,
    /// Extend the boundary slope.
    FlatForward,
}

/// Helper to check and apply extrapolation if x is out of bounds.
///
/// A non-finite `x` returns `NaN` before any knot checks, so interpolators
/// do not repeat the finiteness check. If `x` is before the first knot, calls
/// `on_left` with the extension rule; if `x` is after the last knot, calls
/// `on_right`. [`ExtrapolationPolicy::None`] returns `NaN` for out-of-range
/// `x` without calling either closure. Otherwise returns `None`.
#[inline]
pub(super) fn check_extrapolation<F1, F2>(
    x: f64,
    knots: &[f64],
    extrapolation: ExtrapolationPolicy,
    on_left: F1,
    on_right: F2,
) -> Option<f64>
where
    F1: FnOnce(Extension) -> f64,
    F2: FnOnce(Extension) -> f64,
{
    if !x.is_finite() {
        return Some(f64::NAN);
    }

    if knots.is_empty() {
        return None;
    }

    // Left extrapolation
    if x < knots[0] {
        return Some(apply_extension(extrapolation, on_left));
    }

    // Right extrapolation
    if let Some(&last_knot) = knots.last() {
        if x > last_knot {
            return Some(apply_extension(extrapolation, on_right));
        }
    }

    None
}

/// Resolve an out-of-range policy: `None` is `NaN`, the flat rules call `on_flat`.
fn apply_extension<F>(policy: ExtrapolationPolicy, on_flat: F) -> f64
where
    F: FnOnce(Extension) -> f64,
{
    match policy {
        ExtrapolationPolicy::None => f64::NAN,
        ExtrapolationPolicy::FlatZero => on_flat(Extension::FlatZero),
        ExtrapolationPolicy::FlatForward => on_flat(Extension::FlatForward),
    }
}

/// Validate a finite, strictly increasing interpolation knot axis.
///
/// The values are arbitrary real coordinates; this helper does not require
/// times to be positive or start at zero. It checks monotonicity but not the
/// stronger minimum-spacing requirement used by slope-sensitive strategies.
///
/// # Arguments
///
/// * `knots` - Finite interpolation coordinates in strictly increasing order.
///   At least two values are required.
///
/// # Errors
///
/// Returns [`InputError::TooFewPoints`] for fewer than two knots,
/// [`Error::Validation`](crate::Error::Validation) for NaN or infinity, and
/// [`InputError::NonMonotonicKnots`] when any consecutive pair is equal or
/// decreasing.
pub fn validate_knots(knots: &[f64]) -> crate::Result<()> {
    if knots.len() < 2 {
        return Err(InputError::TooFewPoints.into());
    }
    if knots.iter().any(|k| !k.is_finite()) {
        return Err(crate::Error::Validation("knots must be finite".to_string()));
    }
    if knots.windows(2).any(|w| w[1] <= w[0]) {
        return Err(InputError::NonMonotonicKnots.into());
    }
    Ok(())
}

/// Default minimum relative gap between consecutive knots.
///
/// Knots closer than `gap < MIN_RELATIVE_KNOT_GAP * max(|k[i]|, 1.0)` are
/// rejected to prevent numerical instability in slope/derivative calculations.
pub(crate) const MIN_RELATIVE_KNOT_GAP: f64 = 1e-10;

/// Validate that consecutive knots have sufficient spacing for stable interpolation.
///
/// The minimum gap is relative to knot magnitude:
/// `gap >= min_relative_gap * max(|k[i]|, 1.0)`.
/// This prevents division-by-near-zero in slope calculations while allowing
/// tight spacing for small-magnitude knots. The `max(|k[i]|, 1.0)` floor
/// ensures the threshold never shrinks below `min_relative_gap` for knots near
/// zero.
pub(crate) fn validate_knot_spacing(knots: &[f64], min_relative_gap: f64) -> crate::Result<()> {
    for w in knots.windows(2) {
        let gap = w[1] - w[0];
        let scale = w[0].abs().max(1.0);
        if gap < min_relative_gap * scale {
            return Err(InputError::KnotSpacingTooSmall.into());
        }
    }
    Ok(())
}

/// Fast-path segment lookup for callers that already verified x is finite
/// and in-bounds (e.g., after `check_extrapolation` returned `None`).
///
/// Skips finite/bounds checks. Returns the segment index directly.
/// Caller MUST ensure: x is finite, xs has >= 2 elements, xs[0] <= x <= xs[last].
#[inline(always)]
pub(crate) fn locate_segment_unchecked(xs: &[f64], x: f64) -> usize {
    let idx = xs.partition_point(|k| *k < x);
    if idx == 0 {
        0
    } else {
        idx - 1
    }
}

/// Validate that all values are strictly positive.
pub(crate) fn validate_positive_series(values: &[f64]) -> crate::Result<()> {
    if values.iter().any(|&v| !v.is_finite() || v <= 0.0) {
        return Err(InputError::NonPositiveValue.into());
    }
    Ok(())
}

/// Validate that all values are finite (no NaN/Inf).
pub(crate) fn validate_finite_series(values: &[f64]) -> crate::Result<()> {
    if values.iter().any(|&v| !v.is_finite()) {
        return Err(crate::Error::Validation(
            "series values must be finite".to_string(),
        ));
    }
    Ok(())
}

/// Find the first violation of non-increasing monotonicity with a relative tolerance.
///
/// Returns `Some((index, prev_value, curr_value))` for the first pair where
/// `values[index + 1] > values[index] + tolerance`, or `None` if the sequence
/// is monotone non-increasing within tolerance.
///
/// The tolerance is computed as `base_tol * max(|prev|, 1.0)` to handle both
/// small and large value ranges.
pub(crate) fn find_monotone_violation(values: &[f64], base_tol: f64) -> Option<(usize, f64, f64)> {
    values.windows(2).enumerate().find_map(|(i, w)| {
        let (prev, curr) = (w[0], w[1]);
        let tol = base_tol * prev.abs().max(1.0);
        if curr > prev + tol {
            Some((i, prev, curr))
        } else {
            None
        }
    })
}

/// Flat-extrapolated piecewise-linear interpolation over `(x, y)` knots.
///
/// Knots must be sorted ascending by `x`. Queries outside the knot range
/// clamp to the nearest endpoint value (flat extrapolation), which is the
/// convention every caller in this workspace wants for spread, hazard and
/// PD curves.
///
/// # Arguments
///
/// * `knots` - Ascending `(x, y)` pairs. An empty slice yields `0.0`.
/// * `x` - Abscissa to evaluate, in the same units as the knot `x` values
///   (year fractions for the curve callers in this workspace). Values below
///   the first knot or above the last clamp to that endpoint's `y`.
///
/// # Returns
///
/// The interpolated `y`, or `0.0` when `knots` is empty.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::math::interp::interp_knots_flat;
///
/// let knots = [(0.0, 1.0), (2.0, 3.0)];
/// assert!((interp_knots_flat(&knots, 1.0) - 2.0).abs() < 1e-12);
/// // Flat outside the range.
/// assert!((interp_knots_flat(&knots, -5.0) - 1.0).abs() < 1e-12);
/// assert!((interp_knots_flat(&knots, 99.0) - 3.0).abs() < 1e-12);
/// ```
#[must_use]
pub fn interp_knots_flat(knots: &[(f64, f64)], x: f64) -> f64 {
    if knots.is_empty() {
        return 0.0;
    }
    if x <= knots[0].0 {
        return knots[0].1;
    }
    let last = knots[knots.len() - 1];
    if x >= last.0 {
        return last.1;
    }
    let idx = knots.partition_point(|k| k.0 < x);
    if idx == 0 {
        return knots[0].1;
    }
    let (x0, y0) = knots[idx - 1];
    let (x1, y1) = knots[idx];
    // Guard a zero-width interval: constructors reject duplicate knot times,
    // but public-field structs can still carry `x1 == x0`, and the weight
    // would be `0.0 / 0.0 = NaN` and poison every downstream figure.
    if x1 <= x0 {
        return y1;
    }
    y0 + (x - x0) / (x1 - x0) * (y1 - y0)
}

#[cfg(test)]
mod knot_spacing_tests {
    use super::*;

    #[test]
    fn rejects_knots_too_close() {
        let knots = [1.0, 1.0 + 1e-16];
        let result = validate_knot_spacing(&knots, MIN_RELATIVE_KNOT_GAP);
        assert!(result.is_err());
    }

    #[test]
    fn accepts_knots_with_sufficient_spacing() {
        let knots = [0.0, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0];
        let result = validate_knot_spacing(&knots, MIN_RELATIVE_KNOT_GAP);
        assert!(result.is_ok());
    }

    #[test]
    fn near_zero_knots_use_absolute_floor() {
        let knots = [0.001, 0.002];
        let result = validate_knot_spacing(&knots, MIN_RELATIVE_KNOT_GAP);
        assert!(result.is_ok());
    }

    #[test]
    fn large_knots_use_relative_threshold() {
        let knots = [1_000_000.0, 1_000_000.000_01];
        let result = validate_knot_spacing(&knots, MIN_RELATIVE_KNOT_GAP);
        assert!(result.is_err());
    }
}
