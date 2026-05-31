use super::config::{McEngineBuilder, McEngineConfig, MAX_CAPTURED_PATHS, MAX_NUM_PATHS};
use super::path_capture::PathCaptureMode;
use crate::captured_path_stats::apply_captured_path_statistics;
use crate::estimate::Estimate;
use crate::online_stats::OnlineStats;
use crate::paths::{PathDataset, PathSamplingMethod, ProcessParams, SimulatedPath};
use crate::results::{MoneyEstimate, MonteCarloResult};
use crate::traits::{Discretization, Payoff, RandomStream, StochasticProcess};
use finstack_core::currency::Currency;
use finstack_core::math::linalg::{cholesky_correlation, CorrelationFactor};
use finstack_core::Result;

use std::ops::Range;
use std::sync::Mutex;

use rayon::prelude::*;

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

/// Calculate adaptive chunk size for parallel MC execution.
///
/// Balances load distribution across cores with cache efficiency.
/// Target: 4 chunks per thread for good load balancing.
pub(super) fn adaptive_chunk_size(num_paths: usize) -> usize {
    let num_cpus = rayon::current_num_threads();
    // Target 4 chunks per thread for load balancing
    // Min 100 paths per chunk to amortize overhead
    // Max 10_000 paths to avoid cache thrashing
    (num_paths / (num_cpus * 4)).clamp(100, 10_000)
}

/// Build the engine-side correlation factor for shock transformation.
///
/// Returns `None` when the discretization applies correlation internally
/// (e.g. [`crate::discretization::QeHeston`]) or when the process does not
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
        finstack_core::Error::Validation(format!(
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

pub(crate) fn validate_discounted_payoff(
    path_id: usize,
    payoff_value: f64,
    discount_factor: f64,
) -> Result<f64> {
    let discounted_value = payoff_value * discount_factor;
    if discounted_value.is_finite() {
        Ok(discounted_value)
    } else {
        Err(finstack_core::Error::Validation(format!(
            "non-finite discounted payoff on path {path_id}: payoff={payoff_value}, discount_factor={discount_factor}"
        )))
    }
}

impl McEngine {
    /// Create a builder with the crate's default engine settings.
    #[must_use]
    pub fn builder() -> McEngineBuilder {
        McEngineBuilder::new()
    }

    /// Create an engine from an explicit configuration.
    pub fn new(config: McEngineConfig) -> Self {
        Self { config }
    }

    /// Construct from config (used by builder).
    pub(super) fn from_config(config: McEngineConfig) -> Self {
        Self { config }
    }

    /// Borrow the engine configuration.
    pub fn config(&self) -> &McEngineConfig {
        &self.config
    }

    pub(super) fn validate_runtime<R, P>(
        &self,
        rng: &R,
        process: &P,
        initial_state: &[f64],
        discount_factor: f64,
        process_params: Option<&ProcessParams>,
    ) -> Result<()>
    where
        R: RandomStream,
        P: StochasticProcess,
    {
        if self.config.num_paths == 0 {
            return Err(finstack_core::Error::Validation(
                "Monte Carlo num_paths must be greater than zero".to_string(),
            ));
        }

        if self.config.num_paths > MAX_NUM_PATHS {
            return Err(finstack_core::Error::Validation(format!(
                "Monte Carlo num_paths ({}) exceeds maximum ({})",
                self.config.num_paths, MAX_NUM_PATHS
            )));
        }

        if self.config.chunk_size == Some(0) {
            return Err(finstack_core::Error::Validation(
                "Monte Carlo chunk_size must be greater than zero".to_string(),
            ));
        }

        if self.config.time_grid.num_steps() == 0 {
            return Err(finstack_core::Error::Validation(
                "Monte Carlo time_grid must contain at least one simulation step; \
                 a zero-step grid would simply re-evaluate the payoff at t=0 with no \
                 path dynamics and is rejected to surface the configuration mistake"
                    .to_string(),
            ));
        }

        if process.dim() == 0 {
            return Err(finstack_core::Error::Validation(
                "StochasticProcess::dim() must be at least 1; a zero-dimensional process \
                 has no state to simulate"
                    .to_string(),
            ));
        }

        if initial_state.len() != process.dim() {
            return Err(finstack_core::Error::Validation(format!(
                "initial_state length {} does not match process dimension {}",
                initial_state.len(),
                process.dim()
            )));
        }

        if !discount_factor.is_finite() || discount_factor < 0.0 {
            return Err(finstack_core::Error::Validation(
                "discount_factor must be finite and non-negative".to_string(),
            ));
        }

        if let Some(target) = self.config.target_ci_half_width {
            if !target.is_finite() || target <= 0.0 {
                return Err(finstack_core::Error::Validation(
                    "target_ci_half_width must be finite and positive".to_string(),
                ));
            }

            if self.config.use_parallel {
                return Err(finstack_core::Error::Validation(
                    "target_ci_half_width is currently unsupported with use_parallel=true"
                        .to_string(),
                ));
            }
        }

        if self.config.use_parallel && !rng.supports_splitting() {
            return Err(finstack_core::Error::Validation(
                "Parallel Monte Carlo requires a splittable RNG (e.g., PhiloxRng). \
                 SobolRng does not support stream splitting — use serial mode (use_parallel: false) \
                 or switch to PhiloxRng for parallel execution."
                    .to_string(),
            ));
        }

        if self.config.path_capture.enabled {
            if self.config.antithetic {
                return Err(finstack_core::Error::Validation(
                    "Path capture is currently unsupported with antithetic=true".to_string(),
                ));
            }

            let requested_captured_paths = match self.config.path_capture.capture_mode {
                PathCaptureMode::All => self.config.num_paths,
                PathCaptureMode::Sample { count, .. } => count,
            };

            if requested_captured_paths > MAX_CAPTURED_PATHS {
                return Err(finstack_core::Error::Validation(format!(
                    "Path capture budget exceeded: requested {requested_captured_paths} captured \
                     paths, maximum is {MAX_CAPTURED_PATHS}; use sampled capture for diagnostics"
                )));
            }

            if let PathCaptureMode::Sample { count, .. } = self.config.path_capture.capture_mode {
                if count == 0 || count > self.config.num_paths {
                    return Err(finstack_core::Error::Validation(format!(
                        "Path capture sample count must be between 1 and num_paths (got {count})"
                    )));
                }
            }
        }

        if let Some(params) = process_params {
            params.validate()?;
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
    /// * `num_paths == 0`
    /// * `chunk_size == 0`
    /// * `initial_state.len() != process.dim()`
    /// * `discount_factor` is not finite or is negative
    /// * `target_ci_half_width` is non-positive or combined with parallel mode
    /// * parallel mode is requested with an RNG that does not support splitting
    ///
    /// # Determinism
    ///
    /// With a splittable RNG (e.g. `PhiloxRng`) and auto-stopping disabled, the
    /// serial and parallel paths return bit-identical estimates: both reduce
    /// per-path values through the same chunk -> `OnlineStats::merge` tree, and the
    /// per-path values themselves come from `rng.split(path_id)`, independent of
    /// thread count (see INVARIANTS.md §2.1).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_core::currency::Currency;
    /// use finstack_monte_carlo::prelude::*;
    ///
    /// let engine = McEngine::builder()
    ///     .num_paths(25_000)
    ///     .uniform_grid(1.0, 252)
    ///     .build()
    ///     .expect("valid Monte Carlo configuration");
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
        D: Discretization<P>,
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

        self.validate_runtime(rng, process, initial_state, discount_factor, None)?;

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

        Ok(MoneyEstimate::from_estimate(estimate.0, currency))
    }

    /// Price a payoff and optionally return captured paths.
    ///
    /// This method extends [`Self::price`] by validating and attaching
    /// [`ProcessParams`] metadata and by returning a [`MonteCarloResult`] that
    /// may include a captured [`crate::paths::PathDataset`].
    ///
    /// # Arguments
    ///
    /// * `process_params` - Metadata describing the captured state layout,
    ///   process parameters, and optional correlation matrix.
    ///
    /// For the other arguments, see [`Self::price`].
    ///
    /// # Returns
    ///
    /// A Monte Carlo result containing the discounted estimate and, when path
    /// capture is enabled, a captured-path dataset.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::price`], plus any validation error
    /// raised by [`ProcessParams::validate`](crate::paths::ProcessParams::validate).
    /// The engine also rejects `antithetic = true` together with path capture.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_core::currency::Currency;
    /// use finstack_monte_carlo::prelude::*;
    ///
    /// let engine = McEngine::builder()
    ///     .num_paths(2_000)
    ///     .uniform_grid(1.0, 12)
    ///     .path_capture(PathCaptureConfig::sample(100, 17).with_payoffs())
    ///     .build()
    ///     .expect("valid Monte Carlo configuration");
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
        D: Discretization<P>,
        F: Payoff,
    {
        self.validate_runtime(
            rng,
            process,
            initial_state,
            discount_factor,
            Some(&process_params),
        )?;

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

        Ok(
            self.finalize_captured_result(
                estimate,
                collected_paths,
                currency,
                Some(process_params),
            ),
        )
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
    ) -> MonteCarloResult {
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

        let money_estimate = MoneyEstimate::from_estimate(estimate, currency);
        match paths {
            Some(paths) => MonteCarloResult::with_paths(money_estimate, paths),
            None => MonteCarloResult::new(money_estimate),
        }
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
        D: Discretization<P>,
        F: Payoff,
    {
        if self.config.use_parallel {
            self.price_parallel(
                rng,
                process,
                disc,
                initial_state,
                payoff,
                currency,
                discount_factor,
                capture,
            )
        } else {
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
        D: Discretization<P>,
        F: Payoff,
    {
        let dim = process.dim();
        let num_factors = process.num_factors();
        let work_size = disc.work_size(process);
        let correlation = build_correlation_factor(process, disc)?;

        // Pre-allocate buffers (reused across paths)
        let mut state = vec![0.0; dim];
        let mut z = vec![0.0; num_factors];
        // `z_raw` holds independent shocks when the engine applies correlation;
        // otherwise it stays zero-length to avoid an unused allocation.
        let mut z_raw = vec![
            0.0;
            if correlation.is_some() {
                num_factors
            } else {
                0
            }
        ];
        let mut work = vec![0.0; work_size];
        let mut state_a = vec![0.0; dim];
        let mut z_anti = vec![0.0; num_factors];
        let mut work_anti = vec![0.0; work_size];

        let mut captured_paths: Vec<SimulatedPath> = if capture {
            let estimated_capacity = match self.config.path_capture.capture_mode {
                PathCaptureMode::All => self.config.num_paths,
                PathCaptureMode::Sample { count, .. } => count,
            };
            Vec::with_capacity(estimated_capacity)
        } else {
            Vec::new()
        };

        // Long-lived clones reused across all paths (reset between iterations).
        // The antithetic branch needs an independent second payoff for the
        // mirrored path; we allocate it lazily so non-antithetic runs pay
        // for exactly one clone rather than two.
        let mut payoff_local = payoff.clone();
        let mut payoff_anti: Option<F> = self.config.antithetic.then(|| payoff.clone());
        let use_split_streams = rng.supports_splitting();
        let mut sequential_rng = (!use_split_streams).then(|| rng.clone());

        // Reduce via the same chunk -> merge tree as `price_parallel` so that, for
        // a splittable RNG with auto-stopping disabled, serial and parallel results
        // are bit-identical (INVARIANTS.md §2.1). Auto-stopping has no parallel
        // counterpart, so it runs as a single chunk with a running accumulator that
        // evaluates the stopping rule sequentially.
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
        let mut auto_stopped = false;
        for range in &chunk_ranges {
            let mut stats = OnlineStats::new();
            for path_id in range.clone() {
                let mut split_rng;
                let path_rng = if use_split_streams {
                    split_rng = rng.split(path_id as u64).ok_or_else(|| {
                        finstack_core::Error::Validation(
                            "RandomStream reports splitting support but split() returned None"
                                .to_string(),
                        )
                    })?;
                    &mut split_rng
                } else {
                    sequential_rng.as_mut().ok_or_else(|| {
                        finstack_core::Error::Validation(
                            "serial non-splittable RNG was not initialized".to_string(),
                        )
                    })?
                };

                payoff_local.reset();
                payoff_local.on_path_start(&mut *path_rng);
                if let Some(p) = payoff_anti.as_mut() {
                    p.reset();
                    p.on_path_start(&mut *path_rng);
                }

                let should_capture = capture
                    && self
                        .config
                        .path_capture
                        .should_capture(path_id, self.config.num_paths);

                let payoff_value = if should_capture {
                    // `validate_runtime` rejects antithetic+capture, so this branch
                    // is only reached with antithetic disabled.
                    let (v, path) = self.simulate_path_with_capture(
                        &mut *path_rng,
                        process,
                        disc,
                        initial_state,
                        &mut payoff_local,
                        &mut state,
                        &mut z,
                        &mut z_raw,
                        &mut work,
                        correlation.as_ref(),
                        path_id,
                        discount_factor,
                        currency,
                    )?;
                    captured_paths.push(path);
                    v
                } else if let Some(payoff_anti_mut) = payoff_anti.as_mut() {
                    // antithetic = true ⇒ payoff_anti is Some by construction.
                    self.simulate_antithetic_pair(
                        &mut *path_rng,
                        process,
                        disc,
                        initial_state,
                        &mut payoff_local,
                        payoff_anti_mut,
                        &mut state,
                        &mut state_a,
                        &mut z,
                        &mut z_anti,
                        &mut z_raw,
                        &mut work,
                        &mut work_anti,
                        correlation.as_ref(),
                        currency,
                    )?
                } else {
                    self.simulate_path(
                        &mut *path_rng,
                        process,
                        disc,
                        initial_state,
                        &mut payoff_local,
                        &mut state,
                        &mut z,
                        &mut z_raw,
                        &mut work,
                        correlation.as_ref(),
                        currency,
                    )?
                };

                let discounted_value =
                    validate_discounted_payoff(path_id, payoff_value, discount_factor)?;
                stats.update(discounted_value);

                // Check auto-stop condition. A 5 000-sample warm-up keeps the
                // half-width estimate stable — the standard error of the sample
                // standard error is ~1/√(2n), so at n=1 000 the stopping criterion
                // itself has ≈ 2 % noise which routinely trips the threshold
                // early.
                if let Some(target) = self.config.target_ci_half_width {
                    if stats.count() >= AUTO_STOP_MIN_SAMPLES && stats.ci_half_width() < target {
                        auto_stopped = true;
                        break;
                    }
                }
            }
            combined.merge(&stats);
            if auto_stopped {
                break;
            }
        }

        let num_paths = combined.count();
        let num_simulated_paths = if self.config.antithetic {
            num_paths.saturating_mul(2)
        } else {
            num_paths
        };
        let estimate = Estimate::new(
            combined.mean(),
            combined.stderr(),
            combined.confidence_interval(0.05),
            num_paths,
        )
        .with_std_dev(combined.std_dev())
        .with_num_simulated_paths(num_simulated_paths);

        Ok((estimate, captured_paths))
    }

    /// Parallel pricing implementation with optional path capture.
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
        D: Discretization<P>,
        F: Payoff,
    {
        // Split paths into chunks for parallel processing. `None` requests
        // adaptive chunking; `Some(n)` uses exactly `n`.
        let effective_chunk_size = self
            .config
            .chunk_size
            .unwrap_or_else(|| adaptive_chunk_size(self.config.num_paths));

        let chunks = parallel_path_chunks(self.config.num_paths, effective_chunk_size);
        let captured_sink: Option<Mutex<Vec<SimulatedPath>>> =
            capture.then(|| Mutex::new(Vec::new()));
        let correlation = build_correlation_factor(process, disc)?;
        let correlation_ref = correlation.as_ref();

        // Process chunks in parallel
        let chunk_results: Vec<Result<OnlineStats>> = chunks
            .par_iter()
            .map(|range| {
                let mut stats = OnlineStats::new();
                let dim = process.dim();
                let num_factors = process.num_factors();
                let work_size = disc.work_size(process);

                let mut state = vec![0.0; dim];
                let mut z = vec![0.0; num_factors];
                let mut z_raw = vec![
                    0.0;
                    if correlation_ref.is_some() {
                        num_factors
                    } else {
                        0
                    }
                ];
                let mut work = vec![0.0; work_size];
                let mut state_a = vec![0.0; dim];
                let mut z_anti = vec![0.0; num_factors];
                let mut work_anti = vec![0.0; work_size];
                let mut chunk_paths: Vec<SimulatedPath> = if capture {
                    Vec::with_capacity(range.len() / 10 + 1)
                } else {
                    Vec::new()
                };
                // One per-thread payoff buffer, plus a lazily-allocated second
                // one reused for antithetic pairs (None when antithetic is off,
                // so non-antithetic chunks pay for one clone instead of two).
                let mut payoff_clone = payoff.clone();
                let mut payoff_anti: Option<F> =
                    self.config.antithetic.then(|| payoff.clone());

                for path_id in range.clone() {
                    let mut path_rng = rng.split(path_id as u64).ok_or_else(|| finstack_core::Error::Validation("RandomStream does not support stream splitting; use a splittable generator such as PhiloxRng or run in serial mode without per-path splitting".to_string()))?;

                    payoff_clone.reset();
                    payoff_clone.on_path_start(&mut path_rng);
                    if let Some(p) = payoff_anti.as_mut() {
                        p.reset();
                        p.on_path_start(&mut path_rng);
                    }

                    let should_capture = capture
                        && self
                            .config
                            .path_capture
                            .should_capture(path_id, self.config.num_paths);

                    let payoff_value = if should_capture {
                        let (v, path) = self.simulate_path_with_capture(
                            &mut path_rng,
                            process,
                            disc,
                            initial_state,
                            &mut payoff_clone,
                            &mut state,
                            &mut z,
                            &mut z_raw,
                            &mut work,
                            correlation_ref,
                            path_id,
                            discount_factor,
                            currency,
                        )?;
                        chunk_paths.push(path);
                        v
                    } else if let Some(payoff_anti_mut) = payoff_anti.as_mut() {
                        // antithetic = true ⇒ payoff_anti is Some by construction.
                        self.simulate_antithetic_pair(
                            &mut path_rng,
                            process,
                            disc,
                            initial_state,
                            &mut payoff_clone,
                            payoff_anti_mut,
                            &mut state,
                            &mut state_a,
                            &mut z,
                            &mut z_anti,
                            &mut z_raw,
                            &mut work,
                            &mut work_anti,
                            correlation_ref,
                            currency,
                        )?
                    } else {
                        self.simulate_path(
                            &mut path_rng,
                            process,
                            disc,
                            initial_state,
                            &mut payoff_clone,
                            &mut state,
                            &mut z,
                            &mut z_raw,
                            &mut work,
                            correlation_ref,
                            currency,
                        )?
                    };

                    let discounted_value =
                        validate_discounted_payoff(path_id, payoff_value, discount_factor)?;
                    stats.update(discounted_value);
                }

                if let Some(sink) = captured_sink.as_ref() {
                    if !chunk_paths.is_empty() {
                        // SAFETY: a poisoned mutex indicates a prior panic in
                        // another thread — propagate rather than silently
                        // continue with corrupt state.
                        #[allow(clippy::expect_used)]
                        sink.lock()
                            .expect("Mutex should not be poisoned")
                            .extend(chunk_paths);
                    }
                }

                Ok(stats)
            })
            .collect();

        // Collect and handle errors (fail-fast on first error)
        let chunk_stats: Vec<OnlineStats> =
            chunk_results.into_iter().collect::<Result<Vec<_>>>()?;

        // Deterministically reduce chunk statistics
        let mut combined = OnlineStats::new();
        for chunk_stat in chunk_stats {
            combined.merge(&chunk_stat);
        }

        let num_paths = combined.count();
        let num_simulated_paths = if self.config.antithetic {
            num_paths.saturating_mul(2)
        } else {
            num_paths
        };
        let estimate = Estimate::new(
            combined.mean(),
            combined.stderr(),
            combined.confidence_interval(0.05),
            num_paths,
        )
        .with_std_dev(combined.std_dev())
        .with_num_simulated_paths(num_simulated_paths);

        #[allow(clippy::expect_used)] // Mutex poisoning indicates prior panic in worker thread.
        let captured_paths = captured_sink
            .map(|sink| sink.into_inner().expect("Mutex should not be poisoned"))
            .unwrap_or_default();

        Ok((estimate, captured_paths))
    }
}
