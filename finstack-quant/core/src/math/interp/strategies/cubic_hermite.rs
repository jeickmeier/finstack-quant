//! Monotone cubic Hermite interpolation strategy.

use super::super::{
    traits::InterpolationStrategy,
    types::ExtrapolationPolicy,
    validation::{BoundaryDecision, classify_prime_query, classify_value_query, exact_f64_eq},
};
use super::numerics::*;
use std::vec::Vec;

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
            BoundaryDecision::ExtendLeft => linear_extrapolate(
                x,
                knots[0],
                values[0],
                self.ms[0],
                "cubic hermite left extrapolation",
            ),
            BoundaryDecision::ExtendRight => {
                let last = knots.len() - 1;
                linear_extrapolate(
                    x,
                    knots[last],
                    values[last],
                    self.ms[last],
                    "cubic hermite right extrapolation",
                )
            }
            BoundaryDecision::Interior(i) => {
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
            BoundaryDecision::Interior(i) => {
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
