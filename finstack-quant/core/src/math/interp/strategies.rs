//! Concrete interpolation strategy implementations.
//!
//! Provides strategy types for Linear, LogLinear, CubicHermite, MonotoneConvex,
//! and specialized forward-rate interpolation. The public strategies are kept
//! here because curve builders and calibration code select them by
//! [`InterpStyle`](super::types::InterpStyle); less-common strategies should
//! remain documented and covered by tests before being exposed through bindings.

use super::{
    traits::InterpolationStrategy,
    types::ExtrapolationPolicy,
    utils::{
        BoundaryDecision, classify_prime_query, classify_value_query, exact_f64_eq,
        locate_segment_unchecked, validate_positive_series,
    },
};
use std::vec::Vec;

#[inline]
fn checked_finite(val: f64, context: &str) -> crate::Result<f64> {
    if !val.is_finite() {
        return Err(crate::Error::Validation(format!(
            "{context} produced non-finite result ({val})"
        )));
    }
    Ok(val)
}

#[inline]
fn checked_positive_finite(val: f64, context: &str) -> crate::Result<f64> {
    if !val.is_finite() || val <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "{context} overflowed or underflowed to non-positive/non-finite value ({val})"
        )));
    }
    Ok(val)
}

#[inline]
fn safe_unit_coord(x: f64, x0: f64, x1: f64) -> f64 {
    let dx = x1 - x0;
    if dx.is_finite() {
        (x - x0) / dx
    } else {
        (0.5 * x - 0.5 * x0) / (0.5 * x1 - 0.5 * x0)
    }
}

#[inline]
fn safe_slope(x0: f64, y0: f64, x1: f64, y1: f64) -> crate::Result<f64> {
    let dy = y1 - y0;
    let dx = x1 - x0;
    let slope = if dy.is_finite() && dx.is_finite() {
        dy / dx
    } else {
        (0.5 * y1 - 0.5 * y0) / (0.5 * x1 - 0.5 * x0)
    };
    checked_finite(slope, "segment slope")
}

#[inline]
fn safe_log_ratio(y0: f64, y1: f64) -> crate::Result<f64> {
    if exact_f64_eq(y0, y1) {
        return Ok(0.0);
    }
    let rel = (y1 - y0) / y0;
    let dl = if rel.is_finite() && rel.abs() < 0.5 {
        rel.ln_1p()
    } else {
        let ratio = y1 / y0;
        if ratio.is_finite() && ratio > 0.0 {
            ratio.ln()
        } else {
            y1.ln() - y0.ln()
        }
    };
    if !dl.is_finite() || dl == 0.0 {
        return Err(crate::Error::Validation(format!(
            "log-ratio between distinct positive values ({y0}, {y1}) overflowed or collapsed to zero"
        )));
    }
    Ok(dl)
}

#[inline]
fn eval_scaled_exp(anchor: f64, exponent: f64, context: &str) -> crate::Result<f64> {
    if !exponent.is_finite() {
        return Err(crate::Error::Validation(format!(
            "{context} exponent is not finite ({exponent})"
        )));
    }
    let val = anchor * exponent.exp();
    if val.is_finite() && val > 0.0 {
        return Ok(val);
    }
    let alt = (anchor.ln() + exponent).exp();
    checked_positive_finite(alt, context)
}

/// Strategy for piecewise linear interpolation on ordinate values.
///
/// Accepts arbitrary finite signed ordinates. Simple and monotone per segment
/// without creating artificial extrema between knots.
#[derive(Debug, Clone)]
pub struct LinearStrategy;

impl InterpolationStrategy for LinearStrategy {
    fn from_raw(
        _knots: &[f64],
        _values: &[f64],
        _extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Self> {
        Ok(Self)
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
                let slope = safe_slope(knots[0], values[0], knots[1], values[1])?;
                let y = if slope == 0.0 {
                    values[0]
                } else {
                    values[0] + slope * (x - knots[0])
                };
                checked_finite(y, "linear left extrapolation")
            }
            BoundaryDecision::ExtendRight => {
                let n = knots.len();
                let slope = safe_slope(knots[n - 2], values[n - 2], knots[n - 1], values[n - 1])?;
                let y = if slope == 0.0 {
                    values[n - 1]
                } else {
                    values[n - 1] + slope * (x - knots[n - 1])
                };
                checked_finite(y, "linear right extrapolation")
            }
            BoundaryDecision::Interior => {
                let idx = locate_segment_unchecked(knots, x);
                let x0 = knots[idx];
                let x1 = knots[idx + 1];
                let y0 = values[idx];
                let y1 = values[idx + 1];
                let w = safe_unit_coord(x, x0, x1);
                checked_finite((1.0 - w) * y0 + w * y1, "linear interpolation")
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
            BoundaryDecision::ExtendLeft => safe_slope(knots[0], values[0], knots[1], values[1]),
            BoundaryDecision::ExtendRight => {
                let n = knots.len();
                safe_slope(knots[n - 2], values[n - 2], knots[n - 1], values[n - 1])
            }
            BoundaryDecision::Interior => {
                let idx = locate_segment_unchecked(knots, x);
                safe_slope(knots[idx], values[idx], knots[idx + 1], values[idx + 1])
            }
        }
    }
}

/// Strategy for log-linear interpolation of strictly positive ordinates.
///
/// Performs linear interpolation on `ln(y)` between adjacent knots without
/// reconstructing stored knot values through `exp(ln(y))`. On discount-factor
/// curves, this is equivalent to piecewise-constant instantaneous forwards.
#[derive(Debug, Clone)]
pub struct LogLinearStrategy {
    /// Precomputed segment log-ratios `ln(values[i + 1] / values[i])`.
    log_diffs: Box<[f64]>,
}

impl InterpolationStrategy for LogLinearStrategy {
    fn from_raw(
        knots: &[f64],
        values: &[f64],
        _extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Self> {
        validate_positive_series(values)?;
        let mut log_diffs = Vec::with_capacity(knots.len() - 1);
        for w in values.windows(2) {
            log_diffs.push(safe_log_ratio(w[0], w[1])?);
        }
        Ok(Self {
            log_diffs: log_diffs.into_boxed_slice(),
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
            BoundaryDecision::ExtendLeft => {
                let slope = self.boundary_slope(knots, 0)?;
                let exponent = if slope == 0.0 {
                    0.0
                } else {
                    slope * (x - knots[0])
                };
                eval_scaled_exp(values[0], exponent, "log-linear left extrapolation")
            }
            BoundaryDecision::ExtendRight => {
                let last = knots.len() - 1;
                let slope = self.boundary_slope(knots, last)?;
                let exponent = if slope == 0.0 {
                    0.0
                } else {
                    slope * (x - knots[last])
                };
                eval_scaled_exp(values[last], exponent, "log-linear right extrapolation")
            }
            BoundaryDecision::Interior => {
                let idx = locate_segment_unchecked(knots, x);
                let w = safe_unit_coord(x, knots[idx], knots[idx + 1]);
                eval_scaled_exp(
                    values[idx],
                    w * self.log_diffs[idx],
                    "log-linear interpolation",
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
                let slope = self.boundary_slope(knots, 0)?;
                let exponent = if slope == 0.0 {
                    0.0
                } else {
                    slope * (x - knots[0])
                };
                let y = eval_scaled_exp(values[0], exponent, "log-linear left extrapolation")?;
                checked_finite(y * slope, "log-linear left derivative")
            }
            BoundaryDecision::ExtendRight => {
                let last = knots.len() - 1;
                let slope = self.boundary_slope(knots, last)?;
                let exponent = if slope == 0.0 {
                    0.0
                } else {
                    slope * (x - knots[last])
                };
                let y = eval_scaled_exp(values[last], exponent, "log-linear right extrapolation")?;
                checked_finite(y * slope, "log-linear right derivative")
            }
            BoundaryDecision::Interior => {
                let idx = locate_segment_unchecked(knots, x);
                let x0 = knots[idx];
                let x1 = knots[idx + 1];
                let slope = self.segment_log_slope(x0, x1, idx)?;
                let y = if exact_f64_eq(x, x0) {
                    values[idx]
                } else if exact_f64_eq(x, x1) {
                    values[idx + 1]
                } else {
                    let w = safe_unit_coord(x, x0, x1);
                    eval_scaled_exp(
                        values[idx],
                        w * self.log_diffs[idx],
                        "log-linear interpolation",
                    )?
                };
                checked_finite(y * slope, "log-linear derivative")
            }
        }
    }
}

impl LogLinearStrategy {
    #[inline]
    fn segment_log_slope(&self, x0: f64, x1: f64, seg_idx: usize) -> crate::Result<f64> {
        let dl = self.log_diffs[seg_idx];
        let dx = x1 - x0;
        let slope = if dx.is_finite() {
            dl / dx
        } else {
            (0.5 * dl) / (0.5 * x1 - 0.5 * x0)
        };
        checked_finite(slope, "log-linear segment slope")
    }

    #[inline]
    fn boundary_slope(&self, knots: &[f64], boundary_index: usize) -> crate::Result<f64> {
        let seg_idx = if boundary_index == 0 {
            0
        } else {
            self.log_diffs.len() - 1
        };
        self.segment_log_slope(knots[seg_idx], knots[seg_idx + 1], seg_idx)
    }
}

/// Strategy for piecewise quadratic forward interpolation (natural cubic spline
/// in log-discount space with C¹ instantaneous forwards).
///
/// Builds a natural cubic spline (`y''(t_0) = y''(t_last) = 0`) in `-ln(DF)` space
/// so the resulting instantaneous forward curve is piecewise quadratic and C¹-continuous
/// across knots. Negative forwards are permitted without positivity clamping, and a
/// two-point curve reduces exactly to [`LogLinearStrategy`].
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
        debug_assert!(n >= 2);

        let mut h = Vec::with_capacity(n - 1);
        let mut q = Vec::with_capacity(n - 1);
        for i in 0..n - 1 {
            let width = knots[i + 1] - knots[i];
            if !width.is_finite() || width <= 0.0 {
                return Err(crate::Error::Validation(format!(
                    "piecewise quadratic forward segment width must be finite and positive (got {width})"
                )));
            }
            let dy = -safe_log_ratio(values[i], values[i + 1])?;
            let q_i = checked_finite(dy / width, "piecewise quadratic forward interval forward")?;
            h.push(width);
            q.push(q_i);
        }

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
            BoundaryDecision::Interior => {
                let idx = locate_segment_unchecked(knots, x);
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
            BoundaryDecision::Interior => {
                let idx = locate_segment_unchecked(knots, x);
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
        let exponent = if slope == 0.0 {
            0.0
        } else {
            -slope * (x - knots[boundary_index])
        };
        eval_scaled_exp(
            values[boundary_index],
            exponent,
            "piecewise quadratic forward extrapolation",
        )
    }
}

/// Strategy for shape-preserving piecewise cubic Hermite interpolation (PCHIP).
///
/// Uses Fritsch-Butland (1984) weighted harmonic interior slopes and Moler one-sided
/// three-point endpoint rules, matching SciPy `PchipInterpolator`, and reduces to
/// linear interpolation on two-point grids. Accepts arbitrary finite signed ordinates
/// and fails closed on non-finite slopes or evaluations.
///
/// # References
///
/// - Fritsch, F. N., & Carlson, R. E. (1980). "Monotone Piecewise Cubic
///   Interpolation." *SIAM Journal on Numerical Analysis*, 17(2), 238-246.
///   See [Fritsch-Carlson 1980](../../../../docs/REFERENCES.md#fritsch-carlson-1980).
#[derive(Debug, Clone)]
pub struct CubicHermiteStrategy {
    /// First-derivative values at each knot (PCHIP slopes).
    ms: Box<[f64]>,
}

impl InterpolationStrategy for CubicHermiteStrategy {
    fn from_raw(
        knots: &[f64],
        values: &[f64],
        _extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Self> {
        let ms = compute_monotone_slopes(knots, values)?;
        Ok(Self { ms })
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
                let slope = self.ms[0];
                let y = if slope == 0.0 {
                    values[0]
                } else {
                    values[0] + slope * (x - knots[0])
                };
                checked_finite(y, "cubic hermite left extrapolation")
            }
            BoundaryDecision::ExtendRight => {
                let last = knots.len() - 1;
                let slope = self.ms[last];
                let y = if slope == 0.0 {
                    values[last]
                } else {
                    values[last] + slope * (x - knots[last])
                };
                checked_finite(y, "cubic hermite right extrapolation")
            }
            BoundaryDecision::Interior => {
                let i = locate_segment_unchecked(knots, x);
                let x0 = knots[i];
                let x1 = knots[i + 1];
                let t = safe_unit_coord(x, x0, x1);
                let omt = 1.0 - t;
                let t2 = t * t;
                let t3 = t2 * t;

                let h00 = (1.0 + 2.0 * t) * omt * omt;
                let h10 = t * omt * omt;
                let h01 = t2 * (3.0 - 2.0 * t);
                let h11 = t3 - t2;

                let f0 = values[i];
                let f1 = values[i + 1];
                let m0 = self.ms[i];
                let m1 = self.ms[i + 1];
                let h = x1 - x0;
                let hm0 = if h.is_finite() {
                    h * m0
                } else {
                    (0.5 * x1 - 0.5 * x0) * (2.0 * m0)
                };
                let hm1 = if h.is_finite() {
                    h * m1
                } else {
                    (0.5 * x1 - 0.5 * x0) * (2.0 * m1)
                };
                let val = (h00 * f0 + h01 * f1) + (h10 * hm0 + h11 * hm1);
                checked_finite(val, "cubic hermite interpolation")
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
            BoundaryDecision::ExtendLeft => Ok(self.ms[0]),
            BoundaryDecision::ExtendRight => Ok(self.ms[self.ms.len() - 1]),
            BoundaryDecision::Interior => {
                let i = locate_segment_unchecked(knots, x);
                let x0 = knots[i];
                let x1 = knots[i + 1];
                let t = safe_unit_coord(x, x0, x1);
                let t2 = t * t;

                let h10_prime = 3.0 * t2 - 4.0 * t + 1.0;
                let h11_prime = 3.0 * t2 - 2.0 * t;

                let m0 = self.ms[i];
                let m1 = self.ms[i + 1];
                if exact_f64_eq(x, x0) {
                    return Ok(m0);
                }
                if exact_f64_eq(x, x1) {
                    return Ok(m1);
                }
                let delta = safe_slope(x0, values[i], x1, values[i + 1])?;
                let deriv = 6.0 * t * (1.0 - t) * delta + (h10_prime * m0 + h11_prime * m1);
                checked_finite(deriv, "cubic hermite derivative")
            }
        }
    }
}

/// Compute shape-preserving slopes using Fritsch-Butland harmonic interior means
/// and Moler one-sided endpoint rules (matching SciPy `PchipInterpolator`).
#[inline]
fn compute_monotone_slopes(xs: &[f64], ys: &[f64]) -> crate::Result<Box<[f64]>> {
    let n = xs.len();
    debug_assert_eq!(n, ys.len());

    if n == 2 {
        let slope = safe_slope(xs[0], ys[0], xs[1], ys[1])?;
        return Ok(vec![slope, slope].into_boxed_slice());
    }

    let mut ms = vec![0.0; n];
    let mut h = Vec::with_capacity(n - 1);
    let mut delta = Vec::with_capacity(n - 1);
    let mut needs_half_width = false;

    for i in 0..n - 1 {
        let width = xs[i + 1] - xs[i];
        if !width.is_finite() {
            needs_half_width = true;
        }
        h.push(width);
        delta.push(safe_slope(xs[i], ys[i], xs[i + 1], ys[i + 1])?);
    }

    if needs_half_width {
        for (i, hi) in h.iter_mut().enumerate() {
            *hi = 0.5 * xs[i + 1] - 0.5 * xs[i];
        }
    }

    for i in 1..n - 1 {
        let d_prev = delta[i - 1];
        let d_curr = delta[i];
        if d_prev == 0.0 || d_curr == 0.0 || d_prev.signum() != d_curr.signum() {
            ms[i] = 0.0;
        } else {
            let h_scale = h[i - 1].abs().max(h[i].abs());
            let h_prev_norm = h[i - 1] / h_scale;
            let h_curr_norm = h[i] / h_scale;
            let w1 = 2.0 * h_curr_norm + h_prev_norm;
            let w2 = h_curr_norm + 2.0 * h_prev_norm;

            let d_scale = d_prev.abs().max(d_curr.abs());
            let delta_prev_norm = d_prev / d_scale;
            let delta_curr_norm = d_curr / d_scale;
            let slope = d_scale
                * (((w1 + w2) * delta_prev_norm * delta_curr_norm)
                    / (w1 * delta_curr_norm + w2 * delta_prev_norm));
            ms[i] = checked_finite(slope, "cubic hermite interior slope")?;
        }
    }

    let endpoint_slope = |h_adj: f64, h_far: f64, d_adj: f64, d_far: f64| -> crate::Result<f64> {
        let h_scale = h_adj.abs().max(h_far.abs());
        let ha = h_adj / h_scale;
        let hf = h_far / h_scale;
        let denom = ha + hf;
        let raw = if let Some(diff) = Some(d_adj - d_far).filter(|v| v.is_finite()) {
            d_adj + (ha / denom) * diff
        } else {
            ((2.0 * ha + hf) / denom) * d_adj - (ha / denom) * d_far
        };
        let mut s = checked_finite(raw, "cubic hermite endpoint slope")?;
        if d_adj == 0.0 || s.signum() != d_adj.signum() {
            s = 0.0;
        } else if d_adj.signum() != d_far.signum() && (s / 3.0).abs() > d_adj.abs() {
            let cap = 3.0 * d_adj;
            s = checked_finite(cap, "cubic hermite endpoint slope cap")?;
        }
        Ok(s)
    };

    ms[0] = endpoint_slope(h[0], h[1], delta[0], delta[1])?;
    let last = n - 1;
    ms[last] = endpoint_slope(h[last - 1], h[last - 2], delta[last - 1], delta[last - 2])?;

    Ok(ms.into_boxed_slice())
}

/// Strategy for Hagan-West (2006/2008) monotone-convex discount-factor interpolation.
///
/// Constructs segment-average forwards `f^d_i = -ln(D[i+1] / D[i]) / (t[i+1] - t[i])`,
/// estimates uncollared nodal instantaneous forwards, and evaluates the exact regional
/// zero-integral excess forward polynomial within each interval. On two-point grids,
/// evaluates constant interval forward `f^d_0` (identical to [`LogLinearStrategy`]).
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
                let dt = x - knots[0];
                let f0 = self.left_boundary_forward();
                eval_scaled_exp(
                    values[0],
                    if f0 == 0.0 { 0.0 } else { -f0 * dt },
                    "monotone convex left extrapolation",
                )
            }
            BoundaryDecision::ExtendRight => {
                let last = knots.len() - 1;
                let dt = x - knots[last];
                let f_last = self.right_boundary_forward();
                eval_scaled_exp(
                    values[last],
                    if f_last == 0.0 { 0.0 } else { -f_last * dt },
                    "monotone convex right extrapolation",
                )
            }
            BoundaryDecision::Interior => {
                let i = locate_segment_unchecked(knots, x);
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
                let dt = x - knots[0];
                let f0 = self.left_boundary_forward();
                let df = eval_scaled_exp(
                    values[0],
                    if f0 == 0.0 { 0.0 } else { -f0 * dt },
                    "monotone convex left extrapolation",
                )?;
                checked_finite(-f0 * df, "monotone convex left derivative")
            }
            BoundaryDecision::ExtendRight => {
                let last = knots.len() - 1;
                let dt = x - knots[last];
                let f_last = self.right_boundary_forward();
                let df = eval_scaled_exp(
                    values[last],
                    if f_last == 0.0 { 0.0 } else { -f_last * dt },
                    "monotone convex right extrapolation",
                )?;
                checked_finite(-f_last * df, "monotone convex right derivative")
            }
            BoundaryDecision::Interior => {
                let i = locate_segment_unchecked(knots, x);
                let (df, fwd) = self.segment_df_and_forward(i, x, knots, values)?;
                checked_finite(-fwd * df, "monotone convex derivative")
            }
        }
    }
}

impl MonotoneConvexStrategy {
    fn build_hagan_west(knots: &[f64], dfs: &[f64]) -> crate::Result<Self> {
        let n = knots.len();
        debug_assert!(n >= 2);

        let mut dt = Vec::with_capacity(n - 1);
        let mut fd = Vec::with_capacity(n - 1);
        for i in 0..n - 1 {
            let width = knots[i + 1] - knots[i];
            if !width.is_finite() || width <= 0.0 {
                return Err(crate::Error::Validation(format!(
                    "monotone convex segment width must be finite and positive (got {width})"
                )));
            }
            let dy = -safe_log_ratio(dfs[i], dfs[i + 1])?;
            let fd_i = checked_finite(dy / width, "monotone convex discrete forward")?;
            dt.push(width);
            fd.push(fd_i);
        }

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
