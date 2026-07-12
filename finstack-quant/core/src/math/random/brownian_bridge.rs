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

use std::collections::BTreeSet;

use crate::{Error, Result};

/// Brownian bridge construction order.
///
/// Generates the sequence of time indices to sample in bridge order.
pub struct BrownianBridge {
    /// Number of time steps this bridge was built for.
    num_steps: usize,
    /// Construction order (indices into time grid)
    construction_order: Vec<usize>,
    /// Multipliers for conditional variance
    std_multipliers: Vec<f64>,
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
        let mut construction_order = Vec::with_capacity(num_steps);
        let mut std_multipliers = Vec::with_capacity(num_steps);

        // Binary subdivision
        Self::build_bridge_recursive(0, num_steps, &mut construction_order, &mut std_multipliers);

        Self {
            num_steps,
            construction_order,
            std_multipliers,
        }
    }

    /// Recursive builder for bridge order.
    fn build_bridge_recursive(
        left: usize,
        right: usize,
        order: &mut Vec<usize>,
        multipliers: &mut Vec<f64>,
    ) {
        if right - left <= 1 {
            return;
        }

        // Add midpoint
        let mid = (left + right) / 2;
        order.push(mid);

        // Conditional variance multiplier for Brownian bridge:
        // Var[B(t) | B(s), B(u)] = (t-s)(u-t)/(u-s)
        let left_time = left as f64;
        let mid_time = mid as f64;
        let right_time = right as f64;

        let variance_factor = if right > left {
            ((mid_time - left_time) * (right_time - mid_time)) / (right_time - left_time)
        } else {
            1.0
        };

        multipliers.push(variance_factor.sqrt());

        // Recurse on left and right halves
        Self::build_bridge_recursive(left, mid, order, multipliers);
        Self::build_bridge_recursive(mid, right, order, multipliers);
    }

    /// Get construction order.
    pub fn order(&self) -> &[usize] {
        &self.construction_order
    }

    /// Get standard deviation multipliers.
    pub fn multipliers(&self) -> &[f64] {
        &self.std_multipliers
    }

    /// Apply bridge construction to generate path from independent shocks.
    ///
    /// # Arguments
    ///
    /// * `z` - Independent standard normal shocks (length = num_steps)
    /// * `w_out` - Output Brownian path (length = num_steps + 1)
    /// * `dt` - Time step size
    ///
    /// # Notes
    ///
    /// `w_out[0] = 0` (Brownian motion starts at 0)
    /// `w_out[i] = cumulative Brownian motion at step i`
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when `w_out.len() != z.len() + 1`, when
    /// `z.len()` does not match the number of steps this bridge was built
    /// for, or when `dt` is not finite and positive.
    pub fn construct_path(&self, z: &[f64], w_out: &mut [f64], dt: f64) -> Result<()> {
        let num_steps = z.len();
        self.validate_shocks_and_output(num_steps, w_out.len())?;
        if !dt.is_finite() || dt <= 0.0 {
            return Err(Error::Validation(format!(
                "construct_path: dt must be finite and positive, got {dt}"
            )));
        }

        // Initialize
        w_out.fill(f64::NAN);
        w_out[0] = 0.0;
        if num_steps == 0 {
            return Ok(());
        }

        // Terminal point (standard Brownian motion)
        w_out[num_steps] = z[0] * (num_steps as f64 * dt).sqrt();

        // O(log n) bracket lookups via BTreeSet of populated indices
        let mut populated = BTreeSet::new();
        populated.insert(0);
        populated.insert(num_steps);

        // Fill in using bridge construction
        // z[0] is used for terminal, z[1..] for construction_order
        for (i, &idx) in self.construction_order.iter().enumerate() {
            // Find left and right bracketing points (O(log n) lookup)
            let (left, right) = self.find_brackets(idx, &populated, num_steps);

            // Conditional mean: linear interpolation
            let left_time = left as f64 * dt;
            let idx_time = idx as f64 * dt;
            let right_time = right as f64 * dt;

            let alpha = (idx_time - left_time) / (right_time - left_time);
            let conditional_mean = w_out[left] + alpha * (w_out[right] - w_out[left]);

            // Conditional std dev
            let conditional_std = self.std_multipliers[i] * dt.sqrt();

            // Generate point using z[i+1] (since z[0] is for terminal)
            w_out[idx] = conditional_mean + conditional_std * z[i + 1];
            populated.insert(idx);
        }
        Ok(())
    }

    /// Apply bridge construction on an irregular time grid.
    ///
    /// `times` must contain `num_steps + 1` monotonically increasing time points
    /// with `times[0] == 0.0`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when the documented preconditions are
    /// violated: `times.len() != z.len() + 1`, `w_out.len() != z.len() + 1`,
    /// `z.len()` does not match the bridge's step count, `times[0] != 0.0`,
    /// or `times` is not finite and strictly increasing.
    pub fn construct_path_irregular(
        &self,
        z: &[f64],
        w_out: &mut [f64],
        times: &[f64],
    ) -> Result<()> {
        let num_steps = z.len();
        self.validate_shocks_and_output(num_steps, w_out.len())?;
        if times.len() != num_steps + 1 {
            return Err(Error::Validation(format!(
                "construct_path_irregular: times.len()={} != z.len()+1={}",
                times.len(),
                num_steps + 1
            )));
        }
        if times[0] != 0.0 {
            return Err(Error::Validation(format!(
                "construct_path_irregular: times[0] must be 0.0, got {}",
                times[0]
            )));
        }
        for w in times.windows(2) {
            if !w[1].is_finite() || w[1] <= w[0] {
                return Err(Error::Validation(format!(
                    "construct_path_irregular: times must be finite and strictly increasing, \
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

        let mut populated = BTreeSet::new();
        populated.insert(0);
        populated.insert(num_steps);

        for (i, &idx) in self.construction_order.iter().enumerate() {
            let (left, right) = self.find_brackets(idx, &populated, num_steps);
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
        Ok(())
    }

    /// Validate the shock and output buffer lengths against this bridge.
    ///
    /// `z` carries one shock for the terminal point plus one per bridge
    /// construction step, so a bridge built for `n` steps requires
    /// `z.len() == n` (with `n == construction_order.len() + 1` for `n ≥ 1`)
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

    /// Find left and right bracketing points for bridge construction.
    ///
    /// Uses BTreeSet for O(log n) lookups instead of O(n) linear scan.
    fn find_brackets(
        &self,
        idx: usize,
        populated: &BTreeSet<usize>,
        max_idx: usize,
    ) -> (usize, usize) {
        let left = populated.range(..idx).next_back().copied().unwrap_or(0);
        let right = populated
            .range(idx + 1..)
            .next()
            .copied()
            .unwrap_or(max_idx);
        (left, right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        bridge.construct_path(&z, &mut w, dt).unwrap();

        println!("Brownian path: {:?}", w);

        // Check initial condition
        assert_eq!(w[0], 0.0);

        // Check all points are finite
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

        let mut nan_buffer = vec![f64::NAN; 5];
        bridge.construct_path(&z, &mut nan_buffer, dt).unwrap();

        let mut zero_buffer = vec![0.0; 5];
        bridge.construct_path(&z, &mut zero_buffer, dt).unwrap();

        assert_eq!(zero_buffer, nan_buffer);
    }

    #[test]
    fn test_brownian_bridge_irregular_grid_construction() {
        let bridge = BrownianBridge::new(3);
        let z = vec![1.0, 0.0, 0.0];
        let times = vec![0.0, 0.1, 0.4, 1.0];
        let mut w = vec![f64::NAN; 4];

        bridge.construct_path_irregular(&z, &mut w, &times).unwrap();

        assert_eq!(w[0], 0.0);
        assert!((w[3] - 1.0).abs() < 1e-12);
        assert!((w[1] - 0.1).abs() < 1e-12);
        for value in w {
            assert!(value.is_finite());
        }
    }

    #[test]
    fn test_uniform_grid_matches_irregular_constructor() {
        let bridge = BrownianBridge::new(4);
        let z = vec![0.75, -0.25, 1.25, -1.0];
        let dt = 0.25;
        let times = vec![0.0, dt, 2.0 * dt, 3.0 * dt, 4.0 * dt];
        let mut uniform_path = vec![f64::NAN; 5];
        let mut irregular_path = vec![f64::NAN; 5];

        bridge.construct_path(&z, &mut uniform_path, dt).unwrap();
        bridge
            .construct_path_irregular(&z, &mut irregular_path, &times)
            .unwrap();

        for (uniform, irregular) in uniform_path.iter().zip(irregular_path.iter()) {
            assert!(
                (uniform - irregular).abs() < 1e-12,
                "uniform and irregular constructors diverged: {uniform} vs {irregular}"
            );
        }
    }

    #[test]
    fn test_brownian_bridge_zero_steps_does_not_panic() {
        // new(0) must construct, and the degenerate path is just [0.0].
        let bridge = BrownianBridge::new(0);
        assert!(bridge.order().is_empty());

        let z: Vec<f64> = vec![];
        let mut w = vec![f64::NAN; 1];
        bridge.construct_path(&z, &mut w, 0.25).unwrap();
        assert_eq!(w, vec![0.0]);
    }

    #[test]
    fn zero_and_one_step_bridges_validate_their_own_dimensions() {
        let zero = BrownianBridge::new(0);
        let mut two_points = [0.0; 2];
        assert!(zero.construct_path(&[0.0], &mut two_points, 0.25).is_err());

        let one = BrownianBridge::new(1);
        let mut one_point = [0.0; 1];
        assert!(one.construct_path(&[], &mut one_point, 0.25).is_err());

        let mut valid = [f64::NAN; 2];
        one.construct_path(&[2.0], &mut valid, 0.25).unwrap();
        assert_eq!(valid, [0.0, 1.0]);
    }

    #[test]
    fn test_brownian_bridge_validates_preconditions_in_release() {
        let bridge = BrownianBridge::new(4);
        let z = vec![1.0, 0.5, -0.5, 0.0];

        // Wrong output length
        let mut w_short = vec![0.0; 4];
        assert!(bridge.construct_path(&z, &mut w_short, 0.25).is_err());

        // Wrong shock count for this bridge
        let mut w = vec![0.0; 3];
        assert!(bridge.construct_path(&[1.0, 0.5], &mut w, 0.25).is_err());

        // Bad dt
        let mut w5 = vec![0.0; 5];
        assert!(bridge.construct_path(&z, &mut w5, 0.0).is_err());
        assert!(bridge.construct_path(&z, &mut w5, f64::NAN).is_err());

        // Irregular grid: times[0] != 0, non-monotonic, wrong length
        let mut w5 = vec![0.0; 5];
        assert!(bridge
            .construct_path_irregular(&z, &mut w5, &[0.1, 0.2, 0.3, 0.4, 0.5])
            .is_err());
        assert!(bridge
            .construct_path_irregular(&z, &mut w5, &[0.0, 0.2, 0.15, 0.4, 0.5])
            .is_err());
        assert!(bridge
            .construct_path_irregular(&z, &mut w5, &[0.0, 0.2, 0.4, 0.5])
            .is_err());
    }
}
