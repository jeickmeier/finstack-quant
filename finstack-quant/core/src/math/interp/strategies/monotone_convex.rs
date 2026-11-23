//! Hagan-West monotone convex interpolation strategy.

use super::super::{
    traits::InterpolationStrategy,
    types::ExtrapolationPolicy,
    validation::{
        BoundaryDecision, classify_prime_query, classify_value_query, exact_f64_eq,
        validate_positive_series,
    },
};
use super::numerics::*;

/// Strategy for Hagan-West (2006/2008) monotone-convex discount-factor interpolation.
///
/// Constructs segment-average forwards `f^d_i = -ln(D[i+1] / D[i]) / (t[i+1] - t[i])`,
/// estimates uncollared nodal instantaneous forwards, and evaluates the exact regional
/// zero-integral excess forward polynomial within each interval. On two-point grids,
/// evaluates constant interval forward `f^d_0` (identical to [`LogLinearStrategy`](crate::math::interp::LogLinearStrategy)).
///
/// Positivity of discount factors is validated at construction; no nodal positivity
/// projection or small-deviation Region I override is applied.
///
/// # References
///
/// Hagan, P. S., & West, G. (2006). "Interpolation Methods for Curve Construction."
/// *Applied Mathematical Finance*, 13(2), 89-129. `docs/REFERENCES.md#hagan-west-monotone-convex`
#[derive(Debug, Clone)]
pub struct MonotoneConvexStrategy {
    /// Discrete forward rates f^d_i for each segment (length n-1).
    fd: Box<[f64]>,
    /// Instantaneous forward rates f_i at each knot (length n).
    f: Box<[f64]>,
    /// Segment widths (knots[i+1] - knots[i]).
    dt: Box<[f64]>,
}

impl InterpolationStrategy for MonotoneConvexStrategy {
    fn from_raw(
        knots: &[f64],
        values: &[f64],
        _extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Self> {
        validate_positive_series(values)?;
        Self::build_hagan_west(knots, values)
    }

    fn interp(
        &self,
        x: f64,
        knots: &[f64],
        values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<f64> {
        match classify_value_query(x, knots, values, extrapolation)? {
            BoundaryDecision::Return(val) => Ok(val),
            BoundaryDecision::ExtendLeft => {
                let f0 = self.left_boundary_forward();
                exp_extrapolate(
                    x,
                    knots[0],
                    values[0],
                    -f0,
                    "monotone convex left extrapolation",
                )
            }
            BoundaryDecision::ExtendRight => {
                let last = knots.len() - 1;
                let f_last = self.right_boundary_forward();
                exp_extrapolate(
                    x,
                    knots[last],
                    values[last],
                    -f_last,
                    "monotone convex right extrapolation",
                )
            }
            BoundaryDecision::Interior(i) => {
                let (df, _) = self.segment_df_and_forward(i, x, knots, values)?;
                Ok(df)
            }
        }
    }

    fn interp_prime(
        &self,
        x: f64,
        knots: &[f64],
        values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<f64> {
        match classify_prime_query(x, knots, extrapolation)? {
            BoundaryDecision::Return(val) => Ok(val),
            BoundaryDecision::ExtendLeft => {
                let f0 = self.left_boundary_forward();
                let df = exp_extrapolate(
                    x,
                    knots[0],
                    values[0],
                    -f0,
                    "monotone convex left extrapolation",
                )?;
                checked_finite(-f0 * df, "monotone convex left derivative")
            }
            BoundaryDecision::ExtendRight => {
                let last = knots.len() - 1;
                let f_last = self.right_boundary_forward();
                let df = exp_extrapolate(
                    x,
                    knots[last],
                    values[last],
                    -f_last,
                    "monotone convex right extrapolation",
                )?;
                checked_finite(-f_last * df, "monotone convex right derivative")
            }
            BoundaryDecision::Interior(i) => {
                let (df, fwd) = self.segment_df_and_forward(i, x, knots, values)?;
                checked_finite(-fwd * df, "monotone convex derivative")
            }
        }
    }
}

impl MonotoneConvexStrategy {
    fn build_hagan_west(knots: &[f64], dfs: &[f64]) -> crate::Result<Self> {
        let n = knots.len();
        let (dt, fd) = discrete_forwards(
            knots,
            dfs,
            "monotone convex",
            "monotone convex discrete forward",
        )?;

        let mut f = vec![0.0; n];
        if n == 2 {
            f[0] = fd[0];
            f[1] = fd[0];
        } else {
            for i in 1..n - 1 {
                let l_l = dt[i - 1];
                let l_r = dt[i];
                let scale = l_l.max(l_r);
                let hl = l_l / scale;
                let hr = l_r / scale;
                f[i] = checked_finite(
                    (hl * fd[i] + hr * fd[i - 1]) / (hl + hr),
                    "monotone convex interior forward",
                )?;
            }

            f[0] = checked_finite(1.5 * fd[0] - 0.5 * f[1], "monotone convex left forward")?;
            f[n - 1] = checked_finite(
                1.5 * fd[n - 2] - 0.5 * f[n - 2],
                "monotone convex right forward",
            )?;
        }

        Ok(Self {
            fd: fd.into_boxed_slice(),
            f: f.into_boxed_slice(),
            dt: dt.into_boxed_slice(),
        })
    }

    #[inline]
    fn left_boundary_forward(&self) -> f64 {
        let fd0 = self.fd[0];
        let g_left = self.f[0] - fd0;
        let g_right = self.f[1] - fd0;
        fd0 + Self::forward_adjustment(0.0, g_left, g_right).0
    }

    #[inline]
    fn right_boundary_forward(&self) -> f64 {
        let last_seg = self.fd.len() - 1;
        let fd_last = self.fd[last_seg];
        let g_left = self.f[last_seg] - fd_last;
        let g_right = self.f[last_seg + 1] - fd_last;
        fd_last + Self::forward_adjustment(1.0, g_left, g_right).0
    }

    fn segment_df_and_forward(
        &self,
        i: usize,
        t: f64,
        knots: &[f64],
        values: &[f64],
    ) -> crate::Result<(f64, f64)> {
        let dt_seg = self.dt[i];
        let u = (t - knots[i]) / dt_seg;

        let fd_i = self.fd[i];
        let g_left = self.f[i] - fd_i;
        let g_right = self.f[i + 1] - fd_i;

        let (g_x, g_integral) = Self::forward_adjustment(u, g_left, g_right);
        let forward = checked_finite(fd_i + g_x, "monotone convex instantaneous forward")?;

        let df = if exact_f64_eq(t, knots[i]) {
            values[i]
        } else if exact_f64_eq(t, knots[i + 1]) {
            values[i + 1]
        } else {
            let integral = fd_i * (t - knots[i]) + dt_seg * g_integral;
            eval_scaled_exp(values[i], -integral, "monotone convex discount factor")?
        };

        Ok((df, forward))
    }

    /// Hagan-West (2008), equations 27-34: `(g(u), integral_0^u g)`.
    /// Exact zero deviation axes return `(0.0, 0.0)` everywhere, including endpoints,
    /// without any epsilon-override Region I substitution.
    fn forward_adjustment(x: f64, g0: f64, g1: f64) -> (f64, f64) {
        if g0 == 0.0 || g1 == 0.0 {
            return (0.0, 0.0);
        }
        if x <= 0.0 {
            return (g0, 0.0);
        }
        if x >= 1.0 {
            return (g1, 0.0);
        }
        let scale = g0.abs().max(g1.abs());
        let g0n = g0 / scale;
        let g1n = g1 / scale;
        let opposite = (g0n < 0.0 && g1n > 0.0) || (g0n > 0.0 && g1n < 0.0);
        if opposite && g1n.abs() >= 0.5 * g0n.abs() && g1n.abs() <= 2.0 * g0n.abs() {
            // Region I.
            let g = g0 * (1.0 - 4.0 * x + 3.0 * x * x) + g1 * (-2.0 * x + 3.0 * x * x);
            let integral = x * (1.0 - x) * (g0 * (1.0 - x) - g1 * x);
            return (g, integral);
        }
        if opposite && g1n.abs() > 2.0 * g0n.abs() {
            // Region II: constant left section followed by a quadratic.
            let width = -3.0 * g0n / (g1n - g0n);
            let eta = 1.0 - width;
            let dx = (x - eta).max(0.0);
            let u = dx / width;
            return (
                g0 + (g1 - g0) * u * u,
                g0 * x + (g1 - g0) * dx * u * u / 3.0,
            );
        }
        if opposite {
            // Region III is the time reversal of region II.
            let eta = 3.0 * g1n / (g1n - g0n);
            let dx = (eta - x).max(0.0);
            let u = dx / eta;
            return (
                g1 + (g0 - g1) * u * u,
                -g1 * (1.0 - x) - (g0 - g1) * dx * u * u / 3.0,
            );
        }
        // Region IV: two quadratics meet at their common extremum.
        let sum_n = g0n + g1n;
        let left_width = g1n / sum_n;
        let right_width = g0n / sum_n;
        let a = -g0 * left_width;
        if x <= left_width {
            let u = x / left_width;
            (
                a + (g0 - a) * (1.0 - u) * (1.0 - u),
                a * x + (g0 - a) * x * (1.0 - u + u * u / 3.0),
            )
        } else {
            let dx = 1.0 - x;
            let u = dx / right_width;
            (
                a + (g1 - a) * (1.0 - u) * (1.0 - u),
                -a * dx - (g1 - a) * dx * (1.0 - u + u * u / 3.0),
            )
        }
    }
}
