//! Generic interpolator container with strategy pattern.
//!
//! Provides [`Interpolator<S>`], a unified storage container for all interpolation
//! algorithms, using the strategy pattern to avoid code duplication while maintaining
//! flexibility and type safety.

use super::{
    traits::{InterpFn, InterpolationStrategy},
    types::{ExtrapolationPolicy, ValidationPolicy},
    utils::{validate_finite_series, validate_knots, validate_positive_series},
};
use crate::error::InputError;

/// Generic interpolator with strategy pattern.
///
/// Unifies knot/value storage and validation across all interpolation methods.
/// The strategy `S` encapsulates algorithm-specific precomputed data (slopes,
/// coefficients) and evaluation logic.
///
/// # Design
///
/// - **Shared state**: `knots`, `values`, `extrapolation` are common to all interpolators.
/// - **Strategy-specific state**: Stored in the generic `strategy: S` field.
/// - **Validation**: Centralized in the constructor; strategies can add extra checks.
/// - **Serialization**: Preserved via custom serde on wrapper types (LinearDf, etc.).
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::interp::{
///     ExtrapolationPolicy, InterpFn, Interpolator, LinearStrategy, ValidationPolicy,
/// };
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let interp = Interpolator::<LinearStrategy>::new(
///     vec![0.0, 1.0, 2.0].into_boxed_slice(),
///     vec![1.0, 0.95, 0.90].into_boxed_slice(),
///     ExtrapolationPolicy::FlatZero,
///     ValidationPolicy::Strict,
/// )?;
/// let value = interp.interp(0.5);
/// # let _ = value;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(serialize = "S: serde::Serialize"))]
#[serde(bound(deserialize = "S: serde::de::DeserializeOwned"))]
pub struct Interpolator<S: InterpolationStrategy> {
    /// Strictly increasing knot times.
    knots: Box<[f64]>,
    /// Values at each knot (semantics depend on strategy).
    values: Box<[f64]>,
    /// Strategy-specific precomputed state.
    strategy: S,
    /// Extrapolation policy for out-of-bounds evaluation.
    extrapolation: ExtrapolationPolicy,
}

impl<S: InterpolationStrategy> Interpolator<S> {
    /// Construct a new interpolator with the given strategy and validation policy.
    ///
    /// # Arguments
    /// * `knots` – strictly ascending knot times.
    /// * `values` – corresponding values (semantics depend on strategy).
    /// * `extrapolation` – extrapolation policy.
    /// * `validation` – validation policy controlling whether negative values are allowed.
    ///
    /// # Errors
    /// * `InputError::TooFewPoints` – fewer than two knots.
    /// * `InputError::NonMonotonicKnots` – knots not strictly increasing.
    /// * `InputError::NonPositiveValue` – values not positive (when `validation` is `Strict`).
    /// * Strategy-specific errors from `S::from_raw`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::math::interp::{
    ///     ExtrapolationPolicy, Interpolator, LinearStrategy, ValidationPolicy,
    /// };
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// // Strict validation (default) - requires positive values for discount factors
    /// let df_interp = Interpolator::<LinearStrategy>::new(
    ///     vec![0.0, 1.0, 2.0].into_boxed_slice(),
    ///     vec![1.0, 0.95, 0.90].into_boxed_slice(),
    ///     ExtrapolationPolicy::FlatZero,
    ///     ValidationPolicy::Strict,
    /// )?;
    ///
    /// // AllowNegative - permits negative values for forward rate curves
    /// let fwd_interp = Interpolator::<LinearStrategy>::new(
    ///     vec![0.0, 1.0, 2.0].into_boxed_slice(),
    ///     vec![-0.01, 0.0, 0.01].into_boxed_slice(),
    ///     ExtrapolationPolicy::FlatZero,
    ///     ValidationPolicy::AllowNegative,
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::boxed_local)]
    pub fn new(
        knots: Box<[f64]>,
        values: Box<[f64]>,
        extrapolation: ExtrapolationPolicy,
        validation: ValidationPolicy,
    ) -> crate::Result<Self> {
        // Centralized validation
        if knots.len() != values.len() {
            return Err(InputError::DimensionMismatch.into());
        }
        if knots.len() < 2 {
            return Err(InputError::TooFewPoints.into());
        }
        validate_knots(&knots)?;
        validate_finite_series(&values)?;

        // Apply validation policy
        if validation == ValidationPolicy::Strict {
            validate_positive_series(&values)?;
        }

        // Build strategy-specific state
        let strategy = S::from_raw(&knots, &values, extrapolation)?;

        Ok(Self {
            knots,
            values,
            strategy,
            extrapolation,
        })
    }

    /// Get the extrapolation policy.
    #[inline]
    pub fn extrapolation(&self) -> ExtrapolationPolicy {
        self.extrapolation
    }
}

impl<S: InterpolationStrategy> InterpFn for Interpolator<S> {
    #[inline]
    fn interp(&self, x: f64) -> f64 {
        self.strategy
            .interp(x, &self.knots, &self.values, self.extrapolation)
    }

    #[inline]
    fn interp_prime(&self, x: f64) -> f64 {
        self.strategy
            .interp_prime(x, &self.knots, &self.values, self.extrapolation)
    }
}
