//! Configuration and validation for [`PathDependentPricer`](super::PathDependentPricer).

use super::super::super::engine::{McEngineConfig, PathCaptureConfig};
use finstack_quant_core::Result;
use finstack_quant_core::math::time_grid::TimeGrid;

/// Domain-separation salt for the per-path auxiliary-uniform Philox streams
/// used alongside Sobol asset normals. Keeps the auxiliary key space disjoint
/// from any other component keyed on the same user seed.
pub(super) const SOBOL_AUX_DOMAIN_SALT: u64 = 0x5350_4448_5F41_5558; // "SPDH_AUX"

/// Domain-separation salt for Sobol prefix hash digit permutations.
pub(super) const SOBOL_SCRAMBLE_DOMAIN_SALT: u64 = 0x534F_424F_4C53_4352; // "SOBOLSCR"

/// Number of distinct permutation seeds used to estimate variation among replicate means.
pub(super) const SOBOL_QMC_REPLICATES: usize = 16;

/// Upper bound on captured path points (`num_paths x (num_steps + 1)`) in
/// [`PathDependentPricer::price_with_lrm_greeks_from_payoff`], which keeps every path in
/// memory to rebuild its shocks. Checked before any path is simulated.
pub(crate) const MAX_LRM_CAPTURED_POINTS: usize = 4_000_000;

/// Configuration for path-dependent option pricing.
#[derive(Debug, Clone)]
pub struct PathDependentPricerConfig {
    /// Independent Monte Carlo estimators; at least two are required. With
    /// antithetic sampling each estimator averages two physical paths. For
    /// Sobol sampling this is the total point budget across separately seeded scrambles.
    pub num_paths: usize,
    /// Random seed
    pub seed: u64,
    /// Use parallel execution
    pub use_parallel: bool,
    /// Chunk size for parallel execution
    pub chunk_size: usize,
    /// Path capture configuration
    pub path_capture: PathCaptureConfig,
    /// Steps per year for time discretization (default: 252.0)
    pub steps_per_year: f64,
    /// Minimum number of steps regardless of maturity (default: 8)
    pub min_steps: usize,
    /// Use Sobol quasi-random sequence (default: false)
    pub use_sobol: bool,
    /// Enable antithetic variates (default: false)
    pub antithetic: bool,
    /// Use Brownian bridge ordering for Sobol (QMC) paths (default: false)
    pub use_brownian_bridge: bool,
}

impl Default for PathDependentPricerConfig {
    fn default() -> Self {
        let defaults = &crate::monte_carlo::registry::embedded_defaults_or_panic()
            .rust
            .path_dependent_pricer;
        Self {
            num_paths: defaults.num_paths,
            seed: defaults.seed,
            use_parallel: defaults.use_parallel,
            chunk_size: defaults.chunk_size,
            path_capture: PathCaptureConfig::default(),
            steps_per_year: defaults.steps_per_year,
            min_steps: defaults.min_steps,
            use_sobol: defaults.use_sobol,
            antithetic: defaults.antithetic,
            use_brownian_bridge: defaults.use_brownian_bridge,
        }
    }
}

impl PathDependentPricerConfig {
    /// Create a new configuration.
    pub fn new(num_paths: usize) -> Self {
        Self {
            num_paths,
            ..Default::default()
        }
    }

    /// Set random seed.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Enable/disable parallel execution.
    ///
    /// Note that parallel mode is incompatible with `use_sobol = true` because
    /// Sobol sequences do not support deterministic stream splitting. The
    /// combination is rejected by [`Self::validate`] (and by the pricer at
    /// `price()` time); previous releases silently flipped `use_parallel` to
    /// `false`, which masked configuration mistakes.
    #[must_use]
    pub fn with_parallel(mut self, parallel: bool) -> Self {
        self.use_parallel = parallel;
        self
    }

    /// Enable Sobol quasi-random sequence.
    ///
    /// Defaults `use_brownian_bridge` to `true` when enabling Sobol because
    /// Brownian bridge construction is the standard variance-reduction
    /// companion to QMC. Override afterwards via
    /// [`Self::with_brownian_bridge`] if needed. The combination
    /// `use_sobol = true && use_parallel = true` is rejected by
    /// [`Self::validate`].
    #[must_use]
    pub fn with_sobol(mut self, use_sobol: bool) -> Self {
        self.use_sobol = use_sobol;
        if use_sobol {
            self.use_brownian_bridge = true;
        }
        self
    }

    /// Enable antithetic variates.
    #[must_use]
    pub fn with_antithetic(mut self, antithetic: bool) -> Self {
        self.antithetic = antithetic;
        self
    }

    /// Enable Brownian bridge (only used with Sobol RNG).
    ///
    /// # Arguments
    ///
    /// * `enable` - Whether to enable the optional execution feature.
    #[must_use]
    pub fn with_brownian_bridge(mut self, enable: bool) -> Self {
        self.use_brownian_bridge = enable;
        self
    }

    /// Build a time grid from the configuration's step density and required event times.
    ///
    /// # Arguments
    ///
    /// * `time_to_maturity` - Simulation horizon in years from the valuation
    ///   date; must be positive and finite.
    /// * `required_times` - Event times in years (fixings, observations,
    ///   exercise dates) in `[0, time_to_maturity]` merged into the uniform
    ///   grid as exact knots, so the result may be non-uniform.
    ///
    /// # Errors
    ///
    /// Returns an error when the horizon is invalid or the merged grid fails
    /// validation.
    pub fn build_time_grid(
        &self,
        time_to_maturity: f64,
        required_times: &[f64],
    ) -> Result<TimeGrid> {
        if !self.steps_per_year.is_finite() || self.steps_per_year <= 0.0 || self.min_steps == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "MC grid step density and minimum count must be positive".into(),
            ));
        }
        let steps = (time_to_maturity * self.steps_per_year).round();
        if !steps.is_finite() || steps >= usize::MAX as f64 || time_to_maturity <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "MC grid horizon or step count is outside the supported range".into(),
            ));
        }
        TimeGrid::uniform_with_required_times(
            time_to_maturity,
            self.min_steps.max(steps as usize),
            required_times,
        )
    }

    /// Translate this configuration into the engine configuration for one run.
    ///
    /// Path count, parallelism, chunk size, path capture and antithetic
    /// pairing are copied as configured; no confidence-interval stopping
    /// target is set. Seed, step density and the Sobol flags are not engine
    /// settings: the caller builds `time_grid` and the random stream from them.
    ///
    /// # Arguments
    ///
    /// * `time_grid` - Simulation times in years, starting at zero, typically
    ///   from [`Self::build_time_grid`]; moved into the returned configuration.
    pub fn engine_config(&self, time_grid: TimeGrid) -> McEngineConfig {
        McEngineConfig {
            num_paths: self.num_paths,
            time_grid,
            target_ci_half_width: None,
            use_parallel: self.use_parallel,
            chunk_size: Some(self.chunk_size),
            path_capture: self.path_capture.clone(),
            antithetic: self.antithetic,
        }
    }

    /// Validate the configuration eagerly, before any path is simulated.
    ///
    /// Surface configuration mistakes at builder time rather than at
    /// `price()` time, where the failure is more expensive to debug. The
    /// pricer also calls this internally so callers who skip the explicit
    /// `validate()` step still receive the same diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when:
    ///
    /// - `num_paths < 2` (sampling uncertainty requires two independent estimators),
    /// - `chunk_size == 0`,
    /// - `use_sobol && use_parallel` (Sobol cannot be split into independent
    ///   per-thread streams),
    /// - `use_sobol && antithetic` (not currently supported),
    /// - `steps_per_year` is non-positive or non-finite,
    /// - sampled path capture is enabled with `count == 0` or
    ///   `count > num_paths`.
    pub fn validate(&self) -> Result<()> {
        if self.num_paths < 2 {
            return Err(finstack_quant_core::Error::Validation(
                "PathDependentPricerConfig: num_paths must provide at least two independent estimators"
                    .to_string(),
            ));
        }
        if self.chunk_size == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "PathDependentPricerConfig: chunk_size must be greater than zero".to_string(),
            ));
        }
        if !(self.steps_per_year.is_finite() && self.steps_per_year > 0.0) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "PathDependentPricerConfig: steps_per_year must be finite and positive, got {}",
                self.steps_per_year
            )));
        }
        if self.use_sobol && self.use_parallel {
            return Err(finstack_quant_core::Error::Validation(
                "PathDependentPricerConfig: Sobol QMC requires serial execution; \
                 set use_parallel = false or use_sobol = false"
                    .to_string(),
            ));
        }
        if self.use_sobol && self.antithetic {
            return Err(finstack_quant_core::Error::Validation(
                "PathDependentPricerConfig: antithetic variates are not supported with use_sobol = true"
                    .to_string(),
            ));
        }
        if self.path_capture.enabled
            && let crate::monte_carlo::engine::PathCaptureMode::Sample { count, .. } =
                self.path_capture.capture_mode
            && (count == 0 || count > self.num_paths)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "PathDependentPricerConfig: path-capture sample count must satisfy \
                         1 <= count <= num_paths (got count = {count}, num_paths = {})",
                self.num_paths
            )));
        }
        Ok(())
    }
}
