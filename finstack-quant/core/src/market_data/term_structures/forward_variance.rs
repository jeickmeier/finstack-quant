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
//! Both representations extrapolate flat at their boundaries.
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
#[serde(try_from = "RawForwardVarianceCurve")]
pub struct ForwardVarianceCurve {
    /// Interpolation contract used by the constructor.
    interpolation: Interpolation,
    /// Point times or interval ends (strictly increasing year fractions).
    times: Vec<f64>,
    /// Point samples or constant interval variances (all > 0).
    values: Vec<f64>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
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
        match raw.interpolation {
            Interpolation::Linear => ForwardVarianceCurve::from_points(&points),
            Interpolation::ConstantIntervals => ForwardVarianceCurve::from_intervals(&points),
        }
    }
}

impl Default for ForwardVarianceCurve {
    /// Returns a flat curve at ξ₀ = 0.04 (≡ 20% flat vol).
    ///
    /// Intended as an infallible placeholder for `#[serde(default)]` when a
    /// caller-controlled constructor will overwrite the field anyway. Not a
    /// substitute for market-data-derived curves.
    fn default() -> Self {
        Self {
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: vec![0.04],
        }
    }
}

impl ForwardVarianceCurve {
    /// Creates a flat forward variance curve (constant ξ₀(t) = v0).
    ///
    /// # Arguments
    ///
    /// * `v0` - Finite, strictly positive annualized variance in decimal units
    ///   (for example, 0.04 corresponds to 20% annualized volatility).
    ///
    /// # Errors
    ///
    /// Returns an error if `v0` is not positive.
    pub fn flat(v0: f64) -> crate::Result<Self> {
        if !v0.is_finite() || v0 <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "ForwardVarianceCurve: flat variance must be positive, got {v0}"
            )));
        }
        Ok(Self {
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: vec![v0],
        })
    }

    /// Creates a forward variance curve from (time, forward_variance) pairs.
    ///
    /// Points are sorted by time internally before validation.
    ///
    /// # Arguments
    ///
    /// * `points` - Nonempty `(time, variance)` samples. Times are finite,
    ///   nonnegative year fractions; variances are finite, strictly positive,
    ///   annualized decimal variances. Duplicate times are rejected. Values
    ///   interpolate linearly between samples and extrapolate flat.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `points` is empty
    /// - any time is negative
    /// - times are not strictly increasing (after sorting)
    /// - any forward variance value is not positive
    pub fn from_points(points: &[(f64, f64)]) -> crate::Result<Self> {
        if points.is_empty() {
            return Err(crate::Error::Validation(
                "ForwardVarianceCurve: at least one point is required".to_string(),
            ));
        }

        let mut sorted: Vec<(f64, f64)> = points.to_vec();
        sorted.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut times = Vec::with_capacity(sorted.len());
        let mut values = Vec::with_capacity(sorted.len());

        for (i, &(t, v)) in sorted.iter().enumerate() {
            if !t.is_finite() || t < 0.0 {
                return Err(crate::Error::Validation(format!(
                    "ForwardVarianceCurve: time must be >= 0, got {t} at index {i}"
                )));
            }
            if i > 0 && t <= times[i - 1] {
                return Err(crate::Error::Validation(format!(
                    "ForwardVarianceCurve: times must be strictly increasing, \
                     got {} then {t} at index {i}",
                    times[i - 1]
                )));
            }
            if !v.is_finite() || v <= 0.0 {
                return Err(crate::Error::Validation(format!(
                    "ForwardVarianceCurve: forward variance must be positive, \
                     got {v} at time {t}"
                )));
            }
            times.push(t);
            values.push(v);
        }

        Ok(Self {
            interpolation: Interpolation::Linear,
            times,
            values,
        })
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
    ///   positive year fraction. Variances are finite, strictly positive,
    ///   annualized decimal variances. The last variance extrapolates flat beyond
    ///   the final end, and the first variance extrapolates to negative times.
    ///
    /// # Errors
    ///
    /// Returns an error for empty input, invalid times or variances, or unordered
    /// interval ends. Inputs are not sorted because order defines interval areas.
    pub fn from_intervals(intervals: &[(f64, f64)]) -> crate::Result<Self> {
        let mut previous = 0.0;
        for &(end, _) in intervals {
            if !end.is_finite() || end <= previous {
                return Err(crate::Error::Validation(format!(
                    "ForwardVarianceCurve: interval ends must increase strictly from zero, got {end} after {previous}"
                )));
            }
            previous = end;
        }
        let mut curve = Self::from_points(intervals)?;
        curve.interpolation = Interpolation::ConstantIntervals;
        Ok(curve)
    }

    /// Evaluates the forward variance ξ₀(t) at time `t`.
    ///
    /// Uses the interpolation contract selected by the constructor, with flat
    /// extrapolation at the boundaries.
    ///
    /// # Arguments
    ///
    /// * `t` - Evaluation time in year fractions. Nonfinite times return NaN.
    pub fn value(&self, t: f64) -> f64 {
        debug_assert!(!self.times.is_empty());
        if !t.is_finite() {
            return f64::NAN;
        }

        let n = self.times.len();

        if matches!(self.interpolation, Interpolation::ConstantIntervals) {
            let i = self.times.partition_point(|&end| end <= t).min(n - 1);
            return self.values[i];
        }

        // Flat extrapolation at boundaries
        if t <= self.times[0] {
            return self.values[0];
        }
        if t >= self.times[n - 1] {
            return self.values[n - 1];
        }

        // Find the interval [times[i], times[i+1]] containing t
        // Binary search: find rightmost index where times[i] <= t
        let i = match self
            .times
            .binary_search_by(|x| x.partial_cmp(&t).unwrap_or(std::cmp::Ordering::Equal))
        {
            Ok(idx) => return self.values[idx], // exact knot hit
            Err(idx) => idx - 1,                // t is between idx-1 and idx
        };

        let t0 = self.times[i];
        let t1 = self.times[i + 1];
        let v0 = self.values[i];
        let v1 = self.values[i + 1];

        let w = (t - t0) / (t1 - t0);
        v0 + w * (v1 - v0)
    }

    /// Computes the integrated variance ∫₀ᵗ ξ₀(s) ds.
    ///
    /// Integrates the constructor's interpolation contract exactly and
    /// extrapolates flat beyond the curve boundaries.
    ///
    /// # Arguments
    ///
    /// * `t` - Integration end in year fractions. Returns zero for nonpositive
    ///   times and NaN for nonfinite times.
    pub fn integrated_variance(&self, t: f64) -> f64 {
        debug_assert!(!self.times.is_empty());
        if !t.is_finite() {
            return f64::NAN;
        }

        if t <= 0.0 {
            return 0.0;
        }

        let n = self.times.len();

        if matches!(self.interpolation, Interpolation::ConstantIntervals) {
            let mut start = 0.0;
            let mut integral = 0.0;
            for (&end, &variance) in self.times.iter().zip(&self.values) {
                integral += variance * (t.min(end) - start);
                if t <= end {
                    return integral;
                }
                start = end;
            }
            return integral + self.values[n - 1] * (t - start);
        }

        // If t is at or before the first knot, flat extrapolation from v[0]
        if t <= self.times[0] {
            return self.values[0] * t;
        }

        let mut integral = 0.0;

        // Integrate from 0 to times[0] using flat extrapolation of values[0]
        integral += self.values[0] * self.times[0];

        // Integrate piecewise linear segments
        for i in 0..n - 1 {
            let t0 = self.times[i];
            let t1 = self.times[i + 1];
            let v0 = self.values[i];
            let v1 = self.values[i + 1];

            if t <= t0 {
                // t is before this segment; we already accounted for it
                break;
            }

            let seg_end = t.min(t1);
            let dt = seg_end - t0;

            // Linear interp value at seg_end
            let w = (seg_end - t0) / (t1 - t0);
            let v_end = v0 + w * (v1 - v0);

            // Trapezoidal area for this portion of the segment
            integral += 0.5 * (v0 + v_end) * dt;

            if t <= t1 {
                return integral;
            }
        }

        // t is beyond the last knot — flat extrapolation from values[n-1]
        let last_v = self.values[n - 1];
        integral += last_v * (t - self.times[n - 1]);

        integral
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-12;

    #[test]
    fn flat_curve_value() {
        let c = ForwardVarianceCurve::flat(0.04).unwrap();
        assert!((c.value(0.0) - 0.04).abs() < TOL);
        assert!((c.value(1.0) - 0.04).abs() < TOL);
        assert!((c.value(10.0) - 0.04).abs() < TOL);
    }

    #[test]
    fn flat_curve_integrated_variance() {
        let c = ForwardVarianceCurve::flat(0.04).unwrap();
        for &t in &[0.0, 0.5, 1.0, 5.0, 10.0] {
            let expected = 0.04 * t;
            assert!(
                (c.integrated_variance(t) - expected).abs() < TOL,
                "integrated_variance({t}) = {}, expected {expected}",
                c.integrated_variance(t)
            );
        }
    }

    #[test]
    fn interval_variances_preserve_areas_and_boundary_values() {
        let curve = ForwardVarianceCurve::from_intervals(&[(0.25, 0.04), (1.0, 0.12), (1.5, 0.02)])
            .unwrap();
        for (time, expected) in [(0.25, 0.01), (1.0, 0.1), (1.5, 0.11), (2.0, 0.12)] {
            assert!((curve.integrated_variance(time) - expected).abs() < TOL);
        }
        assert_eq!(curve.value(0.0), 0.04);
        assert_eq!(curve.value(0.25), 0.12);
        assert_eq!(curve.value(1.0), 0.02);
        assert_eq!(curve.value(3.0), 0.02);
        assert!((curve.integrated_variance(0.5) - 0.04).abs() < TOL);
    }

    #[test]
    fn intervals_require_ordered_positive_ends_and_positive_variance() {
        for intervals in [
            vec![],
            vec![(0.0, 0.04)],
            vec![(1.0, 0.04), (0.5, 0.12)],
            vec![(0.5, 0.04), (0.5, 0.12)],
            vec![(f64::NAN, 0.04)],
            vec![(1.0, 0.0)],
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
                assert_eq!(restored.value(time), curve.value(time));
                assert_eq!(
                    restored.integrated_variance(time),
                    curve.integrated_variance(time)
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
        assert!((c.value(0.0) - 0.04).abs() < TOL);
        assert!((c.value(0.5) - 0.04).abs() < TOL);

        // At knots
        assert!((c.value(1.0) - 0.04).abs() < TOL);
        assert!((c.value(3.0) - 0.08).abs() < TOL);

        // Linear interpolation: midpoint at t=2.0
        assert!((c.value(2.0) - 0.06).abs() < TOL);

        // Flat extrapolation right
        assert!((c.value(5.0) - 0.08).abs() < TOL);
    }

    #[test]
    fn integrated_variance_numerical_check() {
        let c = ForwardVarianceCurve::from_points(&[(1.0, 0.04), (3.0, 0.08)]).unwrap();

        // Numerical integration via fine Riemann sum
        let t_end = 4.0;
        let steps = 100_000;
        let dt = t_end / steps as f64;
        let mut numerical = 0.0;
        for i in 0..steps {
            let s = (i as f64 + 0.5) * dt;
            numerical += c.value(s) * dt;
        }

        let analytical = c.integrated_variance(t_end);
        assert!(
            (analytical - numerical).abs() < 1e-6,
            "analytical={analytical}, numerical={numerical}"
        );
    }

    #[test]
    fn validation_rejects_negative_variance() {
        assert!(ForwardVarianceCurve::flat(-0.01).is_err());
        assert!(ForwardVarianceCurve::flat(0.0).is_err());
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
        // Reversed after sort still duplicates
        assert!(
            ForwardVarianceCurve::from_points(&[(2.0, 0.04), (1.0, 0.05), (1.0, 0.06)]).is_err()
        );
    }

    #[test]
    fn validation_rejects_negative_time() {
        assert!(ForwardVarianceCurve::from_points(&[(-1.0, 0.04)]).is_err());
    }

    #[test]
    fn validation_rejects_non_finite_data_and_queries_propagate_nan() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(ForwardVarianceCurve::flat(value).is_err());
            assert!(ForwardVarianceCurve::from_points(&[(value, 0.04)]).is_err());
            assert!(ForwardVarianceCurve::from_points(&[(1.0, value)]).is_err());
        }
        let curve = ForwardVarianceCurve::flat(0.04).unwrap();
        assert!(curve.value(f64::NAN).is_nan());
        assert!(curve.integrated_variance(f64::INFINITY).is_nan());
    }
}
