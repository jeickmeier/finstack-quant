//! Geometric-Brownian-motion path simulation for the host bindings.
//!
//! A GBM-only view of [`simulate_paths`]; the bindings move to
//! [`simulate_paths`] next and this module is then removed.

use crate::monte_carlo::engine::MAX_CAPTURED_PATHS;
use crate::monte_carlo::process::GbmParams;
use crate::monte_carlo::simulate::{
    simulate_paths, PathSimulationSpec, ProcessSpec, SchemeSpec, TimeGridSpec,
    MAX_STORED_VALUES as MAX_GBM_STORED_VALUES,
};
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
}

impl GbmPathConfig {
    /// Create GBM path inputs with seed 42.
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
}

/// Compact captured GBM paths for plotting and diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
/// Uses the same Philox path-ID streams and
/// [`ExactGbm`](crate::monte_carlo::discretization::ExactGbm) discretization as
/// the generic engine, retaining only the requested spots and shared times.
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
/// and time-grid footprint above 64 million scalar values.
pub fn simulate_gbm_paths(config: &GbmPathConfig) -> Result<GbmPathSummary> {
    if !config.spot.is_finite() || config.spot <= 0.0 {
        return Err(Error::Validation(format!(
            "GBM initial spot must be finite and positive, got {}",
            config.spot
        )));
    }

    crate::monte_carlo::require_positive_vol(config.vol)?;
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
    let summary = simulate_paths(&PathSimulationSpec {
        process: ProcessSpec::Gbm(GbmParams {
            r: config.rate,
            q: config.div_yield,
            sigma: config.vol,
        }),
        scheme: SchemeSpec::Default,
        initial_state: vec![config.spot],
        time_grid: TimeGridSpec::Uniform {
            expiry: config.expiry,
            num_steps: config.num_steps,
        },
        num_paths: config.num_paths,
        seed: config.seed,
        antithetic: false,
    })?;

    Ok(GbmPathSummary {
        num_paths: summary.num_paths,
        num_simulated_paths: summary.num_simulated_paths,
        paths: summary.values.chunks(stride).map(<[f64]>::to_vec).collect(),
        times: summary.times,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monte_carlo::discretization::ExactGbm;
    use crate::monte_carlo::engine::{McEngine, McEngineConfig, PathCaptureConfig};
    use crate::monte_carlo::payoff::vanilla::EuropeanCall;
    use crate::monte_carlo::process::{GbmProcess, ProcessMetadata};
    use crate::monte_carlo::rng::philox::PhiloxRng;
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

    /// FNV-1a over the IEEE-754 bit patterns of every time and every spot, in
    /// `times` then row-major path order.
    fn bit_hash(summary: &GbmPathSummary) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for value in summary.times.iter().chain(summary.paths.iter().flatten()) {
            for byte in value.to_bits().to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    }

    #[test]
    fn gbm_paths_are_pinned_bit_for_bit() {
        let a = simulate_gbm_paths(
            &GbmPathConfig::new(100.0, 0.04, 0.01, 0.25, 1.3, 17, 32).with_seed(19),
        )
        .expect("paths");
        let b = simulate_gbm_paths(
            &GbmPathConfig::new(42.5, -0.01, 0.03, 0.6, 0.37, 1, 5).with_seed(7_919),
        )
        .expect("paths");
        assert_eq!(bit_hash(&a), PIN_A_HASH);
        assert_eq!(a.paths[31][17].to_bits(), PIN_A_LAST);
        assert_eq!(bit_hash(&b), PIN_B_HASH);
        assert_eq!(b.paths[4][1].to_bits(), PIN_B_LAST);
    }

    const PIN_A_HASH: u64 = 0x7e9a_6884_54fb_d37c;
    const PIN_A_LAST: u64 = 0x4060_5556_4027_5291;
    const PIN_B_HASH: u64 = 0x2b4a_52ec_fdb1_426b;
    const PIN_B_LAST: u64 = 0x4040_f04d_39a6_c0fb;

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
