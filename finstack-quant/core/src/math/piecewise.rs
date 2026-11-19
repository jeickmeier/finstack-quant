//! Validated piecewise-constant numeric curves.
//!
//! The curve is right-continuous: the value at `times[i]` applies on
//! `[times[i], times[i + 1])`, with the final value extrapolated flat.

use crate::math::interp::exact_f64_eq;

use crate::{Error, Result};

/// A finite, right-continuous piecewise-constant curve.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawPiecewiseConstantCurve")]
pub struct PiecewiseConstantCurve {
    /// Knot times in years, strictly increasing and starting at exactly zero.
    /// Each entry is the left edge of the interval its value applies to.
    #[cfg_attr(
        feature = "json-schema",
        schemars(length(min = 1), inner(range(min = 0.0)))
    )]
    times: Vec<f64>,
    /// Curve value on each interval, finite and non-negative. Same length as
    /// `times`; `values[i]` applies over `[times[i], times[i + 1])`.
    #[cfg_attr(
        feature = "json-schema",
        schemars(length(min = 1), inner(range(min = 0.0)))
    )]
    values: Vec<f64>,
}

#[derive(serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawPiecewiseConstantCurve {
    #[cfg_attr(
        feature = "json-schema",
        schemars(length(min = 1), inner(range(min = 0.0)))
    )]
    times: Vec<f64>,
    #[cfg_attr(
        feature = "json-schema",
        schemars(length(min = 1), inner(range(min = 0.0)))
    )]
    values: Vec<f64>,
}

impl TryFrom<RawPiecewiseConstantCurve> for PiecewiseConstantCurve {
    type Error = Error;

    fn try_from(raw: RawPiecewiseConstantCurve) -> Result<Self> {
        Self::new(raw.times, raw.values)
    }
}

impl PiecewiseConstantCurve {
    /// Create a validated right-continuous curve.
    ///
    /// The first knot must be exactly zero, knots must be strictly increasing,
    /// and all values must be finite and non-negative.
    ///
    /// # Arguments
    ///
    /// * `times` - Segment starts in model years, ascending from zero.
    /// * `values` - Finite nonnegative coefficients beginning at the corresponding knots.
    pub fn new(mut times: Vec<f64>, mut values: Vec<f64>) -> Result<Self> {
        if times.is_empty() || values.is_empty() || times.len() != values.len() {
            return Err(Error::Validation(
                "piecewise curve requires equally sized, non-empty times and values".into(),
            ));
        }
        if times[0] != 0.0 {
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
            if !value.is_finite() || value < 0.0 {
                return Err(Error::Validation(format!(
                    "piecewise curve value at index {index} must be non-negative and finite, got {value}"
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
        times[0] = 0.0;
        for value in &mut values {
            if *value == 0.0 {
                *value = 0.0;
            }
        }
        Ok(Self { times, values })
    }

    /// Construct a constant curve beginning at time zero.
    ///
    /// # Arguments
    ///
    /// * `value` - Finite nonnegative coefficient applying at every model time.
    pub fn constant(value: f64) -> Result<Self> {
        Self::new(vec![0.0], vec![value])
    }

    /// Right-continuous value at `time`, with flat final extrapolation.
    ///
    /// # Arguments
    ///
    /// * `time` - Finite nonnegative model time in years.
    pub fn get_value(&self, time: f64) -> Result<f64> {
        if !time.is_finite() || time < 0.0 {
            return Err(Error::Validation(
                "piecewise lookup requires finite nonnegative time".into(),
            ));
        }
        let index = self.times.partition_point(|knot| *knot <= time);
        Ok(self.values[index - 1])
    }

    /// Segment start times.
    #[must_use]
    pub fn get_times(&self) -> &[f64] {
        &self.times
    }

    /// Segment values.
    #[must_use]
    pub fn get_values(&self) -> &[f64] {
        &self.values
    }

    /// Integrate `σ(u)² exp(-2κ(anchor - u))` on `[start, end]`.
    ///
    /// This is the variance kernel used by one-factor Hull-White state and
    /// bond-option formulas. `anchor` must be at or after `end`.
    ///
    /// # Arguments
    ///
    /// * `kappa` - Finite signed exponential decay rate in inverse years.
    /// * `anchor` - Finite reference time in years, at or beyond the integration end.
    /// * `start` - Nonnegative lower integration time in years.
    /// * `end` - Finite upper integration time in years, at or beyond the start.
    pub fn integrate(&self, kappa: f64, anchor: f64, start: f64, end: f64) -> Result<f64> {
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
        if exact_f64_eq(start, end) {
            return Ok(0.0);
        }
        let first = self.times.partition_point(|&knot| knot <= start) - 1;
        let segments = self.times[first..]
            .iter()
            .enumerate()
            .take_while(|(_, knot)| **knot < end)
            .filter_map(|(offset, &knot)| {
                let i = first + offset;
                let sigma = self.values[i];
                if sigma == 0.0 {
                    return None;
                }
                let left = start.max(knot);
                let right = self.times.get(i + 1).copied().unwrap_or(end).min(end);
                Some(segment_log2(sigma, kappa, anchor, left, right))
            });
        let Some(largest) = segments.clone().reduce(f64::max) else {
            return Ok(0.0);
        };
        if largest.is_finite() {
            let sum =
                crate::math::summation::neumaier_sum(segments.map(|log| (log - largest).exp2()))?;
            let total = (largest + sum.log2()).exp2();
            if total.is_finite() && total > 0.0 {
                return Ok(total);
            }
        }
        Err(Error::Internal(
            "positive weighted piecewise integral exceeds numerical range".into(),
        ))
    }
}

// Work in log2 so a squared coefficient, segment or damping factor need not
// be individually representable. Choose the hotter endpoint for either sign.
fn segment_log2(sigma: f64, kappa: f64, anchor: f64, left: f64, right: f64) -> f64 {
    let d = right - left;
    let scale = 2.0 * sigma.log2();
    if kappa == 0.0 {
        return scale + d.log2();
    }
    let hot = if kappa > 0.0 { right } else { left };
    let damping = -2.0 * (kappa * (anchor - hot));
    let x = 2.0 * (kappa.abs() * d);
    let log_length = if x == 0.0 {
        d.log2()
    } else if x < 1.0 {
        d.log2() + (-(-x).exp_m1() / x).log2()
    } else {
        (-(-x).exp_m1()).log2() - 1.0 - kappa.abs().log2()
    };
    damping.mul_add(std::f64::consts::LOG2_E, scale + log_length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piecewise_constant_integrates_exp_weight_across_knots() {
        let curve = PiecewiseConstantCurve::new(vec![0.0, 1.0], vec![0.01, 0.02])
            .expect("valid volatility schedule");

        let integral = curve.integrate(0.05, 2.0, 0.0, 2.0).expect("integral");

        let first = 0.01_f64.powi(2) * ((-0.10_f64).exp() - (-0.20_f64).exp()) / 0.10;
        let second = 0.02_f64.powi(2) * (1.0 - (-0.10_f64).exp()) / 0.10;
        assert!((integral - (first + second)).abs() < 1.0e-14);
    }

    #[test]
    fn weighted_variance_retains_accuracy_near_zero_mean_reversion() {
        let curve =
            PiecewiseConstantCurve::new(vec![0.0, 1.0], vec![0.01, 0.025]).expect("schedule");
        let value = curve.integrate(1e-12, 2.0, 0.0, 2.0).expect("integral");
        // At zero mean reversion each one-year interval contributes sigma^2;
        // the exact 1e-12 correction is less than 4e-15 in absolute variance.
        assert!((value - (0.01_f64.powi(2) + 0.025_f64.powi(2))).abs() < 4e-15);
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn right_continuous_query_and_extreme_variance() -> Result<()> {
        let c = PiecewiseConstantCurve::new(vec![-0.0, 1.0], vec![1.0, 2.0])?;
        assert_eq!(c.get_times()[0].to_bits(), 0);
        for t in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(c.get_value(t).is_err());
        }
        assert_eq!(c.get_value(1.0)?, 2.0);
        let v = c.integrate(2f64.powi(-50), 2f64.powi(50), 0.0, 1.0)?;
        assert!((v - 0.1353352832366128).abs() < 5e-16);
        assert_eq!(
            PiecewiseConstantCurve::constant(2f64.powi(50))?.integrate(
                0.0,
                1.0,
                0.0,
                2f64.powi(-100)
            )?,
            1.0
        );
        let tiny =
            PiecewiseConstantCurve::new(vec![0.0, 0.25, 0.5, 0.75], vec![2f64.powi(-537); 4])?;
        assert_eq!(tiny.integrate(0.0, 1.0, 0.0, 1.0)?.to_bits(), 1);
        assert!(c.integrate(-f64::MAX, 1.0, 0.0, 1.0).is_err());
        Ok(())
    }
}
