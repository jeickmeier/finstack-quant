//! Productized geometric-Brownian-motion path simulation.
//!
//! This module intentionally returns a compact spot-path summary rather than
//! exposing the generic process/discretization/path graph used by the engine.

use crate::discretization::ExactGbm;
use crate::engine::{McEngine, McEngineConfig, PathCaptureConfig};
use crate::payoff::vanilla::EuropeanCall;
use crate::process::{GbmProcess, ProcessMetadata};
use crate::rng::philox::PhiloxRng;
use crate::time_grid::TimeGrid;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

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
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Enable or disable antithetic path pairing.
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

/// Simulate and capture GBM spot paths through the generic Rust engine.
///
/// # Errors
///
/// Returns validation errors from the GBM process, time grid, or engine. Path
/// capture and antithetic pairing are deliberately incompatible and rejected
/// by the engine.
pub fn simulate_gbm_paths(config: &GbmPathConfig) -> Result<GbmPathSummary> {
    if !config.spot.is_finite() || config.spot <= 0.0 {
        return Err(Error::Validation(format!(
            "GBM initial spot must be finite and positive, got {}",
            config.spot
        )));
    }

    let time_grid = TimeGrid::uniform(config.expiry, config.num_steps)?;
    let mut engine_config =
        McEngineConfig::new(config.num_paths, time_grid).antithetic(config.antithetic);
    engine_config.path_capture = PathCaptureConfig::all();
    let engine = McEngine::new(engine_config);
    let rng = PhiloxRng::new(config.seed);
    let process = GbmProcess::with_params(config.rate, config.div_yield, config.vol)?;
    let discretization = ExactGbm::new();
    let payoff = EuropeanCall::new(0.0, 1.0, config.num_steps);
    let result = engine.price_with_capture(
        &rng,
        &process,
        &discretization,
        &[config.spot],
        &payoff,
        Currency::USD,
        1.0,
        process.metadata(),
    )?;
    let dataset = result.paths.ok_or_else(|| {
        Error::Validation("GBM path capture completed without a path dataset".to_string())
    })?;

    let times = dataset
        .paths
        .first()
        .map(|path| path.points.iter().map(|point| point.time).collect())
        .unwrap_or_default();
    let paths = dataset
        .paths
        .iter()
        .map(|path| {
            path.points
                .iter()
                .map(|point| {
                    point.spot().ok_or_else(|| {
                        Error::Validation(format!(
                            "captured GBM path {} is missing its spot state at step {}",
                            path.path_id, point.step
                        ))
                    })
                })
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(GbmPathSummary {
        num_paths: result.estimate.num_paths,
        num_simulated_paths: result.estimate.num_simulated_paths,
        times,
        paths,
    })
}
