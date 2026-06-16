//! Exact discretization for GBM with discrete dividends.
//!
//! Handles the jump discontinuity at ex-dividend dates while maintaining
//! exact GBM evolution between dividends.
//!
//! # Algorithm
//!
//! For each time step [t, t+Δt]:
//! 1. Identify dividends in (t, t+Δt]
//! 2. Sub-divide interval at dividend times
//! 3. Apply exact GBM evolution between sub-intervals
//! 4. Apply dividend jumps at ex-dividend times
//!
//! # Example
//!
//! ```text
//! Time:     t=0      t=0.25 (div)    t=0.5
//!           |-----------|--------------|
//! Step 1:   S₀ → S₁   (exact GBM)
//! Step 2:   S₁ → S₁'  (dividend jump: S₁' = S₁ - D)
//! Step 3:   S₁' → S₂  (exact GBM)
//! ```
//!
//! # Brownian shocks across sub-intervals
//!
//! Each GBM leg uses an independent standard normal from the `z` slice so that
//! sub-increment variances add correctly. Reusing one shock with
//! `√(Δtᵢ/Δt)` scaling fixes the total variance but distorts intermediate
//! marginals (path-dependent and dividend timing).

use super::super::process::gbm_dividends::{Dividend, GbmWithDividends};
use super::super::traits::Discretization;
use smallvec::SmallVec;

/// Stack-allocated capacity for the typical number of dividend
/// sub-intervals per simulation step (most steps see 0–1 dividends, so
/// the inline buffer of 4 covers virtually all cases without heap
/// allocation in the hot path).
type SubIntervals = SmallVec<[(f64, f64, Option<Dividend>); 4]>;

/// Exact discretization for GBM with discrete dividends.
///
/// Combines exact GBM evolution with discrete dividend jumps.
#[derive(Debug, Clone, Default)]
pub struct ExactGbmWithDividends;

impl ExactGbmWithDividends {
    /// Create a new exact GBM with dividends discretization.
    pub fn new() -> Self {
        Self
    }

    /// Evolve spot under GBM from time t to t+dt.
    ///
    /// Uses exact solution: S(t+dt) = S(t) * exp((r - q - σ²/2)*dt + σ*√dt*Z)
    fn evolve_gbm(&self, spot: f64, r: f64, q: f64, sigma: f64, dt: f64, z: f64) -> f64 {
        let drift = (r - q - 0.5 * sigma * sigma) * dt;
        let diffusion = sigma * dt.sqrt() * z;
        spot * (drift + diffusion).exp()
    }
}

impl Discretization<GbmWithDividends> for ExactGbmWithDividends {
    fn step(
        &self,
        process: &GbmWithDividends,
        t: f64,
        dt: f64,
        x: &mut [f64],
        z: &[f64],
        _work: &mut [f64],
    ) {
        let params = process.params();
        let r = params.r;
        let q = params.q;
        let sigma = params.sigma;

        let mut spot = x[0];

        // Find dividends in (t, t+dt]
        let dividends_in_period = process.dividends_in_interval(t, dt);

        if dividends_in_period.is_empty() {
            // No dividends: simple exact GBM evolution
            spot = self.evolve_gbm(spot, r, q, sigma, dt, z[0]);
        } else {
            let mut z_idx = 0_usize;
            // Sub-divide interval at dividend times. Most steps see 0–1
            // dividends so the inline SmallVec buffer of 4 keeps this on the
            // stack and avoids per-step heap allocation in the hot path.
            let mut sub_intervals: SubIntervals = SmallVec::new();
            let mut prev_time = t;

            for (div_time, div) in &dividends_in_period {
                // Add interval before dividend
                if *div_time > prev_time {
                    sub_intervals.push((prev_time, *div_time, None));
                }
                // Mark dividend application point
                sub_intervals.push((*div_time, *div_time, Some((*div).clone())));
                prev_time = *div_time;
            }

            // Add final interval after last dividend
            if prev_time < t + dt {
                sub_intervals.push((prev_time, t + dt, None));
            }

            for (start, end, div_opt) in sub_intervals.into_iter() {
                if let Some(div) = div_opt {
                    // Apply dividend jump
                    spot = div.apply(spot);
                } else {
                    // Evolve GBM for this sub-interval
                    let sub_dt = end - start;
                    if sub_dt > 1e-10 {
                        let z_seg = z[z_idx];
                        z_idx += 1;
                        spot = self.evolve_gbm(spot, r, q, sigma, sub_dt, z_seg);
                    }
                }
            }
        }

        // Update state
        x[0] = spot;
    }

    fn work_size(&self, _process: &GbmWithDividends) -> usize {
        0
    }

    fn scheme_id(&self) -> &'static str {
        // Satisfies `GbmWithDividends::dedicated_scheme` — this is the only
        // scheme that applies the discrete dividend jumps.
        "exact_gbm_with_dividends"
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::process::gbm::GbmParams;
    use super::super::super::process::gbm_dividends::Dividend;
    use super::*;

    #[test]
    fn test_exact_gbm_div_no_dividends() {
        // Without dividends, should behave like standard GBM
        let gbm_div = GbmWithDividends::new(
            GbmParams::new(0.05, 0.02, 0.2).unwrap(),
            vec![], // No dividends
        )
        .unwrap();

        let disc = ExactGbmWithDividends::new();
        let mut x = vec![100.0];
        let z = vec![0.5];
        let mut work = vec![];

        disc.step(&gbm_div, 0.0, 0.01, &mut x, &z, &mut work);

        // Should have evolved
        assert!(x[0] > 0.0);
        assert!(x[0] != 100.0);
    }

    #[test]
    fn test_exact_gbm_div_with_cash_dividend() {
        // With cash dividend, spot should jump down
        let dividends = vec![(0.005, Dividend::Cash(1.0))]; // Dividend at t=0.005

        let gbm_div =
            GbmWithDividends::new(GbmParams::new(0.05, 0.0, 0.2).unwrap(), dividends).unwrap();

        let disc = ExactGbmWithDividends::new();
        let mut x = vec![100.0];
        // Two GBM legs: [t, div) and (div, t+dt]; matches `num_factors = 1 + schedule.len()`.
        let z = vec![0.0, 0.0];
        let mut work = vec![];

        // Step that crosses dividend
        disc.step(&gbm_div, 0.0, 0.01, &mut x, &z, &mut work);

        // Spot should be less than 100 due to dividend
        // (exact value depends on GBM evolution + dividend jump)
        assert!(x[0] < 100.0);
        assert!(x[0] > 0.0);
    }

    #[test]
    fn test_exact_gbm_div_multiple_dividends() {
        // Multiple dividends in one step
        let dividends = vec![(0.003, Dividend::Cash(0.5)), (0.007, Dividend::Cash(0.5))];

        let gbm_div = GbmWithDividends::new(
            GbmParams::new(0.05, 0.0, 0.1).unwrap(), // Low vol for stability
            dividends,
        )
        .unwrap();

        let disc = ExactGbmWithDividends::new();
        let mut x = vec![100.0];
        let z = vec![0.0, 0.0, 0.0];
        let mut work = vec![];

        disc.step(&gbm_div, 0.0, 0.01, &mut x, &z, &mut work);

        // Should have applied both dividends
        assert!(x[0] < 100.0);
        assert!(x[0] > 98.0); // Approximately 100 - 0.5 - 0.5 = 99 (plus GBM drift)
    }

    #[test]
    fn test_exact_gbm_div_proportional_dividend() {
        let dividends = vec![(0.005, Dividend::Proportional(0.02))]; // 2% dividend

        let gbm_div = GbmWithDividends::new(
            GbmParams::new(0.05, 0.0, 0.0).unwrap(), // Zero vol for deterministic test
            dividends,
        )
        .unwrap();

        let disc = ExactGbmWithDividends::new();
        let mut x = vec![100.0];
        let z = vec![0.0, 0.0];
        let mut work = vec![];

        disc.step(&gbm_div, 0.0, 0.01, &mut x, &z, &mut work);

        // With zero vol and small dt, should be close to 100 * (1 - 0.02) = 98
        // (plus small drift effect)
        assert!(x[0] < 100.0);
        assert!(x[0] > 95.0);
    }

    #[test]
    fn test_spot_remains_positive() {
        // Large dividend that could make spot negative
        let dividends = vec![(0.005, Dividend::Cash(150.0))];

        let gbm_div =
            GbmWithDividends::new(GbmParams::new(0.05, 0.0, 0.2).unwrap(), dividends).unwrap();

        let disc = ExactGbmWithDividends::new();
        let mut x = vec![100.0];
        let z = vec![0.0, 0.0];
        let mut work = vec![];

        disc.step(&gbm_div, 0.0, 0.01, &mut x, &z, &mut work);

        // Dividend application should prevent negative spot
        assert!(x[0] >= 0.0);
    }
}
