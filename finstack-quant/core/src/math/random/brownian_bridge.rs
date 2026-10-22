//! Brownian bridge construction for path-dependent options.
//!
//! The Brownian bridge construction orders random shocks to reduce
//! effective dimension for QMC, particularly effective for barrier
//! and path-dependent options.
//!
//! # Algorithm
//!
//! Instead of generating path sequentially (0 → 1 → 2 → ... → N),
//! use binary subdivision:
//! 1. Generate terminal point (N)
//! 2. Generate midpoint (N/2) conditional on terminal
//! 3. Recursively fill in quarters, eighths, etc.
//!
//! This reordering reduces effective dimension because:
//! - Early dimensions determine overall path shape (most important)
//! - Later dimensions add local detail (less important)
//!
//! # Benefits for QMC
//!
//! - Reduces effective dimension from O(N) to O(log N) for smooth payoffs
//! - Particularly effective for barrier options (hitting time well-approximated by few dimensions)
//! - Can improve convergence from O(N^{-1/2}) to O(N^{-1}) or better
//!
//! Reference: Moskowitz & Caflisch (1996) - "Smoothness and dimension reduction in QMC"

use crate::{Error, Result};

/// One midpoint and its already populated neighbours in bridge order.
struct BridgeStep {
    index: usize,
    left: usize,
    right: usize,
}

/// Brownian bridge construction order.
///
/// Generates the sequence of time indices to sample in bridge order.
pub struct BrownianBridge {
    /// Number of time steps this bridge was built for.
    num_steps: usize,
    /// Midpoints and neighbours emitted together by binary subdivision.
    steps: Vec<BridgeStep>,
}

impl BrownianBridge {
    /// Create a Brownian bridge for N time steps.
    ///
    /// # Arguments
    ///
    /// * `num_steps` - Number of time steps in the path
    ///
    /// # Example
    ///
    /// For num_steps=4:
    /// - Standard order: [0, 1, 2, 3, 4]
    /// - Bridge order:   [4, 2, 1, 3] (terminal, half, quarters, ...)
    pub fn new(num_steps: usize) -> Self {
        let mut steps = Vec::with_capacity(num_steps);
        Self::build_bridge_recursive(0, num_steps, &mut steps);
        Self { num_steps, steps }
    }

    /// Emit midpoints with their known bracketing neighbours.
    fn build_bridge_recursive(left: usize, right: usize, steps: &mut Vec<BridgeStep>) {
        if right - left <= 1 {
            return;
        }
        let index = (left + right) / 2;
        steps.push(BridgeStep { index, left, right });
        Self::build_bridge_recursive(left, index, steps);
        Self::build_bridge_recursive(index, right, steps);
    }

    /// Construction order, exposed to the unit tests that pin the bridge layout.
    #[cfg(test)]
    fn order(&self) -> Vec<usize> {
        self.steps.iter().map(|step| step.index).collect()
    }

    /// Apply bridge construction to generate a Brownian path from independent
    /// shocks on a (uniform or irregular) time grid.
    ///
    /// `times` must contain `num_steps + 1` monotonically increasing time points
    /// with `times[0] == 0.0`. On return `w_out[0] = 0` and `w_out[i]` is the
    /// Brownian motion at `times[i]`; `z[0]` drives the terminal point and
    /// `z[i + 1]` the `i`-th bridge construction step.
    ///
    /// # Arguments
    ///
    /// * `z` - Independent standard normal shocks (length = num_steps)
    /// * `w_out` - Output Brownian path (length = num_steps + 1)
    /// * `times` - Time grid in years (length = num_steps + 1, `times[0] == 0.0`)
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when the documented preconditions are
    /// violated: `times.len() != z.len() + 1`, `w_out.len() != z.len() + 1`,
    /// `z.len()` does not match the bridge's step count, `times[0] != 0.0`,
    /// or `times` is not finite and strictly increasing.
    pub fn construct_path(&self, z: &[f64], w_out: &mut [f64], times: &[f64]) -> Result<()> {
        let num_steps = z.len();
        self.validate_shocks_and_output(num_steps, w_out.len())?;
        if times.len() != num_steps + 1 {
            return Err(Error::Validation(format!(
                "construct_path: times.len()={} != z.len()+1={}",
                times.len(),
                num_steps + 1
            )));
        }
        if times[0] != 0.0 {
            return Err(Error::Validation(format!(
                "construct_path: times[0] must be 0.0, got {}",
                times[0]
            )));
        }
        for w in times.windows(2) {
            if !w[1].is_finite() || w[1] <= w[0] {
                return Err(Error::Validation(format!(
                    "construct_path: times must be finite and strictly increasing, \
                     got {} after {}",
                    w[1], w[0]
                )));
            }
        }

        w_out.fill(f64::NAN);
        w_out[0] = 0.0;
        if num_steps == 0 {
            return Ok(());
        }
        w_out[num_steps] = z[0] * times[num_steps].sqrt();

        // Midpoints and brackets are precomputed; this loop is allocation-free.
        for (
            i,
            &BridgeStep {
                index: idx,
                left,
                right,
            },
        ) in self.steps.iter().enumerate()
        {
            let left_time = times[left];
            let idx_time = times[idx];
            let right_time = times[right];

            let alpha = (idx_time - left_time) / (right_time - left_time);
            let conditional_mean = w_out[left] + alpha * (w_out[right] - w_out[left]);
            let conditional_variance =
                ((idx_time - left_time) * (right_time - idx_time)) / (right_time - left_time);

            w_out[idx] = conditional_mean + conditional_variance.sqrt() * z[i + 1];
        }
        Ok(())
    }

    /// Validate the shock and output buffer lengths against this bridge.
    ///
    /// `z` carries one shock for the terminal point plus one per bridge
    /// construction step, so a bridge built for `n` steps requires
    /// `z.len() == n` (with `n == steps.len() + 1` for `n ≥ 1`)
    /// and `w_out.len() == n + 1`.
    fn validate_shocks_and_output(&self, num_steps: usize, w_out_len: usize) -> Result<()> {
        if w_out_len != num_steps + 1 {
            return Err(Error::Validation(format!(
                "brownian bridge: w_out.len()={w_out_len} != z.len()+1={}",
                num_steps + 1
            )));
        }
        if num_steps != self.num_steps {
            return Err(Error::Validation(format!(
                "brownian bridge: z.len()={num_steps} does not match the bridge's step count \
                 {}",
                self.num_steps
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uniform grid `[0, dt, 2·dt, …, num_steps·dt]`.
    fn uniform_times(num_steps: usize, dt: f64) -> Vec<f64> {
        (0..=num_steps).map(|i| i as f64 * dt).collect()
    }

    #[test]
    fn test_brownian_bridge_order() {
        let bridge = BrownianBridge::new(4);
        let order = bridge.order();

        println!("Bridge order for 4 steps: {:?}", order);

        // First should be midpoint (2)
        assert_eq!(order[0], 2);

        // Should have 3 elements (not counting initial 0 and terminal 4)
        assert!(order.len() >= 2);
    }

    #[test]
    fn test_brownian_bridge_construction() {
        let bridge = BrownianBridge::new(4);

        // Independent shocks
        let z = vec![1.0, 0.5, -0.5, 0.0];
        let mut w = vec![f64::NAN; 5];
        let dt = 0.25;

        bridge
            .construct_path(&z, &mut w, &uniform_times(4, dt))
            .unwrap();

        assert_eq!(w[0], 0.0);

        for &val in &w {
            assert!(val.is_finite());
        }

        // Terminal point should use first shock
        let expected_terminal = z[0] * (4.0 * dt).sqrt();
        assert!((w[4] - expected_terminal).abs() < 1e-10);
    }

    #[test]
    fn test_brownian_bridge_zero_initialized_buffer_matches_nan_initialized_buffer() {
        let bridge = BrownianBridge::new(4);
        let z = vec![1.0, 0.5, -0.5, 0.0];
        let dt = 0.25;

        let times = uniform_times(4, dt);
        let mut nan_buffer = vec![f64::NAN; 5];
        bridge.construct_path(&z, &mut nan_buffer, &times).unwrap();

        let mut zero_buffer = vec![0.0; 5];
        bridge.construct_path(&z, &mut zero_buffer, &times).unwrap();

        assert_eq!(zero_buffer, nan_buffer);
    }

    #[test]
    fn test_brownian_bridge_irregular_grid_construction() {
        let bridge = BrownianBridge::new(3);
        let z = vec![1.0, 0.0, 0.0];
        let times = vec![0.0, 0.1, 0.4, 1.0];
        let mut w = vec![f64::NAN; 4];

        bridge.construct_path(&z, &mut w, &times).unwrap();

        assert_eq!(w[0], 0.0);
        assert!((w[3] - 1.0).abs() < 1e-12);
        assert!((w[1] - 0.1).abs() < 1e-12);
        for value in w {
            assert!(value.is_finite());
        }
    }

    #[test]
    fn test_brownian_bridge_zero_steps_does_not_panic() {
        // new(0) must construct, and the degenerate path is just [0.0].
        let bridge = BrownianBridge::new(0);
        assert!(bridge.order().is_empty());

        let z: Vec<f64> = vec![];
        let mut w = vec![f64::NAN; 1];
        bridge.construct_path(&z, &mut w, &[0.0]).unwrap();
        assert_eq!(w, vec![0.0]);
    }

    #[test]
    fn zero_and_one_step_bridges_validate_their_own_dimensions() {
        let zero = BrownianBridge::new(0);
        let mut two_points = [0.0; 2];
        assert!(zero
            .construct_path(&[0.0], &mut two_points, &[0.0, 0.25])
            .is_err());

        let one = BrownianBridge::new(1);
        let mut one_point = [0.0; 1];
        assert!(one.construct_path(&[], &mut one_point, &[0.0]).is_err());

        let mut valid = [f64::NAN; 2];
        one.construct_path(&[2.0], &mut valid, &[0.0, 0.25])
            .unwrap();
        assert_eq!(valid, [0.0, 1.0]);
    }

    #[test]
    fn test_brownian_bridge_validates_preconditions_in_release() {
        let bridge = BrownianBridge::new(4);
        let z = vec![1.0, 0.5, -0.5, 0.0];

        let times = uniform_times(4, 0.25);

        // Wrong output length
        let mut w_short = vec![0.0; 4];
        assert!(bridge.construct_path(&z, &mut w_short, &times).is_err());

        // Wrong shock count for this bridge
        let mut w = vec![0.0; 3];
        assert!(bridge
            .construct_path(&[1.0, 0.5], &mut w, &[0.0, 0.25, 0.5])
            .is_err());

        // Non-finite times
        let mut w5 = vec![0.0; 5];
        assert!(bridge
            .construct_path(&z, &mut w5, &[0.0, 0.25, f64::NAN, 0.75, 1.0])
            .is_err());

        // times[0] != 0, non-monotonic, wrong length
        let mut w5 = vec![0.0; 5];
        assert!(bridge
            .construct_path(&z, &mut w5, &[0.1, 0.2, 0.3, 0.4, 0.5])
            .is_err());
        assert!(bridge
            .construct_path(&z, &mut w5, &[0.0, 0.2, 0.15, 0.4, 0.5])
            .is_err());
        assert!(bridge
            .construct_path(&z, &mut w5, &[0.0, 0.2, 0.4, 0.5])
            .is_err());
    }

    /// Reference implementation: per-path ordered-set bracket lookup, the
    /// algorithm the precomputed-bracket table replaced. Used to pin the
    /// optimized path bit-for-bit.
    fn reference_construct_path(
        num_steps: usize,
        construction_order: &[usize],
        z: &[f64],
        w_out: &mut [f64],
        times: &[f64],
    ) {
        w_out.fill(f64::NAN);
        w_out[0] = 0.0;
        if num_steps == 0 {
            return;
        }
        w_out[num_steps] = z[0] * times[num_steps].sqrt();

        let mut populated = std::collections::BTreeSet::new();
        populated.insert(0);
        populated.insert(num_steps);
        for (i, &idx) in construction_order.iter().enumerate() {
            let left = populated.range(..idx).next_back().copied().unwrap_or(0);
            let right = populated
                .range(idx + 1..)
                .next()
                .copied()
                .unwrap_or(num_steps);
            let left_time = times[left];
            let idx_time = times[idx];
            let right_time = times[right];
            let alpha = (idx_time - left_time) / (right_time - left_time);
            let conditional_mean = w_out[left] + alpha * (w_out[right] - w_out[left]);
            let conditional_variance =
                ((idx_time - left_time) * (right_time - idx_time)) / (right_time - left_time);
            w_out[idx] = conditional_mean + conditional_variance.sqrt() * z[i + 1];
            populated.insert(idx);
        }
    }

    #[test]
    fn precomputed_brackets_match_per_path_lookup_bit_for_bit() {
        for &num_steps in &[0usize, 1, 2, 3, 4, 5, 8, 17, 64, 255, 256] {
            let bridge = BrownianBridge::new(num_steps);
            let z: Vec<f64> = (0..num_steps)
                .map(|i| ((i % 23) as f64 * 0.113) - 1.2)
                .collect();
            let grids = [
                uniform_times(num_steps, 1.0 / 252.0),
                (0..=num_steps).map(|i| (i * i) as f64 / 252.0).collect(),
            ];
            for times in grids {
                let mut fast = vec![0.0; num_steps + 1];
                bridge.construct_path(&z, &mut fast, &times).expect("valid");
                let mut reference = vec![0.0; num_steps + 1];
                reference_construct_path(num_steps, &bridge.order(), &z, &mut reference, &times);
                assert_eq!(fast, reference, "n={num_steps}");
            }
        }
    }
}
