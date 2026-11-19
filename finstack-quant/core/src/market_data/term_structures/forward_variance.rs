//! Forward variance curve for rough volatility models.
//!
//! The initial forward variance curve ξ₀(t) represents expected instantaneous
//! variance at time `t`, conditional on today's information, under the model's
//! pricing measure:
//!
//! ```text
//! ξ₀(t) = E^Q[V_t | F₀]
//! ```
//!
//! Callers supply or calibrate this curve for rBergomi and related rough
//! volatility models. Differencing ATM implied total variances `σ_ATM²(t) · t`
//! is an optional approximation to its interval averages; it is not an exact
//! variance-swap bootstrap or, under stochastic volatility, generally equal to
//! the expected integrated variance.
//!
//! # Interpolation
//!
//! Point samples use piecewise linear interpolation. Interval forward variances
//! use piecewise constant interpolation so their integrated areas are preserved.
//! Both representations extend flat within the nonnegative model-time domain.
//!
//! # References
//!
//! - Bayer, C., Friz, P., & Gatheral, J. (2016). "Pricing under rough
//!   volatility." *Quantitative Finance*, 16(6), 887–904. `docs/REFERENCES.md#bayer-friz-gatheral-2016`
//! - Gatheral, J., Jaisson, T., & Rosenbaum, M. (2018). "Volatility is rough."
//!   *Quantitative Finance*, 18(6), 933–949. `docs/REFERENCES.md#gatheral-jaisson-rosenbaum-2018`

/// Forward variance curve ξ₀(t) for rough volatility models.
///
/// Represents `ξ₀(t) = E^Q[V_t | F₀]`, expected instantaneous variance under
/// the consuming model's pricing measure. Values are supplied or calibrated by
/// the caller. The integral sets the model's expected accumulated variance.
///
/// Differencing ATM implied total variance is an optional proxy, not an exact
/// variance-swap bootstrap. This type interpolates the supplied curve; it does
/// not perform market calibration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawForwardVarianceCurve", deny_unknown_fields)]
#[cfg_attr(feature = "json-schema", schemars(!try_from))]
pub struct ForwardVarianceCurve {
    /// Interpolation contract used by the constructor.
    interpolation: Interpolation,
    /// Point times or interval ends (strictly increasing year fractions).
    times: Vec<f64>,
    /// Point samples or constant interval variances (all nonnegative).
    values: Vec<f64>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
enum Interpolation {
    Linear,
    ConstantIntervals,
}

/// Raw deserialization state of [`ForwardVarianceCurve`].
///
/// Mirrors the serialized field layout exactly; conversion routes through
/// the matching constructor so deserialized curves satisfy the same invariants
/// as constructed ones, and unknown fields are rejected.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawForwardVarianceCurve {
    interpolation: Interpolation,
    /// Knot times (year fractions).
    times: Vec<f64>,
    /// Forward variance values at knot times.
    values: Vec<f64>,
}

impl TryFrom<RawForwardVarianceCurve> for ForwardVarianceCurve {
    type Error = crate::Error;

    fn try_from(raw: RawForwardVarianceCurve) -> crate::Result<Self> {
        if raw.times.len() != raw.values.len() {
            return Err(crate::Error::Validation(format!(
                "ForwardVarianceCurve: times/values length mismatch ({} vs {})",
                raw.times.len(),
                raw.values.len()
            )));
        }
        let points: Vec<(f64, f64)> = raw.times.into_iter().zip(raw.values).collect();
        Self::from_samples(&points, raw.interpolation)
    }
}

impl ForwardVarianceCurve {
    /// Creates a flat forward variance curve (constant ξ₀(t) = v0).
    ///
    /// # Arguments
    ///
    /// * `v0` - Finite, nonnegative annualized variance in decimal units
    ///   (for example, 0.04 corresponds to 20% annualized volatility).
    ///
    /// # Errors
    ///
    /// Returns an error if `v0` is negative or non-finite.
    pub fn flat(v0: f64) -> crate::Result<Self> {
        Self::from_points(&[(0.0, v0)])
    }

    /// Creates a forward variance curve from (time, forward_variance) pairs.
    ///
    /// # Arguments
    ///
    /// * `points` - Nonempty `(time, variance)` samples. Times are finite,
    ///   nonnegative, strictly increasing year fractions; variances are finite, nonnegative,
    ///   annualized decimal variances. Duplicate times are rejected. Values
    ///   interpolate linearly between samples and extrapolate flat.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `points` is empty
    /// - any time is negative
    /// - times are not strictly increasing in the supplied order
    /// - any variance is negative or non-finite
    pub fn from_points(points: &[(f64, f64)]) -> crate::Result<Self> {
        Self::from_samples(points, Interpolation::Linear)
    }

    /// Creates a piecewise constant curve from interval forward variances.
    ///
    /// Interval averages are preserved exactly: integrating to each interval
    /// end gives the sum of its predecessors' `variance * duration`. At a shared
    /// boundary, evaluation uses the following interval's variance.
    ///
    /// # Arguments
    ///
    /// * `intervals` - Nonempty `(end_time, variance)` pairs in strictly increasing
    ///   end-time order. The first interval starts at zero; every end is a finite,
    ///   positive year fraction. Variances are finite, nonnegative,
    ///   annualized decimal variances. The last variance extrapolates flat beyond
    ///   the final end.
    ///
    /// # Errors
    ///
    /// Returns an error for empty input, invalid times or variances, or unordered
    /// interval ends. Inputs are not sorted because order defines interval areas.
    pub fn from_intervals(intervals: &[(f64, f64)]) -> crate::Result<Self> {
        Self::from_samples(intervals, Interpolation::ConstantIntervals)
    }

    fn from_samples(points: &[(f64, f64)], interpolation: Interpolation) -> crate::Result<Self> {
        if points.is_empty() {
            return Err(crate::Error::Validation(
                "ForwardVarianceCurve: at least one point is required".to_string(),
            ));
        }

        let mut times = Vec::with_capacity(points.len());
        let mut values = Vec::with_capacity(points.len());
        for (i, &(t, v)) in points.iter().enumerate() {
            if !t.is_finite()
                || t < 0.0
                || (matches!(interpolation, Interpolation::ConstantIntervals) && t == 0.0)
            {
                return Err(crate::Error::Validation(format!(
                    "ForwardVarianceCurve: invalid time {t} at index {i} for {interpolation:?}"
                )));
            }
            if i > 0 && t <= times[i - 1] {
                return Err(crate::Error::Validation(format!(
                    "ForwardVarianceCurve: times must be strictly increasing, \
                     got {} then {t} at index {i}",
                    times[i - 1]
                )));
            }
            if !v.is_finite() || v < 0.0 {
                return Err(crate::Error::Validation(format!(
                    "ForwardVarianceCurve: forward variance must be finite and nonnegative, \
                     got {v} at time {t}"
                )));
            }
            times.push(t);
            values.push(v);
        }

        Ok(Self {
            interpolation,
            times,
            values,
        })
    }

    /// Evaluates the forward variance ξ₀(t) at time `t`.
    ///
    /// Uses the interpolation contract selected by the constructor, with flat
    /// extrapolation at the boundaries.
    ///
    /// # Arguments
    ///
    /// * `t` - Finite, nonnegative time in years from the curve's origin.
    ///
    /// # Errors
    ///
    /// Returns a validation error for a negative or non-finite query time.
    pub fn get_value(&self, t: f64) -> crate::Result<f64> {
        if !t.is_finite() || t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "forward variance requires finite nonnegative time, got {t}"
            )));
        }
        let right = self.times.partition_point(|&time| time <= t);
        let last = self.times.len() - 1;
        if matches!(self.interpolation, Interpolation::ConstantIntervals) {
            return Ok(self.values[right.min(last)]);
        }
        if right == 0 {
            return Ok(self.values[0]);
        }
        let left = right - 1;
        if left == last || crate::math::interp::exact_f64_eq(self.times[left], t) {
            return Ok(self.values[left]);
        }
        crate::math::interp::LinearStrategy::interpolate_segment(
            (self.times[left], self.values[left]),
            (self.times[right], self.values[right]),
            t,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-12;

    #[test]
    fn preserves_subnormal_blends_and_zero_intervals() {
        let m = f64::from_bits(1);
        let curve = ForwardVarianceCurve::from_points(&[(0.0, m), (1.0, 2.0 * m)]).unwrap();
        assert_eq!(curve.get_value(0.5).unwrap(), 2.0 * m);
        let wide = ForwardVarianceCurve::from_points(&[(0.0, 0.0), (f64::MAX, f64::MAX)]).unwrap();
        assert_eq!(wide.get_value(0.125).unwrap(), 0.125);
        let intervals = ForwardVarianceCurve::from_intervals(&[(0.25, 0.0), (1.0, 0.12)]).unwrap();
        assert_eq!(intervals.get_value(0.0).unwrap(), 0.0);
        assert_eq!(intervals.get_value(0.25).unwrap(), 0.12);
        assert!(intervals.get_value(-1.0).is_err());
        for mode in ["linear", "constant_intervals"] {
            assert!(
                serde_json::from_value::<ForwardVarianceCurve>(serde_json::json!({
                    "interpolation": mode, "times": [2.0, 1.0], "values": [0.2, 0.1]
                }))
                .is_err()
            );
        }
    }

    #[test]
    fn flat_curve_value() {
        let c = ForwardVarianceCurve::flat(0.04).unwrap();
        assert!((c.get_value(0.0).unwrap() - 0.04).abs() < TOL);
        assert!((c.get_value(1.0).unwrap() - 0.04).abs() < TOL);
        assert!((c.get_value(10.0).unwrap() - 0.04).abs() < TOL);
    }

    #[test]
    fn interval_variances_are_right_continuous_step_values() {
        let curve = ForwardVarianceCurve::from_intervals(&[(0.25, 0.04), (1.0, 0.12), (1.5, 0.02)])
            .unwrap();
        assert_eq!(curve.get_value(0.0).unwrap(), 0.04);
        assert_eq!(curve.get_value(0.25).unwrap(), 0.12);
        assert_eq!(curve.get_value(1.0).unwrap(), 0.02);
        assert_eq!(curve.get_value(3.0).unwrap(), 0.02);
    }

    #[test]
    fn intervals_require_ordered_positive_ends_and_nonnegative_variance() {
        for intervals in [
            vec![],
            vec![(0.0, 0.04)],
            vec![(1.0, 0.04), (0.5, 0.12)],
            vec![(0.5, 0.04), (0.5, 0.12)],
            vec![(f64::NAN, 0.04)],
            vec![(1.0, -0.01)],
            vec![(1.0, f64::INFINITY)],
        ] {
            assert!(ForwardVarianceCurve::from_intervals(&intervals).is_err());
        }
    }

    #[test]
    fn serde_preserves_interpolation_contract() {
        let points = [(0.25, 0.04), (1.0, 0.12)];
        for curve in [
            ForwardVarianceCurve::from_points(&points).unwrap(),
            ForwardVarianceCurve::from_intervals(&points).unwrap(),
        ] {
            let serialized = serde_json::to_value(&curve).unwrap();
            let restored: ForwardVarianceCurve =
                serde_json::from_value(serialized.clone()).unwrap();
            for time in [0.0, 0.25, 0.5, 1.0, 2.0] {
                assert_eq!(
                    restored.get_value(time).unwrap(),
                    curve.get_value(time).unwrap()
                );
            }
            let mut unknown = serialized;
            unknown["unknown_field"] = serde_json::json!(true);
            assert!(serde_json::from_value::<ForwardVarianceCurve>(unknown).is_err());
        }
        let invalid = serde_json::json!({
            "interpolation": "constant_intervals",
            "times": [1.0, 0.5],
            "values": [0.04, 0.12]
        });
        assert!(serde_json::from_value::<ForwardVarianceCurve>(invalid).is_err());
    }

    #[test]
    fn two_point_interp_and_extrap() {
        // Curve: (1.0, 0.04) -> (3.0, 0.08)
        let c = ForwardVarianceCurve::from_points(&[(1.0, 0.04), (3.0, 0.08)]).unwrap();

        // Flat extrapolation left
        assert!((c.get_value(0.0).unwrap() - 0.04).abs() < TOL);
        assert!((c.get_value(0.5).unwrap() - 0.04).abs() < TOL);

        // At knots
        assert!((c.get_value(1.0).unwrap() - 0.04).abs() < TOL);
        assert!((c.get_value(3.0).unwrap() - 0.08).abs() < TOL);

        // Linear interpolation: midpoint at t=2.0
        assert!((c.get_value(2.0).unwrap() - 0.06).abs() < TOL);

        // Flat extrapolation right
        assert!((c.get_value(5.0).unwrap() - 0.08).abs() < TOL);
    }

    #[test]
    fn validation_rejects_negative_variance() {
        assert!(ForwardVarianceCurve::flat(-0.01).is_err());
        assert_eq!(
            ForwardVarianceCurve::flat(0.0)
                .unwrap()
                .get_value(1.0)
                .unwrap(),
            0.0
        );
        assert!(ForwardVarianceCurve::from_points(&[(0.0, -0.01)]).is_err());
    }

    #[test]
    fn validation_rejects_empty_points() {
        assert!(ForwardVarianceCurve::from_points(&[]).is_err());
    }

    #[test]
    fn validation_rejects_non_monotonic_times() {
        // Duplicate times
        assert!(ForwardVarianceCurve::from_points(&[(1.0, 0.04), (1.0, 0.05)]).is_err());
        assert!(ForwardVarianceCurve::from_points(&[(2.0, 0.04), (1.0, 0.05)]).is_err());
        // Reject both a reversal and a duplicate.
        assert!(
            ForwardVarianceCurve::from_points(&[(2.0, 0.04), (1.0, 0.05), (1.0, 0.06)]).is_err()
        );
    }

    #[test]
    fn validation_rejects_negative_time() {
        assert!(ForwardVarianceCurve::from_points(&[(-1.0, 0.04)]).is_err());
    }

    #[test]
    fn validation_rejects_invalid_data_and_queries() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(ForwardVarianceCurve::flat(value).is_err());
            assert!(ForwardVarianceCurve::from_points(&[(value, 0.04)]).is_err());
            assert!(ForwardVarianceCurve::from_points(&[(1.0, value)]).is_err());
        }
        let curve = ForwardVarianceCurve::flat(0.04).unwrap();
        for t in [f64::NAN, f64::NEG_INFINITY, f64::INFINITY, -0.01] {
            assert!(curve.get_value(t).is_err());
        }
    }
}
