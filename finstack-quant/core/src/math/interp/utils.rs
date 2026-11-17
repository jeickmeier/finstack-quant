//! Shared validation and search utilities for interpolators.
//!
//! Provides common helpers for knot validation, segment location, and
//! monotonicity checking used across all interpolation implementations.

use super::types::ExtrapolationPolicy;
use crate::InputError;

/// Action resolved for an interpolation query before interior evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum BoundaryDecision {
    /// Query hits a boundary / out-of-range flat result directly.
    Return(f64),
    /// Query is below `knots[0]` under `ExtrapolationPolicy::Extend`.
    ExtendLeft,
    /// Query is above `knots[last]` under `ExtrapolationPolicy::Extend`.
    ExtendRight,
    /// Query is strictly inside `(knots[0], knots[last])`, or on an exact boundary knot
    /// when evaluating derivatives or interior formulas.
    Interior,
}

/// Validate query finiteness and resolve out-of-range extrapolation and exact knot hits
/// for value evaluation (`interp`).
///
/// Always rejects `!x.is_finite()` regardless of extrapolation policy, rejects
/// out-of-range queries under [`ExtrapolationPolicy::Error`], returns exact boundary
/// stored `values` under [`ExtrapolationPolicy::Flat`], and returns exact stored
/// `values[idx]` whenever `x == knots[idx]` so no strategy introduces log/exp round-trip
/// drift or `0.0 * inf = NaN`.
#[inline]
pub(super) fn classify_value_query(
    x: f64,
    knots: &[f64],
    values: &[f64],
    extrapolation: ExtrapolationPolicy,
) -> crate::Result<BoundaryDecision> {
    if !x.is_finite() {
        return Err(crate::Error::Validation(format!(
            "interpolation query coordinate must be finite (got {x})"
        )));
    }
    if x < knots[0] {
        return match extrapolation {
            ExtrapolationPolicy::Flat => Ok(BoundaryDecision::Return(values[0])),
            ExtrapolationPolicy::Extend => Ok(BoundaryDecision::ExtendLeft),
            ExtrapolationPolicy::Error => Err(crate::Error::Validation(format!(
                "interpolation query {x} is below minimum knot {}",
                knots[0]
            ))),
        };
    }
    let last = knots.len() - 1;
    if x > knots[last] {
        return match extrapolation {
            ExtrapolationPolicy::Flat => Ok(BoundaryDecision::Return(values[last])),
            ExtrapolationPolicy::Extend => Ok(BoundaryDecision::ExtendRight),
            ExtrapolationPolicy::Error => Err(crate::Error::Validation(format!(
                "interpolation query {x} exceeds maximum knot {}",
                knots[last]
            ))),
        };
    }
    let idx = locate_segment_unchecked(knots, x);
    if exact_f64_eq(x, knots[idx]) {
        return Ok(BoundaryDecision::Return(values[idx]));
    }
    if exact_f64_eq(x, knots[idx + 1]) {
        return Ok(BoundaryDecision::Return(values[idx + 1]));
    }
    Ok(BoundaryDecision::Interior)
}

#[inline]
pub(crate) fn exact_f64_eq(a: f64, b: f64) -> bool {
    a.is_finite() && b.is_finite() && (a.to_bits() == b.to_bits() || (a == 0.0 && b == 0.0))
}

/// Validate query finiteness and resolve out-of-range extrapolation for derivative
/// evaluation (`interp_prime`).
///
/// Queries at exact boundary knots (`x == knots[0]` or `x == knots[last]`) remain inside
/// the closed domain and therefore return [`BoundaryDecision::Interior`] (yielding the
/// inward derivative), whereas strictly external queries under
/// [`ExtrapolationPolicy::Flat`] return `Ok(0.0)`.
#[inline]
pub(super) fn classify_prime_query(
    x: f64,
    knots: &[f64],
    extrapolation: ExtrapolationPolicy,
) -> crate::Result<BoundaryDecision> {
    if !x.is_finite() {
        return Err(crate::Error::Validation(format!(
            "interpolation derivative query coordinate must be finite (got {x})"
        )));
    }
    if x < knots[0] {
        return match extrapolation {
            ExtrapolationPolicy::Flat => Ok(BoundaryDecision::Return(0.0)),
            ExtrapolationPolicy::Extend => Ok(BoundaryDecision::ExtendLeft),
            ExtrapolationPolicy::Error => Err(crate::Error::Validation(format!(
                "interpolation derivative query {x} is below minimum knot {}",
                knots[0]
            ))),
        };
    }
    let last = knots.len() - 1;
    if x > knots[last] {
        return match extrapolation {
            ExtrapolationPolicy::Flat => Ok(BoundaryDecision::Return(0.0)),
            ExtrapolationPolicy::Extend => Ok(BoundaryDecision::ExtendRight),
            ExtrapolationPolicy::Error => Err(crate::Error::Validation(format!(
                "interpolation derivative query {x} exceeds maximum knot {}",
                knots[last]
            ))),
        };
    }
    Ok(BoundaryDecision::Interior)
}

/// Validate a finite, strictly increasing interpolation knot axis.
///
/// The values are arbitrary real coordinates; this helper does not require
/// times to be positive or start at zero.
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

/// Fast-path segment lookup for callers that already verified `x` is finite
/// and in-bounds (`knots[0] <= x <= knots[last]`).
#[inline(always)]
pub(crate) fn locate_segment_unchecked(xs: &[f64], x: f64) -> usize {
    let idx = xs.partition_point(|k| *k < x);
    if idx == 0 { 0 } else { idx - 1 }
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
