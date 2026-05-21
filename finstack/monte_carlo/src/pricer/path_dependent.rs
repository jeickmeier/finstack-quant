//! Generic path-dependent option pricer with event scheduling.
//!
//! Handles payoffs that depend on the entire price path (Asians, barriers, lookbacks)
//! with flexible event scheduling.

use super::super::engine::{McEngine, McEngineConfig, PathCaptureConfig};
use super::super::results::{MoneyEstimate, MonteCarloResult};
use super::super::traits::Payoff;
use crate::discretization::exact::ExactGbm;
use crate::estimate::Estimate;
use crate::online_stats::OnlineStats;
use crate::process::gbm::GbmProcess;
use crate::process::metadata::ProcessMetadata;
use crate::rng::brownian_bridge::BrownianBridge;
use crate::rng::philox::PhiloxRng;
use crate::rng::sobol::{SobolRng, MAX_SOBOL_DIMENSION};
use crate::time_grid::TimeGrid;
use crate::traits::{Discretization, RandomStream, StochasticProcess};
use finstack_core::currency::Currency;
use finstack_core::{Error, Result};

/// Configuration for path-dependent option pricing.
#[derive(Debug, Clone)]
pub struct PathDependentPricerConfig {
    /// Number of Monte Carlo paths
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
        let defaults = &crate::registry::embedded_defaults_or_panic()
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
    pub fn with_parallel(mut self, parallel: bool) -> Self {
        self.use_parallel = parallel;
        self
    }

    /// Set chunk size.
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size;
        self
    }

    /// Set path capture configuration.
    pub fn with_path_capture(mut self, config: PathCaptureConfig) -> Self {
        self.path_capture = config;
        self
    }

    /// Enable path capture for all paths.
    pub fn capture_all_paths(mut self) -> Self {
        self.path_capture = PathCaptureConfig::all();
        self
    }

    /// Enable path capture for a sample.
    pub fn capture_sample_paths(mut self, count: usize, seed: u64) -> Self {
        self.path_capture = PathCaptureConfig::sample(count, seed);
        self
    }

    /// Set steps per year for time discretization.
    pub fn with_steps_per_year(mut self, steps: f64) -> Self {
        self.steps_per_year = steps;
        self
    }

    /// Set minimum number of steps.
    pub fn with_min_steps(mut self, min_steps: usize) -> Self {
        self.min_steps = min_steps;
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
    pub fn with_sobol(mut self, use_sobol: bool) -> Self {
        self.use_sobol = use_sobol;
        if use_sobol {
            self.use_brownian_bridge = true;
        }
        self
    }

    /// Enable antithetic variates.
    pub fn with_antithetic(mut self, antithetic: bool) -> Self {
        self.antithetic = antithetic;
        self
    }

    /// Enable Brownian bridge (only used with Sobol RNG).
    pub fn with_brownian_bridge(mut self, enable: bool) -> Self {
        self.use_brownian_bridge = enable;
        self
    }

    /// Build a time grid from the configuration's step density and required event times.
    pub fn build_time_grid(
        &self,
        time_to_maturity: f64,
        required_times: &[f64],
    ) -> Result<TimeGrid> {
        TimeGrid::uniform_with_required_times(
            time_to_maturity,
            self.steps_per_year,
            self.min_steps,
            required_times,
        )
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
    /// Returns [`finstack_core::Error::Validation`] when:
    ///
    /// - `num_paths == 0`,
    /// - `chunk_size == 0`,
    /// - `use_sobol && use_parallel` (Sobol cannot be split into independent
    ///   per-thread streams),
    /// - `use_sobol && antithetic` (not currently supported),
    /// - `steps_per_year` is non-positive or non-finite,
    /// - sampled path capture is enabled with `count == 0` or
    ///   `count > num_paths`.
    pub fn validate(&self) -> Result<()> {
        if self.num_paths == 0 {
            return Err(finstack_core::Error::Validation(
                "PathDependentPricerConfig: num_paths must be greater than zero".to_string(),
            ));
        }
        if self.chunk_size == 0 {
            return Err(finstack_core::Error::Validation(
                "PathDependentPricerConfig: chunk_size must be greater than zero".to_string(),
            ));
        }
        if !(self.steps_per_year.is_finite() && self.steps_per_year > 0.0) {
            return Err(finstack_core::Error::Validation(format!(
                "PathDependentPricerConfig: steps_per_year must be finite and positive, got {}",
                self.steps_per_year
            )));
        }
        if self.use_sobol && self.use_parallel {
            return Err(finstack_core::Error::Validation(
                "PathDependentPricerConfig: Sobol QMC requires serial execution; \
                 set use_parallel = false or use_sobol = false"
                    .to_string(),
            ));
        }
        if self.use_sobol && self.antithetic {
            return Err(finstack_core::Error::Validation(
                "PathDependentPricerConfig: antithetic variates are not supported with use_sobol = true"
                    .to_string(),
            ));
        }
        if self.path_capture.enabled {
            if let crate::engine::PathCaptureMode::Sample { count, .. } =
                self.path_capture.capture_mode
            {
                if count == 0 || count > self.num_paths {
                    return Err(finstack_core::Error::Validation(format!(
                        "PathDependentPricerConfig: path-capture sample count must satisfy \
                         1 <= count <= num_paths (got count = {count}, num_paths = {})",
                        self.num_paths
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Path-dependent option pricer.
///
/// Prices options that depend on the path history (Asians, barriers, lookbacks).
///
/// The pricer is intended for higher-level payoff types that expose required
/// fixing or monitoring times. For direct GBM European pricing, prefer
/// [`crate::pricer::european::EuropeanPricer`]; for custom process /
/// discretization combinations, use [`crate::engine::McEngine`] directly.
///
/// # Examples
///
/// ```ignore
/// use finstack_core::currency::Currency;
/// use finstack_monte_carlo::payoff::asian::{AsianCall, AveragingMethod};
/// use finstack_monte_carlo::pricer::path_dependent::{
///     PathDependentPricer, PathDependentPricerConfig,
/// };
/// use finstack_monte_carlo::process::gbm::GbmProcess;
///
/// let config = PathDependentPricerConfig::new(10_000)
///     .with_seed(42)
///     .with_parallel(false);
/// let pricer = PathDependentPricer::new(config);
/// let process = GbmProcess::with_params(0.05, 0.02, 0.20).unwrap();
/// let payoff = AsianCall::new(
///     100.0,
///     1.0,
///     AveragingMethod::Arithmetic,
///     (1..=252).collect(),
/// );
///
/// let result = pricer
///     .price(
///         &process,
///         100.0,
///         1.0,
///         252,
///         &payoff,
///         Currency::USD,
///         (-0.05_f64).exp(),
///     )
///     .unwrap();
///
/// assert!(result.mean.amount().is_finite());
/// ```
pub struct PathDependentPricer {
    config: PathDependentPricerConfig,
}

impl PathDependentPricer {
    /// Create a new path-dependent pricer.
    pub fn new(config: PathDependentPricerConfig) -> Self {
        Self { config }
    }

    fn validate_sobol_configuration(
        &self,
        time_grid: &TimeGrid,
        num_factors: usize,
    ) -> Result<usize> {
        // Generic-config invariants (parallel/antithetic/sample-count) are
        // enforced once in `PathDependentPricerConfig::validate`, called from
        // each price entry point. Only Sobol-dimension-specific checks live
        // here.

        // Brownian bridge path construction allocates the leading Sobol
        // dimensions to terminal/midpoint increments of a single scalar
        // Brownian motion. With multi-factor processes the bridge would need
        // to be applied per-factor using the increment-covariance Cholesky,
        // which is not yet implemented. Reject the combination to prevent a
        // silently biased result.
        if self.config.use_brownian_bridge && num_factors != 1 {
            return Err(finstack_core::Error::Validation(format!(
                "Brownian-bridge path construction is only supported for single-factor \
                 processes, but the supplied process reports num_factors={num_factors}. \
                 Disable `use_brownian_bridge` or use a single-factor process."
            )));
        }

        let sobol_dimension = if self.config.use_brownian_bridge {
            time_grid.num_steps()
        } else {
            time_grid.num_steps() * num_factors
        };

        if sobol_dimension == 0 || sobol_dimension > MAX_SOBOL_DIMENSION {
            return Err(finstack_core::Error::Validation(format!(
                "Sobol dimension {} is unsupported (maximum {})",
                sobol_dimension, MAX_SOBOL_DIMENSION
            )));
        }

        Ok(sobol_dimension)
    }

    #[allow(clippy::too_many_arguments)]
    fn price_with_sobol<P>(
        &self,
        process: &GbmProcess,
        initial_spot: f64,
        time_grid: TimeGrid,
        payoff: &P,
        currency: Currency,
        discount_factor: f64,
    ) -> Result<MonteCarloResult>
    where
        P: Payoff,
    {
        let sobol_dimension =
            self.validate_sobol_configuration(&time_grid, process.num_factors())?;
        let mut sobol = SobolRng::try_new(sobol_dimension, self.config.seed)
            .map_err(|err| Error::Validation(err.to_string()))?;
        let disc = ExactGbm::new();
        let initial_state = vec![initial_spot];
        let capture_enabled = self.config.path_capture.enabled;

        // Reuse the generic McEngine stepping/capture helpers; we drive a Sobol
        // RNG adapter per path, but the per-step simulate/capture logic lives
        // on the engine so there is a single code path for path construction
        // and path bookkeeping.
        let engine_config = McEngineConfig {
            num_paths: self.config.num_paths,
            time_grid: time_grid.clone(),
            target_ci_half_width: None,
            use_parallel: false,
            chunk_size: self.config.chunk_size,
            path_capture: self.config.path_capture.clone(),
            antithetic: false,
        };
        let engine = McEngine::new(engine_config);

        let num_factors = process.num_factors();
        let num_steps = time_grid.num_steps();
        let bridge = self
            .config
            .use_brownian_bridge
            .then(|| BrownianBridge::new(num_steps));

        let mut stats = OnlineStats::new();
        let mut state = vec![0.0; process.dim()];
        let mut work = vec![0.0; disc.work_size(process)];
        let mut z_step = vec![0.0; num_factors];
        let correlation = crate::engine::build_correlation_factor(process, &disc)?;
        let mut z_raw = vec![
            0.0;
            if correlation.is_some() {
                num_factors
            } else {
                0
            }
        ];
        let mut z_path = vec![0.0; sobol_dimension];
        let mut z_increments = vec![0.0; num_steps * num_factors];
        let mut w_path = vec![0.0; num_steps + 1];
        let mut captured_paths = if capture_enabled {
            let estimated_capacity = match self.config.path_capture.capture_mode {
                crate::engine::PathCaptureMode::All => self.config.num_paths,
                crate::engine::PathCaptureMode::Sample { count, .. } => count,
            };
            Vec::with_capacity(estimated_capacity)
        } else {
            Vec::new()
        };
        let mut payoff_local = payoff.clone();

        for path_id in 0..self.config.num_paths {
            sobol.fill_std_normals(&mut z_path);
            fill_sobol_increments(
                &z_path,
                &mut z_increments,
                &mut w_path,
                bridge.as_ref(),
                &time_grid,
                num_factors,
            );

            // Auxiliary uniforms are drawn from a per-path Philox so the Sobol
            // stream only carries asset-dimension normals.
            let mut adapter = SobolPathStream::new(
                &z_increments,
                PhiloxRng::new(self.config.seed ^ ((path_id as u64) << 1)),
            );

            payoff_local.reset();
            payoff_local.on_path_start(&mut adapter);

            let should_capture = capture_enabled
                && self
                    .config
                    .path_capture
                    .should_capture(path_id, self.config.num_paths);

            let payoff_value = if should_capture {
                let (value, path) = engine.simulate_path_with_capture(
                    &mut adapter,
                    process,
                    &disc,
                    &initial_state,
                    &mut payoff_local,
                    &mut state,
                    &mut z_step,
                    &mut z_raw,
                    &mut work,
                    correlation.as_ref(),
                    path_id,
                    discount_factor,
                    currency,
                )?;
                captured_paths.push(path);
                value
            } else {
                engine.simulate_path(
                    &mut adapter,
                    process,
                    &disc,
                    &initial_state,
                    &mut payoff_local,
                    &mut state,
                    &mut z_step,
                    &mut z_raw,
                    &mut work,
                    correlation.as_ref(),
                    currency,
                )?
            };

            let discounted_value =
                crate::engine::validate_discounted_payoff(path_id, payoff_value, discount_factor)?;
            stats.update(discounted_value);
        }

        let estimate = Estimate::new(
            stats.mean(),
            stats.stderr(),
            stats.confidence_interval(0.05),
            stats.count(),
        )
        .with_std_dev(stats.std_dev());

        Ok(engine.finalize_captured_result(
            estimate,
            captured_paths,
            currency,
            Some(process.metadata()),
        ))
    }

    /// Price a path-dependent option.
    ///
    /// # Arguments
    ///
    /// * `process` - GBM process
    /// * `initial_spot` - Initial spot price
    /// * `time_to_maturity` - Time to maturity in years
    /// * `num_steps` - Number of time steps
    /// * `payoff` - Path-dependent payoff
    /// * `currency` - Currency for result
    /// * `discount_factor` - Discount factor to maturity
    #[allow(clippy::too_many_arguments)]
    pub fn price<P>(
        &self,
        process: &GbmProcess,
        initial_spot: f64,
        time_to_maturity: f64,
        num_steps: usize,
        payoff: &P,
        currency: Currency,
        discount_factor: f64,
    ) -> Result<MoneyEstimate>
    where
        P: Payoff,
    {
        // Create time grid
        let time_grid = TimeGrid::uniform(time_to_maturity, num_steps)?;
        self.price_with_grid(
            process,
            initial_spot,
            time_grid,
            payoff,
            currency,
            discount_factor,
        )
    }

    /// Price a path-dependent option with a custom time grid.
    #[allow(clippy::too_many_arguments)]
    pub fn price_with_grid<P>(
        &self,
        process: &GbmProcess,
        initial_spot: f64,
        time_grid: TimeGrid,
        payoff: &P,
        currency: Currency,
        discount_factor: f64,
    ) -> Result<MoneyEstimate>
    where
        P: Payoff,
    {
        self.config.validate()?;
        if self.config.use_sobol {
            return self
                .price_with_sobol(
                    process,
                    initial_spot,
                    time_grid,
                    payoff,
                    currency,
                    discount_factor,
                )
                .map(|result| result.estimate);
        }

        // Create MC engine. Antithetic pairing is handled inline by the engine
        // (see McEngine::simulate_antithetic_pair); path-capture + antithetic
        // is rejected at validate_runtime.
        let engine_config = McEngineConfig {
            num_paths: self.config.num_paths,
            time_grid,
            target_ci_half_width: None,
            use_parallel: self.config.use_parallel,
            chunk_size: self.config.chunk_size,
            path_capture: self.config.path_capture.clone(),
            antithetic: self.config.antithetic,
        };

        let engine = McEngine::new(engine_config);
        let disc = ExactGbm::new();
        let initial_state = vec![initial_spot];
        let rng = PhiloxRng::new(self.config.seed);

        if engine.config().path_capture.enabled {
            let process_params = process.metadata();
            let result = engine.price_with_capture(
                &rng,
                process,
                &disc,
                &initial_state,
                payoff,
                currency,
                discount_factor,
                process_params,
            )?;
            Ok(result.estimate)
        } else {
            engine.price(
                &rng,
                process,
                &disc,
                &initial_state,
                payoff,
                currency,
                discount_factor,
            )
        }
    }

    /// Price with full Monte Carlo result (including captured paths if enabled).
    ///
    /// This method returns a `MonteCarloResult` which includes the estimate
    /// and optionally captured paths based on the pricer configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn price_with_paths<P>(
        &self,
        process: &GbmProcess,
        initial_spot: f64,
        time_to_maturity: f64,
        num_steps: usize,
        payoff: &P,
        currency: Currency,
        discount_factor: f64,
    ) -> Result<MonteCarloResult>
    where
        P: Payoff,
    {
        self.config.validate()?;
        if self.config.use_sobol {
            let time_grid = TimeGrid::uniform(time_to_maturity, num_steps)?;
            return self.price_with_sobol(
                process,
                initial_spot,
                time_grid,
                payoff,
                currency,
                discount_factor,
            );
        }

        // Create time grid
        let time_grid = TimeGrid::uniform(time_to_maturity, num_steps)?;

        // Create MC engine with path capture
        let engine_config = McEngineConfig {
            num_paths: self.config.num_paths,
            time_grid,
            target_ci_half_width: None,
            use_parallel: self.config.use_parallel,
            chunk_size: self.config.chunk_size,
            path_capture: self.config.path_capture.clone(),
            antithetic: false,
        };
        let engine = McEngine::new(engine_config);

        let disc = ExactGbm::new();
        let initial_state = vec![initial_spot];
        let process_params = process.metadata();

        let rng = PhiloxRng::new(self.config.seed);
        engine.price_with_capture(
            &rng,
            process,
            &disc,
            &initial_state,
            payoff,
            currency,
            discount_factor,
            process_params,
        )
    }

    /// Price and compute LRM Greeks (delta, vega) for GBM using captured paths.
    ///
    /// This uses the Likelihood Ratio Method, deriving the standardized terminal
    /// shock `Z = W_T / √T` from terminal spots under GBM:
    /// `Z = (ln(S_T/S_0) - (r - q - 0.5 σ^2) T) / (σ √T)`.
    #[allow(clippy::too_many_arguments)]
    pub fn price_with_lrm_greeks<P>(
        &self,
        process: &GbmProcess,
        initial_spot: f64,
        time_to_maturity: f64,
        num_steps: usize,
        payoff: &P,
        currency: Currency,
        discount_factor: f64,
        rate: f64,
        dividend_yield: f64,
        volatility: f64,
    ) -> Result<(MoneyEstimate, Option<(f64, f64)>)>
    where
        P: Payoff,
    {
        self.config.validate()?;
        // Force path capture to get terminal spots and final discounted payoff values
        let time_grid = TimeGrid::uniform(time_to_maturity, num_steps)?;
        let engine_config = McEngineConfig {
            num_paths: self.config.num_paths,
            time_grid,
            target_ci_half_width: None,
            use_parallel: self.config.use_parallel,
            chunk_size: self.config.chunk_size,
            path_capture: PathCaptureConfig::all().with_payoffs(),
            antithetic: false,
        };
        let engine = McEngine::new(engine_config);

        let rng = crate::rng::philox::PhiloxRng::new(self.config.seed);
        let disc = ExactGbm::new();
        let initial_state = vec![initial_spot];

        // Process metadata
        let process_params = process.metadata();

        let full = engine.price_with_capture(
            &rng,
            process,
            &disc,
            &initial_state,
            payoff,
            currency,
            discount_factor,
            process_params,
        )?;

        // Extract estimate and paths
        let estimate = full.estimate.clone();
        let paths = match &full.paths {
            Some(ds) => &ds.paths,
            None => return Ok((estimate, None)),
        };

        if paths.is_empty()
            || discount_factor <= 0.0
            || time_to_maturity <= 0.0
            || volatility <= 0.0
        {
            return Ok((estimate, None));
        }

        // Build undiscounted payoffs and standardized terminal shocks.
        let mut payoffs: Vec<f64> = Vec::with_capacity(paths.len());
        let mut terminal_shocks: Vec<f64> = Vec::with_capacity(paths.len());
        let mu = rate - dividend_yield - 0.5 * volatility * volatility;
        for p in paths {
            // Final discounted payoff value is stored; un-discount it
            let undisc = p.final_value / discount_factor;
            payoffs.push(undisc);

            // Terminal spot from last point's state var
            if let Some(last) = p.terminal_point() {
                if let Some(s_t) = last.spot() {
                    let z_t = ((s_t / initial_spot).ln() - mu * time_to_maturity)
                        / (volatility * time_to_maturity.sqrt());
                    terminal_shocks.push(z_t);
                }
            }
        }

        if payoffs.len() == terminal_shocks.len() && !payoffs.is_empty() {
            use super::super::greeks::lrm::{lrm_delta, lrm_vega};
            let (delta, _) = lrm_delta(
                &payoffs,
                &terminal_shocks,
                initial_spot,
                volatility,
                time_to_maturity,
                discount_factor,
            );
            let (vega, _) = lrm_vega(
                &payoffs,
                &terminal_shocks,
                volatility,
                time_to_maturity,
                discount_factor,
            );
            Ok((estimate, Some((delta, vega))))
        } else {
            Ok((estimate, None))
        }
    }

    /// Get configuration.
    pub fn config(&self) -> &PathDependentPricerConfig {
        &self.config
    }
}

/// Fill `z_increments` with per-step standard normals derived from a single
/// Sobol path draw.
///
/// Without a Brownian bridge the Sobol draw is already laid out in
/// `step-major × factor-major` order, so it is copied directly. With a
/// Brownian bridge (single-factor only, as enforced by
/// [`PathDependentPricer::validate_sobol_configuration`]) the draw is
/// converted to a scalar Brownian motion `w_path` and then scaled to standard
/// normals per step by `(w[i+1] - w[i]) / sqrt(dt_i)`.
fn fill_sobol_increments(
    z_path: &[f64],
    z_increments: &mut [f64],
    w_path: &mut [f64],
    bridge: Option<&BrownianBridge>,
    time_grid: &TimeGrid,
    num_factors: usize,
) {
    let num_steps = time_grid.num_steps();
    match bridge {
        Some(bridge) => {
            debug_assert_eq!(num_factors, 1, "Brownian bridge only supports 1 factor");
            if time_grid.is_uniform() {
                bridge.construct_path(z_path, w_path, time_grid.dt(0));
            } else {
                bridge.construct_path_irregular(z_path, w_path, time_grid.times());
            }
            for step in 0..num_steps {
                let dt = time_grid.dt(step);
                z_increments[step] = (w_path[step + 1] - w_path[step]) / dt.sqrt();
            }
        }
        None => {
            z_increments.copy_from_slice(&z_path[..num_steps * num_factors]);
        }
    }
}

/// Per-path [`RandomStream`] adapter that feeds pre-computed standard normals
/// from a Sobol draw into the generic [`McEngine`] step loop while routing
/// uniform draws to an independent Philox stream.
///
/// The adapter cannot be split: each path gets a fresh adapter constructed by
/// the Sobol pricer outer loop, so the engine's per-path `rng.split(..)` call
/// is bypassed by delegating to `simulate_path`/`simulate_path_with_capture`
/// directly.
#[derive(Clone)]
struct SobolPathStream<'a> {
    z_increments: &'a [f64],
    cursor: usize,
    aux: PhiloxRng,
}

impl<'a> SobolPathStream<'a> {
    fn new(z_increments: &'a [f64], aux: PhiloxRng) -> Self {
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

    fn fill_std_normals(&mut self, out: &mut [f64]) {
        let n = out.len();
        let end = self.cursor + n;
        debug_assert!(
            end <= self.z_increments.len(),
            "SobolPathStream exhausted: requested {n} beyond remaining {}",
            self.z_increments.len() - self.cursor
        );
        out.copy_from_slice(&self.z_increments[self.cursor..end]);
        self.cursor = end;
    }

    fn supports_splitting(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{PathDependentPricer, PathDependentPricerConfig};

    #[test]
    fn test_validate_rejects_sobol_plus_parallel() {
        let cfg = PathDependentPricerConfig::new(1_000)
            .with_parallel(true)
            .with_sobol(true);
        let err = cfg
            .validate()
            .expect_err("Sobol + parallel must be rejected");
        assert!(err.to_string().contains("Sobol"));
    }

    #[test]
    fn test_validate_rejects_sobol_plus_antithetic() {
        let cfg = PathDependentPricerConfig::new(1_000)
            .with_parallel(false)
            .with_sobol(true)
            .with_antithetic(true);
        let err = cfg
            .validate()
            .expect_err("Sobol + antithetic must be rejected");
        assert!(err.to_string().contains("antithetic"));
    }

    #[test]
    fn test_validate_rejects_zero_paths() {
        let cfg = PathDependentPricerConfig::new(0);
        let err = cfg.validate().expect_err("zero paths must be rejected");
        assert!(err.to_string().contains("num_paths"));
    }

    use crate::payoff::asian::{AsianCall, AveragingMethod};
    use crate::process::gbm::{GbmParams, GbmProcess};
    use crate::rng::sobol::MAX_SOBOL_DIMENSION;
    use crate::time_grid::TimeGrid;
    use finstack_core::currency::Currency;

    use crate::payoff::lookback::{Lookback, LookbackDirection};

    #[test]
    fn test_path_dependent_pricer_asian() {
        let config = PathDependentPricerConfig::new(10_000)
            .with_seed(42)
            .with_parallel(false);
        let pricer = PathDependentPricer::new(config);

        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.02, 0.2).unwrap());

        // Monthly fixings
        let fixing_steps: Vec<usize> = (0..=12).map(|i| i * 21).collect();
        let asian = AsianCall::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps);

        let result = pricer
            .price(&gbm, 100.0, 1.0, 252, &asian, Currency::USD, 1.0)
            .expect("should succeed");

        // Should get reasonable Asian option value
        assert!(result.mean.amount() > 0.0);
        assert!(result.mean.amount() < 20.0);
    }

    #[test]
    fn test_path_dependent_pricer_lookback() {
        let config = PathDependentPricerConfig::new(10_000)
            .with_seed(42)
            .with_parallel(false);
        let pricer = PathDependentPricer::new(config);

        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.3).unwrap());
        let lookback = Lookback::new(LookbackDirection::Call, 100.0, 1.0, 252);

        let result = pricer
            .price(&gbm, 100.0, 1.0, 252, &lookback, Currency::USD, 1.0)
            .expect("should succeed");

        // Lookback should have positive value
        assert!(result.mean.amount() > 0.0);
    }

    #[test]
    fn test_sobol_price_with_grid_multiple_paths() {
        let config = PathDependentPricerConfig::new(8)
            .with_seed(7)
            .with_parallel(false)
            .with_sobol(true)
            .with_brownian_bridge(false);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let time_grid = TimeGrid::uniform(1.0, 4).expect("grid should build");
        let fixing_steps = vec![1, 2, 3, 4];
        let asian = AsianCall::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps);

        let result = pricer
            .price_with_grid(&gbm, 100.0, time_grid, &asian, Currency::USD, 1.0)
            .expect("Sobol pricing should succeed for multiple paths");

        assert_eq!(result.num_paths, 8);
    }

    #[test]
    fn test_sobol_price_with_paths_multiple_paths() {
        fn interpolated_percentile(sorted_values: &[f64], percentile: f64) -> f64 {
            let rank = percentile * (sorted_values.len() - 1) as f64;
            let lower = rank.floor() as usize;
            let upper = rank.ceil() as usize;
            if lower == upper {
                sorted_values[lower]
            } else {
                let weight = rank - lower as f64;
                sorted_values[lower] * (1.0 - weight) + sorted_values[upper] * weight
            }
        }

        let config = PathDependentPricerConfig::new(8)
            .with_seed(11)
            .with_parallel(false)
            .with_sobol(true)
            .capture_all_paths();
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let fixing_steps = vec![1, 2, 3, 4];
        let asian = AsianCall::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps);

        let result = pricer
            .price_with_paths(&gbm, 100.0, 1.0, 4, &asian, Currency::USD, 1.0)
            .expect("Sobol path capture should succeed for multiple paths");

        assert_eq!(result.estimate.num_paths, 8);
        assert_eq!(result.num_captured_paths(), 8);

        let captured = result.paths.as_ref().expect("paths should be captured");
        let mut final_values: Vec<f64> =
            captured.paths.iter().map(|path| path.final_value).collect();
        final_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let len = final_values.len();
        let expected_median = if len.is_multiple_of(2) {
            (final_values[len / 2 - 1] + final_values[len / 2]) / 2.0
        } else {
            final_values[len / 2]
        };
        let expected_p25 = interpolated_percentile(&final_values, 0.25);
        let expected_p75 = interpolated_percentile(&final_values, 0.75);

        assert_eq!(result.estimate.median, Some(expected_median));
        assert_eq!(result.estimate.percentile_25, Some(expected_p25));
        assert_eq!(result.estimate.percentile_75, Some(expected_p75));
        assert_eq!(result.estimate.min, Some(final_values[0]));
        assert_eq!(result.estimate.max, Some(final_values[len - 1]));
    }

    #[test]
    fn test_sobol_brownian_bridge_supports_irregular_grid() {
        let config = PathDependentPricerConfig::new(8)
            .with_seed(13)
            .with_parallel(false)
            .with_sobol(true)
            .with_brownian_bridge(true);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let time_grid = TimeGrid::from_times(vec![0.0, 0.2, 0.55, 1.0]).expect("grid should build");
        let fixing_steps = vec![1, 2, 3];
        let asian = AsianCall::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps);

        let result = pricer
            .price_with_grid(&gbm, 100.0, time_grid, &asian, Currency::USD, 1.0)
            .expect("Irregular-grid Sobol Brownian bridge pricing should succeed");

        assert_eq!(result.num_paths, 8);
    }

    #[test]
    fn test_sobol_rejects_excessive_dimension() {
        let config = PathDependentPricerConfig::new(1)
            .with_seed(17)
            .with_parallel(false)
            .with_sobol(true)
            .with_brownian_bridge(false);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let fixing_steps = vec![MAX_SOBOL_DIMENSION + 1];
        let asian = AsianCall::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps);

        let err = pricer
            .price(
                &gbm,
                100.0,
                1.0,
                MAX_SOBOL_DIMENSION + 1,
                &asian,
                Currency::USD,
                1.0,
            )
            .expect_err("excessive Sobol dimension should be rejected");

        assert!(err.to_string().contains("Sobol"));
    }
}
