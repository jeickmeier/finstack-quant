//! Shared finite/positive checks, safe slopes, and extrapolation kernels.

use super::super::utils::exact_f64_eq;
use std::vec::Vec;

#[inline]
pub(super) fn checked_finite(val: f64, context: &str) -> crate::Result<f64> {
    if !val.is_finite() {
        return Err(crate::Error::Validation(format!(
            "{context} produced non-finite result ({val})"
        )));
    }
    Ok(val)
}

#[inline]
pub(super) fn checked_positive_finite(val: f64, context: &str) -> crate::Result<f64> {
    if !val.is_finite() || val <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "{context} overflowed or underflowed to non-positive/non-finite value ({val})"
        )));
    }
    Ok(val)
}

#[inline]
pub(super) fn safe_unit_coord(x: f64, x0: f64, x1: f64) -> f64 {
    let dx = x1 - x0;
    if dx.is_finite() {
        (x - x0) / dx
    } else {
        (0.5 * x - 0.5 * x0) / (0.5 * x1 - 0.5 * x0)
    }
}

#[inline]
pub(super) fn safe_slope(x0: f64, y0: f64, x1: f64, y1: f64) -> crate::Result<f64> {
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
pub(super) fn safe_log_ratio(y0: f64, y1: f64) -> crate::Result<f64> {
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
pub(super) fn eval_scaled_exp(anchor: f64, exponent: f64, context: &str) -> crate::Result<f64> {
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

#[inline]
pub(super) fn linear_extrapolate(
    x: f64,
    anchor_x: f64,
    anchor_y: f64,
    slope: f64,
    context: &str,
) -> crate::Result<f64> {
    let y = if slope == 0.0 {
        anchor_y
    } else {
        anchor_y + slope * (x - anchor_x)
    };
    checked_finite(y, context)
}

#[inline]
pub(super) fn exp_extrapolate(
    x: f64,
    anchor_x: f64,
    anchor_y: f64,
    log_slope: f64,
    context: &str,
) -> crate::Result<f64> {
    let exponent = if log_slope == 0.0 {
        0.0
    } else {
        log_slope * (x - anchor_x)
    };
    eval_scaled_exp(anchor_y, exponent, context)
}

pub(super) fn discrete_forwards(
    knots: &[f64],
    values: &[f64],
    width_label: &str,
    fwd_label: &str,
) -> crate::Result<(Vec<f64>, Vec<f64>)> {
    let n = knots.len();
    debug_assert!(n >= 2);
    let mut widths = Vec::with_capacity(n - 1);
    let mut forwards = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        let width = knots[i + 1] - knots[i];
        if !width.is_finite() || width <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "{width_label} segment width must be finite and positive (got {width})"
            )));
        }
        let dy = -safe_log_ratio(values[i], values[i + 1])?;
        let fwd = checked_finite(dy / width, fwd_label)?;
        widths.push(width);
        forwards.push(fwd);
    }
    Ok((widths, forwards))
}
