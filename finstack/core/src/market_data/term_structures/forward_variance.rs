//! Forward variance curve for rough volatility models.
//!
//! The forward variance curve ξ₀(t) represents the market-implied forward
//! variance strip, typically extracted from the implied volatility surface via:
//!
//! ```text
//! ξ₀(t) = d/dt [σ_imp²(t) · t]
//! ```
//!
//! This curve is used as input to the rBergomi and related rough volatility
//! models where the initial forward variance curve governs the term structure of
//! variance.
//!
//! # Interpolation
//!
//! Piecewise linear interpolation between knots with flat extrapolation at the
//! boundaries. This matches the typical step-like structure of forward variance
//! strips extracted from discrete-expiry implied vol data.
//!
//! # References
//!
//! - Bayer, C., Friz, P., & Gatheral, J. (2016). "Pricing under rough
//!   volatility." *Quantitative Finance*, 16(6), 887–904.
//! - Gatheral, J., Jaisson, T., & Rosenbaum, M. (2018). "Volatility is rough."
//!   *Quantitative Finance*, 18(6), 933–949.

/// Forward variance curve ξ₀(t) for rough volatility models.
///
/// Represents the market-implied forward variance strip, typically extracted
/// from the vol surface via: ξ₀(t) = d/dt [σ_imp²(t) · t]
///
/// Used as input to rBergomi and related rough vol models where the initial
/// forward variance curve determines the term structure of variance.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawForwardVarianceCurve")]
pub struct ForwardVarianceCurve {
    /// Knot times (year fractions, strictly increasing, >= 0).
    times: Vec<f64>,
    /// Forward variance values at knot times (all > 0).
    values: Vec<f64>,
}

/// Raw deserialization state of [`ForwardVarianceCurve`].
///
/// Mirrors the serialized field layout exactly; conversion routes through
/// [`ForwardVarianceCurve::from_points`] so deserialized curves satisfy the
/// same invariants as constructed ones, and unknown fields are rejected.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawForwardVarianceCurve {
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
        ForwardVarianceCurve::from_points(&points)
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
            times: vec![0.0],
            values: vec![0.04],
        }
    }
}

impl ForwardVarianceCurve {
    /// Creates a flat forward variance curve (constant ξ₀(t) = v0).
    ///
    /// # Errors
    ///
    /// Returns an error if `v0` is not positive.
    pub fn flat(v0: f64) -> crate::Result<Self> {
        if v0 <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "ForwardVarianceCurve: flat variance must be positive, got {v0}"
            )));
        }
        Ok(Self {
            times: vec![0.0],
            values: vec![v0],
        })
    }

    /// Creates a forward variance curve from (time, forward_variance) pairs.
    ///
    /// Points are sorted by time internally before validation.
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
            if t < 0.0 {
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
            if v <= 0.0 {
                return Err(crate::Error::Validation(format!(
                    "ForwardVarianceCurve: forward variance must be positive, \
                     got {v} at time {t}"
                )));
            }
            times.push(t);
            values.push(v);
        }

        Ok(Self { times, values })
    }

    /// Evaluates the forward variance ξ₀(t) at time `t`.
    ///
    /// Uses linear interpolation between knots and flat extrapolation at the
    /// boundaries.
    pub fn value(&self, t: f64) -> f64 {
        debug_assert!(!self.times.is_empty());

        let n = self.times.len();

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
    /// Uses piecewise linear integration (trapezoidal rule between knots) for
    /// the region covered by the curve, with flat extrapolation beyond the
    /// boundaries.
    pub fn integrated_variance(&self, t: f64) -> f64 {
        debug_assert!(!self.times.is_empty());

        if t <= 0.0 {
            return 0.0;
        }

        let n = self.times.len();

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
}
