//! Elementwise function kernels for `CompiledExpr`.
//!
//! Every kernel maps its first argument element by element; a missing input
//! fills the output with NaN and a result shorter than the output NaN-fills
//! the tail. `round` takes a constant `digits` argument: a non-constant
//! series (for example a data column) is a validation error, and a
//! non-integral or out-of-range constant yields all-NaN output.
//!
//! Windowed, cumulative and reducing functions are evaluated by the
//! `statements` crate; see [`super::Function::is_scalar_evaluable`].

use super::eval::CompiledExpr;

impl CompiledExpr {
    /// Read a scalar from a series that must be constant across every element.
    ///
    /// `series` is non-empty. A series whose elements are not bit-identical is
    /// a validation error. `what` is the argument name embedded in that error.
    #[inline]
    fn constant_scalar(series: &[f64], what: &str) -> crate::Result<f64> {
        let first = series[0];
        if series.iter().any(|v| v.to_bits() != first.to_bits()) {
            return Err(crate::Error::Validation(format!(
                "{what} must be a constant scalar (literal); got a non-constant series"
            )));
        }
        Ok(first)
    }

    /// Map `f` across the first argument.
    ///
    /// A missing input fills `out` with NaN. A result shorter than `out`
    /// NaN-fills the tail.
    pub(super) fn eval_unary_into(arg_results: &[&[f64]], out: &mut [f64], f: impl Fn(f64) -> f64) {
        let Some(base) = arg_results.first() else {
            out.fill(f64::NAN);
            return;
        };
        let n = out.len().min(base.len());
        for i in 0..n {
            out[i] = f(base[i]);
        }
        out[n..].fill(f64::NAN);
    }

    /// Expression `sign`: positive → `1`, negative → `-1`, both zeros → `0`.
    ///
    /// `f64::signum` preserves the sign of zero, so it is not this function.
    fn sign_or_zero(value: f64) -> f64 {
        if value.is_nan() {
            f64::NAN
        } else if value > 0.0 {
            1.0
        } else if value < 0.0 {
            -1.0
        } else {
            0.0
        }
    }

    pub(super) fn eval_sign_into(arg_results: &[&[f64]], out: &mut [f64]) {
        Self::eval_unary_into(arg_results, out, Self::sign_or_zero);
    }

    pub(super) fn eval_pow_into(arg_results: &[&[f64]], out: &mut [f64]) {
        let (Some(base), Some(exponent)) = (arg_results.first(), arg_results.get(1)) else {
            out.fill(f64::NAN);
            return;
        };
        let n = out.len().min(base.len());
        for i in 0..n {
            out[i] = match exponent.get(i) {
                Some(&power) => base[i].powf(power),
                None => f64::NAN,
            };
        }
        out[n..].fill(f64::NAN);
    }

    /// Evaluate `round(x[, digits])` with ties rounding half away from zero.
    ///
    /// `digits` (optional, default 0) must be a constant integral scalar;
    /// a non-constant series is a validation error (matching the
    /// window/step-argument convention) and a non-integral or out-of-range
    /// constant yields all-NaN output.
    pub(super) fn eval_round_into(arg_results: &[&[f64]], out: &mut [f64]) -> crate::Result<()> {
        let digits = match arg_results.get(1) {
            Some(series) if !series.is_empty() => {
                let first = Self::constant_scalar(series, "round() digits argument")?;
                if !first.is_finite()
                    || first.fract() != 0.0
                    || first < i32::MIN as f64
                    || first > i32::MAX as f64
                {
                    None
                } else {
                    Some(first as i32)
                }
            }
            _ => Some(0),
        };
        let Some(digits) = digits else {
            Self::eval_unary_into(arg_results, out, |_| f64::NAN);
            return Ok(());
        };
        Self::eval_unary_into(arg_results, out, |value| {
            crate::math::round_half_away(value, digits)
        });
        Ok(())
    }

    pub(super) fn eval_clamp_into(arg_results: &[&[f64]], out: &mut [f64]) {
        let (Some(base), Some(lo), Some(hi)) =
            (arg_results.first(), arg_results.get(1), arg_results.get(2))
        else {
            out.fill(f64::NAN);
            return;
        };
        let n = out.len().min(base.len());
        for i in 0..n {
            out[i] = match (lo.get(i), hi.get(i)) {
                (Some(&lower), Some(&upper)) => crate::math::clamp_or_nan(base[i], lower, upper),
                _ => f64::NAN,
            };
        }
        out[n..].fill(f64::NAN);
    }
}
