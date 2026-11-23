//! Linear and log-linear interpolation strategies.

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

/// Strategy for piecewise linear interpolation on ordinate values.
///
/// Accepts arbitrary finite signed ordinates. Simple and monotone per segment
/// without creating artificial extrema between knots.
#[derive(Debug, Clone)]
pub struct LinearStrategy;

impl LinearStrategy {
    /// Interpolate between two finite samples on a closed interval.
    ///
    /// # Arguments
    ///
    /// * `left` - Coordinate and level at the lower endpoint.
    /// * `right` - Coordinate and level at the strictly higher endpoint.
    /// * `x` - Finite query coordinate in the same units, within the two endpoints.
    ///
    /// # Errors
    ///
    /// Returns a validation error for invalid endpoints or an out-of-range coordinate.
    pub fn interpolate_segment(left: (f64, f64), right: (f64, f64), x: f64) -> crate::Result<f64> {
        let (x0, y0) = left;
        let (x1, y1) = right;
        if !x0.is_finite()
            || !x1.is_finite()
            || !y0.is_finite()
            || !y1.is_finite()
            || x0 >= x1
            || !(x0..=x1).contains(&x)
        {
            return Err(crate::Error::Validation(
                "linear interpolation requires finite ordered endpoints and an in-range query"
                    .into(),
            ));
        }
        if exact_f64_eq(x, x1) {
            return Ok(y1);
        }
        if exact_f64_eq(x, x0) || exact_f64_eq(y0, y1) {
            return Ok(y0);
        }
        // Anchor at the smaller magnitude to preserve small contributions.
        // FMA rounds a subnormal blend only once.
        let (base, delta, dx) = if y0.abs() <= y1.abs() {
            (y0, y1 - y0, x - x0)
        } else {
            (y1, y0 - y1, x1 - x)
        };
        let width = x1 - x0;
        let weight = dx / width;
        if width.is_finite()
            && delta.is_finite()
            && weight.is_normal()
            && y0.is_sign_negative() == y1.is_sign_negative()
        {
            let value = weight.mul_add(delta, base);
            if value.is_finite() {
                return Ok(value);
            }
        }
        // Extreme coordinate ratios can round to zero before multiplication by
        // a large level. Complete the affine combination before converting back.
        use num_rational::BigRational;
        use num_traits::ToPrimitive;
        let exact = |v| {
            BigRational::from_float(v)
                .ok_or_else(|| crate::Error::Validation("invalid interpolation endpoint".into()))
        };
        let (x0, x1, x) = (exact(x0)?, exact(x1)?, exact(x)?);
        let value = ((&x1 - &x) * exact(y0)? + (x - &x0) * exact(y1)?) / (x1 - x0);
        value.to_f64().ok_or_else(|| {
            crate::Error::Validation("linear interpolation exceeds binary64 range".into())
        })
    }
}

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
                linear_extrapolate(x, knots[0], values[0], slope, "linear left extrapolation")
            }
            BoundaryDecision::ExtendRight => {
                let n = knots.len();
                let slope = safe_slope(knots[n - 2], values[n - 2], knots[n - 1], values[n - 1])?;
                linear_extrapolate(
                    x,
                    knots[n - 1],
                    values[n - 1],
                    slope,
                    "linear right extrapolation",
                )
            }
            BoundaryDecision::Interior(idx) => {
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
            BoundaryDecision::Interior(idx) => {
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
                exp_extrapolate(
                    x,
                    knots[0],
                    values[0],
                    slope,
                    "log-linear left extrapolation",
                )
            }
            BoundaryDecision::ExtendRight => {
                let last = knots.len() - 1;
                let slope = self.boundary_slope(knots, last)?;
                exp_extrapolate(
                    x,
                    knots[last],
                    values[last],
                    slope,
                    "log-linear right extrapolation",
                )
            }
            BoundaryDecision::Interior(idx) => {
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
                let y = exp_extrapolate(
                    x,
                    knots[0],
                    values[0],
                    slope,
                    "log-linear left extrapolation",
                )?;
                checked_finite(y * slope, "log-linear left derivative")
            }
            BoundaryDecision::ExtendRight => {
                let last = knots.len() - 1;
                let slope = self.boundary_slope(knots, last)?;
                let y = exp_extrapolate(
                    x,
                    knots[last],
                    values[last],
                    slope,
                    "log-linear right extrapolation",
                )?;
                checked_finite(y * slope, "log-linear right derivative")
            }
            BoundaryDecision::Interior(idx) => {
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
