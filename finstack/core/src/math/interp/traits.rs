//! Trait interface for interpolation functions.
//!
//! Defines the object-safe trait used by all interpolators, enabling
//! polymorphic interpolation with static or dynamic dispatch.

use core::fmt::Debug;

use super::types::{ExtrapolationPolicy, DERIVATIVE_EPSILON};

/// Object-safe interpolation trait.
///
/// This trait enables polymorphic interpolation using either static dispatch
/// (generics) or dynamic dispatch (`dyn InterpFn`). All interpolator types
/// implement this trait.
///
/// # Required Methods
///
/// - [`interp`](Self::interp) - Evaluate the interpolant at a point
///
/// # Provided Methods
///
/// - [`interp_prime`](Self::interp_prime) - First derivative via finite differences
///
/// # Examples
///
/// ```rust
/// use finstack_core::math::interp::{
///     ExtrapolationPolicy, InterpFn, Interpolator, LinearStrategy, ValidationPolicy,
/// };
///
/// # fn main() -> finstack_core::Result<()> {
/// let knots = vec![0.0, 1.0, 2.0].into_boxed_slice();
/// let values = vec![1.0, 0.95, 0.90].into_boxed_slice();
/// let interp = Interpolator::<LinearStrategy>::new(
///     knots, values, ExtrapolationPolicy::FlatZero, ValidationPolicy::Strict,
/// )?;
///
/// // Use as concrete type
/// let val = interp.interp(0.5);
///
/// // Use as trait object for polymorphism
/// let interp_fn: &dyn InterpFn = &interp;
/// let val2 = interp_fn.interp(0.5);
///
/// assert!((val - val2).abs() < 1e-12);
/// # Ok(())
/// # }
/// ```
pub trait InterpFn: Send + Sync + Debug {
    /// Interpolate at coordinate `x`.
    ///
    /// # Arguments
    ///
    /// * `x` - Evaluation point (typically a year fraction or time)
    ///
    /// # Returns
    ///
    /// The interpolated value at `x`. Behavior outside the knot domain
    /// depends on the extrapolation policy of the underlying interpolator.
    fn interp(&self, x: f64) -> f64;

    /// First derivative at `x`.
    ///
    /// Default implementation uses central finite differences with adaptive step size.
    ///
    /// # Arguments
    ///
    /// * `x` - Evaluation point
    ///
    /// # Returns
    ///
    /// Approximate first derivative: `(f(x+h) - f(x-h)) / (2h)` where
    /// `h = max(|x| × ε, 1e-8)` and `ε` is [`DERIVATIVE_EPSILON`].
    ///
    /// # Note
    ///
    /// Some interpolators override this with analytical derivatives for
    /// better accuracy and performance.
    fn interp_prime(&self, x: f64) -> f64 {
        let h = (x.abs() * DERIVATIVE_EPSILON).max(1e-8);
        let f_plus = self.interp(x + h);
        let f_minus = self.interp(x - h);
        // Fall back to one-sided differences at domain boundaries
        if !f_minus.is_finite() {
            return (f_plus - self.interp(x)) / h;
        }
        if !f_plus.is_finite() {
            return (self.interp(x) - f_minus) / h;
        }
        (f_plus - f_minus) / (2.0 * h)
    }
}

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
/// ```ignore
/// struct MyStrategy {
///     coefficients: Box<[f64]>,
/// }
///
/// impl InterpolationStrategy for MyStrategy {
///     fn from_raw(
///         knots: &[f64],
///         values: &[f64],
///         extrapolation: ExtrapolationPolicy,
///     ) -> crate::Result<Self> {
///         // Precompute coefficients...
///         Ok(Self { coefficients: vec![].into_boxed_slice() })
///     }
///
///     fn interp(&self, x: f64, knots: &[f64], values: &[f64], extrapolation: ExtrapolationPolicy) -> f64 {
///         // Binary search + evaluate
///         0.0
///     }
///
///     fn interp_prime(&self, x: f64, knots: &[f64], values: &[f64], extrapolation: ExtrapolationPolicy) -> f64 {
///         // Analytical derivative if available
///         0.0
///     }
/// }
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
