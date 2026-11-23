//! Path-dependent Monte Carlo pricing engine and LRM Greeks.

use super::super::super::engine::{
    McEngine, McPricingInputs, PathCaptureConfig, PathInputs, PathScratch,
};
use super::super::super::results::{MoneyEstimate, MonteCarloResult};
use super::super::super::traits::Payoff;
use super::config::*;
use super::sobol_bridge::*;
use crate::monte_carlo::discretization::exact::ExactGbm;
use crate::monte_carlo::estimate::Estimate;
use crate::monte_carlo::greeks::lrm::{LrmGreeks, lrm_delta, lrm_vega_from_scores};
use crate::monte_carlo::payoff::asian::{Asian, AsianPair, AveragingMethod, default_fixing_steps};
use crate::monte_carlo::process::gbm::GbmProcess;
use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::monte_carlo::traits::ProcessMetadata;
use crate::monte_carlo::traits::StochasticProcess;
use crate::types::OptionMarketParams;
use finstack_quant_core::cashflow::flat_discount_factor;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::random::{MAX_SOBOL_DIMENSION, SOBOL_PERIOD, SobolRng};
use finstack_quant_core::math::stats::{OnlineCovariance, OnlineStats};
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_core::{Error, Result};

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
    pub(super) config: PathDependentPricerConfig,
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
            time_grid
                .num_steps()
                .checked_mul(num_factors)
                .ok_or_else(|| Error::Validation("Sobol path dimension overflow".into()))?
        };

        if sobol_dimension == 0 || sobol_dimension > MAX_SOBOL_DIMENSION {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Sobol dimension {} is unsupported (maximum {})",
                sobol_dimension, MAX_SOBOL_DIMENSION
            )));
        }

        let replicates = SOBOL_QMC_REPLICATES.min(self.config.num_paths);
        if replicates == 0 || self.config.num_paths.div_ceil(replicates) as u64 > SOBOL_PERIOD {
            return Err(Error::Validation(format!(
                "Sobol replicates must contain 1 through {SOBOL_PERIOD} points"
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

        // Estimate variation across replicate means. Each replicate consumes
        // a complete sequence block under its own prefix hash permutation.
        let mut replicate_means = OnlineStats::new();
        let mut overall = OnlineStats::new();
        let mut replicate_stats = OnlineStats::new();
        self.for_each_sobol_path(
            &time_grid,
            sobol_dimension,
            |path_id, last_in_replicate, adapter| {
                let discounted_value = engine.simulate_discounted(
                    adapter,
                    inputs,
                    &mut scratch,
                    path_id,
                    capture_enabled.then_some(&mut captured_paths),
                )?;
                replicate_stats.update(discounted_value)?;
                overall.update(discounted_value)?;
                if last_in_replicate {
                    if let Some(replicate_mean) = replicate_stats.get_mean()? {
                        replicate_means.update(replicate_mean)?;
                    }
                    replicate_stats = OnlineStats::new();
                }
                Ok(())
            },
        )?;

        // Point estimate across all simulated paths (`overall.get_mean()`),
        // with variation estimated across the R permuted replicate
        // means (`replicate_means.get_stderr()`). When `num_paths % replicates != 0`,
        // an unweighted mean of replicate means would overweight shorter replicates.
        // The 95% half-width is the Student-t interval over the R replicate
        // means (R - 1 degrees of freedom); `validate_runtime` guarantees
        // `num_paths >= 2`, so R >= 2 and the interval is defined.
        let mean = overall.get_mean()?.unwrap_or(f64::NAN);
        let half_width = replicate_means.get_ci_half_width(0.05)?.unwrap_or(f64::NAN);
        let stderr = replicate_means.get_stderr()?.unwrap_or(f64::NAN);
        let std_dev = overall.get_std_dev()?.unwrap_or(f64::NAN);
        let estimate = Estimate::new(
            mean,
            stderr,
            (mean - half_width, mean + half_width),
            overall.get_count(),
        )
        .with_std_dev(std_dev);

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
        sobol_dimension: usize,
        mut visit: F,
    ) -> Result<()>
    where
        F: for<'a> FnMut(usize, bool, &mut SobolPathStream<'a>) -> Result<()>,
    {
        let bridge = self
            .config
            .use_brownian_bridge
            .then(|| SobolBridge::new(time_grid));
        let mut z_path = vec![0.0; sobol_dimension];
        let mut z_increments = vec![0.0; sobol_dimension];
        let replicates = SOBOL_QMC_REPLICATES.min(self.config.num_paths);
        let base_paths = self.config.num_paths / replicates;
        let remainder = self.config.num_paths % replicates;
        let mut path_id = 0usize;
        for replicate in 0..replicates {
            // The Weyl increment assigns a distinct permutation seed to each replicate.
            let scramble_seed = (self.config.seed ^ SOBOL_SCRAMBLE_DOMAIN_SALT)
                .wrapping_add((replicate as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let mut sobol = SobolRng::new(sobol_dimension, Some(scramble_seed))?;
            let replicate_paths = base_paths + usize::from(replicate < remainder);
            for point in 0..replicate_paths {
                sobol.fill_std_normals(&mut z_path)?;
                let increments = if let Some(bridge) = &bridge {
                    bridge.fill(&z_path, &mut z_increments)?;
                    &z_increments
                } else {
                    &z_path
                };
                // Auxiliary uniforms use disjoint per-path Philox counter
                // streams; Sobol carries only the asset normals.
                let mut adapter = SobolPathStream::new(
                    increments,
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
        self.for_each_sobol_path(&time_grid, sobol_dimension, |path_id, _, adapter| {
            let (arithmetic, geometric) = engine.simulate_discounted_pair(
                adapter,
                inputs,
                &mut scratch,
                path_id,
                &AsianPair::control_value,
            )?;
            moments.update(arithmetic, geometric)?;
            Ok(())
        })?;
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
        if num_steps == 0 {
            return Err(Error::Validation(
                "price_with_lrm_greeks needs at least one transition".into(),
            ));
        }
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
        let dts = engine.config().time_grid.dts();
        let drift = rate - div_yield - 0.5 * vol * vol;

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
                let dt = dts[k];
                let sqrt_dt = dt.sqrt();
                let z = ((w[1] / w[0]).ln() - drift * dt) / (vol * sqrt_dt);
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
            dts[0],
            discount_factor,
        )?;
        let vega = lrm_vega_from_scores(&payoffs, &vega_scores, discount_factor)?;
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
