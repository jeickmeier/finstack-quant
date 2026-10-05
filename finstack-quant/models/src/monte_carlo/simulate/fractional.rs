//! Fractional-noise source for the rough-volatility arms of
//! [`simulate_paths`](super::simulate_paths).
//!
//! `rough_bergomi` and `cheyette_rough` read two externally generated values
//! per step: the fractional increment that accumulates into the volatility
//! driver, and a unit-variance normal that the spot (or rate) shock is
//! correlated against. [`FbmSpec`] selects the generator of the first; the
//! second is always the first standard normal the generator consumes for that
//! step, which is independent of everything generated before the step.

use super::{TimeGridSpec, MAX_STORED_VALUES};
use crate::monte_carlo::rng::fbm::{
    create_fbm_generator, FractionalNoiseGenerator, WindowedConditionalFbm,
    WindowedConditionalFbmConfig,
};
use crate::monte_carlo::rng::volterra::RiemannLiouvilleVolterra;
use crate::monte_carlo::TimeGrid;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// Generator of the fractional increments consumed by `rough_bergomi` and
/// `cheyette_rough`.
///
/// Serialized as an internally tagged object: `{"type": "volterra"}`. The
/// Hurst exponent always comes from the process parameters.
///
/// Only `volterra` reproduces the published models: both are defined on the
/// Riemann-Liouville Volterra process, whose increments are not stationary.
/// `cholesky` and `windowed_conditional` drive the same recursion with true
/// fractional Brownian motion instead. Forwards are unchanged. Exact
/// fractional Brownian motion has the same marginal variance `t^(2H)`, so the
/// expected variance is unchanged too, but its autocovariance differs, so
/// smiles and path statistics differ. The windowed generator only
/// approximates the level variance once the path is longer than its window,
/// which biases the expected variance.
///
/// The variants without settings are written `FbmSpec::Volterra {}` so that
/// the tagged object rejects unknown keys.
///
/// # References
///
/// - Hybrid scheme: `docs/REFERENCES.md#bennedsen-lunde-pakkanen-2017`
/// - Rough Bergomi: `docs/REFERENCES.md#bayer-friz-gatheral-2016`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FbmSpec {
    /// Riemann-Liouville Volterra increments from the Bennedsen-Lunde-Pakkanen
    /// hybrid scheme. Needs an equally spaced time grid. `O(n²)` work per path
    /// in the number of steps `n`.
    Volterra {},
    /// Exact fractional Brownian motion increments from the Cholesky factor
    /// of their covariance. Any time grid. `O(n³)` setup, `O(n²)` storage and
    /// work per path; limited to 8,000 steps.
    Cholesky {},
    /// Approximate fractional Brownian motion increments: exact for the first
    /// `near_field_size` steps, then each increment is drawn conditionally on
    /// the previous `near_field_size` increments only. Any time grid.
    /// `O(n · near_field_size)` work per path.
    WindowedConditional {
        /// Positive window length in steps, capped at the number of steps.
        /// Omitted selects `min(max(10, √n), 50)`.
        #[serde(default)]
        near_field_size: Option<usize>,
    },
}

impl Default for FbmSpec {
    /// The Volterra generator, which the pricers use.
    fn default() -> Self {
        Self::Volterra {}
    }
}

impl FbmSpec {
    /// Serialized tag of the variant, used in error messages.
    fn tag(self) -> &'static str {
        match self {
            Self::Volterra {} => "volterra",
            Self::Cholesky {} => "cholesky",
            Self::WindowedConditional { .. } => "windowed_conditional",
        }
    }
}

/// A fractional increment generator bound to one time grid.
pub(super) struct FractionalNoise {
    generator: Box<dyn FractionalNoiseGenerator>,
    normals_per_step: usize,
}

impl FractionalNoise {
    /// Build the generator `fbm` for `time_grid` and Hurst exponent `hurst`.
    ///
    /// `grid_spec` is the specification `time_grid` was built from; a uniform
    /// specification passes its `expiry` to the Volterra generator unchanged.
    pub(super) fn new(
        fbm: FbmSpec,
        hurst: f64,
        grid_spec: &TimeGridSpec,
        time_grid: &TimeGrid,
    ) -> Result<Self> {
        let num_steps = time_grid.num_steps();
        let bounded = |window: usize| {
            num_steps
                .checked_mul(window)
                .filter(|&cells| cells <= MAX_STORED_VALUES)
                .map(|_| ())
                .ok_or_else(|| {
                    Error::Validation(format!(
                        "fbm '{}' needs {num_steps} x {window} generator weights, more than \
                         the {MAX_STORED_VALUES} allowed; use fewer steps or a smaller window",
                        fbm.tag()
                    ))
                })
        };
        let generator: Box<dyn FractionalNoiseGenerator> = match fbm {
            FbmSpec::Volterra {} => {
                let expiry = match grid_spec {
                    TimeGridSpec::Uniform { expiry, .. } => *expiry,
                    TimeGridSpec::Times { .. } if time_grid.is_uniform() => time_grid.t_max(),
                    TimeGridSpec::Times { .. } => {
                        return Err(Error::Validation(
                            "fbm 'volterra' needs an equally spaced time grid; use a uniform \
                             grid, or fbm 'cholesky' or 'windowed_conditional'"
                                .to_string(),
                        ));
                    }
                };
                Box::new(RiemannLiouvilleVolterra::new(expiry, num_steps, hurst)?)
            }
            FbmSpec::Cholesky {} => {
                bounded(num_steps)?;
                create_fbm_generator(time_grid.times(), hurst)?
            }
            FbmSpec::WindowedConditional { near_field_size } => {
                // The generator caps the window at the number of steps; its
                // automatic choice is at most 50.
                bounded(near_field_size.unwrap_or(50).min(num_steps))?;
                Box::new(WindowedConditionalFbm::new(
                    time_grid.times(),
                    hurst,
                    WindowedConditionalFbmConfig { near_field_size },
                )?)
            }
        };
        let normals_per_step = generator.normals_per_step();
        Ok(Self {
            generator,
            normals_per_step,
        })
    }

    /// Number of standard normals one path consumes before its first step.
    pub(super) fn normals_len(&self) -> usize {
        self.generator.num_steps() * self.normals_per_step
    }

    /// Turn one path's `normals` into its fractional `increments`, one per step.
    pub(super) fn generate(&self, normals: &[f64], increments: &mut [f64]) {
        self.generator.generate(normals, increments);
    }

    /// The unit-variance normal driving `step`: the first of the normals the
    /// generator consumes for that step. For the Volterra generator this is
    /// exactly its driving Brownian increment divided by `√Δt`.
    pub(super) fn driving_normal(&self, normals: &[f64], step: usize) -> f64 {
        normals[step * self.normals_per_step]
    }
}
