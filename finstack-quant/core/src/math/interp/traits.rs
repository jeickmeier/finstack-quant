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
///
/// # Example Implementation
///
/// ```
/// use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpolationStrategy};
///
/// #[derive(Debug)]
/// struct MyStrategy {
///     slopes: Box<[f64]>,
/// }
///
/// impl InterpolationStrategy for MyStrategy {
///     fn from_raw(
///         knots: &[f64],
///         values: &[f64],
///         _extrapolation: ExtrapolationPolicy,
///     ) -> finstack_quant_core::Result<Self> {
///         // Precompute per-segment slopes once, at construction.
///         let slopes = knots
///             .windows(2)
///             .zip(values.windows(2))
///             .map(|(t, v)| (v[1] - v[0]) / (t[1] - t[0]))
///             .collect();
///         Ok(Self { slopes })
///     }
///
///     fn interp(
///         &self,
///         x: f64,
///         knots: &[f64],
///         values: &[f64],
///         _extrapolation: ExtrapolationPolicy,
///     ) -> f64 {
///         let i = knots.partition_point(|&t| t <= x).saturating_sub(1);
///         let i = i.min(self.slopes.len().saturating_sub(1));
///         values[i] + self.slopes[i] * (x - knots[i])
///     }
///
///     fn interp_prime(
///         &self,
///         x: f64,
///         knots: &[f64],
///         _values: &[f64],
///         _extrapolation: ExtrapolationPolicy,
///     ) -> f64 {
///         let i = knots.partition_point(|&t| t <= x).saturating_sub(1);
///         self.slopes[i.min(self.slopes.len().saturating_sub(1))]
///     }
/// }
///
/// let knots = [0.0, 1.0, 2.0];
/// let values = [1.0, 2.0, 4.0];
/// let strategy = MyStrategy::from_raw(&knots, &values, ExtrapolationPolicy::FlatZero)?;
/// assert!((strategy.interp(1.5, &knots, &values, ExtrapolationPolicy::FlatZero) - 3.0).abs() < 1e-12);
/// assert!((strategy.interp_prime(1.5, &knots, &values, ExtrapolationPolicy::FlatZero) - 2.0).abs() < 1e-12);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub trait InterpolationStrategy: Send + Sync + Debug {
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
    /// - [`InputError::NonMonotonicKnots`](crate::error::InputError::NonMonotonicKnots):
    ///   Values violate monotonicity requirement (MonotoneConvex strategy)
    /// - [`InputError::TooFewPoints`](crate::error::InputError::TooFewPoints):
    ///   Insufficient knots for the strategy (e.g., cubic requires ≥ 4 points)
    /// - [`InputError::NonPositiveValue`](crate::error::InputError::NonPositiveValue):
    ///   Log-based strategies require positive values for logarithm
    /// - [`InputError::Invalid`](crate::error::InputError::Invalid):
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
