//! Execution, configuration, and diagnostics for Monte Carlo pricing.
//!
use super::path_capture::PathCaptureConfig;
use crate::monte_carlo::TimeGrid;
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
/// Construct with [`Self::new`] or [`Self::uniform`], then pass it to
/// [`McEngine`](super::McEngine). All time values are year fractions.
#[derive(Debug, Clone)]
pub struct McEngineConfig {
    /// Requested number of independent path estimators. Runtime validation
    /// requires `2..=MAX_NUM_PATHS` so the sample variance and confidence
    /// interval are defined. Antithetic pairs each count as one estimator.
    ///
    /// With [`Self::antithetic`] disabled this equals the number of simulated
    /// sample paths. With antithetic pairing enabled the engine runs
    /// `num_paths` iterations, each simulating a `(z, -z)` pair and recording
    /// the pair's mean as a single estimator, so the total simulated paths
    /// become `2 * num_paths`. The produced [`crate::monte_carlo::estimate::Estimate`]
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
    /// * `num_paths` - Requested independent path estimators in
    ///   `2..=MAX_NUM_PATHS`, validated when pricing. Each antithetic pair
    ///   contributes one estimator.
    /// * `time_grid` - Simulation grid in year fractions.
    ///
    /// # Returns
    ///
    /// A configuration using registry-backed parallel defaults, disabled path
    /// capture, and registry-backed antithetic defaults.
    pub fn new(num_paths: usize, time_grid: TimeGrid) -> Self {
        let defaults = &crate::monte_carlo::registry::embedded_defaults_or_panic()
            .rust
            .engine;
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

    /// Create a configuration on a uniform time grid.
    ///
    /// # Arguments
    ///
    /// * `num_paths` - Requested independent path estimators in
    ///   `2..=MAX_NUM_PATHS`, validated when pricing.
    /// * `t_max` - Positive finite simulation horizon in years.
    /// * `num_steps` - Positive number of uniform time steps.
    ///
    /// # Errors
    ///
    /// Returns the time-grid validation error for an invalid horizon or step count.
    pub fn uniform(num_paths: usize, t_max: f64, num_steps: usize) -> Result<Self> {
        Ok(Self::new(num_paths, TimeGrid::uniform(t_max, num_steps)?))
    }

    /// Install path-capture configuration.
    #[must_use]
    pub fn path_capture(mut self, config: PathCaptureConfig) -> Self {
        self.path_capture = config;
        self
    }

    /// Enable or disable parallel execution.
    #[must_use]
    pub fn parallel(mut self, parallel: bool) -> Self {
        self.use_parallel = parallel;
        self
    }

    /// Set the parallel chunk size to exactly `size`.
    ///
    /// Leave unset (the default) to keep the engine's adaptive chunking. Runtime
    /// validation rejects `0`.
    #[must_use]
    pub fn chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = Some(size);
        self
    }

    /// Enable or disable antithetic path pairing.
    ///
    /// Path capture and antithetic pricing are currently mutually exclusive.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether the optional execution feature is enabled.
    #[must_use]
    pub fn antithetic(mut self, enabled: bool) -> Self {
        self.antithetic = enabled;
        self
    }
}
