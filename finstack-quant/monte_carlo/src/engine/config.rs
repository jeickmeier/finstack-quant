use super::path_capture::PathCaptureConfig;
use super::pricing::McEngine;
use crate::time_grid::TimeGrid;
use finstack_quant_core::Result;

/// Maximum number of Monte Carlo paths allowed per simulation run.
pub const MAX_NUM_PATHS: usize = 10_000_000;

/// Maximum number of paths that diagnostics capture may retain in memory.
///
/// Captured paths store every retained path point, state vector, payoff snapshot,
/// and diagnostic cashflow. Keep this separate from [`MAX_NUM_PATHS`] so
/// production pricing can run many paths without accidentally retaining the
/// full simulation in memory.
pub const MAX_CAPTURED_PATHS: usize = 100_000;

/// Stores the runtime configuration for a Monte Carlo pricing run.
///
/// This configuration is consumed by [`McEngine`] and can either be built
/// manually or via [`McEngineBuilder`]. All time values are year fractions.
#[derive(Debug, Clone)]
pub struct McEngineConfig {
    /// Requested number of independent path estimators. Values above
    /// [`MAX_NUM_PATHS`] are rejected (not capped) at runtime. A value of 1
    /// is accepted but yields an undefined (`NaN`) standard error — at least
    /// 2 paths are needed for a sample variance.
    ///
    /// With [`Self::antithetic`] disabled this equals the number of simulated
    /// sample paths. With antithetic pairing enabled the engine runs
    /// `num_paths` iterations, each simulating a `(z, -z)` pair and recording
    /// the pair's mean as a single estimator, so the total simulated paths
    /// become `2 * num_paths`. The produced [`crate::estimate::Estimate`]
    /// reports both counts: `num_paths` for the statistical sample size and
    /// `num_simulated_paths` for the raw simulation work.
    pub num_paths: usize,
    /// Time grid for discretization
    pub time_grid: TimeGrid,
    /// Optional target CI half-width for auto-stopping.
    ///
    /// Auto-stopping is an optional-stopping rule conditioned on the running
    /// confidence interval, so the stopped estimator carries a small bias;
    /// a 5 000-sample warm-up keeps the half-width estimate stable before
    /// the rule is evaluated. Serial-only.
    pub target_ci_half_width: Option<f64>,
    /// Use parallel execution (requires an RNG that supports deterministic
    /// stream splitting, e.g. `PhiloxRng`; rayon support is always compiled
    /// in).
    pub use_parallel: bool,
    /// Chunk size of the deterministic reduction tree. `None` selects a
    /// default that is a pure function of `num_paths` (never of the thread
    /// count); `Some(n)` uses exactly `n` paths per chunk.
    pub chunk_size: Option<usize>,
    /// Path capture configuration
    pub path_capture: PathCaptureConfig,
    /// Use antithetic variance reduction (pair `z` and `-z` per step).
    ///
    /// When enabled each of the `num_paths` iterations simulates a pair of
    /// antithetic paths, doubling the number of simulated sample paths while
    /// keeping the number of independent estimators equal to `num_paths`.
    pub antithetic: bool,
}

impl McEngineConfig {
    /// Create a configuration with default runtime options.
    ///
    /// # Arguments
    ///
    /// * `num_paths` - Requested number of Monte Carlo paths. Runtime validation
    ///   requires this to be greater than zero.
    /// * `time_grid` - Simulation grid in year fractions.
    ///
    /// # Returns
    ///
    /// A configuration using registry-backed parallel defaults, disabled path
    /// capture, and registry-backed antithetic defaults.
    pub fn new(num_paths: usize, time_grid: TimeGrid) -> Self {
        let defaults = &crate::registry::embedded_defaults_or_panic().rust.engine;
        Self {
            num_paths,
            time_grid,
            target_ci_half_width: None,
            use_parallel: defaults.use_parallel,
            chunk_size: None,
            path_capture: PathCaptureConfig::default(),
            antithetic: defaults.antithetic,
        }
    }

    /// Set the target 95% CI half-width for serial auto-stopping.
    ///
    /// The engine currently rejects this option when `use_parallel` is `true`.
    pub fn with_target_ci(mut self, target: f64) -> Self {
        self.target_ci_half_width = Some(target);
        self
    }

    /// Enable or disable parallel execution.
    pub fn with_parallel(mut self, parallel: bool) -> Self {
        self.use_parallel = parallel;
        self
    }

    /// Set the parallel chunk size to exactly `size`.
    ///
    /// Leave unset (the default) to keep the engine's adaptive chunking. Runtime
    /// validation rejects `0`.
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = Some(size);
        self
    }

    /// Install a path-capture configuration for the run.
    pub fn with_path_capture(mut self, config: PathCaptureConfig) -> Self {
        self.path_capture = config;
        self
    }

    /// Enable or disable antithetic path pairing.
    ///
    /// Path capture and antithetic pricing are currently mutually exclusive.
    pub fn with_antithetic(mut self, enabled: bool) -> Self {
        self.antithetic = enabled;
        self
    }

    /// Convenience helper equivalent to `with_path_capture(PathCaptureConfig::all())`.
    pub fn capture_all_paths(mut self) -> Self {
        self.path_capture = PathCaptureConfig::all();
        self
    }

    /// Convenience helper equivalent to `with_path_capture(PathCaptureConfig::sample(count, seed))`.
    pub fn capture_sample_paths(mut self, count: usize, seed: u64) -> Self {
        self.path_capture = PathCaptureConfig::sample(count, seed);
        self
    }
}

/// Builder for [`McEngine`] with ergonomic defaults.
pub struct McEngineBuilder {
    num_paths: usize,
    time_grid: Option<TimeGrid>,
    target_ci: Option<f64>,
    parallel: bool,
    chunk_size: Option<usize>,
    path_capture: PathCaptureConfig,
    antithetic: bool,
}

impl McEngineBuilder {
    /// Create a builder with default settings.
    ///
    /// The builder uses registry-backed path count and parallel defaults, and
    /// no time grid. You must provide a valid grid via
    /// [`Self::time_grid`] or [`Self::uniform_grid`] before calling [`Self::build`].
    pub fn new() -> Self {
        let defaults = &crate::registry::embedded_defaults_or_panic()
            .rust
            .engine_builder;
        Self {
            num_paths: defaults.num_paths,
            time_grid: None,
            target_ci: None,
            parallel: defaults.use_parallel,
            chunk_size: None,
            path_capture: PathCaptureConfig::default(),
            antithetic: defaults.antithetic,
        }
    }

    /// Set the requested number of paths.
    pub fn num_paths(mut self, n: usize) -> Self {
        self.num_paths = n;
        self
    }

    /// Set the simulation time grid.
    pub fn time_grid(mut self, grid: TimeGrid) -> Self {
        self.time_grid = Some(grid);
        self
    }

    /// Set a uniform time grid in year fractions.
    ///
    /// # Arguments
    ///
    /// * `t_max` - Final simulation time in years.
    /// * `num_steps` - Number of time steps between `0` and `t_max`.
    ///
    /// # Returns
    ///
    /// The builder with `time_grid` populated if `TimeGrid::uniform` succeeds.
    /// Invalid inputs leave the builder without a grid, causing
    /// [`Self::build`] to return an error later.
    pub fn uniform_grid(mut self, t_max: f64, num_steps: usize) -> Self {
        self.time_grid = TimeGrid::uniform(t_max, num_steps).ok();
        self
    }

    /// Set the target 95% CI half-width for serial auto-stopping.
    pub fn target_ci(mut self, target: f64) -> Self {
        self.target_ci = Some(target);
        self
    }

    /// Enable or disable parallel execution.
    pub fn parallel(mut self, enable: bool) -> Self {
        self.parallel = enable;
        self
    }

    /// Set the parallel chunk size to exactly `size`. Leave unset for adaptive chunking.
    pub fn chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = Some(size);
        self
    }

    /// Install a path-capture configuration.
    pub fn path_capture(mut self, config: PathCaptureConfig) -> Self {
        self.path_capture = config;
        self
    }

    /// Capture every path in the run.
    pub fn capture_all_paths(mut self) -> Self {
        self.path_capture = PathCaptureConfig::all();
        self
    }

    /// Capture a deterministic sample of paths.
    pub fn capture_sample_paths(mut self, count: usize, seed: u64) -> Self {
        self.path_capture = PathCaptureConfig::sample(count, seed);
        self
    }

    /// Enable or disable antithetic path pairing.
    pub fn antithetic(mut self, enable: bool) -> Self {
        self.antithetic = enable;
        self
    }

    /// Build an [`McEngine`] from the accumulated settings.
    ///
    /// # Errors
    ///
    /// Returns an error when no valid time grid has been configured. This
    /// commonly happens when neither [`Self::time_grid`] nor
    /// [`Self::uniform_grid`] was called, or when `uniform_grid` was called with
    /// invalid inputs.
    pub fn build(self) -> Result<McEngine> {
        let time_grid = self
            .time_grid
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        let config = McEngineConfig {
            num_paths: self.num_paths,
            time_grid,
            target_ci_half_width: self.target_ci,
            use_parallel: self.parallel,
            chunk_size: self.chunk_size,
            path_capture: self.path_capture,
            antithetic: self.antithetic,
        };

        Ok(McEngine::from_config(config))
    }
}

impl Default for McEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}
