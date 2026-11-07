//! Generic path-dependent option pricer with event scheduling.
//!
//! Handles payoffs that depend on the entire price path (Asians, barriers, lookbacks)
//! with flexible event scheduling.

use super::super::engine::{
    McEngine, McEngineConfig, McPricingInputs, PathCaptureConfig, PathInputs, PathScratch,
};
use super::super::results::{MoneyEstimate, MonteCarloResult};
use super::super::traits::Payoff;
use crate::monte_carlo::discretization::exact::ExactGbm;
use crate::monte_carlo::estimate::Estimate;
use crate::monte_carlo::greeks::lrm::{LrmGreeks, lrm_delta, lrm_vega_from_scores};
use crate::monte_carlo::payoff::asian::{Asian, AsianPair, AveragingMethod, default_fixing_steps};
use crate::monte_carlo::process::gbm::GbmProcess;
use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::monte_carlo::traits::ProcessMetadata;
use crate::monte_carlo::traits::{RandomStream, StochasticProcess};
use crate::types::OptionMarketParams;
use finstack_quant_core::cashflow::flat_discount_factor;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::random::BrownianBridge;
use finstack_quant_core::math::random::{MAX_SOBOL_DIMENSION, SobolRng};
use finstack_quant_core::math::stats::{OnlineCovariance, OnlineStats};
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_core::{Error, Result};

/// Domain-separation salt for the per-path auxiliary-uniform Philox streams
/// used alongside Sobol asset normals. Keeps the auxiliary key space disjoint
/// from any other component keyed on the same user seed.
const SOBOL_AUX_DOMAIN_SALT: u64 = 0x5350_4448_5F41_5558; // "SPDH_AUX"

/// Domain-separation salt for the Sobol Owen-scrambling seed. XORing the user
/// seed with this constant (and flooring at 1) guarantees scrambling is never
/// silently disabled by a zero seed.
const SOBOL_SCRAMBLE_DOMAIN_SALT: u64 = 0x534F_424F_4C53_4352; // "SOBOLSCR"

/// Number of independently scrambled Sobol replicates used for the
/// randomized-QMC error estimate.
///
/// Points within one scrambled Sobol sequence are deliberately dependent, so
/// `sample_std/√n` over them is not a valid standard error. The standard
/// remedy (Owen 1997; Glasserman 2003, §5.4) is R independent randomizations:
/// the run is split into R replicates, each with its own Owen-scrambling
/// seed, and the standard error is computed across the R replicate means.
const SOBOL_QMC_REPLICATES: usize = 16;

/// Upper bound on captured path points (`num_paths x (num_steps + 1)`) in
/// [`PathDependentPricer::price_with_lrm_greeks_from_payoff`], which keeps every path in
/// memory to rebuild its shocks. Checked before any path is simulated.
pub(crate) const MAX_LRM_CAPTURED_POINTS: usize = 4_000_000;

/// Configuration for path-dependent option pricing.
#[derive(Debug, Clone)]
pub struct PathDependentPricerConfig {
    /// Independent Monte Carlo estimators; at least two are required. With
    /// antithetic sampling each estimator averages two physical paths. For
    /// Sobol sampling this is the total point budget across independent scrambles.
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
    ///   exercise dates) in `(0, time_to_maturity]` merged into the uniform
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
        TimeGrid::uniform_with_required_times(
            time_to_maturity,
            self.steps_per_year,
            self.min_steps,
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

/// Path-dependent option pricer.
///
/// Prices options that depend on the path history (Asians, barriers, lookbacks).
///
/// The pricer is intended for higher-level payoff types that expose required
/// fixing or monitoring times. For direct GBM European pricing, prefer
/// [`crate::monte_carlo::pricer::european::EuropeanPricer`]; for custom process /
/// discretization combinations, use [`crate::monte_carlo::engine::McEngine`] directly.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::math::time_grid::TimeGrid;
/// use finstack_quant_models::monte_carlo::payoff::asian::{Asian, AveragingMethod};
/// use finstack_quant_models::monte_carlo::pricer::path_dependent::{
///     PathDependentPricer, PathDependentPricerConfig,
/// };
/// use finstack_quant_models::monte_carlo::process::gbm::GbmProcess;
/// use finstack_quant_models::types::OptionType;
///
/// let config = PathDependentPricerConfig::new(10_000)
///     .with_seed(42)
///     .with_parallel(false);
/// let pricer = PathDependentPricer::new(config);
/// let process = finstack_quant_models::monte_carlo::process::gbm::GbmParams::new(0.05, 0.02, 0.20).map(GbmProcess::new).unwrap();
/// let payoff = Asian::new(
///     OptionType::Call,
///     100.0,
///     1.0,
///     AveragingMethod::Arithmetic,
///     (1..=252).collect(),
/// ).expect("nonempty fixing schedule");
///
/// let grid = TimeGrid::uniform(1.0, 252).expect("valid grid");
/// let result = pricer
///     .price(&process, 100.0, grid, &payoff, Currency::USD, (-0.05_f64).exp())
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
            return Err(finstack_quant_core::Error::Validation(format!(
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
            return Err(finstack_quant_core::Error::Validation(format!(
                "Sobol dimension {} is unsupported (maximum {})",
                sobol_dimension, MAX_SOBOL_DIMENSION
            )));
        }

        Ok(sobol_dimension)
    }

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
        let disc = ExactGbm::new();
        let initial_state = vec![initial_spot];
        let capture_enabled = self.config.path_capture.enabled;

        // Reuse the generic McEngine stepping/capture helpers; we drive a Sobol
        // RNG adapter per path, but the per-step simulate/capture logic lives
        // on the engine so there is a single code path for path construction
        // and path bookkeeping.
        // The Sobol loop below is serial and unpaired whatever the
        // configuration says (validation rejects both combinations).
        let mut engine_config = self.config.engine_config(time_grid.clone());
        engine_config.use_parallel = false;
        engine_config.antithetic = false;
        let engine = McEngine::new(engine_config);
        engine.validate_runtime(
            process,
            &initial_state,
            discount_factor,
            Some(&process.metadata()),
        )?;
        McEngine::validate_scheme_pairing(process, &disc)?;
        engine.validate_payoff_schedule(payoff)?;

        let (disc, correlation) = engine.prepare_run(process, &disc)?;
        let inputs = PathInputs {
            process,
            disc: &disc,
            initial_state: &initial_state,
            correlation: correlation.as_ref(),
            currency,
            discount_factor,
        };
        let mut scratch = PathScratch::new(inputs, payoff, false);
        let mut captured_paths = if capture_enabled {
            let estimated_capacity = match self.config.path_capture.capture_mode {
                crate::monte_carlo::engine::PathCaptureMode::All => self.config.num_paths,
                crate::monte_carlo::engine::PathCaptureMode::Sample { count, .. } => count,
            };
            Vec::with_capacity(estimated_capacity)
        } else {
            Vec::new()
        };

        // Randomized QMC: split the run into R independently scrambled
        // replicates. The standard error comes from the R replicate means
        // — the statistically valid error estimate for
        // QMC (points within one scrambled sequence are dependent by
        // construction, so a plain sample stderr over them is meaningless).
        let mut replicate_means = OnlineStats::new();
        let mut overall = OnlineStats::new();
        let mut replicate_stats = OnlineStats::new();
        self.for_each_sobol_path(
            &time_grid,
            process.num_factors(),
            sobol_dimension,
            |path_id, last_in_replicate, adapter| {
                let discounted_value = engine.simulate_discounted(
                    adapter,
                    inputs,
                    &mut scratch,
                    path_id,
                    capture_enabled.then_some(&mut captured_paths),
                )?;
                replicate_stats.update(discounted_value);
                overall.update(discounted_value);
                if last_in_replicate {
                    replicate_means.update(replicate_stats.mean());
                    replicate_stats = OnlineStats::new();
                }
                Ok(())
            },
        )?;

        // Unbiased point estimate across all simulated paths (`overall.mean()`),
        // with RQMC standard error across the R independently scrambled replicate
        // means (`replicate_means.stderr()`). When `num_paths % replicates != 0`,
        // an unweighted mean of replicate means would overweight shorter replicates.
        // The 95% half-width is the Student-t interval over the R replicate
        // means (R - 1 degrees of freedom); `validate_runtime` guarantees
        // `num_paths >= 2`, so R >= 2 and the interval is defined.
        let mean = overall.mean();
        let half_width = replicate_means.ci_half_width();
        let estimate = Estimate::new(
            mean,
            replicate_means.stderr(),
            (mean - half_width, mean + half_width),
            overall.count(),
        )
        .with_std_dev(overall.std_dev());

        let mut result = engine.finalize_captured_result(
            estimate,
            captured_paths,
            currency,
            process.metadata(),
        )?;
        if let Some(run) = result.run.as_mut() {
            run.seed = Some(self.config.seed);
        }
        Ok(result)
    }

    // Own the Sobol sequence and all stream-domain decisions in one driver.
    // Both scalar estimates and paired Asian moments consume the same paths;
    // their different statistics do not alter point or replicate ordering.
    fn for_each_sobol_path<F>(
        &self,
        time_grid: &TimeGrid,
        num_factors: usize,
        sobol_dimension: usize,
        mut visit: F,
    ) -> Result<()>
    where
        F: for<'a> FnMut(usize, bool, &mut SobolPathStream<'a>) -> Result<()>,
    {
        let num_steps = time_grid.num_steps();
        let bridge = self
            .config
            .use_brownian_bridge
            .then(|| BrownianBridge::new(num_steps));
        let mut z_path = vec![0.0; sobol_dimension];
        let mut z_increments = vec![0.0; num_steps * num_factors];
        let mut w_path = vec![0.0; num_steps + 1];
        let replicates = SOBOL_QMC_REPLICATES.min(self.config.num_paths);
        let base_paths = self.config.num_paths / replicates;
        let remainder = self.config.num_paths % replicates;
        let mut path_id = 0usize;
        for replicate in 0..replicates {
            // A zero scramble seed disables Owen scrambling. The Weyl
            // increment assigns a distinct, nonzero seed to each replicate.
            let scramble_seed = (self.config.seed ^ SOBOL_SCRAMBLE_DOMAIN_SALT)
                .wrapping_add((replicate as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
                .max(1);
            let mut sobol = SobolRng::try_new(sobol_dimension, scramble_seed)
                .map_err(|err| Error::Validation(err.to_string()))?;
            let replicate_paths = base_paths + usize::from(replicate < remainder);
            for point in 0..replicate_paths {
                sobol.fill_std_normals(&mut z_path);
                fill_sobol_increments(
                    &z_path,
                    &mut z_increments,
                    &mut w_path,
                    bridge.as_ref(),
                    time_grid,
                    num_factors,
                )?;
                // Auxiliary uniforms use disjoint per-path Philox counter
                // streams; Sobol carries only the asset normals.
                let mut adapter = SobolPathStream::new(
                    &z_increments,
                    PhiloxRng::with_stream(
                        self.config.seed ^ SOBOL_AUX_DOMAIN_SALT,
                        path_id as u64,
                    ),
                );
                visit(path_id, point + 1 == replicate_paths, &mut adapter)?;
                path_id += 1;
            }
        }
        Ok(())
    }

    /// Collect discounted arithmetic and geometric Asian payoff moments on
    /// the same simulated paths, without retaining trajectories or payoffs.
    ///
    /// The geometric control uses the arithmetic payoff's strike, quantity,
    /// fixing indices, fixing multipliers and historical fixing state. For
    /// pseudo-random paths, fixed path-ID chunks are merged in the same order
    /// in serial and parallel runs. For Sobol, moments are folded in point
    /// order and are suitable for fitting a control coefficient and mean;
    /// their point-level variances are **not** an IID uncertainty estimate.
    ///
    /// # Arguments
    ///
    /// * `process` - GBM dynamics, including any cumulative carry schedule.
    /// * `initial_spot` - Finite initial asset price in payoff price units. For
    ///   geometric averaging, callers should supply a positive price; the
    ///   underlying GBM initial-state rules are unchanged.
    /// * `time_grid` - Owned simulation times in years from zero, containing
    ///   every contractual fixing index used by `payoff`.
    /// * `payoff` - Arithmetic Asian payoff. Its existing historical state and
    ///   fixing multipliers are preserved for both accumulators.
    /// * `currency` - Currency in which both discounted payoffs are evaluated.
    /// * `discount_factor` - Finite, non-negative present-value factor from payoff
    ///   payment to valuation, applied once to each payoff after simulation.
    ///
    /// # Returns
    ///
    /// Paired moments with arithmetic payoff as X and geometric payoff as Y,
    /// both discounted and scaled by the payoff quantity. The count is the
    /// configured number of independent pseudo-random paths or dependent
    /// Sobol points. No sampling uncertainty is inferred from these moments.
    ///
    /// # Errors
    ///
    /// Returns validation errors for non-arithmetic payoffs, antithetic or
    /// capture requests, invalid configuration, incompatible Sobol settings,
    /// invalid market inputs or fixing indices. Payoff and simulation errors
    /// propagate, including nonfinite target or control values.
    pub fn price_asian_pair(
        &self,
        process: &GbmProcess,
        initial_spot: f64,
        time_grid: TimeGrid,
        payoff: &Asian,
        currency: Currency,
        discount_factor: f64,
    ) -> Result<OnlineCovariance> {
        self.config.validate()?;
        if self.config.antithetic {
            return Err(Error::Validation(
                "Arithmetic Asian paired moments do not support antithetic sampling".into(),
            ));
        }
        if self.config.path_capture.enabled {
            return Err(Error::Validation(
                "Asian paired moments do not return captured paths".into(),
            ));
        }
        let pair = AsianPair::new(payoff)?;
        let disc = ExactGbm::new();
        let initial_state = [initial_spot];
        let engine = McEngine::new(self.config.engine_config(time_grid.clone()));
        if !self.config.use_sobol {
            let rng = PhiloxRng::new(self.config.seed);
            return engine.paired_moments(
                McPricingInputs {
                    rng: &rng,
                    process,
                    disc: &disc,
                    initial_state: &initial_state,
                    payoff: &pair,
                    currency,
                    discount_factor,
                },
                AsianPair::control_value,
            );
        }

        let sobol_dimension =
            self.validate_sobol_configuration(&time_grid, process.num_factors())?;
        engine.validate_runtime(
            process,
            &initial_state,
            discount_factor,
            Some(&process.metadata()),
        )?;
        McEngine::validate_scheme_pairing(process, &disc)?;
        engine.validate_payoff_schedule(&pair)?;
        let (disc, correlation) = engine.prepare_run(process, &disc)?;
        let inputs = PathInputs {
            process,
            disc: &disc,
            initial_state: &initial_state,
            correlation: correlation.as_ref(),
            currency,
            discount_factor,
        };
        let mut scratch = PathScratch::new(inputs, &pair, false);
        let mut moments = OnlineCovariance::new();
        self.for_each_sobol_path(
            &time_grid,
            process.num_factors(),
            sobol_dimension,
            |path_id, _, adapter| {
                let (arithmetic, geometric) = engine.simulate_discounted_pair(
                    adapter,
                    inputs,
                    &mut scratch,
                    path_id,
                    &AsianPair::control_value,
                )?;
                moments.update(arithmetic, geometric);
                Ok(())
            },
        )?;
        Ok(moments)
    }

    /// Price a path-dependent option on a simulation grid.
    ///
    /// # Arguments
    ///
    /// * `process` - GBM dynamics, including any cumulative carry schedule.
    /// * `initial_spot` - Finite, positive initial asset price in payoff price units.
    /// * `time_grid` - Simulation times in years from zero, for example
    ///   [`TimeGrid::uniform`]`(maturity, num_steps)`; include every
    ///   contractual observation time.
    /// * `payoff` - Path payoff evaluated at the grid's observation indices.
    /// * `currency` - Currency of the discounted estimate.
    /// * `discount_factor` - Present-value factor from payoff payment to
    ///   valuation. Build it with
    ///   [`finstack_quant_core::cashflow::flat_discount_factor`] when working
    ///   from a flat continuously compounded rate and a year fraction.
    ///
    /// # Errors
    ///
    /// Returns validation or simulation errors, including incompatible
    /// capture and variance reduction.
    pub fn price<P>(
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
        self.run_grid(
            process,
            initial_spot,
            time_grid,
            payoff,
            currency,
            discount_factor,
        )
        .map(|result| result.estimate)
    }

    /// Price an arithmetic Asian option under risk-neutral GBM.
    ///
    /// Uses unit notional, arithmetic averaging, and the default post-step
    /// fixing schedule [`default_fixing_steps`]. Discounting is the flat
    /// continuous factor `exp(-rT)`.
    ///
    /// This is the canonical composition behind the host-binding
    /// `PathDependentPricer` Asian pricing methods; both hosts delegate here
    /// rather than assembling process, discount factor, and payoff themselves.
    ///
    /// # Arguments
    ///
    /// * `params` - Call or put on the arithmetic average, spot at time `0`,
    ///   strike in spot units, continuously compounded decimal annual rate and
    ///   dividend yield, decimal annual GBM volatility, and time to expiry in
    ///   years (also the uniform-grid horizon); see [`OptionMarketParams`].
    /// * `num_steps` - Number of time-grid steps between `0` and expiry;
    ///   `None` uses the registry default
    ///   `convenience.path_dependent_pricer.num_steps`.
    /// * `currency` - Currency stamped on the returned estimate; `None` uses the
    ///   registry default `convenience.default_currency`.
    ///
    /// # Errors
    ///
    /// Returns an error when the embedded registry defaults cannot be loaded,
    /// GBM parameters fail validation, the flat discount factor is invalid,
    /// the uniform grid is invalid, or the underlying engine rejects the run.
    pub fn price_asian(
        &self,
        params: &OptionMarketParams,
        num_steps: Option<usize>,
        currency: Option<Currency>,
    ) -> Result<MoneyEstimate> {
        let &OptionMarketParams {
            option_type,
            spot,
            strike,
            rate,
            dividend_yield: div_yield,
            volatility: vol,
            time_to_expiry: expiry,
        } = params;
        let currency = crate::monte_carlo::convenience::resolve_currency(currency)?;
        let num_steps = num_steps.unwrap_or(
            crate::monte_carlo::registry::embedded_defaults()?
                .convenience
                .path_dependent_pricer
                .num_steps,
        );
        crate::monte_carlo::require_positive_vol(vol)?;
        let process = crate::monte_carlo::process::gbm::GbmParams::new(rate, div_yield, vol)
            .map(GbmProcess::new)?;
        let discount_factor = flat_discount_factor(rate, expiry)?;
        let payoff = Asian::new(
            option_type,
            strike,
            1.0,
            AveragingMethod::Arithmetic,
            default_fixing_steps(num_steps),
        )?;
        let time_grid = TimeGrid::uniform(expiry, num_steps)?;
        self.price(
            &process,
            spot,
            time_grid,
            &payoff,
            currency,
            discount_factor,
        )
    }

    /// Price an arithmetic Asian option under risk-neutral GBM together with
    /// its likelihood-ratio delta and vega.
    ///
    /// Uses unit notional, arithmetic averaging over the default post-step
    /// fixing schedule [`default_fixing_steps`], and the flat continuous
    /// discount factor `exp(-rT)`: the contract of
    /// [`Self::price_asian`], run through [`Self::price_with_lrm_greeks_from_payoff`].
    /// This is the canonical composition behind the host-binding
    /// `PathDependentPricer.price_with_lrm_greeks` methods.
    ///
    /// # Arguments
    ///
    /// * `params` - Call or put on the arithmetic average, finite and strictly
    ///   positive spot at time `0`, strike in spot units, continuously
    ///   compounded decimal annual rate and dividend yield, strictly positive
    ///   decimal annual GBM volatility, and time to expiry in years (also the
    ///   uniform-grid horizon); see [`OptionMarketParams`].
    /// * `num_steps` - Number of time-grid steps between `0` and expiry,
    ///   each of which is an averaging date. `None` uses the registry default
    ///   `convenience.greeks.lrm_num_steps`, which is smaller than the
    ///   [`Self::price_asian`] default because the likelihood-ratio estimator
    ///   keeps every path in memory (at most four million path points,
    ///   `num_paths x (num_steps + 1)`).
    /// * `currency` - Currency stamped on the returned price estimate; `None`
    ///   uses the registry default `convenience.default_currency`.
    ///
    /// # Errors
    ///
    /// Returns an error when the embedded registry defaults cannot be loaded,
    /// a validation error when the GBM parameters, discount factor or
    /// grid are invalid, and in every case listed on
    /// [`Self::price_with_lrm_greeks_from_payoff`]: Sobol or antithetic sampling, or more
    /// than four million captured path points.
    pub fn price_with_lrm_greeks(
        &self,
        params: &OptionMarketParams,
        num_steps: Option<usize>,
        currency: Option<Currency>,
    ) -> Result<LrmGreeks> {
        let &OptionMarketParams {
            option_type,
            spot,
            strike,
            rate,
            dividend_yield: div_yield,
            volatility: vol,
            time_to_expiry: expiry,
        } = params;
        let currency = crate::monte_carlo::convenience::resolve_currency(currency)?;
        let num_steps = num_steps.unwrap_or(
            crate::monte_carlo::registry::embedded_defaults()?
                .convenience
                .greeks
                .lrm_num_steps,
        );
        crate::monte_carlo::require_positive_vol(vol)?;
        // Before the fixing schedule, which holds one entry per step.
        self.require_lrm_capture_fits(num_steps)?;
        let process = crate::monte_carlo::process::gbm::GbmParams::new(rate, div_yield, vol)
            .map(GbmProcess::new)?;
        let discount_factor = flat_discount_factor(rate, expiry)?;
        let payoff = Asian::new(
            option_type,
            strike,
            1.0,
            AveragingMethod::Arithmetic,
            default_fixing_steps(num_steps),
        )?;
        let time_grid = TimeGrid::uniform(expiry, num_steps)?;
        self.price_with_lrm_greeks_from_payoff(
            &process,
            spot,
            time_grid,
            &payoff,
            currency,
            discount_factor,
        )
    }

    /// Price with captured paths on a grid containing the contractual events.
    ///
    /// # Arguments
    ///
    /// * `process` - GBM dynamics, including any cumulative carry schedule.
    /// * `initial_spot` - Positive initial asset price in payoff price units.
    /// * `time_grid` - Simulation times in the process clock, starting at zero.
    ///   Include every required observation time; equal-time observations are
    ///   represented by their multiplicity in the payoff.
    /// * `payoff` - Path payoff observing the supplied grid's event indices.
    /// * `currency` - Currency of the returned estimate and captured payoffs.
    /// * `discount_factor` - Present-value factor from payoff payment to valuation.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid simulation inputs, antithetic capture, or
    /// failures evaluating the process or payoff.
    pub fn price_with_paths_and_grid<P>(
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
        // Path capture is incompatible with antithetic pairing (the engine
        // rejects the combination); fail loudly instead of silently pricing
        // with a different estimator than the caller configured.
        if self.config.antithetic {
            return Err(Error::Validation(
                "price_with_paths_and_grid cannot honor antithetic = true: path capture and \
                 antithetic sampling are mutually exclusive. Disable antithetic or \
                 use price() without capture."
                    .to_string(),
            ));
        }
        self.run_grid(
            process,
            initial_spot,
            time_grid,
            payoff,
            currency,
            discount_factor,
        )
    }

    fn run_grid<P: Payoff>(
        &self,
        process: &GbmProcess,
        initial_spot: f64,
        time_grid: TimeGrid,
        payoff: &P,
        currency: Currency,
        discount_factor: f64,
    ) -> Result<MonteCarloResult> {
        self.config.validate()?;
        if self.config.use_sobol {
            return self.price_with_sobol(
                process,
                initial_spot,
                time_grid,
                payoff,
                currency,
                discount_factor,
            );
        }

        let engine = McEngine::new(self.config.engine_config(time_grid));

        let disc = ExactGbm::new();
        let initial_state = vec![initial_spot];
        let process_params = process.metadata();

        let rng = PhiloxRng::new(self.config.seed);
        let mut result = engine.price_with_capture(
            McPricingInputs {
                rng: &rng,
                process,
                disc: &disc,
                initial_state: &initial_state,
                payoff,
                currency,
                discount_factor,
            },
            process_params,
        )?;
        if let Some(run) = result.run.as_mut() {
            run.seed = Some(self.config.seed);
        }
        Ok(result)
    }

    /// Price and compute LRM Greeks (delta, vega) for GBM using captured paths.
    ///
    /// Uses the Likelihood Ratio Method with the score of the **joint path
    /// density** (Glasserman 2003, §7.3), so the estimators are unbiased for
    /// path-dependent payoffs (Asian averages, lookbacks, discretely
    /// monitored barriers), not just terminal payoffs:
    ///
    /// - **Delta** — only the first transition density depends on `S₀`, so
    ///   the score is `z₁ / (S₀ σ √Δt)` where `z₁` is the first step's
    ///   standardized shock.
    /// - **Vega** — the score is the per-step sum
    ///   `Σᵢ [(zᵢ² − 1)/σ − √Δt·zᵢ]`.
    ///
    /// Per-step shocks are reconstructed exactly from consecutive captured
    /// spots under the exact-GBM step:
    /// `zᵢ = (ln(Sᵢ/Sᵢ₋₁) − (r − q − σ²/2)Δt) / (σ √Δt)`.
    ///
    /// # Caveat
    ///
    /// The LR estimator differentiates the path density only. Payoffs must
    /// opt in through [`Payoff::supports_lrm_greeks`], confirming that they
    /// have no explicit dependence on initial spot or volatility. Time-zero
    /// observations and continuous barrier corrections are rejected; use the
    /// CRN finite-difference helpers in [`crate::monte_carlo::greeks::finite_diff`]
    /// for payoffs with explicit parameter dependence. The score reads the
    /// rate, dividend yield, and volatility of the process; time-varying
    /// drift schedules are unsupported by this constant-parameter score.
    ///
    /// # Memory
    ///
    /// Every path is captured in full to rebuild its shocks, so the run is
    /// limited to four million path points, `num_paths x (num_steps + 1)`.
    ///
    /// # Returns
    ///
    /// The price estimate with delta (per unit of spot) and vega (per one
    /// volatility point), each with its standard error and 95% confidence
    /// interval over the same paths.
    ///
    /// # Errors
    ///
    /// Returns a validation error for unsupported payoff sensitivity, Sobol
    /// sampling, antithetic sampling, a drift schedule, a non-uniform
    /// `time_grid`, a non-positive or non-finite initial spot, volatility or
    /// discount factor, more than four million captured path points, or a
    /// captured path without one spot per grid time. Propagates simulation
    /// errors.
    ///
    /// # Arguments
    ///
    /// * `process` - GBM dynamics with constant drift; its rate, dividend
    ///   yield and strictly positive volatility enter the score.
    /// * `initial_spot` - Finite, strictly positive initial spot in payoff price units.
    /// * `time_grid` - Uniform simulation grid from `0` to the horizon in
    ///   years, for example [`TimeGrid::uniform`]`(maturity, num_steps)`.
    /// * `payoff` - Path payoff that reports
    ///   [`Payoff::supports_lrm_greeks`]; evaluated at the grid's step indices.
    /// * `currency` - Currency of the returned price estimate.
    /// * `discount_factor` - Finite, strictly positive present-value factor
    ///   from payoff payment to valuation.
    pub fn price_with_lrm_greeks_from_payoff<P>(
        &self,
        process: &GbmProcess,
        initial_spot: f64,
        time_grid: TimeGrid,
        payoff: &P,
        currency: Currency,
        discount_factor: f64,
    ) -> Result<LrmGreeks>
    where
        P: Payoff,
    {
        self.config.validate()?;
        if !payoff.supports_lrm_greeks() {
            return Err(Error::Validation(
                "LRM Greeks require a payoff independent of initial spot and volatility \
                 when post-zero spots are fixed; use common-random-number finite differences \
                 for time-zero observations, continuous barrier corrections, or custom payoffs \
                 without a verified density-only sensitivity contract"
                    .to_string(),
            ));
        }
        if self.config.use_sobol {
            return Err(Error::Validation(
                "price_with_lrm_greeks does not support Sobol sampling".to_string(),
            ));
        }
        let params = process.params();
        let (rate, div_yield, vol) = (params.r, params.q, params.sigma);
        if process.drift_schedule().is_some() {
            return Err(Error::Validation(
                "LRM score parameters must come from a GBM process with constant drift".to_string(),
            ));
        }
        if !time_grid.is_uniform() {
            return Err(Error::Validation(
                "price_with_lrm_greeks requires a uniform time grid".to_string(),
            ));
        }
        let num_steps = time_grid.num_steps();
        let time_to_maturity = time_grid.t_max();
        // LRM Greeks require full path capture, which is incompatible with
        // antithetic pairing; fail loudly rather than silently changing the
        // estimator the caller configured.
        if self.config.antithetic {
            return Err(Error::Validation(
                "price_with_lrm_greeks cannot honor antithetic = true: it requires \
                 full path capture, which is mutually exclusive with antithetic \
                 sampling."
                    .to_string(),
            ));
        }
        // The scores divide by spot, volatility and the discount factor.
        for (name, value) in [
            ("initial_spot", initial_spot),
            ("vol", vol),
            ("discount_factor", discount_factor),
        ] {
            if !(value.is_finite() && value > 0.0) {
                return Err(Error::Validation(format!(
                    "price_with_lrm_greeks requires a finite, strictly positive {name}, \
                     got {value}"
                )));
            }
        }
        self.require_lrm_capture_fits(num_steps)?;
        // Force path capture to get every spot and the final discounted payoff value
        let mut engine_config = self.config.engine_config(time_grid);
        engine_config.path_capture = PathCaptureConfig::all().with_payoffs();
        engine_config.antithetic = false;
        let engine = McEngine::new(engine_config);

        let rng = crate::monte_carlo::rng::philox::PhiloxRng::new(self.config.seed);
        let disc = ExactGbm::new();
        let initial_state = vec![initial_spot];

        let process_params = process.metadata();

        let full = engine.price_with_capture(
            McPricingInputs {
                rng: &rng,
                process,
                disc: &disc,
                initial_state: &initial_state,
                payoff,
                currency,
                discount_factor,
            },
            process_params,
        )?;

        let dataset = match &full.paths {
            Some(dataset) if !dataset.paths.is_empty() => dataset,
            _ => {
                return Err(Error::Validation(
                    "price_with_lrm_greeks: the simulation captured no paths".to_string(),
                ));
            }
        };

        let spot_index = dataset
            .process_params
            .factor_names
            .iter()
            .position(|name| name == "spot")
            .ok_or_else(|| Error::Validation("captured GBM metadata is missing spot".to_owned()))?;
        let paths = &dataset.paths;

        // Build undiscounted payoffs and per-path joint-density scores by
        // reconstructing each step's standardized shock from consecutive
        // captured spots (exact under the ExactGbm step used above).
        let dt = time_to_maturity / num_steps as f64;
        let sqrt_dt = dt.sqrt();
        let mu_dt = (rate - div_yield - 0.5 * vol * vol) * dt;

        let mut payoffs: Vec<f64> = Vec::with_capacity(paths.len());
        let mut first_shocks: Vec<f64> = Vec::with_capacity(paths.len());
        let mut vega_scores: Vec<f64> = Vec::with_capacity(paths.len());
        for p in paths {
            let spots: Vec<f64> = p
                .points
                .iter()
                .filter_map(|point| point.state.get(spot_index).copied())
                .collect();
            // Grid points = steps + 1 (initial spot at step 0); an incomplete
            // capture would misalign the scores.
            if spots.len() != num_steps + 1 {
                return Err(Error::Validation(format!(
                    "price_with_lrm_greeks: captured path {} holds {} spots, expected {}",
                    p.path_id,
                    spots.len(),
                    num_steps + 1
                )));
            }

            let mut first_shock = 0.0;
            let mut score_sum = 0.0;
            for (k, w) in spots.windows(2).enumerate() {
                let z = ((w[1] / w[0]).ln() - mu_dt) / (vol * sqrt_dt);
                if k == 0 {
                    first_shock = z;
                }
                score_sum += (z * z - 1.0) / vol - sqrt_dt * z;
            }

            // Final discounted payoff value is stored; un-discount it.
            payoffs.push(p.final_value / discount_factor);
            first_shocks.push(first_shock);
            vega_scores.push(score_sum);
        }

        // Delta: only the first transition density depends on S₀, so the
        // terminal-score helper is reused with the FIRST step's shock and Δt.
        let delta = lrm_delta(
            &payoffs,
            &first_shocks,
            initial_spot,
            vol,
            dt,
            discount_factor,
        );
        let vega = lrm_vega_from_scores(&payoffs, &vega_scores, discount_factor);
        Ok(LrmGreeks {
            price: full.estimate,
            delta,
            vega,
        })
    }

    /// Reject a likelihood-ratio run whose captured paths would exceed
    /// [`MAX_LRM_CAPTURED_POINTS`], before any schedule or path is allocated.
    fn require_lrm_capture_fits(&self, num_steps: usize) -> Result<()> {
        if num_steps
            .checked_add(1)
            .and_then(|points| points.checked_mul(self.config.num_paths))
            .is_none_or(|points| points > MAX_LRM_CAPTURED_POINTS)
        {
            return Err(Error::Validation(format!(
                "price_with_lrm_greeks captures every path: num_paths x (num_steps + 1) must \
                 not exceed {MAX_LRM_CAPTURED_POINTS}, got {} paths and {num_steps} steps; \
                 use fewer paths or fewer steps",
                self.config.num_paths
            )));
        }
        Ok(())
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
) -> Result<()> {
    let num_steps = time_grid.num_steps();
    match bridge {
        Some(bridge) => {
            debug_assert_eq!(num_factors, 1, "Brownian bridge only supports 1 factor");
            bridge.construct_path(z_path, w_path, time_grid.times())?;
            for step in 0..num_steps {
                let dt = time_grid.dt(step);
                z_increments[step] = (w_path[step + 1] - w_path[step]) / dt.sqrt();
            }
        }
        None => {
            z_increments.copy_from_slice(&z_path[..num_steps * num_factors]);
        }
    }
    Ok(())
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
        // The pricer sizes `z_increments` to every normal one path draws
        // (`num_steps * num_factors`), so the slice below never overruns; if it
        // did, the slice index panics with the requested range and the length.
        let end = self.cursor + out.len();
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
    fn test_validate_requires_two_independent_estimators() {
        for antithetic in [false, true] {
            for num_paths in [0, 1] {
                let cfg = PathDependentPricerConfig::new(num_paths)
                    .with_parallel(false)
                    .with_sobol(false)
                    .with_antithetic(antithetic);
                let err = cfg.validate().expect_err("too few independent estimators");
                assert!(err.to_string().contains("num_paths"));
            }
            PathDependentPricerConfig::new(2)
                .with_parallel(false)
                .with_sobol(false)
                .with_antithetic(antithetic)
                .validate()
                .expect("two estimators, including two antithetic pairs");
        }
        for num_paths in [0, 1, 2] {
            let result = PathDependentPricerConfig::new(num_paths)
                .with_parallel(false)
                .with_antithetic(false)
                .with_sobol(true)
                .validate();
            assert_eq!(result.is_ok(), num_paths >= 2);
        }
    }

    use crate::monte_carlo::engine::PathCaptureConfig;
    use crate::monte_carlo::paths::PathSamplingMethod;
    use crate::monte_carlo::payoff::asian::{Asian, AveragingMethod};
    use crate::monte_carlo::payoff::vanilla::European;
    use crate::monte_carlo::process::gbm::{GbmParams, GbmProcess};
    use crate::types::OptionType;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::math::random::MAX_SOBOL_DIMENSION;
    use finstack_quant_core::math::time_grid::TimeGrid;

    use crate::monte_carlo::payoff::lookback::Lookback;

    #[test]
    fn sobol_reuses_engine_payoff_and_runtime_validation() {
        let gbm = crate::monte_carlo::process::gbm::GbmParams::new(0.0, 0.0, 0.2)
            .map(GbmProcess::new)
            .unwrap();
        for (paths, maturity_step, discount_factor, message) in [
            (32, 2, 1.0, "event at step 2"),
            (32, 1, -1.0, "discount_factor"),
            (
                crate::monte_carlo::engine::MAX_NUM_PATHS + 1,
                1,
                1.0,
                "maximum",
            ),
        ] {
            let pricer = PathDependentPricer::new(
                PathDependentPricerConfig::new(paths)
                    .with_parallel(false)
                    .with_antithetic(false)
                    .with_sobol(true),
            );
            let payoff = European::new(OptionType::Call, 100.0, 1.0, maturity_step);
            let error = pricer
                .price(
                    &gbm,
                    100.0,
                    finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 1)
                        .expect("valid time grid"),
                    &payoff,
                    Currency::USD,
                    discount_factor,
                )
                .expect_err("specialized Sobol pricing must preserve engine validation");
            assert!(error.to_string().contains(message), "{error}");
        }
    }

    #[test]
    fn lrm_rejects_initial_spot_observations_before_simulation() {
        let pricer =
            PathDependentPricer::new(PathDependentPricerConfig::new(2).with_parallel(false));
        let gbm = crate::monte_carlo::process::gbm::GbmParams::new(0.0, 0.0, 0.2)
            .map(GbmProcess::new)
            .unwrap();
        let asian = Asian::new(
            crate::types::OptionType::Call,
            90.0,
            1.0,
            AveragingMethod::Arithmetic,
            vec![0],
        )
        .unwrap();
        let asian_error = pricer
            .price_with_lrm_greeks_from_payoff(
                &gbm,
                100.0,
                finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 1)
                    .expect("valid time grid"),
                &asian,
                Currency::USD,
                1.0,
            )
            .expect_err("a time-zero fixing has an explicit spot derivative");
        let lookback = Lookback::new(crate::types::OptionType::Call, 90.0, 1.0, 1);
        let lookback_error = pricer
            .price_with_lrm_greeks_from_payoff(
                &gbm,
                100.0,
                finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 1)
                    .expect("valid time grid"),
                &lookback,
                Currency::USD,
                1.0,
            )
            .expect_err("the initial lookback extremum has an explicit spot derivative");
        for error in [asian_error, lookback_error] {
            assert!(error.to_string().contains("common-random-number"));
        }
    }

    #[test]
    fn test_path_dependent_pricer_asian() {
        let config = PathDependentPricerConfig::new(10_000)
            .with_seed(42)
            .with_parallel(false);
        let pricer = PathDependentPricer::new(config);

        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.02, 0.2).unwrap());

        // Monthly fixings
        let fixing_steps: Vec<usize> = (0..=12).map(|i| i * 21).collect();
        let asian = Asian::new(
            crate::types::OptionType::Call,
            100.0,
            1.0,
            AveragingMethod::Arithmetic,
            fixing_steps,
        )
        .expect("nonempty fixing schedule");

        let result = pricer
            .price(
                &gbm,
                100.0,
                finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 252)
                    .expect("valid time grid"),
                &asian,
                Currency::USD,
                1.0,
            )
            .expect("should succeed");

        assert!(result.mean.amount() > 0.0);
        assert!(result.mean.amount() < 20.0);
    }

    #[test]
    fn test_fully_observed_asian_call_and_put() {
        let pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(16)
                .with_seed(42)
                .with_parallel(false),
        );
        let gbm = crate::monte_carlo::process::gbm::GbmParams::new(0.05, 0.0, 0.2)
            .map(GbmProcess::new)
            .expect("valid GBM parameters");

        for averaging in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
            let call = Asian::with_history(
                crate::types::OptionType::Call,
                100.0,
                2.0,
                averaging,
                Vec::new(),
                crate::monte_carlo::payoff::asian::AsianFixingHistory {
                    sum: 220.0,
                    log_sum: 2.0 * 110.0_f64.ln(),
                    count: 2,
                },
            )
            .expect("fully observed call");
            let put = Asian::with_history(
                crate::types::OptionType::Put,
                120.0,
                2.0,
                averaging,
                Vec::new(),
                crate::monte_carlo::payoff::asian::AsianFixingHistory {
                    sum: 220.0,
                    log_sum: 2.0 * 110.0_f64.ln(),
                    count: 2,
                },
            )
            .expect("fully observed put");

            let call_value = pricer
                .price(
                    &gbm,
                    150.0,
                    finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 2)
                        .expect("valid time grid"),
                    &call,
                    Currency::USD,
                    0.95,
                )
                .expect("price from historical call fixings");
            let put_value = pricer
                .price(
                    &gbm,
                    150.0,
                    finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 2)
                        .expect("valid time grid"),
                    &put,
                    Currency::USD,
                    0.95,
                )
                .expect("price from historical put fixings");

            // Every path retains the historical average of 110 through reset;
            // simulated spot levels do not enter an already fixed contract.
            assert!((call_value.mean.amount() - 19.0).abs() < 1e-10);
            assert!((put_value.mean.amount() - 19.0).abs() < 1e-10);
        }
    }

    #[test]
    fn grid_entry_points_preserve_estimates_and_capture_contracts() {
        let gbm = crate::monte_carlo::process::gbm::GbmParams::new(0.05, 0.0, 0.2)
            .map(GbmProcess::new)
            .expect("valid GBM");
        let call = European::new(OptionType::Call, 100.0, 1.0, 4);
        for sobol in [false, true] {
            let config = PathDependentPricerConfig::new(32)
                .with_seed(17)
                .with_parallel(false)
                .with_antithetic(false)
                .with_sobol(sobol);
            let pricer = PathDependentPricer::new(config);
            let estimate = pricer
                .price(
                    &gbm,
                    100.0,
                    TimeGrid::uniform(1.0, 4).expect("grid"),
                    &call,
                    Currency::USD,
                    0.95,
                )
                .expect("estimate");
            let result = pricer
                .price_with_paths_and_grid(
                    &gbm,
                    100.0,
                    TimeGrid::uniform(1.0, 4).expect("grid"),
                    &call,
                    Currency::USD,
                    0.95,
                )
                .expect("result");
            assert_eq!(
                serde_json::to_value(estimate).expect("serialize"),
                serde_json::to_value(result.estimate).expect("serialize")
            );
            assert_eq!(result.run.expect("metadata").seed, Some(17));
        }
        let pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(32)
                .with_parallel(false)
                .with_antithetic(true),
        );
        let estimate = pricer
            .price(
                &gbm,
                100.0,
                TimeGrid::uniform(1.0, 4).expect("grid"),
                &call,
                Currency::USD,
                0.95,
            )
            .expect("antithetic estimate");
        assert_eq!(estimate.num_simulated_paths, 64);
        assert!(
            pricer
                .price_with_paths_and_grid(
                    &gbm,
                    100.0,
                    TimeGrid::uniform(1.0, 4).expect("grid"),
                    &call,
                    Currency::USD,
                    0.95
                )
                .is_err()
        );
    }

    #[test]
    fn test_sampled_path_capture_prices_and_records_subset() {
        let mut config = PathDependentPricerConfig::new(256)
            .with_seed(42)
            .with_parallel(false)
            .with_antithetic(false);
        config.path_capture = PathCaptureConfig::sample(32, 17);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.02, 0.2).expect("valid GBM parameters"));
        let call = European::new(OptionType::Call, 100.0, 1.0, 4);

        let result = pricer
            .price_with_paths_and_grid(
                &gbm,
                100.0,
                TimeGrid::uniform(1.0, 4).expect("grid"),
                &call,
                Currency::USD,
                0.95,
            )
            .expect("sampled path capture should succeed");

        assert!(result.estimate.mean.amount().is_finite());
        assert!(result.estimate.mean.amount() > 0.0);
        let captured = result.paths.as_ref().expect("sampled paths should exist");
        assert_eq!(captured.num_paths_total, 256);
        assert_eq!(
            captured.sampling_method,
            PathSamplingMethod::RandomSample {
                count: 32,
                seed: 17,
            }
        );
        assert!(!captured.is_complete());
        assert!(
            (16..=48).contains(&captured.paths.len()),
            "target sample size 32 should yield an approximate sample, got {}",
            captured.paths.len()
        );
        assert!(captured.paths.iter().all(|path| path.points.len() == 5));
    }

    /// LRM Greeks must use the joint path-density score, not the terminal
    /// marginal score (quant . Discriminating case: a
    /// single-fixing "Asian" at step n/2 is exactly a European call on
    /// `S_{T/2}` paid at T, with closed forms
    /// `delta = e^{-rT} e^{(r-q)τ} N(d₁)` and
    /// `vega = e^{-rT} e^{(r-q)τ} S₀ φ(d₁) √τ` (per unit vol), τ = T/2.
    /// The previous terminal-shock implementation converged to ≈ half the
    /// true delta for this payoff.
    #[test]
    fn test_lrm_greeks_unbiased_for_path_dependent_payoff() {
        use finstack_quant_core::math::special_functions::{norm_cdf, norm_pdf};

        let (s0, k, r, q, sigma, t) = (100.0, 100.0, 0.05, 0.01, 0.2, 1.0);
        let num_steps = 10usize;
        let fixing_step = num_steps / 2;
        let tau = t * fixing_step as f64 / num_steps as f64;

        let config = PathDependentPricerConfig::new(100_000)
            .with_seed(42)
            .with_parallel(false);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(r, q, sigma).unwrap());
        let payoff = Asian::new(
            crate::types::OptionType::Call,
            k,
            1.0,
            AveragingMethod::Arithmetic,
            vec![fixing_step],
        )
        .expect("nonempty fixing schedule");

        let df = (-r * t).exp();
        let greeks = pricer
            .price_with_lrm_greeks_from_payoff(
                &gbm,
                s0,
                finstack_quant_core::math::time_grid::TimeGrid::uniform(t, num_steps)
                    .expect("valid time grid"),
                &payoff,
                Currency::USD,
                df,
            )
            .expect("LRM pricing should succeed");
        let (delta, vega) = (greeks.delta.mean, greeks.vega.mean);
        assert_eq!(greeks.delta.num_paths, 100_000);
        assert_eq!(greeks.price.num_paths, 100_000);
        assert!(greeks.delta.stderr > 0.0 && greeks.vega.stderr > 0.0);

        let d1 = ((s0 / k).ln() + (r - q + 0.5 * sigma * sigma) * tau) / (sigma * tau.sqrt());
        let growth = ((r - q) * tau).exp();
        let delta_true = df * growth * norm_cdf(d1);
        let vega_true = df * growth * s0 * norm_pdf(d1) * tau.sqrt() * 0.01;

        // ≈5σ/8σ statistical tolerances at 100k paths; the old terminal-score
        // estimator was off by ≈50% (≈0.3 absolute in delta), far outside.
        assert!(
            (delta - delta_true).abs() < 0.025,
            "LRM delta {delta} should match closed form {delta_true}"
        );
        assert!(
            (vega - vega_true).abs() < 0.05,
            "LRM vega {vega} should match closed form {vega_true}"
        );
    }

    #[test]
    fn lrm_greeks_reject_degenerate_inputs_and_oversized_capture() {
        let pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(1_000)
                .with_seed(42)
                .with_parallel(false),
        );
        let price = |spot: f64, vol: f64, expiry: f64, num_steps: usize| {
            pricer.price_with_lrm_greeks(
                &crate::types::OptionMarketParams {
                    option_type: crate::types::OptionType::Call,
                    spot,
                    strike: 100.0,
                    rate: 0.03,
                    dividend_yield: 0.01,
                    volatility: vol,
                    time_to_expiry: expiry,
                },
                Some(num_steps),
                Some(Currency::USD),
            )
        };
        price(100.0, 0.2, 1.0, 12).expect("valid inputs price");
        for (label, result) in [
            ("zero spot", price(0.0, 0.2, 1.0, 12)),
            ("negative spot", price(-100.0, 0.2, 1.0, 12)),
            ("non-finite spot", price(f64::NAN, 0.2, 1.0, 12)),
            ("zero vol", price(100.0, 0.0, 1.0, 12)),
            ("zero expiry", price(100.0, 0.2, 0.0, 12)),
            ("zero steps", price(100.0, 0.2, 1.0, 0)),
        ] {
            assert!(result.is_err(), "{label} must be rejected");
        }

        // 1,000 paths x 4,000 points is exactly the cap; one more step exceeds it.
        let at_cap = super::MAX_LRM_CAPTURED_POINTS / 1_000 - 1;
        let error = price(100.0, 0.2, 1.0, at_cap + 1)
            .expect_err("capture above the cap must be rejected before simulating");
        assert!(
            error.to_string().contains("must not exceed 4000000"),
            "{error}"
        );
        // A step count that overflows `num_steps + 1` is rejected the same way.
        assert!(price(100.0, 0.2, 1.0, usize::MAX).is_err());
    }

    /// Likelihood-ratio Asian Greeks against central common-random-number
    /// bumps of the plain Asian pricer on the same seed. The bump estimates
    /// reuse every shock, so their own sampling error is far below the
    /// likelihood-ratio standard error that sets the tolerance.
    #[test]
    fn lrm_asian_greeks_match_common_random_number_bumps() {
        let (spot, strike, rate, div_yield, vol, expiry) = (100.0, 100.0, 0.04, 0.01, 0.25, 1.0);
        let num_steps = 12;
        let pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(100_000)
                .with_seed(7)
                .with_parallel(false),
        );
        for option_type in [
            crate::types::OptionType::Call,
            crate::types::OptionType::Put,
        ] {
            let plain = |spot: f64, vol: f64| -> f64 {
                pricer
                    .price_asian(
                        &crate::types::OptionMarketParams {
                            option_type,
                            spot,
                            strike,
                            rate,
                            dividend_yield: div_yield,
                            volatility: vol,
                            time_to_expiry: expiry,
                        },
                        Some(num_steps),
                        Some(Currency::USD),
                    )
                    .expect("plain Asian price")
                    .mean
                    .amount()
            };
            let greeks = pricer
                .price_with_lrm_greeks(
                    &crate::types::OptionMarketParams {
                        option_type,
                        spot,
                        strike,
                        rate,
                        dividend_yield: div_yield,
                        volatility: vol,
                        time_to_expiry: expiry,
                    },
                    Some(num_steps),
                    Some(Currency::USD),
                )
                .expect("LRM Asian price");

            // Capturing paths must not change the price estimate.
            assert_eq!(
                greeks.price.mean.amount().to_bits(),
                plain(spot, vol).to_bits(),
                "{option_type:?} price"
            );

            let spot_bump = 0.5;
            let bump_delta =
                (plain(spot + spot_bump, vol) - plain(spot - spot_bump, vol)) / (2.0 * spot_bump);
            // Per volatility point: a 0.01 bump each way, divided by two points.
            let bump_vega = (plain(spot, vol + 0.01) - plain(spot, vol - 0.01)) / 2.0;

            assert!(greeks.delta.stderr > 0.0 && greeks.vega.stderr > 0.0);
            assert!(
                (greeks.delta.mean - bump_delta).abs() < 5.0 * greeks.delta.stderr,
                "{option_type:?} delta: LRM {} +/- {}, bump {bump_delta}",
                greeks.delta.mean,
                greeks.delta.stderr
            );
            assert!(
                (greeks.vega.mean - bump_vega).abs() < 5.0 * greeks.vega.stderr,
                "{option_type:?} vega: LRM {} +/- {}, bump {bump_vega}",
                greeks.vega.mean,
                greeks.vega.stderr
            );
        }
    }

    /// Randomized-QMC error estimate: the stderr across independently
    /// scrambled replicates must be finite, positive, and cover the true
    /// error against the Black-Scholes closed form. (A single-scramble
    /// "stderr" over dependent Sobol points has no such guarantee.)
    #[test]
    fn test_sobol_rqmc_stderr_covers_true_error() {
        let (s0, k, r, q, sigma, t) = (100.0, 100.0, 0.05, 0.0, 0.2, 1.0);
        let num_steps = 16usize;
        let config = PathDependentPricerConfig::new(16_384)
            .with_seed(42)
            .with_parallel(false)
            .with_sobol(true);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(r, q, sigma).unwrap());
        let payoff = European::new(OptionType::Call, k, 1.0, num_steps);
        let df = (-r * t).exp();

        let result = pricer
            .price_with_paths_and_grid(
                &gbm,
                s0,
                TimeGrid::uniform(t, num_steps).expect("grid"),
                &payoff,
                Currency::USD,
                df,
            )
            .expect("sobol pricing should succeed");

        let bs =
            crate::closed_form::bs_price(s0, k, r, q, sigma, t, crate::types::OptionType::Call)
                .expect("valid Black-Scholes inputs");
        let mean = result.estimate.mean.amount();
        let stderr = result.estimate.stderr;

        assert!(stderr.is_finite() && stderr > 0.0, "stderr = {stderr}");
        assert!(
            (mean - bs).abs() < 6.0 * stderr,
            "RQMC mean {mean} should match BS {bs} within 6×stderr = {}",
            6.0 * stderr
        );
        // QMC should beat the plain-MC error scale at this path count.
        assert!(
            stderr < 0.05,
            "RQMC stderr should be tight at 16k paths, got {stderr}"
        );
    }

    #[test]
    fn test_path_dependent_pricer_lookback() {
        let config = PathDependentPricerConfig::new(10_000)
            .with_seed(42)
            .with_parallel(false);
        let pricer = PathDependentPricer::new(config);

        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.3).unwrap());
        let lookback = Lookback::new(crate::types::OptionType::Call, 100.0, 1.0, 252);

        let result = pricer
            .price(
                &gbm,
                100.0,
                finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 252)
                    .expect("valid time grid"),
                &lookback,
                Currency::USD,
                1.0,
            )
            .expect("should succeed");

        assert!(result.mean.amount() > 0.0);
    }

    #[test]
    fn test_sobol_price_custom_grid_multiple_paths() {
        let config = PathDependentPricerConfig::new(8)
            .with_seed(7)
            .with_parallel(false)
            .with_sobol(true)
            .with_brownian_bridge(false);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let time_grid = TimeGrid::uniform(1.0, 4).expect("grid should build");
        let fixing_steps = vec![1, 2, 3, 4];
        let asian = Asian::new(
            crate::types::OptionType::Call,
            100.0,
            1.0,
            AveragingMethod::Arithmetic,
            fixing_steps,
        )
        .expect("nonempty fixing schedule");

        let result = pricer
            .price(&gbm, 100.0, time_grid, &asian, Currency::USD, 1.0)
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

        let mut config = PathDependentPricerConfig::new(8)
            .with_seed(11)
            .with_parallel(false)
            .with_sobol(true);
        config.path_capture = PathCaptureConfig::all();
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let fixing_steps = vec![1, 2, 3, 4];
        let asian = Asian::new(
            crate::types::OptionType::Call,
            100.0,
            1.0,
            AveragingMethod::Arithmetic,
            fixing_steps,
        )
        .expect("nonempty fixing schedule");

        let first = pricer
            .price_with_paths_and_grid(
                &gbm,
                100.0,
                TimeGrid::uniform(1.0, 4).expect("grid"),
                &asian,
                Currency::USD,
                1.0,
            )
            .expect("Sobol path capture should succeed for multiple paths");
        let second = pricer
            .price_with_paths_and_grid(
                &gbm,
                100.0,
                TimeGrid::uniform(1.0, 4).expect("grid"),
                &asian,
                Currency::USD,
                1.0,
            )
            .expect("repeated Sobol path capture should succeed");

        assert_eq!(first.estimate.num_paths, 8);
        assert_eq!(first.paths.as_ref().map(|p| p.paths.len()).unwrap_or(0), 8);

        let captured = first.paths.as_ref().expect("paths should be captured");
        let repeated = second
            .paths
            .as_ref()
            .expect("repeated paths should be captured");
        assert_eq!(captured.paths.len(), repeated.paths.len());
        for (path_a, path_b) in captured.paths.iter().zip(&repeated.paths) {
            assert_eq!(path_a.path_id, path_b.path_id);
            assert_eq!(path_a.final_value.to_bits(), path_b.final_value.to_bits());
            assert_eq!(path_a.points.len(), path_b.points.len());
            for (point_a, point_b) in path_a.points.iter().zip(&path_b.points) {
                assert_eq!(point_a.step, point_b.step);
                assert_eq!(point_a.time.to_bits(), point_b.time.to_bits());
                assert_eq!(point_a.state, point_b.state);
                assert_eq!(point_a.payoff_value, point_b.payoff_value);
                assert_eq!(point_a.cashflows, point_b.cashflows);
            }
        }

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

        assert_eq!(first.estimate.median, Some(expected_median));
        assert_eq!(first.estimate.percentile_25, Some(expected_p25));
        assert_eq!(first.estimate.percentile_75, Some(expected_p75));
        assert_eq!(first.estimate.min, Some(final_values[0]));
        assert_eq!(first.estimate.max, Some(final_values[len - 1]));
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
        let asian = Asian::new(
            crate::types::OptionType::Call,
            100.0,
            1.0,
            AveragingMethod::Arithmetic,
            fixing_steps,
        )
        .expect("nonempty fixing schedule");

        let result = pricer
            .price(&gbm, 100.0, time_grid, &asian, Currency::USD, 1.0)
            .expect("Irregular-grid Sobol Brownian bridge pricing should succeed");

        assert_eq!(result.num_paths, 8);
    }

    #[test]
    fn test_sobol_rejects_excessive_dimension() {
        let config = PathDependentPricerConfig::new(2)
            .with_seed(17)
            .with_parallel(false)
            .with_sobol(true)
            .with_brownian_bridge(false);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let fixing_steps = vec![MAX_SOBOL_DIMENSION + 1];
        let asian = Asian::new(
            crate::types::OptionType::Call,
            100.0,
            1.0,
            AveragingMethod::Arithmetic,
            fixing_steps,
        )
        .expect("nonempty fixing schedule");

        let err = pricer
            .price(
                &gbm,
                100.0,
                finstack_quant_core::math::time_grid::TimeGrid::uniform(
                    1.0,
                    MAX_SOBOL_DIMENSION + 1,
                )
                .expect("valid time grid"),
                &asian,
                Currency::USD,
                1.0,
            )
            .expect_err("excessive Sobol dimension should be rejected");

        assert!(err.to_string().contains("Sobol"));
    }

    #[test]
    fn price_gbm_asian_call_atm_is_positive() {
        let pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(2_000)
                .with_seed(42)
                .with_parallel(false),
        );
        let estimate = pricer
            .price_asian(
                &crate::types::OptionMarketParams {
                    option_type: crate::types::OptionType::Call,
                    spot: 100.0,
                    strike: 100.0,
                    rate: 0.05,
                    dividend_yield: 0.0,
                    volatility: 0.2,
                    time_to_expiry: 1.0,
                },
                Some(12),
                Some(Currency::USD),
            )
            .expect("Asian call pricing should succeed");
        assert!(estimate.mean.amount() > 0.0);
    }

    #[test]
    fn price_gbm_asian_put_atm_is_positive() {
        let pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(2_000)
                .with_seed(42)
                .with_parallel(false),
        );
        let estimate = pricer
            .price_asian(
                &crate::types::OptionMarketParams {
                    option_type: crate::types::OptionType::Put,
                    spot: 100.0,
                    strike: 100.0,
                    rate: 0.05,
                    dividend_yield: 0.0,
                    volatility: 0.2,
                    time_to_expiry: 1.0,
                },
                Some(12),
                Some(Currency::USD),
            )
            .expect("Asian put pricing should succeed");
        assert!(estimate.mean.amount() > 0.0);
    }

    /// Seeded GBM Asian convenience prices (mean and standard error) are
    /// pinned for calls and puts. Platform libm yields one pattern on macOS
    /// and another on the Linux libm used by GitHub Actions.
    #[test]
    fn gbm_asian_call_and_put_are_bit_stable() {
        const EXPECTED: &[u64] = &[
            0x40195cdbec2b01bf,
            0x3fdc249afb33387a,
            0x40176ee1abd3fb0f,
            0x3fd593ce944bf326,
            0x4027c98e1c4e20d5,
            0x3fe31c6bc20af5c4,
            0x40027bb6f761ef02,
            0x3fcb758de16af18e,
            0x3fff484575b19363,
            0x3fd05d4dc80f367e,
            0x402ec1b62a42dd34,
            0x3fe04e9f5a5800d4,
            0x401bd9967d816613,
            0x3fce5f17b4811b71,
            0x40155dfccc84298d,
            0x3fc45f08a6f61ea8,
            0x402a9e8a223bfe8b,
            0x3fcbdb8abd307aef,
            0x400061190e6e3a56,
            0x3fc0720e85396db8,
            0x3fff670343656928,
            0x3fc56479aab3b6e9,
            0x402d4a5a81aee856,
            0x3fb5f30fcc19ac92,
            0x4019f174aceea57e,
            0x3fe50d0985e19da9,
            0x3fe521515a7bdef2,
            0x3fc142f4470e821b,
            0x3fcebd9d3d2cc02e,
            0x3fc5c2462919b2c4,
            0x40143d71cec94890,
            0x3fdb41a13d264d1e,
            0xbfd3e8b6c02d1c01,
            0x3facdadbb65e9912,
            0x3f9cbc02b2a93eed,
            0x3fb4c48e3872a8e8,
        ];
        let mut actual = Vec::new();
        for antithetic in [false, true] {
            let pricer = PathDependentPricer::new(
                PathDependentPricerConfig::new(512)
                    .with_seed(7)
                    .with_parallel(false)
                    .with_antithetic(antithetic),
            );
            for (strike, num_steps) in [(100.0, 12), (90.0, 5), (115.0, 30)] {
                let call = pricer
                    .price_asian(
                        &crate::types::OptionMarketParams {
                            option_type: crate::types::OptionType::Call,
                            spot: 100.0,
                            strike,
                            rate: 0.05,
                            dividend_yield: 0.02,
                            volatility: 0.25,
                            time_to_expiry: 1.0,
                        },
                        Some(num_steps),
                        Some(Currency::USD),
                    )
                    .expect("call");
                let put = pricer
                    .price_asian(
                        &crate::types::OptionMarketParams {
                            option_type: crate::types::OptionType::Put,
                            spot: 100.0,
                            strike,
                            rate: 0.05,
                            dividend_yield: 0.02,
                            volatility: 0.25,
                            time_to_expiry: 1.0,
                        },
                        Some(num_steps),
                        Some(Currency::USD),
                    )
                    .expect("put");
                for estimate in [call, put] {
                    actual.push(estimate.mean.amount().to_bits());
                    actual.push(estimate.stderr.to_bits());
                }
            }
        }
        let lrm_pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(256)
                .with_seed(11)
                .with_parallel(false)
                .with_antithetic(false),
        );
        for option_type in [
            crate::types::OptionType::Call,
            crate::types::OptionType::Put,
        ] {
            let greeks = lrm_pricer
                .price_with_lrm_greeks(
                    &crate::types::OptionMarketParams {
                        option_type,
                        spot: 100.0,
                        strike: 100.0,
                        rate: 0.05,
                        dividend_yield: 0.02,
                        volatility: 0.25,
                        time_to_expiry: 1.0,
                    },
                    Some(8),
                    Some(Currency::USD),
                )
                .expect("lrm greeks");
            actual.push(greeks.price.mean.amount().to_bits());
            actual.push(greeks.price.stderr.to_bits());
            actual.push(greeks.delta.mean.to_bits());
            actual.push(greeks.delta.stderr.to_bits());
            actual.push(greeks.vega.mean.to_bits());
            actual.push(greeks.vega.stderr.to_bits());
        }
        const LINUX: &[u64] = &[
            0x40195cdbec2b01bf,
            0x3fdc249afb33387a,
            0x40176ee1abd3fb0f,
            0x3fd593ce944bf326,
            0x4027c98e1c4e20d5,
            0x3fe31c6bc20af5c4,
            0x40027bb6f761ef02,
            0x3fcb758de16af18e,
            0x3fff484575b19363,
            0x3fd05d4dc80f367f,
            0x402ec1b62a42dd34,
            0x3fe04e9f5a5800d4,
            0x401bd9967d816613,
            0x3fce5f17b4811b73,
            0x40155dfccc84298d,
            0x3fc45f08a6f61ea8,
            0x402a9e8a223bfe8b,
            0x3fcbdb8abd307aed,
            0x400061190e6e3a56,
            0x3fc0720e85396db7,
            0x3fff670343656928,
            0x3fc56479aab3b6e9,
            0x402d4a5a81aee856,
            0x3fb5f30fcc19ac92,
            0x4019f174aceea57e,
            0x3fe50d0985e19da9,
            0x3fe521515a7bdef2,
            0x3fc142f4470e821b,
            0x3fcebd9d3d2cc02e,
            0x3fc5c2462919b2c4,
            0x40143d71cec94890,
            0x3fdb41a13d264d1e,
            0xbfd3e8b6c02d1c01,
            0x3facdadbb65e9911,
            0x3f9cbc02b2a93ef6,
            0x3fb4c48e3872a8e9,
        ];
        assert!(
            actual.as_slice() == EXPECTED || actual.as_slice() == LINUX,
            "GBM Asian prices moved off the macOS and Linux libm pins:\n{actual:#x?}"
        );
    }
}

#[cfg(test)]
mod asian_pair_tests {
    use super::{PathDependentPricer, PathDependentPricerConfig};
    use crate::monte_carlo::control_variate::apply_control_variate;
    use crate::monte_carlo::engine::{MAX_NUM_PATHS, PathCaptureConfig};
    use crate::monte_carlo::payoff::asian::{Asian, AsianFixingHistory, AveragingMethod};
    use crate::monte_carlo::process::gbm::{DriftSchedule, GbmParams, GbmProcess};
    use crate::types::OptionType;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::math::random::MAX_SOBOL_DIMENSION;
    use finstack_quant_core::math::stats::OnlineCovariance;
    use finstack_quant_core::math::time_grid::TimeGrid;
    use std::sync::Arc;

    const DISCOUNT_FACTOR: f64 = 0.94;

    fn process() -> GbmProcess {
        let drift = DriftSchedule::new(vec![0.0, 0.2, 0.6, 1.0], vec![0.0, 0.006, 0.024, 0.037])
            .expect("nonflat carry schedule");
        GbmProcess::new(GbmParams::new(0.04, 0.01, 0.3).expect("GBM parameters"))
            .with_drift_schedule(Arc::new(drift))
    }

    fn grid() -> TimeGrid {
        TimeGrid::from_times(vec![0.0, 0.07, 0.18, 0.43, 0.69, 1.0])
            .expect("irregular observation grid")
    }

    fn payoff(option_type: OptionType, quantity: f64, averaging: AveragingMethod) -> Asian {
        Asian::with_history(
            option_type,
            100.0,
            quantity,
            averaging,
            vec![3, 0, 1, 3, 5],
            AsianFixingHistory {
                sum: 193.0,
                log_sum: 90.0_f64.ln() + 103.0_f64.ln(),
                count: 2,
            },
        )
        .expect("seasoned Asian contract")
        .with_fixing_multipliers(&[1.0, 0.95, 1.03, 1.08, 0.97])
        .expect("distinct multipliers at repeated fixing steps")
    }

    fn config(num_paths: usize, sobol: bool) -> PathDependentPricerConfig {
        PathDependentPricerConfig {
            chunk_size: 17,
            ..PathDependentPricerConfig::new(num_paths)
                .with_seed(0x1234_5678)
                .with_parallel(false)
                .with_antithetic(false)
                .with_sobol(sobol)
        }
    }

    fn values(moments: &OnlineCovariance) -> [f64; 5] {
        [
            moments.mean_x(),
            moments.mean_y(),
            moments.variance_x(),
            moments.variance_y(),
            moments.covariance(),
        ]
    }

    // Retain the old two-pass, global point-order fold as an independent
    // reference. No candidate pair implementation is used here.
    fn captured_reference(
        config: &PathDependentPricerConfig,
        option_type: OptionType,
        quantity: f64,
    ) -> OnlineCovariance {
        let mut capture_config = config.clone();
        capture_config.path_capture = PathCaptureConfig::all();
        let pricer = PathDependentPricer::new(capture_config);
        let gbm = process();
        let capture = |averaging| {
            pricer
                .price_with_paths_and_grid(
                    &gbm,
                    100.0,
                    grid(),
                    &payoff(option_type, quantity, averaging),
                    Currency::USD,
                    DISCOUNT_FACTOR,
                )
                .expect("independent scalar capture")
        };
        let arithmetic = capture(AveragingMethod::Arithmetic);
        let geometric = capture(AveragingMethod::Geometric);
        for result in [&arithmetic, &geometric] {
            assert_eq!(result.estimate.num_paths, config.num_paths);
            assert_eq!(result.estimate.num_simulated_paths, config.num_paths);
            let paths = result.paths.as_ref().expect("complete path capture");
            assert_eq!(paths.num_paths_total, config.num_paths);
            assert_eq!(paths.paths.len(), config.num_paths);
        }
        let mut reference = OnlineCovariance::new();
        for (path_id, (arithmetic, geometric)) in arithmetic
            .paths
            .as_ref()
            .expect("arithmetic capture")
            .paths
            .iter()
            .zip(&geometric.paths.as_ref().expect("geometric capture").paths)
            .enumerate()
        {
            assert_eq!(arithmetic.path_id, path_id);
            assert_eq!(geometric.path_id, path_id);
            assert!(arithmetic.final_value.is_finite() && geometric.final_value.is_finite());
            reference.update(arithmetic.final_value, geometric.final_value);
        }
        reference
    }

    fn assert_moments_close(
        actual: &OnlineCovariance,
        reference: &OnlineCovariance,
        quantity: f64,
    ) {
        assert_eq!(actual.count(), reference.count());
        for (index, (actual, reference)) in values(actual)
            .into_iter()
            .zip(values(reference))
            .enumerate()
        {
            let scale = if index < 2 {
                quantity
            } else {
                quantity * quantity
            };
            assert!(actual.is_finite() && reference.is_finite());
            assert!(
                ((actual - reference) / scale).abs() <= 1e-10,
                "moment {index}: actual={actual:.17e}, reference={reference:.17e}, scale={scale}"
            );
        }
    }

    #[test]
    fn asian_pair_philox_matches_captured_moments_and_parallel_bits() {
        let config = config(65, false);
        let gbm = process();
        for option_type in [OptionType::Call, OptionType::Put] {
            for quantity in [1.0, 250.0] {
                let reference = captured_reference(&config, option_type, quantity);
                let arithmetic = payoff(option_type, quantity, AveragingMethod::Arithmetic);
                let price = |parallel| {
                    PathDependentPricer::new(config.clone().with_parallel(parallel))
                        .price_asian_pair(
                            &gbm,
                            100.0,
                            grid(),
                            &arithmetic,
                            Currency::USD,
                            DISCOUNT_FACTOR,
                        )
                        .expect("paired Philox moments")
                };
                let serial = price(false);
                let parallel = price(true);
                assert_eq!(serial.count(), config.num_paths);
                assert_moments_close(&serial, &reference, quantity);
                assert_moments_close(&parallel, &reference, quantity);
                assert_eq!(
                    values(&serial).map(f64::to_bits),
                    values(&parallel).map(f64::to_bits)
                );
            }
        }
    }

    #[test]
    fn asian_pair_sobol_preserves_global_point_fold_on_irregular_grid() {
        let gbm = process();
        // 35 points allocate three points to three scrambles and two to the
        // other thirteen. A replicate-level fold or beta would differ.
        for bridge in [false, true] {
            let config = config(35, true).with_brownian_bridge(bridge);
            for option_type in [OptionType::Call, OptionType::Put] {
                for quantity in [1.0, 250.0] {
                    let reference = captured_reference(&config, option_type, quantity);
                    let actual = PathDependentPricer::new(config.clone())
                        .price_asian_pair(
                            &gbm,
                            100.0,
                            grid(),
                            &payoff(option_type, quantity, AveragingMethod::Arithmetic),
                            Currency::USD,
                            DISCOUNT_FACTOR,
                        )
                        .expect("paired Sobol moments");
                    assert_eq!(actual.count(), 35);
                    assert_eq!(
                        values(&actual).map(f64::to_bits),
                        values(&reference).map(f64::to_bits),
                        "bridge={bridge}, direction={option_type:?}, quantity={quantity}"
                    );
                }
            }
        }
    }

    #[test]
    fn asian_pair_zero_volatility_and_fully_observed_contracts_preserve_constant_payoffs() {
        let zero_volatility =
            GbmProcess::new(GbmParams::new(0.0, 0.0, 0.0).expect("zero-volatility GBM"));
        let future = Asian::new(
            OptionType::Call,
            90.0,
            1.0,
            AveragingMethod::Arithmetic,
            vec![0, 2, 2, 5],
        )
        .expect("future deterministic fixings");
        let observed = Asian::with_history(
            OptionType::Call,
            90.0,
            1.0,
            AveragingMethod::Arithmetic,
            Vec::new(),
            AsianFixingHistory {
                sum: 200.0,
                log_sum: 2.0 * 100.0_f64.ln(),
                count: 2,
            },
        )
        .expect("fully observed contract");
        for (gbm, arithmetic) in [(zero_volatility, future), (process(), observed)] {
            for sobol in [false, true] {
                for chunk_size in [7, 35] {
                    let config = PathDependentPricerConfig {
                        chunk_size,
                        ..config(35, sobol)
                    };
                    let actual = PathDependentPricer::new(config)
                        .price_asian_pair(
                            &gbm,
                            100.0,
                            grid(),
                            &arithmetic,
                            Currency::USD,
                            DISCOUNT_FACTOR,
                        )
                        .expect("deterministic paired payoffs");
                    assert_eq!(actual.count(), 35);
                    assert!((actual.mean_x() - 10.0 * DISCOUNT_FACTOR).abs() < 1e-12);
                    assert!((actual.mean_y() - 10.0 * DISCOUNT_FACTOR).abs() < 1e-12);
                    // One Philox chunk and the global Sobol fold give exact
                    // zero. Weighted multi-chunk means can round, but the
                    // variance must stay below the existing CV cutoff.
                    if sobol || chunk_size == 35 {
                        assert_eq!(actual.variance_x(), 0.0);
                        assert_eq!(actual.variance_y(), 0.0);
                        assert_eq!(actual.covariance(), 0.0);
                    } else {
                        assert!(actual.variance_x().abs() < 1e-10);
                        assert!(actual.variance_y().abs() < 1e-10);
                        assert!(actual.covariance().abs() < 1e-10);
                    }
                }
            }
        }
    }

    #[test]
    fn asian_pair_preserves_control_adjustment_on_both_sides_of_variance_cutoff() {
        let config = config(65, false);
        let gbm = process();
        let unit_reference = captured_reference(&config, OptionType::Call, 1.0);
        assert!(unit_reference.variance_y().is_finite() && unit_reference.variance_y() > 0.0);
        for target_variance in [0.5e-10, 2.0e-10] {
            let quantity = (target_variance / unit_reference.variance_y()).sqrt();
            let reference = captured_reference(&config, OptionType::Call, quantity);
            let actual = PathDependentPricer::new(config.clone())
                .price_asian_pair(
                    &gbm,
                    100.0,
                    grid(),
                    &payoff(OptionType::Call, quantity, AveragingMethod::Arithmetic),
                    Currency::USD,
                    DISCOUNT_FACTOR,
                )
                .expect("paired moments near variance cutoff");
            assert_moments_close(&actual, &reference, quantity);
            let applies_control = target_variance > 1e-10;
            assert_eq!(reference.variance_y() > 1e-10, applies_control);
            assert_eq!(actual.variance_y() > 1e-10, applies_control);

            // A synthetic control anchor makes the existing coefficient
            // cutoff observable. This tests its propagation, not an Asian
            // analytical pricing formula.
            let control_mean = 0.9 * reference.mean_y();
            let adjust = |moments: &OnlineCovariance| {
                apply_control_variate(
                    moments.mean_x(),
                    moments.variance_x(),
                    moments.mean_y(),
                    moments.variance_y(),
                    moments.covariance(),
                    control_mean,
                    moments.count(),
                )
            };
            let actual_adjusted = adjust(&actual);
            let reference_adjusted = adjust(&reference);
            assert!(((actual_adjusted.mean - reference_adjusted.mean) / quantity).abs() <= 1e-10);
            let actual_variance = actual.count() as f64 * actual_adjusted.stderr.powi(2);
            let reference_variance = reference.count() as f64 * reference_adjusted.stderr.powi(2);
            assert!(((actual_variance - reference_variance) / quantity.powi(2)).abs() <= 1e-10);
            if applies_control {
                assert!(actual_adjusted.mean < actual.mean_x());
            } else {
                assert_eq!(actual_adjusted.mean.to_bits(), actual.mean_x().to_bits());
            }
        }
    }

    #[test]
    fn asian_pair_rejects_incompatible_requests_and_invalid_configuration() {
        let gbm = process();
        let arithmetic = payoff(OptionType::Call, 1.0, AveragingMethod::Arithmetic);
        let geometric = payoff(OptionType::Call, 1.0, AveragingMethod::Geometric);
        for sobol in [false, true] {
            let base = config(35, sobol);
            let error = PathDependentPricer::new(base.clone())
                .price_asian_pair(
                    &gbm,
                    100.0,
                    grid(),
                    &geometric,
                    Currency::USD,
                    DISCOUNT_FACTOR,
                )
                .expect_err("geometric target is not an arithmetic/control pair");
            assert!(error.to_string().contains("arithmetic"), "{error}");

            let mut full_capture = base.clone();
            full_capture.path_capture = PathCaptureConfig::all();
            let mut sample_capture = base.clone();
            sample_capture.path_capture = PathCaptureConfig::sample(4, 17);
            let mut zero_chunk = base.clone();
            zero_chunk.chunk_size = 0;
            let mut zero_step_density = base.clone();
            zero_step_density.steps_per_year = 0.0;
            let mut nonfinite_step_density = base.clone();
            nonfinite_step_density.steps_per_year = f64::NAN;
            for (config, expected_message) in [
                (config(0, sobol), "num_paths"),
                (config(1, sobol), "num_paths"),
                (config(MAX_NUM_PATHS + 1, sobol), "maximum"),
                (zero_chunk, "chunk_size"),
                (zero_step_density, "steps_per_year"),
                (nonfinite_step_density, "steps_per_year"),
                (base.with_antithetic(true), "antithetic"),
                (full_capture, "captured paths"),
                (sample_capture, "captured paths"),
            ] {
                let error = PathDependentPricer::new(config)
                    .price_asian_pair(
                        &gbm,
                        100.0,
                        grid(),
                        &arithmetic,
                        Currency::USD,
                        DISCOUNT_FACTOR,
                    )
                    .expect_err("invalid paired request must fail");
                assert!(error.to_string().contains(expected_message), "{error}");
            }
        }
    }

    #[test]
    fn asian_pair_preserves_runtime_and_payoff_errors() {
        let gbm = process();
        let arithmetic = payoff(OptionType::Call, 1.0, AveragingMethod::Arithmetic);
        for sobol in [false, true] {
            let pricer = PathDependentPricer::new(config(35, sobol));
            let schedule_error = pricer
                .price_asian_pair(
                    &gbm,
                    100.0,
                    TimeGrid::uniform(1.0, 4).expect("short grid"),
                    &arithmetic,
                    Currency::USD,
                    DISCOUNT_FACTOR,
                )
                .expect_err("fixing step 5 must not silently disappear");
            assert!(schedule_error.to_string().contains("event at step 5"));
            for discount_factor in [-1.0, f64::NAN, f64::INFINITY] {
                let error = pricer
                    .price_asian_pair(
                        &gbm,
                        100.0,
                        grid(),
                        &arithmetic,
                        Currency::USD,
                        discount_factor,
                    )
                    .expect_err("invalid discount factor");
                assert!(error.to_string().contains("discount_factor"), "{error}");
            }
            for spot in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert!(
                    pricer
                        .price_asian_pair(
                            &gbm,
                            spot,
                            grid(),
                            &arithmetic,
                            Currency::USD,
                            DISCOUNT_FACTOR,
                        )
                        .is_err(),
                    "a fixing must reject nonfinite spot"
                );
            }
            let invalid_control = Asian::with_history(
                OptionType::Call,
                100.0,
                1.0,
                AveragingMethod::Arithmetic,
                Vec::new(),
                AsianFixingHistory {
                    sum: 100.0,
                    log_sum: f64::INFINITY,
                    count: 1,
                },
            )
            .expect("history is validated when evaluated");
            assert!(
                pricer
                    .price_asian_pair(
                        &gbm,
                        100.0,
                        grid(),
                        &invalid_control,
                        Currency::USD,
                        DISCOUNT_FACTOR,
                    )
                    .is_err(),
                "a finite arithmetic payoff must not hide an invalid geometric control"
            );
        }
    }

    #[test]
    fn asian_pair_rejects_unsupported_sobol_execution_and_dimensions() {
        let gbm = process();
        let arithmetic = payoff(OptionType::Call, 1.0, AveragingMethod::Arithmetic);
        let parallel_error = PathDependentPricer::new(config(35, true).with_parallel(true))
            .price_asian_pair(
                &gbm,
                100.0,
                grid(),
                &arithmetic,
                Currency::USD,
                DISCOUNT_FACTOR,
            )
            .expect_err("Sobol stream cannot be split across workers");
        assert!(parallel_error.to_string().contains("serial execution"));
        for bridge in [false, true] {
            let error = PathDependentPricer::new(config(35, true).with_brownian_bridge(bridge))
                .price_asian_pair(
                    &gbm,
                    100.0,
                    TimeGrid::uniform(1.0, MAX_SOBOL_DIMENSION + 1)
                        .expect("grid beyond Sobol dimension cap"),
                    &arithmetic,
                    Currency::USD,
                    DISCOUNT_FACTOR,
                )
                .expect_err("unsupported Sobol dimension");
            assert!(error.to_string().contains("Sobol dimension"), "{error}");
        }
    }

    #[test]
    fn geometric_asian_antithetic_pricing_remains_available() {
        let gbm = process();
        for option_type in [OptionType::Call, OptionType::Put] {
            let geometric = payoff(option_type, 1.0, AveragingMethod::Geometric);
            for parallel in [false, true] {
                let estimate = PathDependentPricer::new(
                    config(31, false)
                        .with_parallel(parallel)
                        .with_antithetic(true),
                )
                .price(
                    &gbm,
                    100.0,
                    grid(),
                    &geometric,
                    Currency::USD,
                    DISCOUNT_FACTOR,
                )
                .expect("ordinary geometric antithetic price");
                assert_eq!(estimate.num_paths, 31);
                assert_eq!(estimate.num_simulated_paths, 62);
                assert_eq!(estimate.mean.currency(), Currency::USD);
                assert!(estimate.mean.amount().is_finite() && estimate.mean.amount() > 0.0);
                assert!(estimate.stderr.is_finite());
            }
        }
    }
}
