//! Validated piecewise-constant numeric curves.
//!
//! The curve is left-continuous: the value at `times[i]` applies on
//! `[times[i], times[i + 1])`, with the final value extrapolated flat.

use crate::{Error, Result};

/// A finite, left-continuous piecewise-constant curve.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PiecewiseConstantCurve {
    times: Vec<f64>,
    values: Vec<f64>,
}

impl PiecewiseConstantCurve {
    /// Create a validated left-continuous curve.
    ///
    /// The first knot must be exactly zero, knots must be strictly increasing,
    /// and all values must be finite and strictly positive.
    pub fn new(times: Vec<f64>, values: Vec<f64>) -> Result<Self> {
        if times.is_empty() || values.is_empty() || times.len() != values.len() {
            return Err(Error::Validation(
                "piecewise curve requires equally sized, non-empty times and values".into(),
            ));
        }
        if times[0].to_bits() != 0.0_f64.to_bits() {
            return Err(Error::Validation(format!(
                "piecewise curve first knot must be 0.0, got {}",
                times[0]
            )));
        }
        for (index, (&time, &value)) in times.iter().zip(&values).enumerate() {
            if !time.is_finite() || time < 0.0 {
                return Err(Error::Validation(format!(
                    "piecewise curve time at index {index} must be finite and non-negative, got {time}"
                )));
            }
            if !value.is_finite() || value <= 0.0 {
                return Err(Error::Validation(format!(
                    "piecewise curve value at index {index} must be positive and finite, got {value}"
                )));
            }
            if index > 0 && time <= times[index - 1] {
                return Err(Error::Validation(format!(
                    "piecewise curve times must be strictly increasing: {} follows {}",
                    time,
                    times[index - 1]
                )));
            }
        }
        Ok(Self { times, values })
    }

    /// Construct a constant curve beginning at time zero.
    pub fn constant(value: f64) -> Result<Self> {
        Self::new(vec![0.0], vec![value])
    }

    /// Left-continuous value at `time`, with flat final extrapolation.
    #[must_use]
    pub fn value_at(&self, time: f64) -> f64 {
        let index = self.times.partition_point(|knot| *knot <= time);
        self.values[index.saturating_sub(1)]
    }

    /// Segment start times.
    #[must_use]
    pub fn times(&self) -> &[f64] {
        &self.times
    }

    /// Segment values.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Integrate `σ(u)² exp(-2κ(anchor - u))` on `[start, end]`.
    ///
    /// This is the variance kernel used by one-factor Hull-White state and
    /// bond-option formulas. `anchor` must be at or after `end`.
    pub fn integrate_squared_exp_weight(
        &self,
        kappa: f64,
        anchor: f64,
        start: f64,
        end: f64,
    ) -> Result<f64> {
        if !kappa.is_finite()
            || !anchor.is_finite()
            || !start.is_finite()
            || !end.is_finite()
            || start < 0.0
            || end < start
            || anchor < end
        {
            return Err(Error::Validation(format!(
                "invalid weighted piecewise integral: kappa={kappa}, anchor={anchor}, start={start}, end={end}"
            )));
        }
        if end - start <= crate::math::ZERO_TOLERANCE {
            return Ok(0.0);
        }

        let mut total = 0.0;
        let mut left = start;
        while left < end {
            let segment = self
                .times
                .partition_point(|knot| *knot <= left)
                .saturating_sub(1);
            let right = self.times.get(segment + 1).copied().unwrap_or(end).min(end);
            let sigma_sq = self.values[segment] * self.values[segment];
            let contribution = if kappa.abs() < 1.0e-12 {
                sigma_sq * (right - left)
            } else {
                sigma_sq
                    * ((-2.0 * kappa * (anchor - right)).exp()
                        - (-2.0 * kappa * (anchor - left)).exp())
                    / (2.0 * kappa)
            };
            total += contribution;
            left = right;
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piecewise_constant_integrates_exp_weight_across_knots() {
        let curve = PiecewiseConstantCurve::new(vec![0.0, 1.0], vec![0.01, 0.02])
            .expect("valid volatility schedule");

        let integral = curve
            .integrate_squared_exp_weight(0.05, 2.0, 0.0, 2.0)
            .expect("integral");

        let first = 0.01_f64.powi(2) * ((-0.10_f64).exp() - (-0.20_f64).exp()) / 0.10;
        let second = 0.02_f64.powi(2) * (1.0 - (-0.10_f64).exp()) / 0.10;
        assert!((integral - (first + second)).abs() < 1.0e-14);
    }
}
