//! Productized geometric-Brownian-motion path simulation.
//!
//! This module intentionally returns a compact spot-path summary rather than
//! exposing the generic process/discretization/path graph used by the engine.

use crate::monte_carlo::discretization::ExactGbm;
use crate::monte_carlo::engine::MAX_CAPTURED_PATHS;
use crate::monte_carlo::process::GbmProcess;
use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::monte_carlo::traits::{Discretization, RandomStream};
use crate::monte_carlo::TimeGrid;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

// Includes compact paths and the shared time-grid/copy storage. Bound the
// product before constructing even the time grid, which itself allocates.
const MAX_GBM_STORED_VALUES: usize = 64_000_000;

/// Inputs for a compact GBM path simulation.
#[derive(Debug, Clone, PartialEq)]
pub struct GbmPathConfig {
    /// Initial spot.
    pub spot: f64,
    /// Continuously compounded risk-free rate.
    pub rate: f64,
    /// Continuous dividend yield.
    pub div_yield: f64,
    /// Annualized volatility.
    pub vol: f64,
    /// Simulation horizon in years.
    pub expiry: f64,
    /// Number of simulation steps.
    pub num_steps: usize,
    /// Number of independent path estimators requested.
    pub num_paths: usize,
    /// Root seed for deterministic Philox streams.
    pub seed: u64,
    /// Whether to use antithetic pairing.
    ///
    /// Path capture currently rejects this combination.
    pub antithetic: bool,
}

impl GbmPathConfig {
    /// Create GBM path inputs with seed 42 and antithetic pairing disabled.
    ///
    /// Inputs are validated when [`simulate_gbm_paths`] executes.
    ///
    /// # Arguments
    ///
    /// * `spot` - Finite positive initial spot in the underlying's price units.
    /// * `rate` - Finite continuously compounded annual risk-free rate as a decimal.
    /// * `div_yield` - Finite continuous annual dividend yield as a decimal.
    /// * `vol` - Finite positive annualized lognormal volatility as a decimal.
    /// * `expiry` - Finite positive simulation horizon in years.
    /// * `num_steps` - Positive uniform interval count, subject to the aggregate storage budget.
    /// * `num_paths` - Captured paths in `1..=100_000`, subject to the aggregate storage budget.
    pub fn new(
        spot: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        num_steps: usize,
        num_paths: usize,
    ) -> Self {
        Self {
            spot,
            rate,
            div_yield,
            vol,
            expiry,
            num_steps,
            num_paths,
            seed: 42,
            antithetic: false,
        }
    }

    /// Set the deterministic random seed.
    ///
    /// # Arguments
    ///
    /// * `seed` - Root Philox seed; each path ID selects its own deterministic stream.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Enable or disable antithetic path pairing.
    ///
    /// # Arguments
    ///
    /// * `antithetic` - `true` requests an unsupported captured-path combination,
    ///   rejected by [`simulate_gbm_paths`]; `false` simulates each path once.
    #[must_use]
    pub fn with_antithetic(mut self, antithetic: bool) -> Self {
        self.antithetic = antithetic;
        self
    }
}

/// Compact captured GBM paths for plotting and diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GbmPathSummary {
    /// Number of independent estimators requested.
    pub num_paths: usize,
    /// Total number of sample paths simulated.
    pub num_simulated_paths: usize,
    /// Shared path times in year fractions, including time zero.
    pub times: Vec<f64>,
    /// Captured spot paths in deterministic path-id order.
    pub paths: Vec<Vec<f64>>,
}

/// Simulate compact GBM spot paths with canonical exact GBM transitions.
///
/// Uses the same Philox path-ID streams and [`ExactGbm`] discretization as the
/// generic engine, retaining only the requested spots and shared times.
///
/// # Arguments
///
/// * `config` - GBM process, time-grid, RNG-seed, and path-count configuration;
///   captured paths are returned in deterministic path-ID order.
///
/// # Errors
///
/// Returns validation errors for invalid process/grid inputs, zero paths,
/// more than 100,000 captured paths, non-finite simulated spots, or an output
/// and time-grid footprint above 64 million scalar values. Antithetic pairing
/// remains unsupported for captured-path output.
pub fn simulate_gbm_paths(config: &GbmPathConfig) -> Result<GbmPathSummary> {
    if !config.spot.is_finite() || config.spot <= 0.0 {
        return Err(Error::Validation(format!(
            "GBM initial spot must be finite and positive, got {}",
            config.spot
        )));
    }

    crate::monte_carlo::require_positive_vol(config.vol)?;
    if config.antithetic {
        return Err(Error::Validation(
            "Path capture is currently unsupported with antithetic=true".to_string(),
        ));
    }
    if config.num_paths == 0 || config.num_paths > MAX_CAPTURED_PATHS {
        return Err(Error::Validation(format!(
            "GBM captured num_paths must be in 1..={MAX_CAPTURED_PATHS}, got {}",
            config.num_paths
        )));
    }
    let stride = config.num_steps.checked_add(1).ok_or_else(|| {
        Error::Validation("GBM step count exceeds supported output size".to_string())
    })?;
    config
        .num_paths
        .checked_add(3)
        .and_then(|rows| rows.checked_mul(stride))
        .filter(|&values| values <= MAX_GBM_STORED_VALUES)
        .ok_or_else(|| {
            Error::Validation(format!(
                "GBM captured paths and shared time grids exceed {MAX_GBM_STORED_VALUES} values"
            ))
        })?;
    let time_grid = TimeGrid::uniform(config.expiry, config.num_steps)?;
    let rng = PhiloxRng::new(config.seed);
    let process = GbmProcess::with_params(config.rate, config.div_yield, config.vol)?;
    let discretization = ExactGbm::new();
    let capture_path = |path_id: usize| -> Result<Vec<f64>> {
        let mut path_rng = rng.substream(path_id as u64);
        let mut state = [config.spot];
        let mut z = [0.0];
        let mut path = Vec::with_capacity(stride);
        path.push(config.spot);
        for step in 0..config.num_steps {
            path_rng.fill_std_normals(&mut z);
            discretization.step(
                &process,
                time_grid.time(step),
                time_grid.dt(step),
                &mut state,
                &z,
                &mut [],
            );
            if !state[0].is_finite() {
                return Err(Error::Validation(format!(
                    "non-finite GBM spot on path {path_id} at step {}",
                    step + 1
                )));
            }
            path.push(state[0]);
        }
        Ok(path)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let paths = {
        use rayon::prelude::*;
        if crate::monte_carlo::registry::embedded_defaults()?
            .rust
            .engine
            .use_parallel
        {
            (0..config.num_paths)
                .into_par_iter()
                .map(capture_path)
                .collect::<Result<Vec<_>>>()?
        } else {
            (0..config.num_paths)
                .map(capture_path)
                .collect::<Result<Vec<_>>>()?
        }
    };
    #[cfg(target_arch = "wasm32")]
    let paths = (0..config.num_paths)
        .map(capture_path)
        .collect::<Result<Vec<_>>>()?;
    let times = std::iter::once(0.0)
        .chain((0..config.num_steps).map(|step| time_grid.time(step) + time_grid.dt(step)))
        .collect();

    Ok(GbmPathSummary {
        num_paths: config.num_paths,
        num_simulated_paths: config.num_paths,
        times,
        paths,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monte_carlo::engine::{McEngine, McEngineConfig, PathCaptureConfig};
    use crate::monte_carlo::payoff::vanilla::EuropeanCall;
    use crate::monte_carlo::process::ProcessMetadata;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn compact_capture_matches_generic_engine_paths_and_times() {
        let config = GbmPathConfig::new(100.0, 0.04, 0.01, 0.25, 1.3, 17, 32).with_seed(19);
        let compact = simulate_gbm_paths(&config).expect("compact paths");
        let engine = McEngine::new(
            McEngineConfig::uniform(32, 1.3, 17)
                .expect("grid")
                .antithetic(false)
                .parallel(false)
                .path_capture(PathCaptureConfig::all()),
        );
        let process = GbmProcess::with_params(0.04, 0.01, 0.25).expect("process");
        let rich = engine
            .price_with_capture(
                &PhiloxRng::new(19),
                &process,
                &ExactGbm::new(),
                &[100.0],
                &EuropeanCall::new(0.0, 1.0, 17),
                Currency::USD,
                1.0,
                process.metadata(),
            )
            .expect("rich paths")
            .paths
            .expect("dataset");
        for (compact_path, rich_path) in compact.paths.iter().zip(&rich.paths) {
            let spots: Vec<_> = rich_path
                .points
                .iter()
                .map(|point| point.spot().expect("spot"))
                .collect();
            assert_eq!(compact_path, &spots);
        }
        let times: Vec<_> = rich.paths[0]
            .points
            .iter()
            .map(|point| point.time)
            .collect();
        assert_eq!(compact.times, times);
        assert_eq!(compact.num_simulated_paths, 32);
    }

    #[test]
    fn compact_capture_rejects_workload_before_allocation() {
        for (steps, paths) in [
            (usize::MAX, 1),
            (MAX_GBM_STORED_VALUES, 1),
            (1, MAX_CAPTURED_PATHS + 1),
            (1, 0),
        ] {
            let config = GbmPathConfig::new(100.0, 0.04, 0.0, 0.2, 1.0, steps, paths);
            assert!(simulate_gbm_paths(&config).is_err());
        }
    }
}
