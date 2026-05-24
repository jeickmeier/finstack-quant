//! Interpolation types and style enumeration.
//!
//! Defines the `InterpStyle` enum for selecting interpolation methods and
//! the internal `Interp` enum for static dispatch in hot paths.

use super::generic::Interpolator;
use super::strategies::{
    CubicHermiteStrategy, LinearStrategy, LogLinearStrategy, MonotoneConvexStrategy,
    PiecewiseQuadraticForwardStrategy,
};
use super::traits::InterpFn;

/// Epsilon for finite difference derivative calculations.
///
/// This is the default value used in [`InterpFn::interp_prime`] for numerical
/// derivatives.
pub const DERIVATIVE_EPSILON: f64 = 1e-6;

/// Validation policy for interpolator input values.
///
/// Controls whether the interpolator validates that all values are positive
/// (required for discount factors) or allows any finite values (needed for
/// forward rate curves where negative rates are possible).
///
/// # Examples
///
/// ```rust
/// use finstack_core::math::interp::{
///     ExtrapolationPolicy, Interpolator, LinearStrategy, ValidationPolicy,
/// };
///
/// # fn main() -> finstack_core::Result<()> {
/// // Strict validation (default) - requires positive values
/// let strict = Interpolator::<LinearStrategy>::new(
///     vec![0.0, 1.0, 2.0].into_boxed_slice(),
///     vec![1.0, 0.95, 0.90].into_boxed_slice(),
///     ExtrapolationPolicy::FlatZero,
///     ValidationPolicy::Strict,
/// )?;
///
/// // AllowNegative - permits negative values (e.g., for forward curves)
/// let relaxed = Interpolator::<LinearStrategy>::new(
///     vec![0.0, 1.0, 2.0].into_boxed_slice(),
///     vec![-0.01, 0.0, 0.01].into_boxed_slice(),
///     ExtrapolationPolicy::FlatZero,
///     ValidationPolicy::AllowNegative,
/// )?;
/// # Ok(())
/// # }
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ValidationPolicy {
    /// Require all values to be positive (default).
    ///
    /// This is appropriate for discount factors and other quantities that must
    /// remain positive to avoid arbitrage.
    #[default]
    Strict,
    /// Allow any finite values, including negative and zero.
    ///
    /// Use this for forward rate curves where negative rates are allowed
    /// (e.g., EUR, CHF, JPY markets since 2014).
    ///
    /// **Note:** Some interpolation methods (LogLinear, MonotoneConvex) require
    /// positive values for mathematical reasons and will still reject negative
    /// values even with this policy.
    AllowNegative,
}

/// Extrapolation policy for evaluation outside the knot range.
///
/// All [`InterpolationStrategy`](super::traits::InterpolationStrategy)
/// implementations honour this policy at both ends of the knot range.
/// Choose based on what behaviour the downstream pricer expects:
///
/// - `FlatZero` (default): hold the boundary value constant. Safe for
///   discount factors and survival probabilities.
/// - `FlatForward`: extend the boundary slope. Useful for forward-rate
///   curves where extrapolated rates should track the local term-structure
///   slope.
/// - `Nan`: refuse to extrapolate; return `NaN` and require the caller to
///   detect and handle out-of-range queries explicitly. Recommended for
///   production risk systems.
///
/// `#[non_exhaustive]` so that future policies (e.g. linear-zero,
/// asymptotic) can be added without breaking downstream code.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExtrapolationPolicy {
    /// Constant value extension.
    #[default]
    FlatZero,
    /// Tangent/forward extension using boundary slope.
    FlatForward,
    /// Return `NaN` for queries outside the knot range.
    ///
    /// Use this to prevent silent extrapolation in production risk systems
    /// where extrapolated values should be explicitly opted-in.
    /// Callers should check `.is_nan()` on the result and handle accordingly.
    ///
    /// # When to Use
    ///
    /// - Risk management systems where extrapolated values are unreliable
    /// - Validation pipelines where out-of-range queries indicate data errors
    /// - Any context where silent extrapolation could mask problems
    ///
    /// # Note
    ///
    /// Since interpolation returns `f64` (not `Result`), this policy returns
    /// `f64::NAN` rather than an error. This avoids changing the hot-path
    /// signature while still making out-of-range queries detectable.
    None,
}

/// Enum of supported interpolation styles. The default is `Linear`.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InterpStyle {
    /// Linear interpolation in values.
    #[default]
    Linear,
    /// Log‐linear interpolation of positive values.
    LogLinear,
    /// Monotone‐convex interpolation of positive, non-increasing values.
    MonotoneConvex,
    /// Cubic Hermite interpolation (monotone-preserving slopes).
    CubicHermite,
    /// Piecewise quadratic forwards (smooth forward curve, C²).
    PiecewiseQuadraticForward,
}

// ---------------------------------------------------------------------------
// Display + FromStr for ExtrapolationPolicy
// ---------------------------------------------------------------------------

impl std::fmt::Display for ExtrapolationPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            ExtrapolationPolicy::FlatZero => "flat_zero",
            ExtrapolationPolicy::FlatForward => "flat_forward",
            ExtrapolationPolicy::None => "none",
        };
        f.write_str(label)
    }
}

impl crate::parse::NormalizedEnum for ExtrapolationPolicy {
    const VARIANTS: &'static [(&'static str, Self)] = &[
        ("flat_zero", Self::FlatZero),
        ("flatzero", Self::FlatZero),
        ("flat", Self::FlatZero),
        ("flat_forward", Self::FlatForward),
        ("flatforward", Self::FlatForward),
        ("none", Self::None),
    ];
}

impl std::str::FromStr for ExtrapolationPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parse::parse_normalized_enum(s)
    }
}

// ---------------------------------------------------------------------------
// Display + FromStr for InterpStyle
// ---------------------------------------------------------------------------

impl std::fmt::Display for InterpStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            InterpStyle::Linear => "linear",
            InterpStyle::LogLinear => "log_linear",
            InterpStyle::MonotoneConvex => "monotone_convex",
            InterpStyle::CubicHermite => "cubic_hermite",
            InterpStyle::PiecewiseQuadraticForward => "piecewise_quadratic_forward",
        };
        f.write_str(label)
    }
}

impl crate::parse::NormalizedEnum for InterpStyle {
    const VARIANTS: &'static [(&'static str, Self)] = &[
        ("linear", Self::Linear),
        ("log_linear", Self::LogLinear),
        ("loglinear", Self::LogLinear),
        ("flat_fwd", Self::LogLinear),
        ("flat_forward", Self::LogLinear),
        ("monotone_convex", Self::MonotoneConvex),
        ("monotoneconvex", Self::MonotoneConvex),
        ("cubic_hermite", Self::CubicHermite),
        ("cubichermite", Self::CubicHermite),
        (
            "piecewise_quadratic_forward",
            Self::PiecewiseQuadraticForward,
        ),
        ("piecewise_quadratic", Self::PiecewiseQuadraticForward),
        ("pqf", Self::PiecewiseQuadraticForward),
    ];
}

impl std::str::FromStr for InterpStyle {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parse::parse_normalized_enum(s)
    }
}

/// Crate-private enum enabling static dispatch for interpolation in hot loops.
///
/// Storing this enum (instead of `Box<dyn InterpFn>`) allows the compiler to
/// inline calls to `interp` and `interp_prime` for each concrete variant.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Interp {
    Linear(Interpolator<LinearStrategy>),
    LogLinear(Interpolator<LogLinearStrategy>),
    MonotoneConvex(Interpolator<MonotoneConvexStrategy>),
    CubicHermite(Interpolator<CubicHermiteStrategy>),
    PiecewiseQuadraticForward(Interpolator<PiecewiseQuadraticForwardStrategy>),
}

impl Interp {
    #[inline]
    pub(crate) fn interp(&self, x: f64) -> f64 {
        match self {
            Interp::Linear(i) => i.interp(x),
            Interp::LogLinear(i) => i.interp(x),
            Interp::MonotoneConvex(i) => i.interp(x),
            Interp::CubicHermite(i) => i.interp(x),
            Interp::PiecewiseQuadraticForward(i) => i.interp(x),
        }
    }

    /// Get the interpolation style of this Interp
    pub(crate) fn style(&self) -> InterpStyle {
        match self {
            Interp::Linear(_) => InterpStyle::Linear,
            Interp::LogLinear(_) => InterpStyle::LogLinear,
            Interp::MonotoneConvex(_) => InterpStyle::MonotoneConvex,
            Interp::CubicHermite(_) => InterpStyle::CubicHermite,
            Interp::PiecewiseQuadraticForward(_) => InterpStyle::PiecewiseQuadraticForward,
        }
    }

    /// Get the extrapolation policy of this Interp
    pub(crate) fn extrapolation(&self) -> ExtrapolationPolicy {
        match self {
            Interp::Linear(i) => i.extrapolation(),
            Interp::LogLinear(i) => i.extrapolation(),
            Interp::MonotoneConvex(i) => i.extrapolation(),
            Interp::CubicHermite(i) => i.extrapolation(),
            Interp::PiecewiseQuadraticForward(i) => i.extrapolation(),
        }
    }
}

impl InterpStyle {
    /// Return the effective validation policy for this style.
    ///
    /// LogLinear and MonotoneConvex always require strict (positive) values
    /// for mathematical reasons, regardless of what the caller requested.
    #[inline]
    fn effective_validation(self, validation: ValidationPolicy) -> ValidationPolicy {
        match self {
            InterpStyle::LogLinear | InterpStyle::MonotoneConvex => ValidationPolicy::Strict,
            _ => validation,
        }
    }

    /// Build a boxed interpolator implementing [`InterpFn`].
    ///
    /// # Performance Note
    ///
    /// This method returns `Box<dyn InterpFn>` which incurs heap allocation and
    /// dynamic dispatch overhead. For hot paths (e.g., curve evaluation in pricing
    /// loops), prefer using the concrete interpolator types directly:
    ///
    /// ```rust
    /// use finstack_core::math::interp::{MonotoneConvex, ExtrapolationPolicy, ValidationPolicy};
    ///
    /// # fn main() -> finstack_core::Result<()> {
    /// // Preferred: direct construction (no heap allocation, static dispatch)
    /// let interp = Interpolator::<MonotoneConvexStrategy>::new(
    ///     vec![0.0, 1.0, 2.0].into_boxed_slice(),
    ///     vec![1.0, 0.95, 0.90].into_boxed_slice(),
    ///     ExtrapolationPolicy::FlatZero,
    ///     ValidationPolicy::Strict,
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    /// * `knots` – strictly ascending knot times.
    /// * `values` – corresponding values.
    /// * `extrapolation` – extrapolation policy.
    /// * `validation` – validation policy controlling whether negative values are allowed.
    pub fn build(
        self,
        knots: Box<[f64]>,
        values: Box<[f64]>,
        extrapolation: ExtrapolationPolicy,
        validation: ValidationPolicy,
    ) -> crate::Result<Box<dyn InterpFn>> {
        let effective_validation = self.effective_validation(validation);
        match self {
            InterpStyle::Linear => Ok(Box::new(Interpolator::<LinearStrategy>::new(
                knots,
                values,
                extrapolation,
                effective_validation,
            )?)),
            InterpStyle::LogLinear => Ok(Box::new(Interpolator::<LogLinearStrategy>::new(
                knots,
                values,
                extrapolation,
                effective_validation,
            )?)),
            InterpStyle::MonotoneConvex => {
                Ok(Box::new(Interpolator::<MonotoneConvexStrategy>::new(
                    knots,
                    values,
                    extrapolation,
                    effective_validation,
                )?))
            }
            InterpStyle::CubicHermite => Ok(Box::new(Interpolator::<CubicHermiteStrategy>::new(
                knots,
                values,
                extrapolation,
                effective_validation,
            )?)),
            InterpStyle::PiecewiseQuadraticForward => Ok(Box::new(Interpolator::<
                PiecewiseQuadraticForwardStrategy,
            >::new(
                knots,
                values,
                extrapolation,
                effective_validation,
            )?)),
        }
    }

    /// Build an enum-backed interpolator enabling static dispatch.
    ///
    /// # Arguments
    /// * `knots` – strictly ascending knot times.
    /// * `values` – corresponding values.
    /// * `extrapolation` – extrapolation policy.
    /// * `validation` – validation policy controlling whether negative values are allowed.
    ///
    /// **Note:** LogLinear and MonotoneConvex require positive values for mathematical
    /// reasons (log transform, monotonicity). These will enforce `Strict` validation
    /// regardless of the passed policy.
    #[inline]
    pub(crate) fn build_enum(
        self,
        knots: Box<[f64]>,
        values: Box<[f64]>,
        extrapolation: ExtrapolationPolicy,
        validation: ValidationPolicy,
    ) -> crate::Result<Interp> {
        let effective_validation = self.effective_validation(validation);
        let interp = match self {
            InterpStyle::Linear => Interp::Linear(Interpolator::<LinearStrategy>::new(
                knots,
                values,
                extrapolation,
                effective_validation,
            )?),
            InterpStyle::LogLinear => Interp::LogLinear(Interpolator::<LogLinearStrategy>::new(
                knots,
                values,
                extrapolation,
                effective_validation,
            )?),
            InterpStyle::MonotoneConvex => {
                Interp::MonotoneConvex(Interpolator::<MonotoneConvexStrategy>::new(
                    knots,
                    values,
                    extrapolation,
                    effective_validation,
                )?)
            }
            InterpStyle::CubicHermite => {
                Interp::CubicHermite(Interpolator::<CubicHermiteStrategy>::new(
                    knots,
                    values,
                    extrapolation,
                    effective_validation,
                )?)
            }
            InterpStyle::PiecewiseQuadraticForward => Interp::PiecewiseQuadraticForward(
                Interpolator::<PiecewiseQuadraticForwardStrategy>::new(
                    knots,
                    values,
                    extrapolation,
                    effective_validation,
                )?,
            ),
        };
        Ok(interp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_interp_style(label: &str, expected: InterpStyle) {
        assert!(matches!(label.parse::<InterpStyle>(), Ok(value) if value == expected));
    }

    fn assert_extrapolation_policy(label: &str, expected: ExtrapolationPolicy) {
        assert!(matches!(label.parse::<ExtrapolationPolicy>(), Ok(value) if value == expected));
    }

    fn standard_knots() -> Box<[f64]> {
        vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice()
    }

    fn standard_dfs() -> Box<[f64]> {
        vec![1.0, 0.95, 0.9, 0.85].into_boxed_slice()
    }

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    // ========================================================================
    // build_enum Tests with ValidationPolicy
    // ========================================================================

    #[test]
    fn build_enum_linear_strict() {
        let result = InterpStyle::Linear.build_enum(
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatZero,
            ValidationPolicy::Strict,
        );
        assert!(result.is_ok());
        let interp = result.expect("should build in test");
        assert!(approx_eq(interp.interp(0.0), 1.0, 1e-12));
    }

    #[test]
    fn build_enum_log_linear_strict() {
        let result = InterpStyle::LogLinear.build_enum(
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatForward,
            ValidationPolicy::Strict,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn build_enum_monotone_convex_strict() {
        let result = InterpStyle::MonotoneConvex.build_enum(
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatZero,
            ValidationPolicy::Strict,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn build_enum_cubic_hermite_strict() {
        let result = InterpStyle::CubicHermite.build_enum(
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatForward,
            ValidationPolicy::Strict,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn build_enum_piecewise_quadratic_strict() {
        let result = InterpStyle::PiecewiseQuadraticForward.build_enum(
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatZero,
            ValidationPolicy::Strict,
        );
        assert!(result.is_ok());
    }

    // ========================================================================
    // ValidationPolicy::AllowNegative Tests
    // ========================================================================

    #[test]
    fn allow_negative_linear_with_negative_values() {
        let knots = vec![0.0, 1.0, 2.0].into_boxed_slice();
        let values = vec![-0.5, -0.2, 0.1].into_boxed_slice();
        let result = InterpStyle::Linear.build_enum(
            knots,
            values,
            ExtrapolationPolicy::FlatZero,
            ValidationPolicy::AllowNegative,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn allow_negative_cubic_with_negative_values() {
        let knots = vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice();
        let values = vec![-1.0, -0.5, 0.0, 0.5].into_boxed_slice();
        let result = InterpStyle::CubicHermite.build_enum(
            knots,
            values,
            ExtrapolationPolicy::FlatForward,
            ValidationPolicy::AllowNegative,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn log_linear_rejects_negative_even_with_allow_negative_policy() {
        let knots = vec![0.0, 1.0, 2.0].into_boxed_slice();
        let values = vec![-0.5, 0.2, 0.5].into_boxed_slice();
        let result = InterpStyle::LogLinear.build_enum(
            knots,
            values,
            ExtrapolationPolicy::FlatZero,
            ValidationPolicy::AllowNegative, // Should still enforce Strict for LogLinear
        );
        // LogLinear requires positive values for log transform
        assert!(result.is_err());
    }

    #[test]
    fn monotone_convex_rejects_negative_even_with_allow_negative_policy() {
        let knots = vec![0.0, 1.0, 2.0].into_boxed_slice();
        let values = vec![-1.0, -0.5, -0.2].into_boxed_slice();
        let result = InterpStyle::MonotoneConvex.build_enum(
            knots,
            values,
            ExtrapolationPolicy::FlatZero,
            ValidationPolicy::AllowNegative, // Should still enforce Strict for MonotoneConvex
        );
        assert!(result.is_err());
    }

    #[test]
    fn strict_rejects_negative_for_linear() {
        let knots = vec![0.0, 1.0, 2.0].into_boxed_slice();
        let values = vec![-0.5, -0.2, 0.1].into_boxed_slice();
        let result = InterpStyle::Linear.build_enum(
            knots,
            values,
            ExtrapolationPolicy::FlatZero,
            ValidationPolicy::Strict,
        );
        assert!(result.is_err());
    }

    // ========================================================================
    // Interp Enum Method Tests
    // ========================================================================

    #[test]
    fn interp_method_all_variants() {
        let styles = vec![
            InterpStyle::Linear,
            InterpStyle::LogLinear,
            InterpStyle::MonotoneConvex,
            InterpStyle::CubicHermite,
            InterpStyle::PiecewiseQuadraticForward,
        ];

        for style in styles {
            let interp = style
                .build_enum(
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatZero,
                    ValidationPolicy::Strict,
                )
                .expect("should build in test");
            assert!(approx_eq(interp.interp(0.0), 1.0, 1e-10));
        }
    }

    #[test]
    fn style_method_returns_correct_type() {
        let linear = InterpStyle::Linear
            .build_enum(
                standard_knots(),
                standard_dfs(),
                ExtrapolationPolicy::FlatZero,
                ValidationPolicy::Strict,
            )
            .expect("should build in test");
        assert_eq!(linear.style(), InterpStyle::Linear);

        let log_linear = InterpStyle::LogLinear
            .build_enum(
                standard_knots(),
                standard_dfs(),
                ExtrapolationPolicy::FlatZero,
                ValidationPolicy::Strict,
            )
            .expect("should build in test");
        assert_eq!(log_linear.style(), InterpStyle::LogLinear);
    }

    #[test]
    fn extrapolation_method_works() {
        let flat_zero = InterpStyle::Linear
            .build_enum(
                standard_knots(),
                standard_dfs(),
                ExtrapolationPolicy::FlatZero,
                ValidationPolicy::Strict,
            )
            .expect("should build in test");
        let _ = flat_zero.extrapolation();

        let flat_fwd = InterpStyle::LogLinear
            .build_enum(
                standard_knots(),
                standard_dfs(),
                ExtrapolationPolicy::FlatForward,
                ValidationPolicy::Strict,
            )
            .expect("should build in test");
        let _ = flat_fwd.extrapolation();
    }

    // ========================================================================
    // ExtrapolationPolicy Tests
    // ========================================================================

    #[test]
    fn extrapolation_default_is_flat_zero() {
        let default = ExtrapolationPolicy::default();
        match default {
            ExtrapolationPolicy::FlatZero => {}
            _ => panic!("Default should be FlatZero"),
        }
    }

    // ========================================================================
    // ValidationPolicy Tests
    // ========================================================================

    #[test]
    fn validation_policy_default_is_strict() {
        let default = ValidationPolicy::default();
        assert_eq!(default, ValidationPolicy::Strict);
    }

    #[test]
    fn validation_policy_equality() {
        assert_eq!(ValidationPolicy::Strict, ValidationPolicy::Strict);
        assert_eq!(
            ValidationPolicy::AllowNegative,
            ValidationPolicy::AllowNegative
        );
        assert_ne!(ValidationPolicy::Strict, ValidationPolicy::AllowNegative);
    }

    // ========================================================================
    // DERIVATIVE_EPSILON Tests
    // ========================================================================

    #[test]
    fn derivative_epsilon_defined() {
        assert_eq!(DERIVATIVE_EPSILON, 1e-6);
    }

    #[test]
    fn derivative_epsilon_usage() {
        let epsilon = DERIVATIVE_EPSILON;
        let f = |x: f64| x * x;
        let x = 2.0;
        let true_deriv = 2.0 * x;
        let approx_deriv = (f(x + epsilon) - f(x)) / epsilon;
        assert!((approx_deriv - true_deriv).abs() < 1e-5);
    }

    // ========================================================================
    // InterpStyle PartialEq Tests
    // ========================================================================

    #[test]
    fn interp_style_equality() {
        assert_eq!(InterpStyle::Linear, InterpStyle::Linear);
        assert_eq!(InterpStyle::LogLinear, InterpStyle::LogLinear);
    }

    #[test]
    fn interp_style_inequality() {
        assert_ne!(InterpStyle::Linear, InterpStyle::LogLinear);
        assert_ne!(InterpStyle::LogLinear, InterpStyle::MonotoneConvex);
    }

    // ========================================================================
    // InterpStyle FromStr / Display roundtrip tests
    // ========================================================================

    #[test]
    fn interp_style_display_roundtrip() {
        let all = [
            InterpStyle::Linear,
            InterpStyle::LogLinear,
            InterpStyle::MonotoneConvex,
            InterpStyle::CubicHermite,
            InterpStyle::PiecewiseQuadraticForward,
        ];

        for style in &all {
            let label = style.to_string();
            assert!(
                matches!(label.parse::<InterpStyle>(), Ok(value) if value == *style),
                "roundtrip failed for {label}"
            );
        }
    }

    #[test]
    fn interp_style_from_str_aliases() {
        assert_interp_style("loglinear", InterpStyle::LogLinear);
        assert_interp_style("flat_fwd", InterpStyle::LogLinear);
        assert_interp_style("flat_forward", InterpStyle::LogLinear);
        assert_interp_style("monotoneconvex", InterpStyle::MonotoneConvex);
        assert_interp_style("cubichermite", InterpStyle::CubicHermite);
        assert_interp_style("pqf", InterpStyle::PiecewiseQuadraticForward);
        assert_interp_style(
            "piecewise_quadratic",
            InterpStyle::PiecewiseQuadraticForward,
        );
    }

    #[test]
    fn interp_style_from_str_unknown() {
        assert!("garbage".parse::<InterpStyle>().is_err());
    }

    // ========================================================================
    // ExtrapolationPolicy FromStr / Display roundtrip tests
    // ========================================================================

    #[test]
    fn extrap_policy_display_roundtrip() {
        let all = [
            ExtrapolationPolicy::FlatZero,
            ExtrapolationPolicy::FlatForward,
            ExtrapolationPolicy::None,
        ];

        for policy in &all {
            let label = policy.to_string();
            assert!(
                matches!(label.parse::<ExtrapolationPolicy>(), Ok(value) if value == *policy),
                "roundtrip failed for {label}"
            );
        }
    }

    #[test]
    fn extrap_policy_from_str_aliases() {
        assert_extrapolation_policy("flatzero", ExtrapolationPolicy::FlatZero);
        assert_extrapolation_policy("flat", ExtrapolationPolicy::FlatZero);
        assert_extrapolation_policy("flatforward", ExtrapolationPolicy::FlatForward);
    }

    #[test]
    fn extrap_policy_from_str_unknown() {
        assert!("garbage".parse::<ExtrapolationPolicy>().is_err());
    }
}
