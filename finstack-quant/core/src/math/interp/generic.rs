//! Generic interpolator container with strategy pattern.
//!
//! Provides [`Interpolator<S>`], a unified storage container for all interpolation
//! algorithms, using the strategy pattern to avoid code duplication while maintaining
//! flexibility and type safety.

use super::{
    strategies::{
        CubicHermiteStrategy, LinearStrategy, LogLinearStrategy, MonotoneConvexStrategy,
        PiecewiseQuadraticForwardStrategy,
    },
    traits::InterpolationStrategy,
    types::ExtrapolationPolicy,
    utils::{validate_finite_series, validate_knots},
};
use crate::InputError;

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
/// - **Validation**: Centralized in the constructor; positive-only strategies add
///   strictly positive value validation in `from_raw`.
/// - **Serialization**: Stores canonical knots, values, and extrapolation policy.
///   Deserialization validates these inputs and rebuilds strategy caches.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::interp::{
///     ExtrapolationPolicy, Interpolator, LinearStrategy,
/// };
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let interp = Interpolator::<LinearStrategy>::new(
///     vec![0.0, 1.0, 2.0].into_boxed_slice(),
///     vec![1.0, 0.95, 0.90].into_boxed_slice(),
///     ExtrapolationPolicy::Flat,
/// )?;
/// let value = interp.interp(0.5)?;
/// # let _ = value;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, serde::Serialize)]
#[serde(bound(serialize = ""))]
pub struct Interpolator<S> {
    /// Strictly increasing knot times.
    knots: Box<[f64]>,
    /// Values at each knot (semantics depend on strategy).
    values: Box<[f64]>,
    /// Strategy-specific precomputed state.
    #[serde(skip)]
    strategy: S,
    /// Extrapolation policy for out-of-bounds evaluation.
    extrapolation: ExtrapolationPolicy,
}

impl<S> Interpolator<S> {
    /// Get the extrapolation policy.
    #[inline]
    pub fn extrapolation(&self) -> ExtrapolationPolicy {
        self.extrapolation
    }
}

#[allow(clippy::boxed_local)]
fn new_with_strategy<S: InterpolationStrategy>(
    knots: Box<[f64]>,
    values: Box<[f64]>,
    extrapolation: ExtrapolationPolicy,
) -> crate::Result<Interpolator<S>> {
    if knots.len() != values.len() {
        return Err(InputError::DimensionMismatch.into());
    }
    validate_knots(&knots)?;
    validate_finite_series(&values)?;

    let strategy = S::from_raw(&knots, &values, extrapolation)?;

    Ok(Interpolator {
        knots,
        values,
        strategy,
        extrapolation,
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInterpolator {
    knots: Box<[f64]>,
    values: Box<[f64]>,
    extrapolation: ExtrapolationPolicy,
}

#[inline]
fn interp_with_strategy<S: InterpolationStrategy>(
    interp: &Interpolator<S>,
    x: f64,
) -> crate::Result<f64> {
    interp
        .strategy
        .interp(x, &interp.knots, &interp.values, interp.extrapolation)
}

#[inline]
fn interp_prime_with_strategy<S: InterpolationStrategy>(
    interp: &Interpolator<S>,
    x: f64,
) -> crate::Result<f64> {
    interp
        .strategy
        .interp_prime(x, &interp.knots, &interp.values, interp.extrapolation)
}

macro_rules! impl_interpolator_strategy {
    ($($strategy:ty),+ $(,)?) => {
        $(
            impl<'de> serde::Deserialize<'de> for Interpolator<$strategy> {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: serde::Deserializer<'de>,
                {
                    let raw = RawInterpolator::deserialize(deserializer)?;
                    new_with_strategy(raw.knots, raw.values, raw.extrapolation)
                        .map_err(serde::de::Error::custom)
                }
            }

            impl Interpolator<$strategy> {
                /// Construct a new interpolator with the given strategy and extrapolation policy.
                ///
                /// # Arguments
                ///
                /// * `knots` - Strictly ascending finite knot coordinates (at least 2).
                /// * `values` - Corresponding finite knot values (strictly positive for log-space strategies).
                /// * `extrapolation` - Extrapolation policy applied outside the knot domain.
                ///
                /// # Errors
                ///
                /// * [`InputError::DimensionMismatch`] - `knots` and `values` differ in length.
                /// * [`InputError::TooFewPoints`] - Fewer than two knots.
                /// * [`InputError::NonMonotonicKnots`] - Knots not strictly increasing.
                /// * [`InputError::NonPositiveValue`] - Values not strictly positive for positive-domain strategies.
                /// * [`Error::Validation`](crate::Error::Validation) - Non-finite inputs or unrepresentable strategy slopes/coefficients.
                #[allow(clippy::boxed_local)]
                pub fn new(
                    knots: Box<[f64]>,
                    values: Box<[f64]>,
                    extrapolation: ExtrapolationPolicy,
                ) -> crate::Result<Self> {
                    new_with_strategy(knots, values, extrapolation)
                }

                /// Interpolate at coordinate `x`.
                ///
                /// # Arguments
                ///
                /// * `x` - Finite evaluation coordinate. Behavior outside
                ///   `[knots[0], knots[last]]` follows the interpolator's extrapolation policy.
                ///
                /// # Errors
                ///
                /// Returns [`Error::Validation`](crate::Error::Validation) when `x` is
                /// non-finite, out of range under [`ExtrapolationPolicy::Error`], or when
                /// evaluation overflows/underflows out of the valid output domain.
                #[inline]
                pub fn interp(&self, x: f64) -> crate::Result<f64> {
                    interp_with_strategy(self, x)
                }

                /// First derivative at `x`, computed analytically by the strategy.
                ///
                /// # Arguments
                ///
                /// * `x` - Finite evaluation coordinate, in the same units as the knots.
                ///
                /// # Errors
                ///
                /// Returns [`Error::Validation`](crate::Error::Validation) when `x` is
                /// non-finite, out of range under [`ExtrapolationPolicy::Error`], or when
                /// the analytical derivative is non-finite.
                #[inline]
                pub fn interp_prime(&self, x: f64) -> crate::Result<f64> {
                    interp_prime_with_strategy(self, x)
                }
            }
        )+
    };
}

impl_interpolator_strategy!(
    LinearStrategy,
    LogLinearStrategy,
    PiecewiseQuadraticForwardStrategy,
    CubicHermiteStrategy,
    MonotoneConvexStrategy,
);
