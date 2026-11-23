//! Piecewise quadratic forward interpolation strategy.

use super::super::{
    traits::InterpolationStrategy,
    types::ExtrapolationPolicy,
    utils::{
        BoundaryDecision, classify_prime_query, classify_value_query, exact_f64_eq,
        validate_positive_series,
    },
};
use super::numerics::*;
use std::vec::Vec;

/// Strategy for piecewise quadratic forward interpolation (natural cubic spline
/// in log-discount space with C¹ instantaneous forwards).
///
/// Builds a natural cubic spline (`y''(t_0) = y''(t_last) = 0`) in `-ln(DF)` space
/// so the resulting instantaneous forward curve is piecewise quadratic and C¹-continuous
/// across knots. Negative forwards are permitted without positivity clamping, and a
/// two-point curve reduces exactly to [`LogLinearStrategy`](crate::math::interp::LogLinearStrategy).
///
/// # References
///
/// - de Boor, C. *A Practical Guide to Splines*. Canonical reference for the
///   underlying natural cubic spline construction. See
///   [De Boor Splines](../../../../docs/REFERENCES.md#de-boor-splines).
#[derive(Debug, Clone)]
pub struct PiecewiseQuadraticForwardStrategy {
    /// Cubic spline polynomial coefficients for `y(t) - y_i = b s + c s² + d s³`.
    b: Box<[f64]>,
    c: Box<[f64]>,
    d: Box<[f64]>,
}

impl InterpolationStrategy for PiecewiseQuadraticForwardStrategy {
    fn from_raw(
        knots: &[f64],
        values: &[f64],
        _extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Self> {
        validate_positive_series(values)?;

        let n = knots.len();
        let (h, q) = discrete_forwards(
            knots,
            values,
            "piecewise quadratic forward",
            "piecewise quadratic forward interval forward",
        )?;

        if n == 2 {
            return Ok(Self {
                b: vec![q[0]].into_boxed_slice(),
                c: vec![0.0].into_boxed_slice(),
                d: vec![0.0].into_boxed_slice(),
            });
        }

        // Row-scaled natural cubic spline solve (M_0 = M_{n-1} = 0):
        // Dividing interior row i by (h[i-1] + h[i]) makes subdiagonal + superdiagonal == 1.0
        // and the diagonal == 2.0 regardless of coordinate units.
        let mut mu = vec![0.0; n];
        let mut z = vec![0.0; n];
        for i in 1..n - 1 {
            let h_l = h[i - 1];
            let h_r = h[i];
            let scale = h_l.abs().max(h_r.abs());
            let hl_n = h_l / scale;
            let hr_n = h_r / scale;
            let denom = hl_n + hr_n;
            let sub = hl_n / denom;
            let sup = hr_n / denom;
            let rhs = 6.0 * (q[i] - q[i - 1]) / scale / denom;
            let diag = 2.0 - sub * mu[i - 1];
            if !diag.is_finite() || diag <= 0.0 {
                return Err(crate::Error::Validation(
                    "cubic spline tridiagonal system is numerically singular".to_string(),
                ));
            }
            mu[i] = sup / diag;
            z[i] = (rhs - sub * z[i - 1]) / diag;
            if !mu[i].is_finite() || !z[i].is_finite() {
                return Err(crate::Error::Validation(
                    "cubic spline tridiagonal system produced non-finite state".to_string(),
                ));
            }
        }

        let mut m = vec![0.0; n];
        for j in (0..n - 1).rev() {
            m[j] = checked_finite(z[j] - mu[j] * m[j + 1], "cubic spline second derivative")?;
        }

        let mut b_coeff = Vec::with_capacity(n - 1);
        let mut c_coeff = Vec::with_capacity(n - 1);
        let mut d_coeff = Vec::with_capacity(n - 1);
        for i in 0..n - 1 {
            let hi = h[i];
            let bi = checked_finite(
                q[i] - hi * (2.0 * m[i] + m[i + 1]) / 6.0,
                "cubic spline linear coefficient",
            )?;
            let ci = checked_finite(m[i] / 2.0, "cubic spline quadratic coefficient")?;
            let di = checked_finite(
                (m[i + 1] - m[i]) / (6.0 * hi),
                "cubic spline cubic coefficient",
            )?;
            b_coeff.push(bi);
            c_coeff.push(ci);
            d_coeff.push(di);
        }

        Ok(Self {
            b: b_coeff.into_boxed_slice(),
            c: c_coeff.into_boxed_slice(),
            d: d_coeff.into_boxed_slice(),
        })
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
            BoundaryDecision::ExtendLeft => self.extend_df(x, knots, values, 0),
            BoundaryDecision::ExtendRight => self.extend_df(x, knots, values, knots.len() - 1),
            BoundaryDecision::Interior(idx) => {
                let s = x - knots[idx];
                let dy = s * (self.b[idx] + s * (self.c[idx] + s * self.d[idx]));
                eval_scaled_exp(
                    values[idx],
                    -dy,
                    "piecewise quadratic forward interpolation",
                )
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
                let df = self.extend_df(x, knots, values, 0)?;
                let slope = self.boundary_slope(knots, 0);
                checked_finite(-slope * df, "piecewise quadratic forward left derivative")
            }
            BoundaryDecision::ExtendRight => {
                let last = knots.len() - 1;
                let df = self.extend_df(x, knots, values, last)?;
                let slope = self.boundary_slope(knots, last);
                checked_finite(-slope * df, "piecewise quadratic forward right derivative")
            }
            BoundaryDecision::Interior(idx) => {
                let x0 = knots[idx];
                let x1 = knots[idx + 1];
                let s = x - x0;
                let y_prime = self.b[idx] + s * (2.0 * self.c[idx] + 3.0 * s * self.d[idx]);
                let df = if exact_f64_eq(x, x0) {
                    values[idx]
                } else if exact_f64_eq(x, x1) {
                    values[idx + 1]
                } else {
                    let dy = s * (self.b[idx] + s * (self.c[idx] + s * self.d[idx]));
                    eval_scaled_exp(values[idx], -dy, "piecewise quadratic forward evaluation")?
                };
                checked_finite(-y_prime * df, "piecewise quadratic forward derivative")
            }
        }
    }
}

impl PiecewiseQuadraticForwardStrategy {
    #[inline]
    fn boundary_slope(&self, knots: &[f64], boundary_index: usize) -> f64 {
        if boundary_index == 0 {
            self.b[0]
        } else {
            let last_seg = self.b.len() - 1;
            let h = knots[boundary_index] - knots[boundary_index - 1];
            self.b[last_seg] + h * (2.0 * self.c[last_seg] + 3.0 * h * self.d[last_seg])
        }
    }

    #[inline]
    fn extend_df(
        &self,
        x: f64,
        knots: &[f64],
        values: &[f64],
        boundary_index: usize,
    ) -> crate::Result<f64> {
        let slope = self.boundary_slope(knots, boundary_index);
        exp_extrapolate(
            x,
            knots[boundary_index],
            values[boundary_index],
            -slope,
            "piecewise quadratic forward extrapolation",
        )
    }
}
