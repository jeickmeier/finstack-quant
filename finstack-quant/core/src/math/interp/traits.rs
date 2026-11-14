//! Strategy trait implemented by each interpolation algorithm.

use core::fmt::Debug;

use super::types::ExtrapolationPolicy;

/// Strategy trait for interpolation algorithms.
///
/// Encapsulates the strategy-specific precomputed state (slopes, coefficients)
/// and evaluation logic. Used by the generic `Interpolator<S>` container
/// to delegate computation while sharing knot/value storage and validation.
///
/// # Required Methods
///
/// - [`from_raw`](Self::from_raw) - Construct strategy from knots and values
/// - [`interp`](Self::interp) - Evaluate interpolant at a point
/// - [`interp_prime`](Self::interp_prime) - Evaluate first derivative
///
/// # Design
///
/// Each concrete strategy (Linear, LogLinear, CubicHermite, MonotoneConvex)
/// implements this trait to provide:
/// - Construction from raw knots/values with validation
/// - Interpolation and derivative evaluation given knots/values/extrapolation
///
/// The generic `Interpolator<S>` struct holds the shared `knots`, `values`,
/// and `extrapolation` fields, while the strategy holds any precomputed data
/// (e.g., slopes for cubic, coefficients for monotone-convex).
///
/// # Implementation Guide
///
/// When implementing a new interpolation strategy:
///
/// 1. **Precompute** any per-segment coefficients in [`from_raw`](Self::from_raw)
/// 2. **Use binary search** in [`interp`](Self::interp) to find the segment
/// 3. **Handle extrapolation** by checking bounds before evaluation
/// 4. **Implement analytical derivatives** in [`interp_prime`](Self::interp_prime)
///    when possible for better accuracy
pub(crate) trait InterpolationStrategy: Send + Sync + Debug {
    /// Build strategy-specific state from raw knots and values.
    ///
    /// # Arguments
    ///
    /// * `knots` – strictly ascending knot times (already validated by caller).
    /// * `values` – corresponding values (already validated by caller).
    /// * `extrapolation` – extrapolation policy.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error`] when strategy-specific validation fails:
    ///
    /// - [`InputError::NonMonotonicKnots`](crate::InputError::NonMonotonicKnots):
    ///   Values violate monotonicity requirement (MonotoneConvex strategy)
    /// - [`InputError::TooFewPoints`](crate::InputError::TooFewPoints):
    ///   Insufficient knots for the strategy (e.g., cubic requires ≥ 4 points)
    /// - [`InputError::NonPositiveValue`](crate::InputError::NonPositiveValue):
    ///   Log-based strategies require positive values for logarithm
    /// - [`Error::Validation`](crate::Error::Validation):
    ///   General validation failure (non-finite values, degenerate segments)
    ///
    /// # Implementation Note
    ///
    /// Callers (typically `Interpolator<S>`) should validate knots are strictly
    /// ascending before calling this method. This method handles only
    /// strategy-specific validation such as monotonicity or positivity constraints.
    fn from_raw(
        knots: &[f64],
        values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Self>
    where
        Self: Sized;

    /// Interpolate at coordinate `x` using the strategy's algorithm.
    ///
    /// # Arguments
    /// * `x` – evaluation point.
    /// * `knots` – knot times (from parent Interpolator).
    /// * `values` – values (from parent Interpolator).
    /// * `extrapolation` – extrapolation policy (from parent Interpolator).
    fn interp(
        &self,
        x: f64,
        knots: &[f64],
        values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> f64;

    /// First derivative at `x` using the strategy's algorithm.
    ///
    /// # Arguments
    /// * `x` – evaluation point.
    /// * `knots` – knot times (from parent Interpolator).
    /// * `values` – values (from parent Interpolator).
    /// * `extrapolation` – extrapolation policy (from parent Interpolator).
    fn interp_prime(
        &self,
        x: f64,
        knots: &[f64],
        values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> f64;
}
