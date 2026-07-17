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
        locate_segment_unchecked, validate_knot_spacing, validate_positive_series,
        MIN_RELATIVE_KNOT_GAP,
    },
};
use std::vec::Vec;

// -----------------------------------------------------------------------------
// LinearStrategy
// -----------------------------------------------------------------------------

/// Strategy for piecewise linear interpolation on discount factors.
///
/// Simple linear interpolation between knot points. Fast and straightforward
/// but may produce negative forward rates (arbitrage) if discount factors
/// aren't carefully spaced. Prefer LogLinear or MonotoneConvex for yield curves.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LinearStrategy;

impl InterpolationStrategy for LinearStrategy {
    fn from_raw(
        knots: &[f64],
        _values: &[f64],
        _extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Self> {
        validate_knot_spacing(knots, MIN_RELATIVE_KNOT_GAP)?;
        Ok(Self)
    }

    fn interp(
        &self,
        x: f64,
        knots: &[f64],
        values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> f64 {
        use super::utils::check_extrapolation;

        if !x.is_finite() {
            return f64::NAN;
        }

        // Handle extrapolation based on policy
        // Safe access with NaN fallback for empty slices (shouldn't happen by construction)
        if let Some(val) = check_extrapolation(
            x,
            knots,
            extrapolation,
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => values.first().copied().unwrap_or(f64::NAN),
                ExtrapolationPolicy::FlatForward => {
                    let slope = segment_slope(knots, values, 0, 1);
                    values.first().copied().unwrap_or(f64::NAN) + slope * (x - knots[0])
                }
                _ => f64::NAN,
            },
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => values.last().copied().unwrap_or(f64::NAN),
                ExtrapolationPolicy::FlatForward => {
                    let n = knots.len();
                    let slope = segment_slope(knots, values, n - 2, n - 1);
                    values.last().copied().unwrap_or(f64::NAN) + slope * (x - knots[n - 1])
                }
                _ => f64::NAN,
            },
        ) {
            return val;
        }

        // Interior linear interpolation.
        // Exact knot hits are handled correctly: locate_segment returns idx where
        // knots[idx] <= x, so w = 0.0 when x == knots[idx] and w = 1.0 when
        // x == knots[idx+1] — both produce the exact knot value without a
        // separate binary search.
        // Safe: check_extrapolation returning None guarantees x is in [knots[0], knots[last]].
        let idx = locate_segment_unchecked(knots, x);
        let x0 = knots[idx];
        let x1 = knots[idx + 1];
        let y0 = values[idx];
        let y1 = values[idx + 1];
        let w = (x - x0) / (x1 - x0);
        y0 + w * (y1 - y0)
    }

    fn interp_prime(
        &self,
        x: f64,
        knots: &[f64],
        values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> f64 {
        use super::utils::check_extrapolation;

        if !x.is_finite() {
            return f64::NAN;
        }

        // Handle extrapolation based on policy
        if let Some(val) = check_extrapolation(
            x,
            knots,
            extrapolation,
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => 0.0,
                ExtrapolationPolicy::FlatForward => segment_slope(knots, values, 0, 1),
                _ => f64::NAN,
            },
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => 0.0,
                ExtrapolationPolicy::FlatForward => {
                    let n = knots.len();
                    segment_slope(knots, values, n - 2, n - 1)
                }
                _ => f64::NAN,
            },
        ) {
            return val;
        }

        // Interior: safe after check_extrapolation returned None
        let idx = locate_segment_unchecked(knots, x);
        segment_slope(knots, values, idx, idx + 1)
    }
}

// Helper for linear slope calculation
#[inline]
fn segment_slope(knots: &[f64], values: &[f64], left_index: usize, right_index: usize) -> f64 {
    let x0 = knots[left_index];
    let x1 = knots[right_index];
    let y0 = values[left_index];
    let y1 = values[right_index];
    (y1 - y0) / (x1 - x0)
}

// -----------------------------------------------------------------------------
// LogLinearStrategy
// -----------------------------------------------------------------------------

/// Strategy for log-linear interpolation of discount factors.
///
/// Performs linear interpolation on ln(DF), equivalent to piecewise-constant
/// zero rates. Guarantees positive forward rates and is commonly used for
/// government bond curves and simple yield curve construction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogLinearStrategy {
    /// Precomputed log(values) for efficient evaluation.
    log_values: Box<[f64]>,
}

impl InterpolationStrategy for LogLinearStrategy {
    fn from_raw(
        knots: &[f64],
        values: &[f64],
        _extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Self> {
        validate_knot_spacing(knots, MIN_RELATIVE_KNOT_GAP)?;
        validate_positive_series(values)?;
        // Precompute log(values) for efficiency
        let log_values: Vec<f64> = values.iter().map(|v| v.ln()).collect();
        Ok(Self {
            log_values: log_values.into_boxed_slice(),
        })
    }

    fn interp(
        &self,
        x: f64,
        knots: &[f64],
        _values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> f64 {
        use super::utils::check_extrapolation;

        if !x.is_finite() {
            return f64::NAN;
        }

        // Handle extrapolation based on policy
        // Safe access with NaN fallback for empty slices (shouldn't happen by construction)
        if let Some(val) = check_extrapolation(
            x,
            knots,
            extrapolation,
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => {
                    self.log_values.first().copied().unwrap_or(f64::NAN).exp()
                }
                ExtrapolationPolicy::FlatForward => {
                    let slope = log_segment_slope(&self.log_values, knots, 0, 1);
                    let first_log = self.log_values.first().copied().unwrap_or(f64::NAN);
                    let extrapolated_log = first_log + slope * (x - knots[0]);
                    extrapolated_log.exp()
                }
                _ => f64::NAN,
            },
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => {
                    self.log_values.last().copied().unwrap_or(f64::NAN).exp()
                }
                ExtrapolationPolicy::FlatForward => {
                    let n = knots.len();
                    let slope = log_segment_slope(&self.log_values, knots, n - 2, n - 1);
                    let last_log = self.log_values.last().copied().unwrap_or(f64::NAN);
                    let extrapolated_log = last_log + slope * (x - knots[n - 1]);
                    extrapolated_log.exp()
                }
                _ => f64::NAN,
            },
        ) {
            return val;
        }

        // Interior interpolation (exact knot hits produce w=0 or w=1,
        // yielding the exact knot value without a separate search).
        // Safe: check_extrapolation returning None guarantees x is in bounds.
        let idx = locate_segment_unchecked(knots, x);
        let x0 = knots[idx];
        let x1 = knots[idx + 1];
        let y0 = self.log_values[idx];
        let y1 = self.log_values[idx + 1];
        let w = (x - x0) / (x1 - x0);
        (y0 + w * (y1 - y0)).exp()
    }

    fn interp_prime(
        &self,
        x: f64,
        knots: &[f64],
        _values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> f64 {
        // For log-linear interpolation: f(x) = exp(y0 + w*(y1-y0)) where w = (x-x0)/(x1-x0)
        // The derivative is: df/dx = f(x) * (y1-y0)/(x1-x0)

        use super::utils::check_extrapolation;

        if !x.is_finite() {
            return f64::NAN;
        }

        // At boundaries, handle based on extrapolation policy
        if let Some(val) = check_extrapolation(
            x,
            knots,
            extrapolation,
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => 0.0,
                ExtrapolationPolicy::FlatForward => {
                    let slope = log_segment_slope(&self.log_values, knots, 0, 1);
                    let extrapolated_log = self.log_values[0] + slope * (x - knots[0]);
                    extrapolated_log.exp() * slope
                }
                _ => f64::NAN,
            },
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => 0.0,
                ExtrapolationPolicy::FlatForward => {
                    let n = knots.len();
                    let slope = log_segment_slope(&self.log_values, knots, n - 2, n - 1);
                    let extrapolated_log = self.log_values[n - 1] + slope * (x - knots[n - 1]);
                    extrapolated_log.exp() * slope
                }
                _ => f64::NAN,
            },
        ) {
            return val;
        }

        // Compute interpolated value and slope in a single binary search.
        // Safe: check_extrapolation returning None guarantees x is in bounds.
        let idx = locate_segment_unchecked(knots, x);
        let x0 = knots[idx];
        let x1 = knots[idx + 1];
        let y0 = self.log_values[idx];
        let y1 = self.log_values[idx + 1];
        let w = (x - x0) / (x1 - x0);
        let f_val = (y0 + w * (y1 - y0)).exp();
        let slope = (y1 - y0) / (x1 - x0);

        // Derivative: f(x) * (slope in log space)
        f_val * slope
    }
}

impl LogLinearStrategy {
    /// Access the log values (for serialization).
    pub fn log_values(&self) -> &[f64] {
        &self.log_values
    }
}

// Helper for log-linear slope calculation
#[inline]
fn log_segment_slope(
    log_values: &[f64],
    knots: &[f64],
    left_index: usize,
    right_index: usize,
) -> f64 {
    let x0 = knots[left_index];
    let x1 = knots[right_index];
    let y0 = log_values[left_index];
    let y1 = log_values[right_index];
    (y1 - y0) / (x1 - x0)
}

// -----------------------------------------------------------------------------
// PiecewiseQuadraticForwardStrategy
// -----------------------------------------------------------------------------

/// Strategy for piecewise quadratic forward interpolation (smooth forwards).
///
/// Builds a natural cubic spline in log-discount space so that the resulting
/// instantaneous forward curve is piecewise quadratic and C²-continuous.
/// This matches the “smooth forward” construction commonly used by Bloomberg.
///
/// # References
///
/// - de Boor, C. *A Practical Guide to Splines*. Canonical reference for the
///   underlying natural cubic spline construction. See
///   [De Boor Splines](../../../../docs/REFERENCES.md#de-boor-splines).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PiecewiseQuadraticForwardStrategy {
    /// Knot locations (copied for boundary evaluation).
    knots: Box<[f64]>,
    /// Cubic spline coefficients for log discount factor: y = a + b s + c s² + d s³.
    a: Box<[f64]>,
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
        // Enforce positivity (log transform requires DF > 0)
        validate_positive_series(values)?;
        validate_knot_spacing(knots, MIN_RELATIVE_KNOT_GAP)?;

        let n = knots.len();
        debug_assert!(n >= 2);

        // Convert to log discount factors (y = -ln(P))
        let y: Vec<f64> = values.iter().map(|&p| -p.ln()).collect();

        // Segment widths
        let mut h = Vec::with_capacity(n - 1);
        for w in knots.windows(2) {
            let width = w[1] - w[0];
            if width <= 0.0 {
                return Err(crate::error::InputError::NonMonotonicKnots.into());
            }
            h.push(width);
        }

        // Natural cubic spline second derivatives (m)
        let mut alpha = vec![0.0; n];
        for i in 1..n - 1 {
            alpha[i] = (3.0 / h[i]) * (y[i + 1] - y[i]) - (3.0 / h[i - 1]) * (y[i] - y[i - 1]);
        }

        let mut l = vec![0.0; n];
        let mut mu = vec![0.0; n];
        let mut z = vec![0.0; n];

        l[0] = 1.0;
        for i in 1..n - 1 {
            l[i] = 2.0 * (knots[i + 1] - knots[i - 1]) - h[i - 1] * mu[i - 1];
            if l[i].abs() < f64::EPSILON {
                return Err(crate::error::InputError::Invalid.into());
            }
            mu[i] = h[i] / l[i];
            z[i] = (alpha[i] - h[i - 1] * z[i - 1]) / l[i];
        }
        l[n - 1] = 1.0;

        let mut m = vec![0.0; n];
        for j in (0..n - 1).rev() {
            m[j] = z[j] - mu[j] * m[j + 1];
        }

        // Build cubic coefficients for each segment
        let mut a_coeff = Vec::with_capacity(n - 1);
        let mut b_coeff = Vec::with_capacity(n - 1);
        let mut c_coeff = Vec::with_capacity(n - 1);
        let mut d_coeff = Vec::with_capacity(n - 1);

        for i in 0..n - 1 {
            let hi = h[i];
            let ai = y[i];
            let bi = (y[i + 1] - y[i]) / hi - hi * (2.0 * m[i] + m[i + 1]) / 6.0;
            let ci = m[i] / 2.0;
            let di = (m[i + 1] - m[i]) / (6.0 * hi);
            a_coeff.push(ai);
            b_coeff.push(bi);
            c_coeff.push(ci);
            d_coeff.push(di);
        }

        Ok(Self {
            knots: knots.to_vec().into_boxed_slice(),
            a: a_coeff.into_boxed_slice(),
            b: b_coeff.into_boxed_slice(),
            c: c_coeff.into_boxed_slice(),
            d: d_coeff.into_boxed_slice(),
        })
    }

    fn interp(
        &self,
        x: f64,
        knots: &[f64],
        _values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> f64 {
        use super::utils::check_extrapolation;

        if !x.is_finite() {
            return f64::NAN;
        }

        if let Some(val) = check_extrapolation(
            x,
            knots,
            extrapolation,
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => self.boundary_df(0),
                ExtrapolationPolicy::FlatForward => self.flat_forward_df(x, 0),
                _ => f64::NAN,
            },
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => self.boundary_df(knots.len() - 1),
                ExtrapolationPolicy::FlatForward => self.flat_forward_df(x, knots.len() - 1),
                _ => f64::NAN,
            },
        ) {
            return val;
        }

        // Safe: check_extrapolation returning None guarantees x is in bounds.
        let idx = locate_segment_unchecked(knots, x);
        let s = x - knots[idx];
        let y = self.a[idx] + self.b[idx] * s + self.c[idx] * s * s + self.d[idx] * s * s * s;
        (-y).exp()
    }

    fn interp_prime(
        &self,
        x: f64,
        knots: &[f64],
        _values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> f64 {
        use super::utils::check_extrapolation;

        if !x.is_finite() {
            return f64::NAN;
        }

        if let Some(val) = check_extrapolation(
            x,
            knots,
            extrapolation,
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => 0.0,
                ExtrapolationPolicy::FlatForward => self.flat_forward_df_prime(x, 0),
                _ => f64::NAN,
            },
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => 0.0,
                ExtrapolationPolicy::FlatForward => self.flat_forward_df_prime(x, knots.len() - 1),
                _ => f64::NAN,
            },
        ) {
            return val;
        }

        // Safe: check_extrapolation returning None guarantees x is in bounds.
        let idx = locate_segment_unchecked(knots, x);
        let s = x - knots[idx];

        let y = self.a[idx] + self.b[idx] * s + self.c[idx] * s * s + self.d[idx] * s * s * s;
        let y_prime = self.b[idx] + 2.0 * self.c[idx] * s + 3.0 * self.d[idx] * s * s;
        let df = (-y).exp();
        -y_prime * df
    }
}

impl PiecewiseQuadraticForwardStrategy {
    #[inline]
    fn boundary_df(&self, boundary_index: usize) -> f64 {
        if boundary_index == 0 {
            (-self.a[0]).exp()
        } else {
            let last_seg = self.a.len() - 1;
            let h = self.knots[boundary_index] - self.knots[boundary_index - 1];
            let y = self.a[last_seg]
                + self.b[last_seg] * h
                + self.c[last_seg] * h * h
                + self.d[last_seg] * h * h * h;
            (-y).exp()
        }
    }

    #[inline]
    fn boundary_slope(&self, boundary_index: usize) -> f64 {
        if boundary_index == 0 {
            self.b[0]
        } else {
            let last_seg = self.a.len() - 1;
            let h = self.knots[boundary_index] - self.knots[boundary_index - 1];
            self.b[last_seg] + 2.0 * self.c[last_seg] * h + 3.0 * self.d[last_seg] * h * h
        }
    }

    #[inline]
    fn flat_forward_df(&self, x: f64, boundary_index: usize) -> f64 {
        let t0 = self.knots[boundary_index];
        let y0 = -self.boundary_df(boundary_index).ln();
        let slope = self.boundary_slope(boundary_index);
        let y = y0 + slope * (x - t0);
        (-y).exp()
    }

    #[inline]
    fn flat_forward_df_prime(&self, x: f64, boundary_index: usize) -> f64 {
        let df = self.flat_forward_df(x, boundary_index);
        let slope = self.boundary_slope(boundary_index);
        -slope * df
    }
}

// -----------------------------------------------------------------------------
// CubicHermiteStrategy
// -----------------------------------------------------------------------------

/// Strategy for monotone cubic Hermite interpolation (PCHIP).
///
/// Implements the Piecewise Cubic Hermite Interpolating Polynomial with
/// Fritsch-Carlson slope selection. Preserves monotonicity of input data,
/// ensuring no spurious oscillations. Requires monotone input discount factors.
///
/// # References
///
/// - Fritsch, F. N., & Carlson, R. E. (1980). "Monotone Piecewise Cubic
///   Interpolation." *SIAM Journal on Numerical Analysis*, 17(2), 238-246.
///   See [Fritsch-Carlson 1980](../../../../docs/REFERENCES.md#fritsch-carlson-1980).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
        validate_knot_spacing(knots, MIN_RELATIVE_KNOT_GAP)?;
        // Pre-compute monotone slopes (PCHIP / Fritsch-Carlson)
        let ms = compute_monotone_slopes(knots, values);
        Ok(Self { ms })
    }

    fn interp(
        &self,
        x: f64,
        knots: &[f64],
        values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> f64 {
        use super::utils::check_extrapolation;

        if !x.is_finite() {
            return f64::NAN;
        }

        // Handle extrapolation based on policy
        // Safe access with NaN fallback for empty slices (shouldn't happen by construction)
        if let Some(val) = check_extrapolation(
            x,
            knots,
            extrapolation,
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => values.first().copied().unwrap_or(f64::NAN),
                ExtrapolationPolicy::FlatForward => {
                    let x0 = knots[0];
                    let slope = self.ms.first().copied().unwrap_or(0.0);
                    let dx = x - x0;
                    values.first().copied().unwrap_or(f64::NAN) + slope * dx
                }
                _ => f64::NAN,
            },
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => values.last().copied().unwrap_or(f64::NAN),
                ExtrapolationPolicy::FlatForward => {
                    let x_last = knots.last().copied().unwrap_or(0.0);
                    let slope = self.ms.last().copied().unwrap_or(0.0);
                    let dx = x - x_last;
                    values.last().copied().unwrap_or(f64::NAN) + slope * dx
                }
                _ => f64::NAN,
            },
        ) {
            return val;
        }

        // Interior interpolation using cubic Hermite.
        // Exact knot hits produce t=0 or t=1, giving the exact knot value
        // through the basis functions without a separate binary search.
        // Safe: check_extrapolation returning None guarantees x is in bounds.
        let i = locate_segment_unchecked(knots, x);
        let x0 = knots[i];
        let x1 = knots[i + 1];
        let h = x1 - x0;
        // Normalised coordinate t ∈ (0,1)
        let t = (x - x0) / h;
        let t2 = t * t;
        let t3 = t2 * t;

        // Basis functions
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;

        // Values and slopes
        let f0 = values[i];
        let f1 = values[i + 1];
        let m0 = self.ms[i];
        let m1 = self.ms[i + 1];

        // Cubic Hermite formula
        h00 * f0 + h10 * h * m0 + h01 * f1 + h11 * h * m1
    }

    fn interp_prime(
        &self,
        x: f64,
        knots: &[f64],
        values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> f64 {
        use super::utils::check_extrapolation;

        if !x.is_finite() {
            return f64::NAN;
        }

        // Handle extrapolation based on policy
        if let Some(val) = check_extrapolation(
            x,
            knots,
            extrapolation,
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => 0.0,
                ExtrapolationPolicy::FlatForward => self.ms[0],
                _ => f64::NAN,
            },
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => 0.0,
                ExtrapolationPolicy::FlatForward => self.ms[self.ms.len() - 1],
                _ => f64::NAN,
            },
        ) {
            return val;
        }

        // Safe: check_extrapolation returning None guarantees x is in bounds.
        // Exact knot hits produce t=0 or t=1, giving the exact slope
        // through the derivative basis functions.
        let i = locate_segment_unchecked(knots, x);
        let x0 = knots[i];
        let x1 = knots[i + 1];
        let h = x1 - x0;
        // Normalised coordinate t ∈ (0,1)
        let t = (x - x0) / h;
        let t2 = t * t;

        // Derivative of basis functions w.r.t. t
        let h00_prime = 6.0 * t2 - 6.0 * t;
        let h10_prime = 3.0 * t2 - 4.0 * t + 1.0;
        let h01_prime = -6.0 * t2 + 6.0 * t;
        let h11_prime = 3.0 * t2 - 2.0 * t;

        // Values and slopes
        let f0 = values[i];
        let f1 = values[i + 1];
        let m0 = self.ms[i];
        let m1 = self.ms[i + 1];

        // Derivative w.r.t. t
        let df_dt = h00_prime * f0 + h10_prime * h * m0 + h01_prime * f1 + h11_prime * h * m1;

        // Convert to derivative w.r.t. x using chain rule: df/dx = (df/dt) * (dt/dx) = (df/dt) / h
        df_dt / h
    }
}

impl CubicHermiteStrategy {
    /// Access the slopes (for serialization or inspection).
    pub fn slopes(&self) -> &[f64] {
        &self.ms
    }
}

/// Compute shape-preserving slopes using the Fritsch-Carlson monotone scheme
/// (a.k.a. PCHIP slopes).
#[inline]
fn compute_monotone_slopes(xs: &[f64], ys: &[f64]) -> Box<[f64]> {
    let n = xs.len();
    debug_assert_eq!(n, ys.len());

    // When only two points are available we fall back to linear
    if n == 2 {
        let slope = (ys[1] - ys[0]) / (xs[1] - xs[0]);
        return vec![slope, slope].into_boxed_slice();
    }

    let mut ms = vec![0.0; n];

    // Compute intervals in a single iterator pass
    let (h, delta): (Vec<f64>, Vec<f64>) = xs
        .windows(2)
        .zip(ys.windows(2))
        .map(|(xw, yw)| {
            let hi = xw[1] - xw[0];
            let di = (yw[1] - yw[0]) / hi;
            (hi, di)
        })
        .unzip();

    // Interior points
    for i in 1..n - 1 {
        if delta[i - 1] == 0.0 || delta[i] == 0.0 || delta[i - 1].signum() != delta[i].signum() {
            ms[i] = 0.0;
        } else {
            let w1 = 2.0 * h[i] + h[i - 1];
            let w2 = h[i] + 2.0 * h[i - 1];
            ms[i] = (w1 + w2) / (w1 / delta[i - 1] + w2 / delta[i]);
        }
    }

    // Endpoints (monotone one-sided estimates)
    // m0
    ms[0] = ((2.0 * h[0] + h[1]) * delta[0] - h[0] * delta[1]) / (h[0] + h[1]);
    if ms[0].signum() != delta[0].signum() {
        ms[0] = 0.0;
    } else if delta[0].signum() != delta[1].signum() && ms[0].abs() > 3.0 * delta[0].abs() {
        ms[0] = 3.0 * delta[0];
    }

    // m_{n-1}
    let last = n - 1;
    ms[last] = ((2.0 * h[last - 1] + h[last - 2]) * delta[last - 1]
        - h[last - 1] * delta[last - 2])
        / (h[last - 1] + h[last - 2]);
    if ms[last].signum() != delta[last - 1].signum() {
        ms[last] = 0.0;
    } else if delta[last - 2].signum() != delta[last - 1].signum()
        && ms[last].abs() > 3.0 * delta[last - 1].abs()
    {
        ms[last] = 3.0 * delta[last - 1];
    }

    ms.into_boxed_slice()
}

// -----------------------------------------------------------------------------
// MonotoneConvexStrategy
// -----------------------------------------------------------------------------

/// Default epsilon for near-zero slope detection in MonotoneConvex.
pub const DEFAULT_MONOTONE_CONVEX_EPSILON: f64 = 1e-14;

/// Strategy for monotone-convex discount-factor interpolation (Hagan & West, 2006).
///
/// Implements the full Hagan–West monotone-convex interpolation method that operates
/// on **forward rates** (not yields). This is the industry standard for yield curve
/// construction used by Bloomberg and other systems, guaranteeing continuous forward
/// rates.
///
/// # Negative-rate curves
///
/// Per Hagan & West (2006), forward-positivity enforcement is an *optional
/// amelioration* that is only valid for positive curves. This strategy
/// auto-detects the regime at construction: if **all** discrete forwards are
/// non-negative, the positivity projection is applied (forwards stay ≥ 0); if
/// **any** discrete forward is negative (an increasing-DF / DF > 1 segment,
/// as in EUR/CHF/JPY negative-rate markets), the projection is skipped
/// entirely so negative forwards interpolate faithfully without clamping.
/// DF values must still be strictly positive and finite.
///
/// # Algorithm Overview
///
/// 1. Compute discrete forward rates from discount factors
/// 2. Estimate instantaneous forward rates at knot points using weighted interpolation
/// 3. Use a special quadratic function g(x) to interpolate forward rates within segments
/// 4. The integral of g(x) over each segment is zero, ensuring the average forward rate
///    matches the discrete forward rate
///
/// # References
///
/// Hagan, P. S., & West, G. (2006). "Interpolation Methods for Curve Construction."
/// *Applied Mathematical Finance*, 13(2), 89-129.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonotoneConvexStrategy {
    /// Discrete forward rates f^d_i for each segment (length n-1).
    fd: Box<[f64]>,
    /// Instantaneous forward rates f_i at each knot (length n).
    f: Box<[f64]>,
    /// Segment widths (knots[i+1] - knots[i]).
    dt: Box<[f64]>,
    /// Cumulative log discount factors at each knot: -ln(DF[i]).
    log_df: Box<[f64]>,
    /// Numerical tolerance for near-zero detection.
    epsilon: f64,
}

impl InterpolationStrategy for MonotoneConvexStrategy {
    fn from_raw(
        knots: &[f64],
        values: &[f64],
        _extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Self> {
        // DF values must be strictly positive and finite; increasing DFs
        // (negative forward rates) are accepted to support negative-rate
        // curves (EUR/CHF/JPY).
        validate_positive_series(values)?;
        validate_knot_spacing(knots, MIN_RELATIVE_KNOT_GAP)?;

        // Build using default epsilon
        let epsilon = DEFAULT_MONOTONE_CONVEX_EPSILON;
        Self::build_hagan_west(knots, values, epsilon)
    }

    fn interp(
        &self,
        x: f64,
        knots: &[f64],
        values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> f64 {
        use super::utils::check_extrapolation;

        if !x.is_finite() {
            return f64::NAN;
        }

        // Handle extrapolation based on policy
        // Safe access with NaN fallback for empty slices (shouldn't happen by construction)
        if let Some(val) = check_extrapolation(
            x,
            knots,
            extrapolation,
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => values.first().copied().unwrap_or(f64::NAN),
                ExtrapolationPolicy::FlatForward => {
                    // Extrapolate using f[0] as constant forward rate
                    let dt = x - knots[0];
                    let f0 = self.f.first().copied().unwrap_or(0.0);
                    let log_df0 = self.log_df.first().copied().unwrap_or(0.0);
                    let extra_integral = f0 * dt;
                    (-(log_df0 + extra_integral)).exp()
                }
                _ => f64::NAN,
            },
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => values.last().copied().unwrap_or(f64::NAN),
                ExtrapolationPolicy::FlatForward => {
                    // Extrapolate using f[n-1] as constant forward rate
                    let x_last = knots.last().copied().unwrap_or(0.0);
                    let dt = x - x_last;
                    let f_last = self.f.last().copied().unwrap_or(0.0);
                    let log_df_last = self.log_df.last().copied().unwrap_or(0.0);
                    let extra_integral = f_last * dt;
                    (-(log_df_last + extra_integral)).exp()
                }
                _ => f64::NAN,
            },
        ) {
            return val;
        }

        // Interior interpolation using Hagan-West formula.
        // Exact knot hits produce x=0 in interpolate_segment, giving the
        // exact knot value without a separate binary search.
        // Safe: check_extrapolation returning None guarantees x is in bounds.
        let i = locate_segment_unchecked(knots, x);
        self.interpolate_segment(i, x, knots)
    }

    fn interp_prime(
        &self,
        x: f64,
        knots: &[f64],
        _values: &[f64],
        extrapolation: ExtrapolationPolicy,
    ) -> f64 {
        use super::utils::check_extrapolation;

        if !x.is_finite() {
            return f64::NAN;
        }

        // Handle extrapolation based on policy
        if let Some(val) = check_extrapolation(
            x,
            knots,
            extrapolation,
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => 0.0,
                ExtrapolationPolicy::FlatForward => {
                    // d/dx[DF] = -f * DF for constant forward rate
                    let dt = x - knots[0];
                    let extra_integral = self.f[0] * dt;
                    let df = (-(self.log_df[0] + extra_integral)).exp();
                    -self.f[0] * df
                }
                _ => f64::NAN,
            },
            |policy| match policy {
                ExtrapolationPolicy::FlatZero => 0.0,
                ExtrapolationPolicy::FlatForward => {
                    let n = knots.len();
                    let dt = x - knots[n - 1];
                    let extra_integral = self.f[n - 1] * dt;
                    let df = (-(self.log_df[n - 1] + extra_integral)).exp();
                    -self.f[n - 1] * df
                }
                _ => f64::NAN,
            },
        ) {
            return val;
        }

        // Interior: d/dx[DF(t)] = -f(t) * DF(t)
        // Safe: check_extrapolation returning None guarantees x is in bounds.
        let i = locate_segment_unchecked(knots, x);
        let (df, fwd) = self.segment_df_and_forward(i, x, knots);
        -fwd * df
    }
}

impl MonotoneConvexStrategy {
    /// Construct with custom epsilon for near-zero slope detection.
    ///
    /// Implements Hagan-West monotone-convex interpolation of strictly
    /// positive discount factors. Increasing discount factors (and therefore
    /// negative forward rates) are allowed; monotonicity amelioration is only
    /// applied when all inferred discrete forwards are non-negative. `epsilon`
    /// controls near-zero slope handling and must lie in `(0, 1e-6]`.
    ///
    /// # Errors
    ///
    /// Returns an error if `epsilon` is outside that range, consecutive knots
    /// are too closely spaced for stable slope calculations, or any supplied
    /// value is non-finite or not strictly positive.
    ///
    /// # Panics
    ///
    /// Panics if fewer than two knots are supplied or `values.len()` does not
    /// match `knots.len()`. Callers needing input-shape validation should use
    /// the generic [`Interpolator`](super::Interpolator) constructor instead.
    pub fn with_epsilon(knots: &[f64], values: &[f64], epsilon: f64) -> crate::Result<Self> {
        use crate::error::InputError;

        // Validate epsilon is reasonable
        if epsilon <= 0.0 || epsilon > 1e-6 {
            return Err(InputError::Invalid.into());
        }

        // Same knot-spacing validation as the other strategy constructors.
        validate_knot_spacing(knots, MIN_RELATIVE_KNOT_GAP)?;
        // DF values must be strictly positive and finite; increasing DFs
        // (negative forward rates) are accepted.
        validate_positive_series(values)?;

        Self::build_hagan_west(knots, values, epsilon)
    }

    /// Build the Hagan-West monotone convex coefficients.
    ///
    /// This implements the algorithm from Hagan & West (2006):
    /// 1. Compute discrete forward rates from discount factors
    /// 2. Estimate instantaneous forward rates at knots
    /// 3. Apply monotonicity constraints
    fn build_hagan_west(knots: &[f64], dfs: &[f64], epsilon: f64) -> crate::Result<Self> {
        let n = knots.len();
        debug_assert!(n >= 2);

        // Step 1: Compute log discount factors and segment widths
        let log_df: Vec<f64> = dfs.iter().map(|&p| -p.ln()).collect();
        let dt: Vec<f64> = knots.windows(2).map(|w| w[1] - w[0]).collect();

        // Step 2: Compute discrete forward rates
        // f^d_i = (log_df[i] - log_df[i-1]) / dt[i-1] for segment [i-1, i]
        // We store f^d for segment i as fd[i] = forward rate from knot i to knot i+1
        let fd: Vec<f64> = (0..n - 1)
            .map(|i| (log_df[i + 1] - log_df[i]) / dt[i])
            .collect();

        // Step 3: Compute instantaneous forward rates at each knot
        // Interior knots: weighted average of adjacent discrete forwards
        // f_i = (λ_L * f^d_{i+1} + λ_R * f^d_i) / (λ_L + λ_R)
        // where λ_L = dt[i-1], λ_R = dt[i]
        let mut f: Vec<f64> = vec![0.0; n];

        if n == 2 {
            // Two-point case: linear forward rate
            f[0] = fd[0];
            f[1] = fd[0];
        } else {
            // Interior knots
            for i in 1..n - 1 {
                let lambda_l = dt[i - 1]; // width of left segment
                let lambda_r = dt[i]; // width of right segment
                                      // fd[i-1] is discrete fwd for segment to the left
                                      // fd[i] is discrete fwd for segment to the right
                f[i] = (lambda_l * fd[i] + lambda_r * fd[i - 1]) / (lambda_l + lambda_r);
            }

            // Boundary conditions from Hagan-West (2006):
            //
            // Extrapolate the *instantaneous* forward at the first/last knot from the
            // adjacent *discrete* forwards:
            //   f_0     = f^d_0 - 0.5 * (f^d_1     - f^d_0)
            //   f_{n-1} = f^d_{n-2} + 0.5 * (f^d_{n-2} - f^d_{n-3})
            //
            // This matches the standard "linear" extrapolation of discrete forwards at
            // the ends and avoids coupling the boundary forwards to the interior knot
            // estimates (which can otherwise amplify endpoint sensitivity for long tenors).
            //
            // Note: n >= 3 here (n == 2 handled above), so fd has at least 2 elements.
            // For n == 3: fd[n-2] = fd[1], fd[n-3] = fd[0] are both valid.
            // For n >= 4: all indices are well within bounds.
            // We use defensive indexing to guard against future code changes.
            f[0] = 1.5 * fd[0] - 0.5 * fd.get(1).copied().unwrap_or(fd[0]);
            let last_idx = (n.saturating_sub(3)).min(fd.len().saturating_sub(1));
            f[n - 1] = 1.5 * fd[n - 2] - 0.5 * fd[last_idx];

            // Apply the Hagan-West forward-positivity amelioration only when
            // every discrete forward is non-negative. Per Hagan & West (2006),
            // positivity enforcement is an optional amelioration that is only
            // valid for positive curves; if any discrete forward is negative
            // the input is a genuine negative-rate curve (DF > 1 somewhere)
            // and the projection is skipped so negative forwards interpolate
            // faithfully instead of being clamped toward zero.
            if fd.iter().all(|&v| v >= 0.0) {
                Self::apply_monotonicity_constraints(&mut f, &fd, epsilon);
            }
        }

        Ok(Self {
            fd: fd.into_boxed_slice(),
            f: f.into_boxed_slice(),
            dt: dt.into_boxed_slice(),
            log_df: log_df.into_boxed_slice(),
            epsilon,
        })
    }

    /// Apply Hagan-West (2006) monotonicity constraints to ensure non-negative
    /// forward rates across each segment.
    ///
    /// # Notation
    ///
    /// For segment `i`, the instantaneous forward model is
    ///
    /// ```text
    /// f(x) = fd_i + g(x),        x ∈ [0, 1]
    /// g(x) = α (1 - 4x + 3x²) + β (-2x + 3x²)
    /// α    = f[i]   - fd[i]      (left deviation from discrete forward)
    /// β    = f[i+1] - fd[i]      (right deviation from discrete forward)
    /// ```
    ///
    /// The constraint is `f(x) ≥ 0` on `[0, 1]`. When violated we project
    /// `(α, β)` onto the boundary of the feasibility region.
    ///
    /// # Projection strategy
    ///
    /// Because `g(x; α, β) = α · p(x) + β · q(x)` is linear in `(α, β)` for
    /// each fixed `x`, scaling `(α, β) → (η α, η β)` for some `η ∈ [0, 1]`
    /// scales `g` uniformly and in particular scales the segment's minimum
    /// deviation `(min_f − fd_i)` by `η`.
    ///
    /// If the unadjusted segment has `min_f < 0` (constraint violated), we
    /// pick the unique `η` that makes the scaled segment touch zero:
    ///
    /// ```text
    /// fd_i + η · (min_f - fd_i) = 0   ⇒   η = fd_i / (fd_i - min_f).
    /// ```
    ///
    /// Because `min_f < 0 ≤ fd_i` implies `fd_i - min_f > fd_i ≥ 0`, `η`
    /// lies in `(0, 1)` whenever `fd_i > 0`. This single-factor projection:
    ///
    /// * preserves the direction / ratio `α : β` (no axis-aligned
    ///   collapse), and therefore preserves the qualitative shape of the
    ///   segment forward;
    /// * reduces to flat `f[i] = f[i+1] = fd_i` only in the limit
    ///   `fd_i → 0⁺`, avoiding the pre-fix behaviour of collapsing to flat
    ///   whenever `fd_i` was merely "small enough" to be in a ZIRP regime.
    ///
    /// # Region taxonomy (for readers cross-referencing H&W Fig 6)
    ///
    /// The four-region Figure 6 taxonomy classifies `(α, β)` by sign and
    /// by whether the binding minimum occurs at an endpoint or at the
    /// interior critical point `x_c = (2α + β) / (3(α + β))`. The scalar
    /// projection above is exactly the H&W projection at the endpoint-
    /// binding regions and is a valid — if slightly more conservative —
    /// projection in the interior-binding regions; for curves where the
    /// interior minimum is the binding constraint it deviates from
    /// QuantLib's per-axis projection by at most `O(bp²)`, well within
    /// tolerance for this workspace's downstream consumers.
    ///
    /// # Reference
    ///
    /// Hagan, P. S., & West, G. (2006). "Interpolation Methods for
    /// Curve Construction." *Applied Mathematical Finance*, 13(2), §6
    /// and Figure 6.
    ///
    /// # Fixpoint sweep
    ///
    /// Knot value `f[i+1]` is shared between segments `i` and `i+1`, so the
    /// projection of a later segment can re-violate an earlier one. The
    /// sequential sweep is therefore repeated (bounded number of passes)
    /// until no segment requires projection.
    fn apply_monotonicity_constraints(f: &mut [f64], fd: &[f64], _epsilon: f64) {
        // Numerical floor protecting the `η = fd_i / (fd_i - min_f)`
        // division from pathologically tiny denominators.
        const DENOM_EPS: f64 = 1e-300;
        // Bounded fixpoint iteration: projecting segment i mutates the shared
        // knot value f[i+1], which can re-violate the previously projected
        // segment i (its β changes). Each projection only shrinks |α|, |β|
        // towards the feasible flat point, so the sweep is a contraction and
        // a small number of passes suffices in practice.
        const MAX_PROJECTION_PASSES: usize = 3;

        let n = f.len();
        for _pass in 0..MAX_PROJECTION_PASSES {
            let mut mutated = false;
            for i in 0..n - 1 {
                let fd_i = fd[i];
                let alpha = f[i] - fd_i;
                let beta = f[i + 1] - fd_i;

                // Trivially feasible: already flat within this segment.
                if alpha == 0.0 && beta == 0.0 {
                    continue;
                }

                // Check whether f(x) = fd_i + g(x) ever goes negative on [0, 1].
                let min_f = Self::segment_min_forward(alpha, beta, fd_i);
                if min_f >= 0.0 {
                    continue;
                }
                mutated = true;

                // The non-negative-forward invariant is infeasible when
                // fd_i ≤ 0: any non-zero (α, β) will push the segment below
                // zero somewhere and no η > 0 can rescue it. Flatten. (Since
                // the projection only runs when all discrete forwards are
                // non-negative, this branch only sees fd_i == 0; genuine
                // negative-rate curves skip the projection entirely.)
                if fd_i <= 0.0 {
                    f[i] = fd_i;
                    f[i + 1] = fd_i;
                    continue;
                }

                // Scalar-η projection onto the boundary min(f) = 0.
                let denom = fd_i - min_f; // strictly > 0 because min_f < 0 < fd_i
                if denom <= DENOM_EPS {
                    f[i] = fd_i;
                    f[i + 1] = fd_i;
                    continue;
                }

                let eta = (fd_i / denom).clamp(0.0, 1.0);
                f[i] = fd_i + eta * alpha;
                f[i + 1] = fd_i + eta * beta;

                // Numerical safety: after projection the segment minimum is 0
                // analytically. Floating-point can still produce a tiny
                // negative value at a knot — clamp to the H&W non-negativity
                // invariant.
                if f[i] < 0.0 {
                    f[i] = 0.0;
                }
                if f[i + 1] < 0.0 {
                    f[i + 1] = 0.0;
                }
            }
            if !mutated {
                break;
            }
        }
    }

    /// Compute the minimum forward rate across segment [0,1] for given
    /// alpha/beta deviations from discrete forward fd_i.
    fn segment_min_forward(alpha: f64, beta: f64, fd_i: f64) -> f64 {
        // f(x) = fd_i + alpha*(1 - 4x + 3x^2) + beta*(-2x + 3x^2)
        // Evaluate at endpoints
        let f_at_0 = fd_i + alpha; // g(0) = alpha
        let f_at_1 = fd_i + beta; // g(1) = beta
        let mut min_f = f_at_0.min(f_at_1);

        // Check critical point: g'(x) = alpha*(-4 + 6x) + beta*(-2 + 6x) = 0
        // => x_c = (4*alpha + 2*beta) / (6*(alpha + beta))
        //        = (2*alpha + beta) / (3*(alpha + beta))
        let sum = alpha + beta;
        if sum.abs() > 1e-15 {
            let x_c = (2.0 * alpha + beta) / (3.0 * sum);
            if x_c > 0.0 && x_c < 1.0 {
                let x2 = x_c * x_c;
                let g_c = alpha * (1.0 - 4.0 * x_c + 3.0 * x2) + beta * (-2.0 * x_c + 3.0 * x2);
                min_f = min_f.min(fd_i + g_c);
            }
        }
        min_f
    }

    /// Compute the forward rate at time t within segment i.
    ///
    /// Uses the Hagan-West formula:
    /// Interpolate the discount factor at time t within segment i.
    ///
    /// DF(t) = DF(t_i) * exp(-∫_{t_i}^{t} f(s) ds)
    ///
    /// The integral of f(s) = f^d + g(x) from t_i to t is:
    /// ∫ = f^d * (t - t_i) + dt * G(x)
    ///
    /// where G(x) = ∫_0^x g(u) du
    ///            = g_left * (x - 2x² + x³) + g_right * (-x² + x³)
    fn interpolate_segment(&self, i: usize, t: f64, knots: &[f64]) -> f64 {
        let dt_seg = self.dt[i];
        let x = (t - knots[i]) / dt_seg;
        let x2 = x * x;
        let x3 = x2 * x;

        // g values relative to THIS segment's discrete forward
        let fd_i = self.fd[i];
        let g_left = self.f[i] - fd_i;
        let g_right = self.f[i + 1] - fd_i;

        // G(x) = integral of g from 0 to x
        // G(x) = g_left * (x - 2x² + x³) + g_right * (-x² + x³)
        let g_integral = g_left * (x - 2.0 * x2 + x3) + g_right * (-x2 + x3);

        // Total integral from t_i to t
        let integral = fd_i * (t - knots[i]) + dt_seg * g_integral;

        // DF(t) = exp(-(log_df[i] + integral))
        (-(self.log_df[i] + integral)).exp()
    }

    /// Compute both the discount factor `DF(t)` and the instantaneous forward
    /// rate `f(t)` within segment `i` in a single pass, returning `(df, forward)`.
    ///
    /// Shares the `x`, `x²`, `x³` and g-deviation computations so derivative
    /// queries ([`interp_prime`](MonotoneConvexStrategy::interp_prime), used in
    /// CS01/DV01 bump loops) compute the discount factor and the segment forward
    /// rate together instead of recomputing the polynomial and a second `exp`.
    fn segment_df_and_forward(&self, i: usize, t: f64, knots: &[f64]) -> (f64, f64) {
        let dt_seg = self.dt[i];
        let x = (t - knots[i]) / dt_seg;
        let x2 = x * x;
        let x3 = x2 * x;

        // g values relative to THIS segment's discrete forward.
        let fd_i = self.fd[i];
        let g_left = self.f[i] - fd_i;
        let g_right = self.f[i + 1] - fd_i;

        // Forward: f(t) = fd_i + g(x).
        let g_x = g_left * (1.0 - 4.0 * x + 3.0 * x2) + g_right * (-2.0 * x + 3.0 * x2);
        let forward = fd_i + g_x;

        // DF: integral of f from t_i to t, then DF(t) = exp(-(log_df[i] + integral)).
        let g_integral = g_left * (x - 2.0 * x2 + x3) + g_right * (-x2 + x3);
        let integral = fd_i * (t - knots[i]) + dt_seg * g_integral;
        let df = (-(self.log_df[i] + integral)).exp();

        (df, forward)
    }

    /// Get the epsilon value used for near-zero detection.
    pub fn epsilon(&self) -> f64 {
        self.epsilon
    }

    /// Access the discrete forward rates (for inspection/debugging).
    pub fn discrete_forwards(&self) -> &[f64] {
        &self.fd
    }

    /// Access the instantaneous forward rates at knots (for inspection/debugging).
    pub fn instantaneous_forwards(&self) -> &[f64] {
        &self.f
    }
}
