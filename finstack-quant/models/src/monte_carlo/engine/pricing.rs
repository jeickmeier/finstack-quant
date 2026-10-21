//! Execution, configuration, and diagnostics for Monte Carlo pricing.
//!
use super::config::{McEngineConfig, MAX_CAPTURED_PATHS, MAX_NUM_PATHS};
use super::path_capture::PathCaptureMode;
use crate::monte_carlo::captured_path_stats::apply_captured_path_statistics;
use crate::monte_carlo::estimate::Estimate;
use crate::monte_carlo::paths::{PathDataset, PathSamplingMethod, ProcessParams, SimulatedPath};
use crate::monte_carlo::results::{MoneyEstimate, MonteCarloResult};
use crate::monte_carlo::traits::{Discretization, Payoff, RandomStream, StochasticProcess};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::linalg::{cholesky_correlation, CorrelationFactor};
use finstack_quant_core::math::stats::OnlineStats;
use finstack_quant_core::Result;

use std::ops::Range;

/// Generic Monte Carlo execution engine.
///
/// [`McEngine`] prices a payoff by simulating discounted path values under a
/// supplied process and discretization scheme. It can run serially or in
/// parallel and optionally capture paths for diagnostics.
pub struct McEngine {
    pub(super) config: McEngineConfig,
}

/// Minimum number of samples required before the serial auto-stop logic is
/// allowed to fire.
///
/// The half-width threshold is driven by the sample standard error, which
/// itself has relative noise `≈ 1/√(2n)`. At `n = 1 000` that is ≈ 2 %, large
/// enough that noise in the stderr estimate can cross the threshold before
/// the mean has stabilised. `5 000` drops the noise to ≈ 1 % and empirically
/// eliminates premature termination at the cost of a negligible number of
/// extra paths when the threshold is easy.
pub(super) const AUTO_STOP_MIN_SAMPLES: usize = 5_000;

/// Calculate the default chunk size for MC execution.
///
/// This is a **pure function of `num_paths`** by design: the chunk partition
/// fixes the `OnlineStats::merge` reduction tree, and floating-point merges
/// are order-sensitive, so any dependence on `rayon::current_num_threads()`
/// would make results differ across machines, `RAYON_NUM_THREADS` settings,
/// and native-vs-wasm hosts for the same `(seed, num_paths, num_steps)`.
///
/// Targets 64 chunks for large runs (ample work-stealing granularity up to
/// 64 threads), with a 100-path floor to amortize per-chunk overhead and a
/// 10 000-path ceiling to bound per-chunk cache footprint.
pub(super) fn adaptive_chunk_size(num_paths: usize) -> usize {
    (num_paths / 64).clamp(100, 10_000)
}

/// Build the engine-side correlation factor for shock transformation.
///
/// Returns `None` when the discretization applies correlation internally
/// (e.g. [`crate::monte_carlo::discretization::qe_heston::QeHeston`]) or when the process does not
/// declare a correlation matrix via [`StochasticProcess::factor_correlation`].
/// Otherwise returns a Cholesky factor of the declared matrix, which the
/// engine applies to raw independent shocks before each
/// [`Discretization::step`] call.
pub(crate) fn build_correlation_factor<P, D>(
    process: &P,
    disc: &D,
) -> Result<Option<CorrelationFactor>>
where
    P: StochasticProcess,
    D: Discretization<P>,
{
    if disc.applies_correlation_internally() {
        return Ok(None);
    }
    let Some(matrix) = process.factor_correlation() else {
        return Ok(None);
    };
    let n = process.num_factors();
    let factor = cholesky_correlation(&matrix, n).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "failed to Cholesky-factor process correlation matrix: {e}"
        ))
    })?;
    Ok(Some(factor))
}

/// Pre-sized chunk index ranges for parallel path loops (avoids `Vec` reallocations).
pub(super) fn parallel_path_chunks(num_paths: usize, chunk_size: usize) -> Vec<Range<usize>> {
    let num_chunks = num_paths.div_ceil(chunk_size);
    let mut chunks = Vec::with_capacity(num_chunks);
    for start in (0..num_paths).step_by(chunk_size) {
        let end = (start + chunk_size).min(num_paths);
        chunks.push(start..end);
    }
    chunks
}

fn validate_discounted_payoff(
    path_id: usize,
    payoff_value: f64,
    discount_factor: f64,
) -> Result<f64> {
    let discounted_value = payoff_value * discount_factor;
    if discounted_value.is_finite() {
        Ok(discounted_value)
    } else {
        Err(finstack_quant_core::Error::Validation(format!(
            "non-finite discounted payoff on path {path_id}: payoff={payoff_value}, discount_factor={discount_factor}"
        )))
    }
}

impl McEngine {
    /// Create an engine from an explicit configuration.
    pub fn new(config: McEngineConfig) -> Self {
        Self { config }
    }

    /// Borrow the engine configuration.
    pub fn config(&self) -> &McEngineConfig {
        &self.config
    }

    /// Validate that a process requiring a dedicated scheme is paired with it.
    ///
    /// Processes with dynamics a generic scheme cannot see (discrete
    /// dividends, jumps) declare a [`StochasticProcess::dedicated_scheme`];
    /// pairing them with anything else silently simulates the diffusion only.
    pub(crate) fn validate_scheme_pairing<P, D>(process: &P, disc: &D) -> Result<()>
    where
        P: StochasticProcess,
        D: Discretization<P>,
    {
        if let Some(required) = process.dedicated_scheme() {
            let actual = disc.scheme_id();
            if actual != required {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "this process requires its dedicated discretization '{required}' but was \
                     paired with '{actual}': a generic scheme silently simulates only the \
                     diffusion (dropping dividends/jumps) and produces systematically wrong \
                     prices"
                )));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_scheme_grid<P, D>(&self, disc: &D) -> Result<()>
    where
        P: StochasticProcess,
        D: Discretization<P>,
    {
        if let Some(expected_steps) = disc.expected_num_steps() {
            let grid_steps = self.config.time_grid.num_steps();
            if expected_steps != grid_steps {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "discretization '{}' was precomputed for {expected_steps} steps but time_grid has {grid_steps} steps",
                    disc.scheme_id()
                )));
            }
        }
        Ok(())
    }

    /// Validate that the payoff's event schedule fits the configured grid.
    ///
    /// A maturity or fixing step beyond the grid would silently never fire
    /// (terminal spots stay 0.0, Asian averages shrink), so reject the
    /// configuration up front.
    pub(crate) fn validate_payoff_schedule<F: Payoff>(&self, payoff: &F) -> Result<()> {
        if let Some(max_step) = payoff.max_event_step() {
            let steps = self.config.time_grid.num_steps();
            if max_step > steps {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "payoff expects an event at step {max_step} but the time grid has only \
                     {steps} steps: maturity/fixing steps beyond the grid would silently \
                     never fire"
                )));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_runtime<P>(
        &self,
        process: &P,
        initial_state: &[f64],
        discount_factor: f64,
        process_params: Option<&ProcessParams>,
    ) -> Result<()>
    where
        P: StochasticProcess,
    {
        if self.config.num_paths < 2 {
            return Err(finstack_quant_core::Error::Validation(
                "Monte Carlo num_paths must be at least 2 independent estimators; \
                 each antithetic pair counts as one estimator"
                    .to_string(),
            ));
        }

        if self.config.num_paths > MAX_NUM_PATHS {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Monte Carlo num_paths ({}) exceeds maximum ({})",
                self.config.num_paths, MAX_NUM_PATHS
            )));
        }

        if self.config.chunk_size == Some(0) {
            return Err(finstack_quant_core::Error::Validation(
                "Monte Carlo chunk_size must be greater than zero".to_string(),
            ));
        }

        if self.config.time_grid.num_steps() == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "Monte Carlo time_grid must contain at least one simulation step; \
                 a zero-step grid would simply re-evaluate the payoff at t=0 with no \
                 path dynamics and is rejected to surface the configuration mistake"
                    .to_string(),
            ));
        }

        if process.dim() == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "StochasticProcess::dim() must be at least 1; a zero-dimensional process \
                 has no state to simulate"
                    .to_string(),
            ));
        }

        if process.requires_injected_noise() {
            return Err(finstack_quant_core::Error::Validation(
                "this process requires externally injected noise (e.g. Volterra/fBM \
                 increments for rough-volatility models); the generic engine would fill \
                 those factor slots with i.i.d. N(0,1) draws and silently produce wrong \
                 dynamics. Drive it through engine_fractional::simulate_path_fractional \
                 or its dedicated pricer instead."
                    .to_string(),
            ));
        }

        if initial_state.len() != process.dim() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "initial_state length {} does not match process dimension {}",
                initial_state.len(),
                process.dim()
            )));
        }

        if !discount_factor.is_finite() || discount_factor < 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "discount_factor must be finite and non-negative".to_string(),
            ));
        }

        if let Some(target) = self.config.target_ci_half_width {
            if !target.is_finite() || target <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(
                    "target_ci_half_width must be finite and positive".to_string(),
                ));
            }

            if self.config.use_parallel {
                return Err(finstack_quant_core::Error::Validation(
                    "target_ci_half_width is currently unsupported with use_parallel=true"
                        .to_string(),
                ));
            }
        }

        if self.config.path_capture.enabled {
            if self.config.antithetic {
                return Err(finstack_quant_core::Error::Validation(
                    "Path capture is currently unsupported with antithetic=true".to_string(),
                ));
            }

            let requested_captured_paths = match self.config.path_capture.capture_mode {
                PathCaptureMode::All => self.config.num_paths,
                PathCaptureMode::Sample { count, .. } => count,
            };

            if requested_captured_paths > MAX_CAPTURED_PATHS {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Path capture budget exceeded: requested {requested_captured_paths} captured \
                     paths, maximum is {MAX_CAPTURED_PATHS}; use sampled capture for diagnostics"
                )));
            }

            if let PathCaptureMode::Sample { count, .. } = self.config.path_capture.capture_mode {
                if count == 0 || count > self.config.num_paths {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Path capture sample count must be between 1 and num_paths (got {count})"
                    )));
                }
            }
        }

        process.validate_time_grid(&self.config.time_grid)?;
        if let Some(params) = process_params {
            params.validate()?;
        }

        Ok(())
    }

    pub(crate) fn validate_rng<R: RandomStream>(&self, rng: &R) -> Result<()> {
        if rng.is_quasi_random() {
            return Err(finstack_quant_core::Error::Validation(
                "the generic Monte Carlo engine does not support quasi-random streams \
                 (e.g. SobolRng): it consumes draws step-by-step, destroying the \
                 per-path low-discrepancy point structure, and reports an i.i.d. \
                 standard error that is statistically invalid over dependent QMC \
                 points. Use PathDependentPricer with use_sobol=true, which draws one \
                 Sobol point per path and estimates the error across independently \
                 scrambled replicates, or switch to PhiloxRng."
                    .to_string(),
            ));
        }

        if self.config.use_parallel && !rng.supports_splitting() {
            return Err(finstack_quant_core::Error::Validation(
                "Parallel Monte Carlo requires a splittable RNG (e.g., PhiloxRng); \
                 the supplied generator reports supports_splitting() = false — use \
                 serial mode (use_parallel: false) or switch to PhiloxRng."
                    .to_string(),
            ));
        }

        Ok(())
    }

    /// Price a payoff via discounted Monte Carlo simulation.
    ///
    /// # Type Parameters
    ///
    /// * `R` - Random stream type
    /// * `P` - Stochastic process type
    /// * `D` - Discretization scheme type
    /// * `F` - Payoff type
    ///
    /// # Arguments
    ///
    /// * `rng` - Random stream used to generate per-path shocks.
    /// * `process` - Stochastic process defining drift, diffusion, and state layout.
    /// * `disc` - Time-stepping scheme compatible with `process`.
    /// * `initial_state` - Initial process state. Its length must equal `process.dim()`.
    /// * `payoff` - Payoff accumulator evaluated on each simulated path.
    /// * `currency` - Currency tag for the returned [`MoneyEstimate`].
    /// * `discount_factor` - Present-value multiplier for the payoff horizon.
    ///   Scalar only: the engine multiplies every path by this single factor
    ///   and does not apply a pathwise numeraire. Hull-White and LMM
    ///   derivative pricing belongs in the valuations pathwise-numeraire
    ///   pricers. Do not pass a curve discount factor on top of
    ///   `lmm_numeraire`. Callers holding a flat continuously compounded
    ///   rate and a year fraction should build the scalar with
    ///   [`finstack_quant_core::cashflow::flat_discount_factor`].
    ///
    /// # Returns
    ///
    /// A discounted Monte Carlo estimate in `currency`, including the mean,
    /// standard error, and 95% confidence interval of discounted path values.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// * `num_paths < 2` independent estimators, including antithetic pair means
    /// * `chunk_size == 0`
    /// * `initial_state.len() != process.dim()`
    /// * `discount_factor` is not finite or is negative
    /// * `target_ci_half_width` is non-positive or combined with parallel mode
    /// * the RNG is quasi-random (e.g. `SobolRng`); quasi-Monte Carlo pricing
    ///   goes through `PathDependentPricer` instead
    /// * parallel mode is requested with an RNG that does not support splitting
    ///
    /// # Determinism
    ///
    /// With a splittable RNG (e.g. `PhiloxRng`) and auto-stopping disabled, the
    /// serial and parallel paths return bit-identical estimates: both reduce
    /// per-path values through the same chunk -> `OnlineStats::merge` tree, and the
    /// per-path values themselves come from `rng.split(path_id)`, independent of
    /// thread count. The default chunk size is a pure function of `num_paths`
    /// (never of the thread count), so results are also bit-identical across
    /// machines, `RAYON_NUM_THREADS` settings, and native-vs-wasm hosts.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_models::monte_carlo::discretization::ExactGbm;
    /// use finstack_quant_models::monte_carlo::engine::{McEngine, McEngineConfig};
    /// use finstack_quant_models::monte_carlo::payoff::vanilla::EuropeanCall;
    /// use finstack_quant_models::monte_carlo::process::gbm::GbmProcess;
    /// use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
    ///
    /// let engine = McEngine::new(
    ///     McEngineConfig::uniform(25_000, 1.0, 252)
    ///         .expect("valid Monte Carlo configuration"),
    /// );
    ///
    /// let rng = PhiloxRng::new(11);
    /// let process = GbmProcess::with_params(0.03, 0.01, 0.20).unwrap();
    /// let disc = ExactGbm::new();
    /// let payoff = EuropeanCall::new(100.0, 1.0, 252);
    /// let discount_factor = (-0.03_f64).exp();
    ///
    /// let result = engine
    ///     .price(
    ///         &rng,
    ///         &process,
    ///         &disc,
    ///         &[100.0],
    ///         &payoff,
    ///         Currency::USD,
    ///         discount_factor,
    ///     )
    ///     .expect("pricing should succeed");
    ///
    /// assert!(result.mean.amount() >= 0.0);
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn price<R, P, D, F>(
        &self,
        rng: &R,
        process: &P,
        disc: &D,
        initial_state: &[f64],
        payoff: &F,
        currency: Currency,
        discount_factor: f64,
    ) -> Result<MoneyEstimate>
    where
        R: RandomStream,
        P: StochasticProcess,
        D: Discretization<P> + Clone,
        F: Payoff,
    {
        let _span = tracing::info_span!(
            "mc_price",
            num_paths = self.config.num_paths,
            num_steps = self.config.time_grid.num_steps(),
            parallel = self.config.use_parallel,
            antithetic = self.config.antithetic,
            chunk_size = ?self.config.chunk_size,
        )
        .entered();

        self.validate_rng(rng)?;
        self.validate_runtime(process, initial_state, discount_factor, None)?;
        Self::validate_scheme_pairing(process, disc)?;
        self.validate_scheme_grid(disc)?;
        self.validate_payoff_schedule(payoff)?;

        let estimate = self.run_loops(
            rng,
            process,
            disc,
            initial_state,
            payoff,
            currency,
            discount_factor,
            /* capture = */ false,
        )?;

        tracing::debug!(
            mean = estimate.0.mean,
            stderr = estimate.0.stderr,
            num_paths = estimate.0.num_paths,
            "mc_price completed"
        );

        MoneyEstimate::from_estimate(estimate.0, currency)
    }

    /// Price a payoff and optionally return captured paths.
    ///
    /// This method extends [`Self::price`] by validating and attaching
    /// [`ProcessParams`] metadata and by returning a [`MonteCarloResult`] that
    /// may include a captured [`crate::monte_carlo::paths::PathDataset`].
    ///
    /// # Arguments
    ///
    /// * `rng` - Random stream supplying the Monte Carlo uniforms / Gaussians
    /// * `process` - Stochastic process that evolves the simulated state vector
    /// * `disc` - Discretization scheme paired with `process` for path stepping
    /// * `initial_state` - Process state at time zero; length must match the factor count
    /// * `payoff` - Pathwise payoff evaluated on each simulated trajectory
    /// * `currency` - Currency stamped onto the returned monetary estimate
    /// * `discount_factor` - Scalar present-value factor applied to payoff units
    /// * `process_params` - Metadata describing the captured state layout,
    ///   process parameters, and optional correlation matrix
    ///
    /// # Returns
    ///
    /// A Monte Carlo result containing the discounted estimate and, when path
    /// capture is enabled, a captured-path dataset.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::price`], plus any validation error
    /// raised by [`ProcessParams::validate`](crate::monte_carlo::paths::ProcessParams::validate).
    /// The engine also rejects `antithetic = true` together with path capture.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_models::monte_carlo::discretization::ExactGbm;
    /// use finstack_quant_models::monte_carlo::engine::{
    ///     McEngine, McEngineConfig, PathCaptureConfig,
    /// };
    /// use finstack_quant_models::monte_carlo::paths::ProcessParams;
    /// use finstack_quant_models::monte_carlo::payoff::vanilla::EuropeanCall;
    /// use finstack_quant_models::monte_carlo::process::gbm::GbmProcess;
    /// use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
    ///
    /// let engine = McEngine::new(
    ///     McEngineConfig::uniform(2_000, 1.0, 12)
    ///         .expect("valid Monte Carlo configuration")
    ///         .path_capture(PathCaptureConfig::sample(100, 17).with_payoffs()),
    /// );
    ///
    /// let rng = PhiloxRng::new(5);
    /// let process = GbmProcess::with_params(0.03, 0.01, 0.20).unwrap();
    /// let disc = ExactGbm::new();
    /// let payoff = EuropeanCall::new(100.0, 1.0, 12);
    /// let discount_factor = (-0.03_f64).exp();
    /// let process_params = ProcessParams::new("GBM").with_factors(vec!["spot".to_string()]);
    ///
    /// let result = engine
    ///     .price_with_capture(
    ///         &rng,
    ///         &process,
    ///         &disc,
    ///         &[100.0],
    ///         &payoff,
    ///         Currency::USD,
    ///         discount_factor,
    ///         process_params,
    ///     )
    ///     .expect("pricing with capture should succeed");
    ///
    /// assert!(result.estimate.mean.amount().is_finite());
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn price_with_capture<R, P, D, F>(
        &self,
        rng: &R,
        process: &P,
        disc: &D,
        initial_state: &[f64],
        payoff: &F,
        currency: Currency,
        discount_factor: f64,
        process_params: ProcessParams,
    ) -> Result<MonteCarloResult>
    where
        R: RandomStream,
        P: StochasticProcess,
        D: Discretization<P> + Clone,
        F: Payoff,
    {
        self.validate_rng(rng)?;
        self.validate_runtime(
            process,
            initial_state,
            discount_factor,
            Some(&process_params),
        )?;
        Self::validate_scheme_pairing(process, disc)?;
        self.validate_scheme_grid(disc)?;
        self.validate_payoff_schedule(payoff)?;

        let capture_enabled = self.config.path_capture.enabled;

        let (estimate, collected_paths) = self.run_loops(
            rng,
            process,
            disc,
            initial_state,
            payoff,
            currency,
            discount_factor,
            capture_enabled,
        )?;

        self.finalize_captured_result(estimate, collected_paths, currency, Some(process_params))
    }

    /// Assemble a [`MonteCarloResult`] from a raw [`Estimate`] and the paths
    /// collected by a per-path loop.
    ///
    /// Used by both the generic engine loops in this module and the Sobol
    /// path-dependent pricer so that result formatting (path sorting,
    /// captured-path statistics, currency tagging, dataset construction) lives
    /// in exactly one place.
    ///
    /// `process_params` is optional because Sobol validation currently happens
    /// in the pricer layer and does not require the engine to revalidate the
    /// metadata; supplying `Some(..)` is equivalent to calling the public
    /// [`Self::price_with_capture`] entry point.
    pub(crate) fn finalize_captured_result(
        &self,
        mut estimate: Estimate,
        mut collected_paths: Vec<SimulatedPath>,
        currency: Currency,
        process_params: Option<ProcessParams>,
    ) -> finstack_quant_core::Result<MonteCarloResult> {
        let capture_enabled = self.config.path_capture.enabled;
        let paths = if capture_enabled {
            let sampling_method = match &self.config.path_capture.capture_mode {
                PathCaptureMode::All => PathSamplingMethod::All,
                PathCaptureMode::Sample { count, seed } => PathSamplingMethod::RandomSample {
                    count: *count,
                    seed: *seed,
                },
            };
            // Sort for deterministic ordering (parallel chunks arrive out of order;
            // serial is already sorted so this is O(n) in that case).
            collected_paths.sort_by_key(|p| p.path_id);
            let params = process_params.unwrap_or_else(|| ProcessParams::new("unspecified"));
            let mut dataset = PathDataset::new(estimate.num_paths, sampling_method, params);
            for path in collected_paths {
                dataset.add_path(path);
            }
            estimate = apply_captured_path_statistics(estimate, &dataset.paths);
            Some(dataset)
        } else {
            None
        };

        let money_estimate = MoneyEstimate::from_estimate(estimate, currency)?;
        // Stamp the run with its execution policy so results are auditable
        // and replayable. The engine cannot observe the RNG seed (it receives
        // a constructed stream); pricers that derive the stream from a seed
        // fill `run.seed` on the returned result.
        let run = crate::monte_carlo::results::RunMetadata {
            seed: None,
            use_parallel: self.config.use_parallel,
            antithetic: self.config.antithetic,
            chunk_size: self
                .config
                .chunk_size
                .unwrap_or_else(|| adaptive_chunk_size(self.config.num_paths)),
            num_steps: self.config.time_grid.num_steps(),
        };
        let result = match paths {
            Some(paths) => MonteCarloResult::with_paths(money_estimate, paths),
            None => MonteCarloResult::new(money_estimate),
        };
        Ok(result.with_run_metadata(run))
    }

    /// Dispatch to serial or parallel path loop and return the aggregate
    /// estimate along with any captured paths.
    ///
    /// When `capture` is `false`, the returned `Vec<SimulatedPath>` is empty.
    /// When `capture` is `true`, the engine validates that antithetic is
    /// disabled (see [`Self::validate_runtime`]) and fills the vector with the
    /// sampled subset of captured paths in path-id order.
    #[allow(clippy::too_many_arguments)]
    fn run_loops<R, P, D, F>(
        &self,
        rng: &R,
        process: &P,
        disc: &D,
        initial_state: &[f64],
        payoff: &F,
        currency: Currency,
        discount_factor: f64,
        capture: bool,
    ) -> Result<(Estimate, Vec<SimulatedPath>)>
    where
        R: RandomStream,
        P: StochasticProcess,
        D: Discretization<P> + Clone,
        F: Payoff,
    {
        #[cfg(not(target_arch = "wasm32"))]
        if self.config.use_parallel {
            return self.price_parallel(
                rng,
                process,
                disc,
                initial_state,
                payoff,
                currency,
                discount_factor,
                capture,
            );
        }
        self.price_serial(
            rng,
            process,
            disc,
            initial_state,
            payoff,
            currency,
            discount_factor,
            capture,
        )
    }

    /// Prepare the scheme and correlation factor shared by every path of a run.
    ///
    /// Hoists per-run, path-independent scheme constants out of the per-step
    /// hot path (e.g. exp(-κΔt) in exact rate / QE variance schemes). The
    /// prepared copy is shared read-only by every path (it is `Sync`).
    pub(crate) fn prepare_run<P, D>(
        &self,
        process: &P,
        disc: &D,
    ) -> Result<(D, Option<CorrelationFactor>)>
    where
        P: StochasticProcess,
        D: Discretization<P> + Clone,
    {
        let mut prepared_disc = disc.clone();
        prepared_disc.prepare(process, &self.config.time_grid);
        let correlation = build_correlation_factor(process, &prepared_disc)?;
        Ok((prepared_disc, correlation))
    }

    /// Simulate one path (or antithetic pair) and return its discounted value.
    ///
    /// Resets the scratch payoffs, then dispatches to the captured, antithetic
    /// or plain path primitive. A path is captured only when `captured` is
    /// `Some` and the capture sampler selects `path_id`; `validate_runtime`
    /// rejects antithetic together with capture, so a captured path is never
    /// paired.
    pub(crate) fn simulate_discounted<R, P, D, F>(
        &self,
        rng: &mut R,
        inputs: PathInputs<'_, P, D>,
        scratch: &mut PathScratch<F>,
        path_id: usize,
        captured: Option<&mut Vec<SimulatedPath>>,
    ) -> Result<f64>
    where
        R: RandomStream,
        P: StochasticProcess,
        D: Discretization<P>,
        F: Payoff,
    {
        let s = scratch;
        s.payoff.reset();
        let captured = captured.filter(|_| {
            self.config
                .path_capture
                .should_capture(path_id, self.config.num_paths)
        });
        let payoff_value = if let Some(paths) = captured {
            let (v, path) = self.simulate_path_with_capture(
                rng,
                inputs.process,
                inputs.disc,
                inputs.initial_state,
                &mut s.payoff,
                &mut s.state,
                &mut s.z,
                &mut s.z_raw,
                &mut s.work,
                inputs.correlation,
                path_id,
                inputs.discount_factor,
                inputs.currency,
            )?;
            paths.push(path);
            v
        } else if let Some(payoff_anti) = s.payoff_anti.as_mut() {
            payoff_anti.reset();
            self.simulate_antithetic_pair(
                rng,
                inputs.process,
                inputs.disc,
                inputs.initial_state,
                &mut s.payoff,
                payoff_anti,
                &mut s.state,
                &mut s.state_anti,
                &mut s.z,
                &mut s.z_anti,
                &mut s.z_raw,
                &mut s.work,
                &mut s.work_anti,
                inputs.correlation,
                inputs.currency,
            )?
        } else {
            self.simulate_path(
                rng,
                inputs.process,
                inputs.disc,
                inputs.initial_state,
                &mut s.payoff,
                &mut s.state,
                &mut s.z,
                &mut s.z_raw,
                &mut s.work,
                inputs.correlation,
                inputs.currency,
            )?
        };
        validate_discounted_payoff(path_id, payoff_value, inputs.discount_factor)
    }

    /// Accumulate the discounted values of the paths in `range`.
    ///
    /// Returns the chunk statistics and whether the serial auto-stop rule
    /// fired. The rule needs a single-chunk run (`price_serial` arranges
    /// that); a 5 000-sample warm-up keeps the half-width estimate stable,
    /// because the standard error of the sample standard error is
    /// ~1/√(2n), so at n=1 000 the stopping criterion itself has ≈ 2 % noise
    /// which routinely trips the threshold early.
    fn run_chunk<R, P, D, F>(
        &self,
        range: Range<usize>,
        rng: &mut RngSource<'_, R>,
        inputs: PathInputs<'_, P, D>,
        scratch: &mut PathScratch<F>,
        mut captured: Option<&mut Vec<SimulatedPath>>,
    ) -> Result<(OnlineStats, bool)>
    where
        R: RandomStream,
        P: StochasticProcess,
        D: Discretization<P>,
        F: Payoff,
    {
        let mut stats = OnlineStats::new();
        for path_id in range {
            let discounted_value = match rng {
                RngSource::Split(root) => {
                    let mut path_rng = root.split(path_id as u64).ok_or_else(|| {
                        finstack_quant_core::Error::Validation(
                            "RandomStream reports splitting support but split() returned None"
                                .to_string(),
                        )
                    })?;
                    self.simulate_discounted(
                        &mut path_rng,
                        inputs,
                        scratch,
                        path_id,
                        captured.as_deref_mut(),
                    )?
                }
                RngSource::Sequential(stream) => self.simulate_discounted(
                    &mut **stream,
                    inputs,
                    scratch,
                    path_id,
                    captured.as_deref_mut(),
                )?,
            };
            stats.update(discounted_value);

            if let Some(target) = self.config.target_ci_half_width {
                if stats.count() >= AUTO_STOP_MIN_SAMPLES && stats.ci_half_width() < target {
                    return Ok((stats, true));
                }
            }
        }
        Ok((stats, false))
    }

    /// Wrap merged run statistics in an [`Estimate`].
    fn run_estimate(&self, combined: &OnlineStats) -> Estimate {
        let num_paths = combined.count();
        let num_simulated_paths = if self.config.antithetic {
            num_paths.saturating_mul(2)
        } else {
            num_paths
        };
        Estimate::from_stats(combined).with_num_simulated_paths(num_simulated_paths)
    }

    /// Serial pricing implementation with optional path capture.
    #[allow(clippy::too_many_arguments)]
    fn price_serial<R, P, D, F>(
        &self,
        rng: &R,
        process: &P,
        disc: &D,
        initial_state: &[f64],
        payoff: &F,
        currency: Currency,
        discount_factor: f64,
        capture: bool,
    ) -> Result<(Estimate, Vec<SimulatedPath>)>
    where
        R: RandomStream,
        P: StochasticProcess,
        D: Discretization<P> + Clone,
        F: Payoff,
    {
        let (disc, correlation) = self.prepare_run(process, disc)?;
        let inputs = PathInputs {
            process,
            disc: &disc,
            initial_state,
            correlation: correlation.as_ref(),
            currency,
            discount_factor,
        };
        let mut scratch = PathScratch::new(inputs, payoff, self.config.antithetic);

        let mut captured_paths: Vec<SimulatedPath> = if capture {
            let estimated_capacity = match self.config.path_capture.capture_mode {
                PathCaptureMode::All => self.config.num_paths,
                PathCaptureMode::Sample { count, .. } => count,
            };
            Vec::with_capacity(estimated_capacity)
        } else {
            Vec::new()
        };

        // A non-splittable stream is consumed sequentially and carries across
        // chunks.
        let mut sequential_rng = (!rng.supports_splitting()).then(|| rng.clone());
        let mut rng_source = match sequential_rng.as_mut() {
            Some(stream) => RngSource::Sequential(stream),
            None => RngSource::Split(rng),
        };

        // Reduce via the same chunk -> merge tree as `price_parallel` so that, for
        // a splittable RNG with auto-stopping disabled, serial and parallel results
        // are bit-identical (workspace determinism invariant). Auto-stopping has no
        // parallel counterpart, so it runs as a single chunk with a running
        // accumulator that evaluates the stopping rule sequentially.
        let chunk_ranges: Vec<Range<usize>> = if self.config.target_ci_half_width.is_some() {
            std::iter::once(0..self.config.num_paths).collect()
        } else {
            let effective_chunk_size = self
                .config
                .chunk_size
                .unwrap_or_else(|| adaptive_chunk_size(self.config.num_paths));
            parallel_path_chunks(self.config.num_paths, effective_chunk_size)
        };

        let mut combined = OnlineStats::new();
        for range in chunk_ranges {
            let (stats, auto_stopped) = self.run_chunk(
                range,
                &mut rng_source,
                inputs,
                &mut scratch,
                capture.then_some(&mut captured_paths),
            )?;
            combined.merge(&stats);
            if auto_stopped {
                break;
            }
        }

        Ok((self.run_estimate(&combined), captured_paths))
    }

    /// Parallel pricing implementation with optional path capture.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::too_many_arguments)]
    fn price_parallel<R, P, D, F>(
        &self,
        rng: &R,
        process: &P,
        disc: &D,
        initial_state: &[f64],
        payoff: &F,
        currency: Currency,
        discount_factor: f64,
        capture: bool,
    ) -> Result<(Estimate, Vec<SimulatedPath>)>
    where
        R: RandomStream,
        P: StochasticProcess,
        D: Discretization<P> + Clone,
        F: Payoff,
    {
        use rayon::prelude::*;

        let (disc, correlation) = self.prepare_run(process, disc)?;
        let inputs = PathInputs {
            process,
            disc: &disc,
            initial_state,
            correlation: correlation.as_ref(),
            currency,
            discount_factor,
        };

        // `None` requests adaptive chunking; `Some(n)` uses exactly `n`.
        let effective_chunk_size = self
            .config
            .chunk_size
            .unwrap_or_else(|| adaptive_chunk_size(self.config.num_paths));
        let chunks = parallel_path_chunks(self.config.num_paths, effective_chunk_size);

        let chunk_results: Vec<Result<(OnlineStats, Vec<SimulatedPath>)>> = chunks
            .into_par_iter()
            .map(|range| {
                let mut scratch = PathScratch::new(inputs, payoff, self.config.antithetic);
                let mut chunk_paths = Vec::new();
                let (stats, _) = self.run_chunk(
                    range,
                    &mut RngSource::Split(rng),
                    inputs,
                    &mut scratch,
                    capture.then_some(&mut chunk_paths),
                )?;
                Ok((stats, chunk_paths))
            })
            .collect();

        let mut combined = OnlineStats::new();
        let mut captured_paths = Vec::new();
        for chunk_result in chunk_results {
            let (chunk_stat, chunk_paths) = chunk_result?;
            combined.merge(&chunk_stat);
            captured_paths.extend(chunk_paths);
        }

        Ok((self.run_estimate(&combined), captured_paths))
    }
}

/// Path-invariant inputs shared by every path of one run.
pub(crate) struct PathInputs<'a, P, D> {
    /// Process driving the simulated state.
    pub(crate) process: &'a P,
    /// Prepared discretization (see [`McEngine::prepare_run`]).
    pub(crate) disc: &'a D,
    /// State at time zero; its length equals `process.dim()`.
    pub(crate) initial_state: &'a [f64],
    /// Engine-applied shock correlation, if any.
    pub(crate) correlation: Option<&'a CorrelationFactor>,
    /// Currency of the payoff amounts.
    pub(crate) currency: Currency,
    /// Scalar present-value factor applied to every path.
    pub(crate) discount_factor: f64,
}

impl<P, D> Clone for PathInputs<'_, P, D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P, D> Copy for PathInputs<'_, P, D> {}

/// Per-path scratch buffers and payoff accumulators reused across paths.
///
/// The antithetic buffers and second payoff exist only for antithetic runs.
pub(crate) struct PathScratch<F> {
    state: Vec<f64>,
    z: Vec<f64>,
    /// Independent shocks when the engine applies correlation; zero-length
    /// otherwise.
    z_raw: Vec<f64>,
    work: Vec<f64>,
    state_anti: Vec<f64>,
    z_anti: Vec<f64>,
    work_anti: Vec<f64>,
    payoff: F,
    payoff_anti: Option<F>,
}

impl<F: Payoff> PathScratch<F> {
    /// Allocate scratch sized for `inputs.process` and `inputs.disc`.
    pub(crate) fn new<P, D>(inputs: PathInputs<'_, P, D>, payoff: &F, antithetic: bool) -> Self
    where
        P: StochasticProcess,
        D: Discretization<P>,
    {
        let dim = inputs.process.dim();
        let num_factors = inputs.process.num_factors();
        let work_size = inputs.disc.work_size(inputs.process);
        let anti = |n: usize| vec![0.0; if antithetic { n } else { 0 }];
        Self {
            state: vec![0.0; dim],
            z: vec![0.0; num_factors],
            z_raw: vec![
                0.0;
                if inputs.correlation.is_some() {
                    num_factors
                } else {
                    0
                }
            ],
            work: vec![0.0; work_size],
            state_anti: anti(dim),
            z_anti: anti(num_factors),
            work_anti: anti(work_size),
            payoff: payoff.clone(),
            payoff_anti: antithetic.then(|| payoff.clone()),
        }
    }
}

/// Where each path's random stream comes from.
enum RngSource<'a, R> {
    /// Splittable root: path `i` draws from `root.split(i)`.
    Split(&'a R),
    /// One non-splittable stream consumed path after path.
    Sequential(&'a mut R),
}
