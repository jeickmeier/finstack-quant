//! Brownian-bridge ordering and Sobol quasi-Monte Carlo stream.

use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::monte_carlo::traits::RandomStream;
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_core::{Error, Result};

/// Split standardized interval increments directly on the actual grid.
pub(super) struct SobolBridge {
    pub(super) splits: Vec<BridgeSplit>,
}

pub(super) struct BridgeSplit {
    pub(super) left: usize,
    pub(super) mid: usize,
    pub(super) left_weight: f64,
    pub(super) right_weight: f64,
}

impl SobolBridge {
    pub(super) fn new(grid: &TimeGrid) -> Self {
        fn append(times: &[f64], left: usize, right: usize, splits: &mut Vec<BridgeSplit>) {
            if right - left <= 1 {
                return;
            }
            let mid = left + (right - left) / 2;
            let root_span = (times[right] - times[left]).sqrt();
            splits.push(BridgeSplit {
                left,
                mid,
                left_weight: (times[mid] - times[left]).sqrt() / root_span,
                right_weight: (times[right] - times[mid]).sqrt() / root_span,
            });
            append(times, left, mid, splits);
            append(times, mid, right, splits);
        }
        // Sobol admission bounds the number of intervals to 1..=40.
        let mut splits = Vec::with_capacity(grid.num_steps() - 1);
        append(grid.times(), 0, grid.num_steps(), &mut splits);
        Self { splits }
    }

    pub(super) fn fill(&self, z: &[f64], increments: &mut [f64]) -> Result<()> {
        // Each parent occupies its leftmost output slot until its split.
        // The one-step bridge therefore returns its terminal shock exactly.
        increments[0] = z[0];
        for (split, &innovation) in self.splits.iter().zip(&z[1..]) {
            let parent = increments[split.left];
            let left = weighted_shock(split.left_weight, parent, split.right_weight, innovation);
            let right = weighted_shock(split.right_weight, parent, -split.left_weight, innovation);
            if !left.is_finite() || !right.is_finite() {
                return Err(Error::Internal(
                    "Brownian bridge increment is out of range".into(),
                ));
            }
            increments[split.left] = left;
            increments[split.mid] = right;
        }
        Ok(())
    }
}

pub(super) fn weighted_shock(a: f64, parent: f64, b: f64, innovation: f64) -> f64 {
    let combine = |p: f64, z: f64| {
        let product = a * p;
        // Include the rounding error of the first product in a near cancellation.
        b.mul_add(z, product) + a.mul_add(p, -product)
    };
    let value = combine(parent, innovation);
    // Subnormal terms may lose bits even before the final sum. Exact binary
    // scaling allows the two terms to round together on return to the grid.
    const UP: f64 = (1_u64 << 54) as f64;
    if value.abs() < f64::MIN_POSITIVE && parent.abs().max(innovation.abs()) <= f64::MAX / UP {
        combine(parent * UP, innovation * UP) / UP
    } else {
        value
    }
}

/// Per-path [`RandomStream`] adapter that feeds pre-computed standard normals
/// from a Sobol draw into the generic [`McEngine`] step loop while routing
/// uniform draws to a separate Philox stream.
///
/// The adapter cannot be split: each path gets a fresh adapter constructed by
/// the Sobol pricer outer loop, so the engine's per-path `rng.split(..)` call
/// is bypassed by delegating to `simulate_path`/`simulate_path_with_capture`
/// directly.
#[derive(Clone)]
pub(super) struct SobolPathStream<'a> {
    pub(super) z_increments: &'a [f64],
    pub(super) cursor: usize,
    pub(super) aux: PhiloxRng,
}

impl<'a> SobolPathStream<'a> {
    pub(super) fn new(z_increments: &'a [f64], aux: PhiloxRng) -> Self {
        Self {
            z_increments,
            cursor: 0,
            aux,
        }
    }
}

impl<'a> RandomStream for SobolPathStream<'a> {
    /// Per-path Sobol adapters never split; the outer Sobol pricer owns a
    /// single adapter per path, so this always returns `None`.
    fn split(&self, _stream_id: u64) -> Option<Self> {
        None
    }

    fn fill_u01(&mut self, out: &mut [f64]) {
        self.aux.fill_u01(out);
    }

    fn fill_std_normals(&mut self, out: &mut [f64]) -> Result<()> {
        let remaining = &self.z_increments[self.cursor..];
        let draws = remaining.get(..out.len()).ok_or_else(|| {
            Error::Validation(format!(
                "Sobol path requires {} normals but only {} coordinates remain",
                out.len(),
                remaining.len()
            ))
        })?;
        out.copy_from_slice(draws);
        self.cursor += draws.len();
        Ok(())
    }

    fn supports_splitting(&self) -> bool {
        false
    }
}
