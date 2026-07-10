//! PD term structure: cumulative and marginal default probability curves.
//!
//! Stores (tenor, cumulative_pd) pairs with log-linear interpolation on
//! survival probability (equivalent to piecewise-constant hazard rates).

use serde::{Deserialize, Serialize};

use crate::credit::migration::TransitionMatrix;

use super::error::PdCalibrationError;

/// A term structure of cumulative default probabilities.
///
/// Stores (tenor, cumulative_pd) pairs where cumulative PD is monotonically
/// non-decreasing and bounded in [0, 1]. Interpolation is log-linear on
/// survival probability (equivalent to piecewise-constant hazard rates).
///
/// Construct via [`PdTermStructureBuilder`]; this type has no public
/// constructor.
///
/// # Examples
///
/// Build a term structure from explicit cumulative PDs and read off
/// cumulative, marginal, and instantaneous hazard rates:
///
/// ```
/// use finstack_quant_core::credit::pd::PdTermStructureBuilder;
///
/// let ts = PdTermStructureBuilder::new()
///     .with_cumulative_pds(&[(1.0, 0.002), (3.0, 0.008), (5.0, 0.018)])
///     .build()
///     .expect("valid term structure");
///
/// // Cumulative PD interpolates log-linearly on survival probability.
/// let pd_2y = ts.cumulative_pd(2.0);
/// assert!(pd_2y > 0.002 && pd_2y < 0.008);
///
/// // Marginal PD over [t1, t2] is conditional on survival to t1.
/// let fwd = ts.marginal_pd(1.0, 2.0);
/// assert!(fwd > 0.0 && fwd < 1.0);
///
/// // Hazard rate is piecewise constant between grid points.
/// let h = ts.hazard_rate(2.0);
/// assert!(h > 0.0);
///
/// // Inspect the underlying grid.
/// assert_eq!(ts.tenors(), &[1.0, 3.0, 5.0]);
/// assert_eq!(ts.cumulative_pds(), &[0.002, 0.008, 0.018]);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdTermStructure {
    /// Sorted tenor grid in years.
    tenors: Vec<f64>,
    /// Cumulative default probabilities at each tenor.
    cumulative_pds: Vec<f64>,
}

impl PdTermStructure {
    /// Start building a PD term structure from cumulative PDs, transition
    /// matrices, or other sources.
    ///
    /// This is the preferred entry point, consistent with other curve builders.
    #[must_use]
    pub fn builder() -> PdTermStructureBuilder {
        PdTermStructureBuilder::new()
    }

    /// Cumulative default probability at an arbitrary horizon via
    /// log-linear interpolation on survival probability.
    ///
    /// - For t <= first tenor: flat extrapolation (constant hazard rate).
    /// - For t >= last tenor: flat extrapolation of final hazard rate.
    #[must_use]
    pub fn cumulative_pd(&self, t: f64) -> f64 {
        if !t.is_finite() {
            return f64::NAN;
        }
        if t <= 0.0 {
            return 0.0;
        }
        if self.tenors.is_empty() {
            return 0.0;
        }

        let n = self.tenors.len();

        if t <= self.tenors[0] {
            // Flat extrapolation: constant hazard from time 0 to first tenor
            let h = self.hazard_rate_segment(0);
            1.0 - (-h * t).exp()
        } else if t >= self.tenors[n - 1] {
            // Flat extrapolation of final hazard rate
            let h = self.hazard_rate_segment(n - 1);
            let s_last = 1.0 - self.cumulative_pds[n - 1];
            let dt = t - self.tenors[n - 1];
            1.0 - s_last * (-h * dt).exp()
        } else {
            // Binary search for the enclosing interval
            let idx = match self.tenors.binary_search_by(|probe| {
                probe.partial_cmp(&t).unwrap_or(std::cmp::Ordering::Equal)
            }) {
                Ok(i) => return self.cumulative_pds[i],
                Err(i) => i - 1,
            };

            // Log-linear interpolation on survival probability
            let t0 = self.tenors[idx];
            let t1 = self.tenors[idx + 1];
            let s0 = 1.0 - self.cumulative_pds[idx];
            let s1 = 1.0 - self.cumulative_pds[idx + 1];

            if s0 <= 0.0 || s1 <= 0.0 {
                // Degenerate: survival is zero, PD = 1
                return 1.0;
            }

            let frac = (t - t0) / (t1 - t0);
            let ln_s = s0.ln() * (1.0 - frac) + s1.ln() * frac;
            1.0 - ln_s.exp()
        }
    }

    /// Marginal (forward) default probability between t1 and t2,
    /// conditional on survival to t1.
    ///
    /// marginal = (S(t1) - S(t2)) / S(t1)
    ///
    /// Returns 0.0 if t2 <= t1 or survival at t1 is zero.
    #[must_use]
    pub fn marginal_pd(&self, t1: f64, t2: f64) -> f64 {
        if !t1.is_finite() || !t2.is_finite() {
            return f64::NAN;
        }
        if t2 <= t1 {
            return 0.0;
        }
        let s1 = 1.0 - self.cumulative_pd(t1);
        if s1 <= 0.0 {
            return 0.0;
        }
        let s2 = 1.0 - self.cumulative_pd(t2);
        (s1 - s2) / s1
    }

    /// Annualised hazard rate at time t (piecewise constant).
    ///
    /// For t in [t_i, t_{i+1}]: h = -ln(S(t_{i+1})/S(t_i)) / (t_{i+1} - t_i)
    #[must_use]
    pub fn hazard_rate(&self, t: f64) -> f64 {
        if !t.is_finite() {
            return f64::NAN;
        }
        if self.tenors.is_empty() {
            return 0.0;
        }

        let n = self.tenors.len();

        if t <= self.tenors[0] {
            self.hazard_rate_segment(0)
        } else if t >= self.tenors[n - 1] {
            self.hazard_rate_segment(n - 1)
        } else {
            let idx = match self.tenors.binary_search_by(|probe| {
                probe.partial_cmp(&t).unwrap_or(std::cmp::Ordering::Equal)
            }) {
                Ok(i) => i,
                Err(i) => i - 1,
            };
            self.hazard_rate_between(idx, idx + 1)
        }
    }

    /// Tenor grid.
    #[must_use]
    pub fn tenors(&self) -> &[f64] {
        &self.tenors
    }

    /// Cumulative PDs at the tenor grid points.
    #[must_use]
    pub fn cumulative_pds(&self) -> &[f64] {
        &self.cumulative_pds
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Hazard rate for the first segment (from time 0 to first tenor).
    fn hazard_rate_segment(&self, idx: usize) -> f64 {
        if idx == 0 {
            let s = 1.0 - self.cumulative_pds[0];
            if s <= 0.0 || self.tenors[0] <= 0.0 {
                return 0.0;
            }
            -s.ln() / self.tenors[0]
        } else if idx < self.tenors.len() {
            self.hazard_rate_between(idx.saturating_sub(1), idx)
        } else {
            // Beyond last tenor: use last segment's rate
            let n = self.tenors.len();
            if n < 2 {
                self.hazard_rate_segment(0)
            } else {
                self.hazard_rate_between(n - 2, n - 1)
            }
        }
    }

    /// Hazard rate between two tenor grid points.
    fn hazard_rate_between(&self, i: usize, j: usize) -> f64 {
        let s_i = 1.0 - self.cumulative_pds[i];
        let s_j = 1.0 - self.cumulative_pds[j];
        let dt = self.tenors[j] - self.tenors[i];
        if s_i <= 0.0 || s_j <= 0.0 || dt <= 0.0 {
            return 0.0;
        }
        -(s_j / s_i).ln() / dt
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for [`PdTermStructure`] from multiple sources.
///
/// Accepts PD data from transition matrices, explicit (tenor, pd) pairs,
/// or other sources. When multiple sources provide data for overlapping
/// tenors, the builder averages them.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::credit::pd::{PdTermStructureBuilder, PdTermStructure};
///
/// let ts = PdTermStructureBuilder::new()
///     .with_cumulative_pds(&[(1.0, 0.002), (3.0, 0.008), (5.0, 0.018)])
///     .build()
///     .expect("valid term structure");
///
/// let pd_2y = ts.cumulative_pd(2.0);
/// assert!(pd_2y > 0.002 && pd_2y < 0.008);
/// ```
#[derive(Debug)]
pub struct PdTermStructureBuilder {
    points: Vec<(f64, f64)>,
}

impl PdTermStructureBuilder {
    /// Create a new empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self { points: Vec::new() }
    }

    /// Add explicit (tenor, cumulative_pd) pairs.
    #[must_use]
    pub fn with_cumulative_pds(mut self, pairs: &[(f64, f64)]) -> Self {
        self.points.extend_from_slice(pairs);
        self
    }

    /// Extract cumulative PDs from a [`TransitionMatrix`] for a given
    /// initial rating, at tenors that are integer multiples of the matrix's
    /// horizon.
    ///
    /// Uses matrix powers: with horizon `h`, `P(k·h) = P^k` and
    /// `PD(k·h) = P^k[rating, default]`. Each tenor must satisfy
    /// `tenor = k·h` for some non-negative integer `k` (within `1e-9` on the
    /// step count `tenor / h`); tenors are **not** silently rounded. For example,
    /// a 6-month matrix (`h = 0.5`) supports tenors 0.5, 1.0, 1.5, …
    /// ()` was ignored
    /// and tenors were silently rounded to integer powers.)
    ///
    /// # Errors
    ///
    /// - [`PdCalibrationError::NoDefaultState`] if the matrix has no default state.
    /// - [`PdCalibrationError::UnknownRating`] if `initial_rating` is not in the scale.
    /// - [`PdCalibrationError::InvalidTenor`] if any tenor is negative or non-finite.
    /// - [`PdCalibrationError::TenorNotMultipleOfHorizon`] if any tenor is not
    ///   an integer multiple of `tm.horizon()`; use a generator-based
    ///   projection (`GeneratorMatrix` + `project`) for such tenors.
    pub fn from_transition_matrix(
        mut self,
        tm: &TransitionMatrix,
        initial_rating: &str,
        tenors: &[f64],
    ) -> Result<Self, PdCalibrationError> {
        let scale = tm.scale();
        let default_idx = scale
            .default_state()
            .ok_or(PdCalibrationError::NoDefaultState)?;
        let rating_idx =
            scale
                .index_of(initial_rating)
                .ok_or_else(|| PdCalibrationError::UnknownRating {
                    rating: initial_rating.to_owned(),
                })?;

        // Compute matrix powers for tenors that are integer multiples of the
        // matrix horizon.
        let base = tm.as_matrix().clone();
        let n = base.nrows();
        let horizon = tm.horizon();

        /// Absolute tolerance on the step count `tenor / horizon` for
        /// accepting a tenor as an integer multiple of the matrix horizon.
        const MULTIPLE_TOL: f64 = 1e-9;

        for &tenor in tenors {
            if !tenor.is_finite() || tenor < 0.0 {
                return Err(PdCalibrationError::InvalidTenor { value: tenor });
            }
            let steps = tenor / horizon;
            let power_f = steps.round();
            if (steps - power_f).abs() > MULTIPLE_TOL {
                return Err(PdCalibrationError::TenorNotMultipleOfHorizon { tenor, horizon });
            }
            let power = power_f as u32;
            if power == 0 {
                self.points.push((tenor, 0.0));
                continue;
            }

            // Compute base^power by repeated squaring
            let mut result = nalgebra::DMatrix::identity(n, n);
            let mut current_base = base.clone();
            let mut exp = power;
            while exp > 0 {
                if exp % 2 == 1 {
                    result = &result * &current_base;
                }
                current_base = &current_base * &current_base;
                exp /= 2;
            }

            let pd = result[(rating_idx, default_idx)];
            self.points.push((tenor, pd.clamp(0.0, 1.0)));
        }

        Ok(self)
    }

    /// Build the term structure, enforcing monotonicity.
    ///
    /// If cumulative PDs are not monotonically non-decreasing after sorting
    /// by tenor, applies isotonic regression to enforce monotonicity.
    ///
    /// # Errors
    ///
    /// - [`PdCalibrationError::EmptyTermStructure`] if no points were added.
    /// - [`PdCalibrationError::InvalidTenor`] if any tenor is <= 0.
    /// - [`PdCalibrationError::NonMonotonicCumulativePds`] if the cumulative
    ///   PDs are still decreasing after isotonic regression (defensive check;
    ///   should be unreachable).
    pub fn build(self) -> Result<PdTermStructure, PdCalibrationError> {
        if self.points.is_empty() {
            return Err(PdCalibrationError::EmptyTermStructure);
        }

        // Validate tenors and cumulative probabilities before sorting or
        // isotonic regression. NaN must never participate in ordering or
        // clamping because both can turn it into plausible data.
        for &(t, pd) in &self.points {
            if t <= 0.0 || !t.is_finite() {
                return Err(PdCalibrationError::InvalidTenor { value: t });
            }
            if !pd.is_finite() {
                return Err(PdCalibrationError::NonFiniteValue { value: pd });
            }
            if !(0.0..=1.0).contains(&pd) {
                return Err(PdCalibrationError::ValueOutOfRange {
                    value: pd,
                    min: 0.0,
                    max: 1.0,
                });
            }
        }

        // Sort by tenor, average duplicate tenors
        let mut sorted: Vec<(f64, f64)> = self.points;
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        // Merge duplicate tenors by averaging
        let mut merged: Vec<(f64, f64)> = Vec::new();
        let mut i = 0;
        while i < sorted.len() {
            let mut j = i + 1;
            let mut sum_pd = sorted[i].1;
            let mut count = 1.0;
            while j < sorted.len() && (sorted[j].0 - sorted[i].0).abs() < 1e-12 {
                sum_pd += sorted[j].1;
                count += 1.0;
                j += 1;
            }
            merged.push((sorted[i].0, sum_pd / count));
            i = j;
        }

        // Enforce monotonicity via pool-adjacent-violators (isotonic regression)
        let mut pds: Vec<f64> = merged.iter().map(|p| p.1).collect();
        isotonic_regression(&mut pds);

        // Belt and braces: validate the documented monotonicity invariant.
        if pds.windows(2).any(|w| w[1] < w[0]) {
            return Err(PdCalibrationError::NonMonotonicCumulativePds);
        }

        let tenors: Vec<f64> = merged.iter().map(|p| p.0).collect();

        Ok(PdTermStructure {
            tenors,
            cumulative_pds: pds,
        })
    }
}

impl Default for PdTermStructureBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Pool-adjacent-violators algorithm for isotonic (non-decreasing) regression.
///
/// Maintains blocks of `(value_sum, count)`. Scanning forward, each new point
/// starts its own block; while the previous block's mean exceeds the new
/// block's mean, the blocks are merged (weighted average). Finally each
/// block's mean is expanded back over its member positions, guaranteeing a
/// monotone non-decreasing result (e.g. input `[3, 1, 1]` pools to
/// `[5/3, 5/3, 5/3]`).
///
/// # References
///
/// - Barlow, Bartholomew, Bremner & Brunk (1972). *Statistical Inference
///   Under Order Restrictions*. Wiley. Chapter 1 (PAV algorithm).
fn isotonic_regression(values: &mut [f64]) {
    let n = values.len();
    if n <= 1 {
        return;
    }

    // Blocks of pooled values: (sum of member values, member count).
    let mut sums: Vec<f64> = Vec::with_capacity(n);
    let mut counts: Vec<usize> = Vec::with_capacity(n);

    for &v in values.iter() {
        sums.push(v);
        counts.push(1);
        // Merge backward while the previous block's mean violates monotonicity.
        while sums.len() > 1 {
            let last = sums.len() - 1;
            let mean_last = sums[last] / counts[last] as f64;
            let mean_prev = sums[last - 1] / counts[last - 1] as f64;
            if mean_prev > mean_last {
                sums[last - 1] += sums[last];
                counts[last - 1] += counts[last];
                sums.pop();
                counts.pop();
            } else {
                break;
            }
        }
    }

    // Expand block means back over their member positions, clamped to [0, 1].
    let mut idx = 0;
    for (sum, count) in sums.iter().zip(counts.iter()) {
        let mean = (sum / *count as f64).clamp(0.0, 1.0);
        for _ in 0..*count {
            values[idx] = mean;
            idx += 1;
        }
    }
}
