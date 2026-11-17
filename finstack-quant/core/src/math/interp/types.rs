//! Interpolation types and style enumeration.
//!
//! Defines the `InterpStyle` enum for selecting interpolation methods and
//! the internal `Interp` enum for static dispatch in hot paths.

use super::generic::Interpolator;
use super::strategies::{
    CubicHermiteStrategy, LinearStrategy, LogLinearStrategy, MonotoneConvexStrategy,
    PiecewiseQuadraticForwardStrategy,
};

/// Extrapolation policy for evaluation outside the knot range.
///
/// Every interpolation strategy honours this policy at both ends of the knot
/// range. Callers must choose an explicit policy:
///
/// - `Flat`: hold the boundary ordinate constant outside the knot range
///   (and return derivative `0.0` strictly outside the knot range).
/// - `Extend`: extend the boundary segment's slope in the interpolation style's
///   working space (table below).
/// - `Error`: refuse to extrapolate; return an explicit error for queries
///   outside `[knots[0], knots[last]]`.
///
/// What `Extend` extends depends on the [`InterpStyle`]:
///
/// | Style | `Extend` extends | On a discount-factor curve | On a rate-valued curve |
/// |---|---|---|---|
/// | `Linear` | the value linearly | - | rates keep trending |
/// | `CubicHermite` | the value linearly, with the end-knot slope | - | rates keep trending |
/// | `LogLinear` | `ln(value)` linearly | flat forward rate | - |
/// | `PiecewiseQuadraticForward` | `ln(value)` linearly | flat forward rate | - |
/// | `MonotoneConvex` | the boundary instantaneous forward | flat forward rate | - |
///
/// The market-standard tail for a discount-factor curve is a flat forward
/// rate (`Extend` with a log-space style). For a curve that stores rates or
/// spreads, holding the last rate constant is `Flat`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExtrapolationPolicy {
    /// Hold the boundary value constant outside `[knots[0], knots[last]]`.
    Flat,
    /// Extend the boundary segment's slope in the interpolation style's
    /// working space; see the table on [`ExtrapolationPolicy`].
    Extend,
    /// Return an error for queries outside `[knots[0], knots[last]]`.
    Error,
}

/// Enum of supported interpolation styles. The default is `Linear`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InterpStyle {
    /// Linear interpolation in values.
    #[default]
    Linear,
    /// Log‐linear interpolation of positive values.
    LogLinear,
    /// Hagan‐West monotone‐convex interpolation of positive values.
    MonotoneConvex,
    /// Cubic Hermite interpolation (Fritsch‐Butland / Moler PCHIP slopes).
    CubicHermite,
    /// Piecewise quadratic forwards (C¹ forwards and C² log discount factors).
    PiecewiseQuadraticForward,
}

// Display / FromStr are the serde names (`rename_all = "snake_case"`).
impl std::fmt::Display for ExtrapolationPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl std::str::FromStr for ExtrapolationPolicy {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::wire::serde_parse(s)
    }
}

impl std::fmt::Display for InterpStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl std::str::FromStr for InterpStyle {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::wire::serde_parse(s)
    }
}

/// Crate-private enum enabling static dispatch for interpolation in hot loops.
///
/// Storing this enum (instead of a boxed trait object) allows the compiler to
/// inline calls to `interp` for each concrete variant.
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
    pub(crate) fn interp(&self, x: f64) -> crate::Result<f64> {
        match self {
            Interp::Linear(i) => i.interp(x),
            Interp::LogLinear(i) => i.interp(x),
            Interp::MonotoneConvex(i) => i.interp(x),
            Interp::CubicHermite(i) => i.interp(x),
            Interp::PiecewiseQuadraticForward(i) => i.interp(x),
        }
    }

    #[inline]
    pub(crate) fn interp_prime(&self, x: f64) -> crate::Result<f64> {
        match self {
            Interp::Linear(i) => i.interp_prime(x),
            Interp::LogLinear(i) => i.interp_prime(x),
            Interp::MonotoneConvex(i) => i.interp_prime(x),
            Interp::CubicHermite(i) => i.interp_prime(x),
            Interp::PiecewiseQuadraticForward(i) => i.interp_prime(x),
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
    /// Build an enum-backed interpolator enabling static dispatch.
    ///
    /// # Arguments
    /// * `knots` – strictly ascending finite knot coordinates.
    /// * `values` – corresponding finite values (positive for `LogLinear`,
    ///   `MonotoneConvex`, and `PiecewiseQuadraticForward`).
    /// * `extrapolation` – extrapolation policy applied outside the knot domain.
    #[inline]
    pub(crate) fn build_enum(
        self,
        knots: Box<[f64]>,
        values: Box<[f64]>,
        extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Interp> {
        let interp = match self {
            InterpStyle::Linear => Interp::Linear(Interpolator::<LinearStrategy>::new(
                knots,
                values,
                extrapolation,
            )?),
            InterpStyle::LogLinear => Interp::LogLinear(Interpolator::<LogLinearStrategy>::new(
                knots,
                values,
                extrapolation,
            )?),
            InterpStyle::MonotoneConvex => Interp::MonotoneConvex(Interpolator::<
                MonotoneConvexStrategy,
            >::new(
                knots, values, extrapolation
            )?),
            InterpStyle::CubicHermite => Interp::CubicHermite(
                Interpolator::<CubicHermiteStrategy>::new(knots, values, extrapolation)?,
            ),
            InterpStyle::PiecewiseQuadraticForward => Interp::PiecewiseQuadraticForward(
                Interpolator::<PiecewiseQuadraticForwardStrategy>::new(
                    knots,
                    values,
                    extrapolation,
                )?,
            ),
        };
        Ok(interp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standard_knots() -> Box<[f64]> {
        vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice()
    }

    fn standard_dfs() -> Box<[f64]> {
        vec![1.0, 0.95, 0.9, 0.85].into_boxed_slice()
    }

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn build_enum_linear() {
        let result = InterpStyle::Linear.build_enum(
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat,
        );
        assert!(result.is_ok());
        let interp = result.expect("should build in test");
        assert!(approx_eq(interp.interp(0.0).expect("knot hit"), 1.0, 1e-12));
    }

    #[test]
    fn build_enum_log_linear() {
        let result = InterpStyle::LogLinear.build_enum(
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Extend,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn build_enum_monotone_convex() {
        let result = InterpStyle::MonotoneConvex.build_enum(
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn build_enum_cubic_hermite() {
        let result = InterpStyle::CubicHermite.build_enum(
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Extend,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn build_enum_piecewise_quadratic() {
        let result = InterpStyle::PiecewiseQuadraticForward.build_enum(
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn signed_linear_and_cubic_accept_negative_values() {
        let linear = InterpStyle::Linear.build_enum(
            vec![0.0, 1.0, 2.0].into_boxed_slice(),
            vec![-0.5, -0.2, 0.1].into_boxed_slice(),
            ExtrapolationPolicy::Flat,
        );
        assert!(linear.is_ok());

        let cubic = InterpStyle::CubicHermite.build_enum(
            vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice(),
            vec![-1.0, -0.5, 0.0, 0.5].into_boxed_slice(),
            ExtrapolationPolicy::Extend,
        );
        assert!(cubic.is_ok());
    }

    #[test]
    fn positive_strategies_reject_non_positive_values() {
        let knots = vec![0.0, 1.0, 2.0].into_boxed_slice();
        let values = vec![-0.5, 0.2, 0.5].into_boxed_slice();
        for style in [
            InterpStyle::LogLinear,
            InterpStyle::MonotoneConvex,
            InterpStyle::PiecewiseQuadraticForward,
        ] {
            assert!(
                style
                    .build_enum(knots.clone(), values.clone(), ExtrapolationPolicy::Flat)
                    .is_err(),
                "{style} must reject negative values"
            );
        }
    }

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
                .build_enum(standard_knots(), standard_dfs(), ExtrapolationPolicy::Flat)
                .expect("should build in test");
            assert_eq!(interp.interp(0.0).expect("knot hit"), 1.0);
        }
    }

    #[test]
    fn style_method_returns_correct_type() {
        let linear = InterpStyle::Linear
            .build_enum(standard_knots(), standard_dfs(), ExtrapolationPolicy::Flat)
            .expect("should build in test");
        assert_eq!(linear.style(), InterpStyle::Linear);

        let log_linear = InterpStyle::LogLinear
            .build_enum(standard_knots(), standard_dfs(), ExtrapolationPolicy::Flat)
            .expect("should build in test");
        assert_eq!(log_linear.style(), InterpStyle::LogLinear);
    }

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
    fn interp_style_from_str_rejects_noncanonical_spellings() {
        for rejected in [
            "loglinear",
            "flat_fwd",
            "flat_forward",
            "monotoneconvex",
            "cubichermite",
            "pqf",
            "piecewise_quadratic",
            "Linear",
        ] {
            assert!(rejected.parse::<InterpStyle>().is_err());
        }
    }

    #[test]
    fn interp_style_from_str_unknown() {
        assert!("garbage".parse::<InterpStyle>().is_err());
    }

    #[test]
    fn extrap_policy_display_roundtrip() {
        let all = [
            ExtrapolationPolicy::Flat,
            ExtrapolationPolicy::Extend,
            ExtrapolationPolicy::Error,
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
    fn extrap_policy_from_str_rejects_legacy_and_noncanonical_spellings() {
        for rejected in [
            "flat_zero",
            "flat_forward",
            "none",
            "flatzero",
            "flatforward",
            "Flat",
            "Extend",
            "Error",
        ] {
            assert!(
                rejected.parse::<ExtrapolationPolicy>().is_err(),
                "{rejected} must be rejected"
            );
        }
    }

    #[test]
    fn extrap_policy_from_str_unknown() {
        assert!("garbage".parse::<ExtrapolationPolicy>().is_err());
    }
}
