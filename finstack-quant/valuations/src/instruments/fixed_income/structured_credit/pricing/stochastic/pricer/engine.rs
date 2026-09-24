//! Stochastic structured-credit scenario waterfall pricing engine.

use super::config::{PricingMode, StochasticPricerConfig};
use super::result::{StochasticPricingResult, TranchePricingResult};
use crate::cashflow::builder::schedule::weighted_average_life_from_principal;
use crate::instruments::fixed_income::structured_credit::pricing::simulation_engine::{
    prepare_deal_simulation, simulate_prepared, InstrumentPathFlowSource, PathShocks,
    PerNameDefaultEngine, PerNamePeriodInput, PeriodPoolShock, PreparedDealSimulation,
    PreparedInstrumentSchedules, StochasticPathFlowSource,
};
use crate::instruments::fixed_income::structured_credit::types::{
    StructuredCredit, Tranche, TrancheCashflows, TrancheSeniority,
};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::stats::OnlineStats;
use finstack_quant_core::money::Money;
use finstack_quant_core::{HashMap, Result};
use finstack_quant_models::correlation::{CopulaSpec, LatentFactorSpec, RecoverySpec};
use finstack_quant_models::credit::pool::{
    MacroCreditFactors, PerNameCopulaDefault, StochasticDefault, StochasticDefaultSpec,
    StochasticPrepaySpec, StochasticPrepayment,
};
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::traits::RandomStream;
use std::sync::Arc;

#[cfg(test)]
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::calibrations::{
    clo_default_spec, rmbs_default_spec, rmbs_prepay_spec,
};

/// Seed salt for the per-name idiosyncratic-draw RNG.
///
/// Per-name copula simulation draws each name's idiosyncratic shock `εᵢ` from
/// a Philox stream seeded with `config.seed ^ PER_NAME_SEED_SALT`. XOR-salting
/// the seed places the idiosyncratic streams in a stream space fully disjoint
/// from the systematic-factor streams (which use the unsalted seed), so the
/// two never collide regardless of path count or pricing mode. The salt value
/// itself is arbitrary; only its disjointness from `0` matters.
const PER_NAME_SEED_SALT: u64 = 0x5350_4552_4E41_4D45; // "SPERNAME"

/// Salt for the INDEPENDENT component of the prepayment factor.
///
/// XOR-salting keeps this stream disjoint from the credit-factor,
/// per-name and tree-tail streams, so adding it leaves every existing draw
/// sequence bit-identical — the credit factors, and therefore all existing
/// default results, are unchanged.
const PREPAY_FACTOR_SEED_SALT: u64 = 0x5052_4550_4159_5A32; // "PREPAYZ2"

/// Seed salt for the instrument-collateral process draws.
///
/// Revolver spread and utilization shocks of instrument pools are drawn from
/// a Philox stream seeded with `config.seed ^ INSTRUMENT_PATH_SEED_SALT`,
/// disjoint from the systematic-factor, per-name and tree-tail stream
/// spaces, so adding instrument collateral never perturbs the draws of the
/// other channels.
const INSTRUMENT_PATH_SEED_SALT: u64 = 0x494E_5354_5255_4D50; // "INSTRUMP"

/// Seed salt for the tree-mode tail-month RNG.
///
/// Tree-mode paths enumerate base-`branch_count` digits of the path index for
/// the leading months; months beyond `log_branch(path_count)` carry no digit
/// information and are drawn from a Philox substream seeded with
/// `config.seed ^ TREE_TAIL_SEED_SALT` instead (see `StochasticPricer::tree_path_factors`).
/// The salt keeps these tail streams disjoint from the systematic-factor and
/// per-name stream spaces.
const TREE_TAIL_SEED_SALT: u64 = 0x5452_4545_5441_494C; // "TREETAIL"

/// Stochastic pricing engine for structured credit.
///
/// Each scenario path feeds period SMM/MDR/recovery assumptions into the same
/// waterfall simulation used by deterministic tranche valuation. PV is computed
/// from actual dated tranche payments, not from terminal expected loss shortcuts.
pub(crate) struct StochasticPricer {
    config: StochasticPricerConfig,
}

/// Loop-invariant state built once per pricing run and shared read-only
/// across all paths.
///
/// The default/prepay model builds are pure functions of immutable config
/// fields, so building them once here — instead of inside every path's month
/// loop, where 10k paths × 360 months would mean millions of identical
/// `Box<dyn>` allocations plus deep hazard-curve clones — is bit-identical:
/// every trait method takes `&self` and no model carries cross-call state
/// (path burnout lives in the caller's `&mut f64`).
pub(crate) struct PreparedRun {
    /// Default model from
    /// [`build_with_seasoning_offset`](StochasticDefaultSpec::build_with_seasoning_offset)
    /// on the run's constant `initial_seasoning`. Gates the pool-wide MDR
    /// channel and sources the copula marginal-PD plan.
    pub(crate) default_model: Option<Box<dyn StochasticDefault>>,
    /// Prepayment model from [`StochasticPrepaySpec::build`]. Gates the
    /// stochastic SMM channel.
    pub(crate) prepay_model: Option<Box<dyn StochasticPrepayment>>,
    /// `Some(rho)` iff the default spec is a copula variant, with the deal
    /// correlation override applied. Gates the per-name overlay channel —
    /// NOT `default_model.is_some()`, which is also true for non-copula
    /// stochastic specs.
    pub(crate) copula_rho: Option<f64>,
    /// Configured systematic-factor mean-reversion speed κ (per year).
    pub(crate) factor_kappa: f64,
    /// Monthly autocorrelation φ = e^{−κ/12}.
    pub(crate) factor_phi: f64,
    /// Validated prepay/default factor correlation.
    pub(crate) factor_correlation: Option<f64>,
    /// Loop-invariant deal simulation (validation, calendar, schedule,
    /// waterfall) prepared once for this run's valuation date.
    pub(crate) sim: PreparedDealSimulation,
    /// Bucketed instrument schedules when the pool holds instrument
    /// collateral; every path drives the instrument engine from them.
    pub(crate) instrument_schedules: Option<PreparedInstrumentSchedules>,
    /// Deal asset correlation loading instrument-collateral spread shocks on
    /// the systematic factor.
    pub(crate) asset_correlation: f64,
}

impl StochasticPricer {
    /// Create a new stochastic pricer.
    pub(crate) fn new(config: StochasticPricerConfig) -> Self {
        Self { config }
    }

    /// Build the loop-invariant [`PreparedRun`] for one pricing invocation.
    ///
    /// # Errors
    ///
    /// Propagates invalid default-spec construction (e.g. Student-t dof ≤ 2)
    /// and unsupported latent-factor specs, once and before any path runs.
    fn prepare_run(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
    ) -> Result<PreparedRun> {
        // Fail fast on invalid default specs and keep the built model so the
        // per-path hot loops can assume a validated, already-built spec.
        let default_model = self
            .config
            .tree_config
            .default_spec
            .build_with_seasoning_offset(self.config.tree_config.initial_seasoning)?;
        // Prepare the loop-invariant deal simulation once. A `None` here
        // (exhausted pool) yields no tranche results, which the collector
        // reports as a missing-tranche validation error; raise it here with
        // the same message.
        let sim =
            prepare_deal_simulation(instrument, self.config.valuation_date)?.ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "stochastic waterfall omitted tranche result '{}'",
                    instrument
                        .tranches
                        .tranches
                        .first()
                        .map(|t| t.id.as_str())
                        .unwrap_or_default()
                ))
            })?;
        let prepay_model = self.config.tree_config.prepay_spec.build();
        let copula_rho = self.copula_rho();
        let factor_kappa = self.factor_mean_reversion();
        let factor_correlation = self.factor_correlation()?;
        let instrument_schedules = if instrument.pool.instruments.is_some() {
            Some(PreparedInstrumentSchedules::prepare(
                instrument, context, &sim,
            )?)
        } else {
            None
        };
        let asset_correlation = match copula_rho {
            Some(rho) => rho,
            None => instrument.effective_asset_correlation()?,
        };
        Ok(PreparedRun {
            default_model,
            prepay_model,
            copula_rho,
            factor_kappa,
            factor_phi: (-factor_kappa / 12.0).exp(),
            factor_correlation,
            sim,
            instrument_schedules,
            asset_correlation,
        })
    }

    /// Price the full deal and all tranches through scenario-level waterfalls.
    pub(crate) fn price(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
    ) -> Result<StochasticPricingResult> {
        let prepared = self.prepare_run(instrument, context)?;
        match &self.config.pricing_mode {
            PricingMode::Tree => self.price_tree(instrument, context, &prepared),
            PricingMode::MonteCarlo {
                num_paths,
                antithetic,
            } => self.price_monte_carlo(instrument, context, *num_paths, *antithetic, &prepared),
            PricingMode::Hybrid {
                tree_periods,
                mc_paths,
            } => self.price_hybrid(instrument, context, *tree_periods, *mc_paths, &prepared),
        }
    }

    fn price_tree(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        prepared: &PreparedRun,
    ) -> Result<StochasticPricingResult> {
        let terminal_paths = self
            .config
            .tree_config
            .terminal_path_count(self.config.tree_config.num_periods);
        if terminal_paths > self.config.max_tree_paths {
            return Err(finstack_quant_core::Error::Validation(format!(
                "structured_credit_stochastic tree requires {terminal_paths} terminal paths, \
                 above max_tree_paths={}",
                self.config.max_tree_paths
            )));
        }

        let branch_count = self.config.tree_config.branch_count.max(1);
        let path_count = terminal_paths.max(1);
        let per_name_simulator = self.per_name_simulator()?;
        // Tree mode draws no antithetic pairs: every path is an independent
        // stratified node, so the std-error is the plain i.i.d. estimator.
        let mut collector = ScenarioCollector::new(
            instrument,
            path_count,
            false,
            self.tracks_option_cost(prepared),
        )?;
        for path_index in 0..path_count {
            let shocks =
                self.tree_path_shocks(instrument, path_index, path_count, branch_count, prepared)?;
            // Tree mode draws no antithetic pairs — each path is an
            // independent stratified node, so the per-name substream is
            // per-path and never negated.
            let per_name_engine = per_name_simulator
                .as_ref()
                .map(|sim| self.per_name_engine(sim, path_index, false));
            let output = self.price_path(
                instrument,
                context,
                prepared,
                shocks,
                per_name_engine,
                (path_index, false),
            )?;
            collector.record_output(output);
        }
        collector.finalize(self, PricingMode::Tree)
    }

    fn price_monte_carlo(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        num_paths: usize,
        antithetic: bool,
        prepared: &PreparedRun,
    ) -> Result<StochasticPricingResult> {
        if num_paths == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "Monte Carlo pricing requires at least one simulation path".to_string(),
            ));
        }

        // Antithetic pairing is only effective when `num_paths` is even — an
        // odd trailing path is drawn independently (see `monte_carlo_path_factors`)
        // and cannot be paired. Pair-aware std-error therefore requires an even
        // path count; with an odd count the antithetic flag is dropped so the
        // collector falls back to the plain i.i.d. estimator.
        self.price_factor_sets(
            instrument,
            context,
            |path_index| {
                self.monte_carlo_path_factors(instrument, path_index, num_paths, antithetic)
            },
            num_paths,
            PricingMode::MonteCarlo {
                num_paths,
                antithetic,
            },
            prepared,
        )
    }

    fn price_hybrid(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        tree_periods: usize,
        mc_paths: usize,
        prepared: &PreparedRun,
    ) -> Result<StochasticPricingResult> {
        if tree_periods == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "Hybrid pricing requires at least one tree prefix period".to_string(),
            ));
        }
        if mc_paths == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "Hybrid pricing requires at least one Monte Carlo suffix path".to_string(),
            ));
        }

        let branch_count = self.config.tree_config.branch_count.max(1);
        let prefix_count = self
            .config
            .tree_config
            .terminal_path_count(tree_periods)
            .max(1);
        let total_paths = prefix_count.checked_mul(mc_paths).ok_or_else(|| {
            finstack_quant_core::Error::Validation("Hybrid pricing path count overflow".to_string())
        })?;
        if total_paths > self.config.max_tree_paths {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Hybrid pricing requires {total_paths} paths, above max_tree_paths={}",
                self.config.max_tree_paths
            )));
        }

        let months_per_period = instrument.frequency.months().unwrap_or(1).max(1) as usize;
        let month_count = self.month_count(instrument);
        let prefix_months = tree_periods
            .saturating_mul(months_per_period)
            .min(month_count);
        let suffix_months = month_count.saturating_sub(prefix_months);
        let has_stochastic_rates = self.has_stochastic_rates();

        // Path `prefix_index * mc_paths + suffix_index` continues tree prefix
        // `prefix_index` with Monte Carlo suffix draws from its own Philox
        // substream: `Philox(seed).substream(path_id)` is statistically
        // independent for any pair of distinct path ids, so the hybrid suffix
        // factors carry no inter-path correlation.
        let hybrid_factors = |path_index: usize| {
            let prefix_index = path_index / mc_paths;
            let prefix =
                self.tree_path_factors(prefix_index, prefix_count, branch_count, prefix_months);
            let mut rng = PhiloxRng::new(self.config.tree_config.seed).substream(path_index as u64);
            let mut factors = Vec::with_capacity(prefix.len() + suffix_months);
            factors.extend_from_slice(&prefix);
            for _ in 0..suffix_months {
                factors.push(if has_stochastic_rates {
                    rng.next_std_normal()
                } else {
                    0.0
                });
            }
            factors
        };

        self.price_factor_sets(
            instrument,
            context,
            hybrid_factors,
            total_paths,
            PricingMode::Hybrid {
                tree_periods,
                mc_paths,
            },
            prepared,
        )
    }

    /// Price `total_paths` scenario paths, drawing each path's monthly
    /// factors from `path_factors(path_index)` inside the (parallel) path
    /// loop so no path's factors outlive its pricing.
    fn price_factor_sets(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        path_factors: impl Fn(usize) -> Vec<f64> + Sync,
        total_paths: usize,
        pricing_mode: PricingMode,
        prepared: &PreparedRun,
    ) -> Result<StochasticPricingResult> {
        // Antithetic pairing is only effective when the path count is even —
        // an odd trailing path is drawn independently (see
        // `monte_carlo_path_factors`) and cannot be paired. Pair-aware
        // std-error therefore requires an even path count; with an odd count
        // the antithetic flag is dropped so the collector falls back to the
        // plain i.i.d. estimator. Tree and Hybrid modes draw no pairs.
        let (num_paths, antithetic) = match &pricing_mode {
            PricingMode::MonteCarlo {
                num_paths,
                antithetic,
            } => (*num_paths, *antithetic && num_paths.is_multiple_of(2)),
            _ => (total_paths, false),
        };
        let per_name_simulator = self.per_name_simulator()?;
        // `(0..n).into_par_iter()` is an order-preserving
        // `IndexedParallelIterator`: `collect()` returns outputs in path
        // order regardless of rayon scheduling, and each path keeps a stable
        // index for its factor and idiosyncratic-draw substreams. Both
        // properties are required for bit-identical serial/parallel results
        // (the downstream Welford accumulation is order-sensitive).
        let price_factors = |path_index: usize| {
            let factors = path_factors(path_index);
            let shocks = self.path_shocks_from_factors(
                instrument,
                &factors,
                (path_index, antithetic),
                prepared,
            )?;
            let per_name_engine = per_name_simulator
                .as_ref()
                .map(|sim| self.per_name_engine(sim, path_index, antithetic));
            self.price_path(
                instrument,
                context,
                prepared,
                shocks,
                per_name_engine,
                (path_index, antithetic),
            )
        };

        #[cfg(not(target_arch = "wasm32"))]
        let outputs: Vec<PathScenarioOutput> = {
            use rayon::prelude::*;
            (0..total_paths)
                .into_par_iter()
                .map(price_factors)
                .collect::<Result<Vec<_>>>()?
        };

        #[cfg(target_arch = "wasm32")]
        let outputs: Vec<PathScenarioOutput> = (0..total_paths)
            .map(price_factors)
            .collect::<Result<Vec<_>>>()?;

        let mut collector = ScenarioCollector::new(
            instrument,
            num_paths,
            antithetic,
            self.tracks_option_cost(prepared),
        )?;
        for output in outputs {
            collector.record_output(output);
        }

        collector.finalize(self, pricing_mode)
    }

    /// Monthly systematic factors of Monte Carlo path `path_index`.
    ///
    /// One base Philox RNG seeded from `config.seed`; each path draws from a
    /// counter-based substream, so a path's factors do not depend on which
    /// other paths run or in what order. With `antithetic`, path `2k + 1` is
    /// the negation of path `2k`: both members share `substream(k)`, so the
    /// pair is perfectly correlated while pairs stay independent. A trailing
    /// unpaired path (odd `num_paths`) and every non-antithetic path draw from
    /// `substream(path_index)`.
    fn monte_carlo_path_factors(
        &self,
        instrument: &StructuredCredit,
        path_index: usize,
        num_paths: usize,
        antithetic: bool,
    ) -> Vec<f64> {
        let base_rng = PhiloxRng::new(self.config.tree_config.seed);
        let paired = antithetic && (path_index % 2 == 1 || path_index + 1 < num_paths);
        if paired {
            let mut rng = base_rng.substream((path_index / 2) as u64);
            let mut factors = self.random_factors(instrument, &mut rng);
            if path_index % 2 == 1 {
                factors.iter_mut().for_each(|z| *z = -*z);
            }
            factors
        } else {
            let mut rng = base_rng.substream(path_index as u64);
            self.random_factors(instrument, &mut rng)
        }
    }

    /// Extract the copula specification and asset correlation when the
    /// scenario default model is a copula.
    ///
    /// Per-name simulation only applies to copula default models; other
    /// stochastic default models (factor-correlated, intensity-process,
    /// hazard-curve) keep the pool-wide MDR path.
    fn copula_default(&self) -> Option<(CopulaSpec, f64)> {
        match &self.config.tree_config.default_spec {
            StochasticDefaultSpec::Copula { copula_spec, .. } => {
                Some((copula_spec.clone(), self.copula_rho()?))
            }
            _ => None,
        }
    }

    /// Asset correlation of a copula default model: the explicit deal
    /// `CorrelationStructure` overrides the copula spec's scalar; `None` for
    /// non-copula default models.
    fn copula_rho(&self) -> Option<f64> {
        match &self.config.tree_config.default_spec {
            StochasticDefaultSpec::Copula { correlation, .. } => Some(
                self.config
                    .tree_config
                    .asset_correlation_override
                    .unwrap_or(*correlation),
            ),
            _ => None,
        }
    }

    /// Build the per-name copula default simulator, if the scenario uses a
    /// copula default model. Shared (cheap `Arc` clone) across all paths.
    ///
    /// # Errors
    ///
    /// Propagates copula construction failures (no silent Gaussian fallback).
    fn per_name_simulator(&self) -> Result<Option<Arc<PerNameCopulaDefault>>> {
        self.copula_default()
            .map(|(spec, correlation)| Ok(Arc::new(PerNameCopulaDefault::new(&spec, correlation)?)))
            .transpose()
    }

    /// Construct the per-path per-name default engine.
    ///
    /// Each path draws its idiosyncratic `εᵢ` shocks from a Philox substream
    /// seeded with the salted seed, so the idiosyncratic stream space is
    /// disjoint from the systematic-factor streams.
    ///
    /// # Antithetic pairing (item 5)
    ///
    /// When `antithetic` is `true` the paths were generated as antithetic
    /// pairs `(2k, 2k+1)` with `monte_carlo_path_factors` negating the
    /// systematic factors of `2k+1`. For the variance reduction to actually
    /// work, the per-name idiosyncratic channel must be paired too: both
    /// members of pair `k` draw from the SAME substream `substream(k)`, and
    /// the second member (`2k+1`) negates every `εᵢ`. The previous engine
    /// gave each path its own independent substream `substream(path_index)`,
    /// so paired paths had *uncorrelated* idiosyncratic shocks — the
    /// antithetic cancellation was lost on the per-name channel and the
    /// reported MC confidence interval was too narrow.
    fn per_name_engine(
        &self,
        simulator: &Arc<PerNameCopulaDefault>,
        path_index: usize,
        antithetic: bool,
    ) -> PerNameDefaultEngine {
        let base = PhiloxRng::new(self.config.tree_config.seed ^ PER_NAME_SEED_SALT);
        let idio_recovery_vol = self.idiosyncratic_recovery_vol();
        if antithetic {
            // Both members of pair k share substream(k); the odd member is
            // the antithetic partner (negates idiosyncratic draws).
            let pair_index = (path_index / 2) as u64;
            let rng = base.substream(pair_index);
            if path_index % 2 == 1 {
                PerNameDefaultEngine::new_antithetic(
                    Arc::clone(simulator),
                    self.config.pool_granularity,
                    rng,
                    idio_recovery_vol,
                )
            } else {
                PerNameDefaultEngine::new(
                    Arc::clone(simulator),
                    self.config.pool_granularity,
                    rng,
                    idio_recovery_vol,
                )
            }
        } else {
            let rng = base.substream(path_index as u64);
            PerNameDefaultEngine::new(
                Arc::clone(simulator),
                self.config.pool_granularity,
                rng,
                idio_recovery_vol,
            )
        }
    }

    /// Whether the run values revolver draws against their fair spread.
    fn tracks_option_cost(&self, prepared: &PreparedRun) -> bool {
        prepared
            .instrument_schedules
            .as_ref()
            .is_some_and(PreparedInstrumentSchedules::has_draw_option_cost)
    }

    /// Path-local stream for the instrument-collateral process draws.
    ///
    /// Mirrors the per-name pairing: antithetic pairs `(2k, 2k+1)` share
    /// `substream(k)` and the odd member negates its draws; independent
    /// paths use `substream(path_index)`.
    fn instrument_path_rng(&self, path_index: usize, antithetic: bool) -> (PhiloxRng, bool) {
        let base = PhiloxRng::new(self.config.tree_config.seed ^ INSTRUMENT_PATH_SEED_SALT);
        if antithetic {
            (base.substream((path_index / 2) as u64), path_index % 2 == 1)
        } else {
            (base.substream(path_index as u64), false)
        }
    }

    fn price_path(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        prepared: &PreparedRun,
        shocks: Vec<PeriodPoolShock>,
        per_name_engine: Option<PerNameDefaultEngine>,
        (path_index, antithetic): (usize, bool),
    ) -> Result<PathScenarioOutput> {
        // Counterfactual tranche cashflows of the same path with revolver
        // draws accruing at their fair spread; `None` without stochastic
        // revolvers.
        let mut counterfactual: Option<HashMap<String, TrancheCashflows>> = None;
        let run = match prepared.instrument_schedules.as_ref() {
            Some(schedules) => {
                let (rng, negate) = self.instrument_path_rng(path_index, antithetic);
                let replay = schedules
                    .has_draw_option_cost()
                    .then(|| (shocks.clone(), per_name_engine.clone(), rng.clone()));
                let mut source = InstrumentPathFlowSource::new(
                    schedules,
                    PathShocks::new(shocks),
                    per_name_engine,
                    prepared.asset_correlation,
                    rng,
                    negate,
                );
                let run = simulate_prepared(instrument, context, &prepared.sim, &mut source)?;
                if let Some((shocks, per_name_engine, rng)) = replay {
                    // Same streams, same shocks: only the draw interest differs.
                    let mut replayed = InstrumentPathFlowSource::new(
                        schedules,
                        PathShocks::new(shocks),
                        per_name_engine,
                        prepared.asset_correlation,
                        rng,
                        negate,
                    )
                    .with_counterfactual(source.take_draw_records());
                    counterfactual = Some(
                        simulate_prepared(instrument, context, &prepared.sim, &mut replayed)?
                            .tranches,
                    );
                }
                run
            }
            None => {
                let mut source = match per_name_engine {
                    Some(engine) => StochasticPathFlowSource::with_per_name(shocks, engine),
                    None => StochasticPathFlowSource::new(shocks),
                };
                simulate_prepared(instrument, context, &prepared.sim, &mut source)?
            }
        };
        let path_results = run.tranches;
        let unfunded_draws = run.diagnostics.unfunded_draws.amount() > 0.0;
        let collateral_draws = run.diagnostics.draws_from_reserve.amount()
            + run.diagnostics.draws_from_principal.amount();

        let mut deal_pv = 0.0;
        let mut deal_loss = 0.0;
        let mut draw_option_cost = 0.0;
        let mut tranches = Vec::with_capacity(instrument.tranches.tranches.len());
        for (idx, tranche) in instrument.tranches.tranches.iter().enumerate() {
            let tranche_result = path_results.get(tranche.id.as_str()).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "stochastic waterfall omitted tranche result '{}'",
                    tranche.id
                ))
            })?;
            let mut metrics = PathTrancheMetrics::from_cashflows(
                tranche_result,
                self.config.valuation_date,
                &self.config.discount_curve,
            )?;
            if let Some(fair) = counterfactual
                .as_ref()
                .and_then(|results| results.get(tranche.id.as_str()))
            {
                let fair_pv = PathTrancheMetrics::from_cashflows(
                    fair,
                    self.config.valuation_date,
                    &self.config.discount_curve,
                )?
                .pv;
                metrics.option_cost = metrics.pv - fair_pv;
            }
            deal_pv += metrics.pv;
            deal_loss += metrics.loss;
            draw_option_cost += metrics.option_cost;
            tranches.push((idx, metrics));
        }
        Ok(PathScenarioOutput {
            deal_pv,
            deal_loss,
            tranches,
            unfunded_draws,
            collateral_draws,
            draw_option_cost,
        })
    }

    fn random_factors(&self, instrument: &StructuredCredit, rng: &mut PhiloxRng) -> Vec<f64> {
        let month_count = self.month_count(instrument);
        if !self.has_stochastic_rates() {
            return vec![0.0; month_count];
        }
        (0..month_count).map(|_| rng.next_std_normal()).collect()
    }

    fn tree_path_shocks(
        &self,
        instrument: &StructuredCredit,
        path_index: usize,
        path_count: usize,
        branch_count: usize,
        prepared: &PreparedRun,
    ) -> Result<Vec<PeriodPoolShock>> {
        let month_count = self.month_count(instrument);
        let factors = self.tree_path_factors(path_index, path_count, branch_count, month_count);
        self.path_shocks_from_factors(instrument, &factors, (path_index, false), prepared)
    }

    fn tree_path_factors(
        &self,
        mut path_index: usize,
        path_count: usize,
        branch_count: usize,
        month_count: usize,
    ) -> Vec<f64> {
        let path_count = path_count.max(1);
        let branch_count = branch_count.max(1);
        let original_path_index = path_index;
        let mut factors = Vec::with_capacity(month_count);

        // Number of leading months the base-`branch_count` digits of the path
        // index can actually resolve: the largest `m` with
        // `branch_count^m <= path_count`. Trailing months beyond that are drawn
        // from a per-path Philox substream so the tail continues to diffuse.
        let mut resolved_months = 0usize;
        let mut capacity = 1usize;
        while resolved_months < month_count {
            match capacity.checked_mul(branch_count) {
                Some(next) if next <= path_count => {
                    capacity = next;
                    resolved_months += 1;
                }
                _ => break,
            }
        }
        let mut tail_rng =
            (resolved_months < month_count && self.has_stochastic_rates()).then(|| {
                PhiloxRng::new(self.config.tree_config.seed ^ TREE_TAIL_SEED_SALT)
                    .substream(original_path_index as u64)
            });

        for month in 0..month_count {
            let z = if !self.has_stochastic_rates() {
                0.0
            } else if month < resolved_months {
                let branch = path_index % branch_count;
                path_index /= branch_count;
                let p = (branch as f64 + 0.5) / branch_count as f64;
                finstack_quant_core::math::standard_normal_inv_cdf(p)
            } else if let Some(rng) = tail_rng.as_mut() {
                rng.next_std_normal()
            } else {
                0.0
            };
            factors.push(z);
        }
        factors
    }

    /// Configured systematic-factor mean-reversion speed κ (per year).
    ///
    /// For [`LatentFactorSpec::SingleFactor`], κ is taken from the factor spec
    /// (clamped to non-negative). Other factor specs default to `0.0`.
    ///
    /// Semantics match the OU / random-walk convention used elsewhere in this
    /// module and in `factor_model.rs`:
    ///
    /// * `κ = 0` → φ = 1 → one systematic draw held across the horizon
    ///   (canonical single-factor copula; Li 2000; Vasicek 2002)
    /// * `κ → ∞` → φ → 0 → independent monthly factors
    ///
    /// Pair with [`Self::period_factor_scale`]: a persistent factor aggregated
    /// without renormalization would leave the period factor with variance
    /// other than 1 and de-calibrate the copula barrier `Φ⁻¹(PD)`.
    fn factor_mean_reversion(&self) -> f64 {
        match &self.config.tree_config.factor_spec {
            LatentFactorSpec::SingleFactor { mean_reversion, .. } => mean_reversion.max(0.0),
            _ => 0.0,
        }
    }

    /// Configured prepay/default factor correlation.
    ///
    /// `Some(rho)` for a two-factor spec, **including `rho == 0`**: zero means
    /// the prepayment factor is independent of credit, which is a different
    /// model from sharing one factor. `None` is returned only for
    /// [`LatentFactorSpec::SingleFactor`], where prepayment and credit are
    /// driven by the same factor (implied correlation +1).
    ///
    /// # Errors
    ///
    /// [`LatentFactorSpec::MultiFactor`] is not supported by this lattice.
    fn factor_correlation(&self) -> finstack_quant_core::Result<Option<f64>> {
        match &self.config.tree_config.factor_spec {
            LatentFactorSpec::SingleFactor { .. } => Ok(None),
            LatentFactorSpec::TwoFactor { correlation, .. } => {
                Ok(Some(correlation.clamp(-1.0, 1.0)))
            }
            other => Err(finstack_quant_core::Error::Validation(format!(
                "structured-credit stochastic pricer supports SingleFactor and TwoFactor \
                 latent specs; got {other:?}"
            ))),
        }
    }

    /// Monthly factor autocorrelation `φ = e^{−κ/12}`.
    #[cfg(test)]
    fn factor_phi(&self) -> f64 {
        (-self.factor_mean_reversion() / 12.0).exp()
    }

    /// Normalizing divisor that keeps the aggregated period factor `N(0,1)`.
    ///
    /// The period-representative systematic factor is `Z_period = (Σ Zₘ)/s`.
    /// The copula barrier `c = Φ⁻¹(PDₜ)` assumes a standard-normal factor, so
    /// `s` must equal `sqrt(Var(Σ Zₘ))`.
    ///
    /// For the stationary AR(1) path from [`Self::evolved_factors`], marginal
    /// variance is 1 and autocorrelation at lag `h` is `φ^h`:
    ///
    /// ```text
    /// Var(Σ_{m=1..M} Zₘ) = Σ_i Σ_j φ^{|i−j|} = M + 2·Σ_{k=1..M−1} (M−k)·φ^k
    /// ```
    ///
    /// Limits: `φ = 0` ⇒ `s = √M` (independent months); `φ = 1` ⇒ `s = M`
    /// (`Z_period = Z₁`, single-draw canonical copula).
    fn period_factor_scale(months: usize, phi: f64) -> f64 {
        let m = months as f64;
        if months <= 1 {
            return 1.0;
        }
        let mut variance = m;
        let mut phi_k = 1.0;
        for k in 1..months {
            phi_k *= phi;
            variance += 2.0 * (m - k as f64) * phi_k;
        }
        variance.max(f64::MIN_POSITIVE).sqrt()
    }

    /// Evolve monthly factor innovations into a stationary AR(1)/OU path.
    ///
    /// The raw draws `ε_m ~ N(0,1)` are treated as innovations of the exact
    /// OU discretization with monthly step `Δt = 1/12`:
    ///
    /// ```text
    /// Z_1 = ε_1,    Z_m = φ·Z_{m−1} + √(1−φ²)·ε_m,    φ = e^{−κΔt}
    /// ```
    ///
    /// Each `Z_m` keeps the stationary `N(0,1)` marginal, so the conditional
    /// MDR/SMM models and the copula barriers `Φ⁻¹(PD)` stay correctly
    /// calibrated. The factor autocorrelation at lag `h` months is
    /// `φ^h = e^{−κh/12}`; the effective correlation half-life is
    /// `12·ln 2 / κ` months (e.g. κ = 0.5 → ≈ 16.6 months). κ → ∞ recovers
    /// the previous i.i.d.-per-month behavior.
    ///
    /// The transform is linear in the innovations, so antithetic negation of
    /// the raw draws negates the whole evolved path and the variance
    /// reduction is preserved.
    fn evolved_factors(innovations: &[f64], kappa: f64) -> Vec<f64> {
        let phi = (-kappa / 12.0).exp();
        let innovation_scale = (1.0 - phi * phi).max(0.0).sqrt();
        let mut evolved = Vec::with_capacity(innovations.len());
        let mut state = 0.0;
        for (month, eps) in innovations.iter().enumerate() {
            state = if month == 0 {
                *eps
            } else {
                phi * state + innovation_scale * eps
            };
            evolved.push(state);
        }
        evolved
    }

    /// Evolved prepayment factor series of one path, correlated with the
    /// evolved `credit_factors` at the configured level; `None` when the
    /// factor spec is single-factor (prepayment then shares the credit
    /// factor).
    ///
    /// A SECOND factor for prepayment. Handing ONE scalar to both
    /// `conditional_smm` and `conditional_mdr` would drive prepayment and
    /// default off the same realization, forcing their implied correlation to
    /// +1 (or -1 through a negative loading) regardless of the -0.30
    /// configured in the shipped RMBS/CLO calibrations. Construction is the
    /// standard two-factor decomposition
    ///     Z_prepay = rho * Z_credit + sqrt(1 - rho^2) * Z_indep
    /// which gives `Z_prepay` a unit-variance standard-normal marginal and
    /// exactly `rho` correlation with the credit factor.
    ///
    /// `Z_indep` comes from a SALTED stream so the credit draws are
    /// untouched. Antithetic pairs `(2k, 2k+1)` share `substream(k)` and the
    /// odd member negates its draws, mirroring the credit factors (negated by
    /// `monte_carlo_path_factors`) and the per-name streams, so the pair's
    /// prepayment factors are exact negatives; independent paths use
    /// `substream(path_index)`.
    ///
    /// # Arguments
    ///
    /// * `credit_factors` - The path's evolved monthly credit factors.
    /// * `path_index` - Path number within the run.
    /// * `antithetic` - Whether paths are drawn as antithetic pairs.
    /// * `prepared` - Run state carrying the factor correlation and κ.
    fn prepay_factors(
        &self,
        credit_factors: &[f64],
        (path_index, antithetic): (usize, bool),
        prepared: &PreparedRun,
    ) -> Option<Vec<f64>> {
        let rho = prepared.factor_correlation?;
        let base = PhiloxRng::new(self.config.tree_config.seed ^ PREPAY_FACTOR_SEED_SALT);
        let (mut rng, negate) = if antithetic {
            (base.substream((path_index / 2) as u64), path_index % 2 == 1)
        } else {
            (base.substream(path_index as u64), false)
        };
        let independent: Vec<f64> = (0..credit_factors.len())
            .map(|_| {
                let draw = rng.next_std_normal();
                if negate {
                    -draw
                } else {
                    draw
                }
            })
            .collect();
        let evolved_independent = Self::evolved_factors(&independent, prepared.factor_kappa);
        let scale = (1.0 - rho * rho).max(0.0).sqrt();
        Some(
            credit_factors
                .iter()
                .zip(evolved_independent.iter())
                .map(|(zc, zi)| rho * zc + scale * zi)
                .collect(),
        )
    }

    fn path_shocks_from_factors(
        &self,
        instrument: &StructuredCredit,
        factors: &[f64],
        path: (usize, bool),
        prepared: &PreparedRun,
    ) -> Result<Vec<PeriodPoolShock>> {
        // AR(1)/OU persistence for MC, tree, and hybrid paths: monthly draws
        // are innovations. Applied unconditionally — persistence is a property
        // of the factor, not of which channels are simulated.
        let evolved_storage = Self::evolved_factors(factors, prepared.factor_kappa);
        let credit_factors: &[f64] = &evolved_storage;
        let prepay_storage = self.prepay_factors(credit_factors, path, prepared);
        // SingleFactor: prepayment shares the credit factor, i.e. implied
        // correlation +1.
        let prepay_factors: &[f64] = prepay_storage.as_deref().unwrap_or(credit_factors);
        let factors: &[f64] = credit_factors;
        let months_per_period = instrument.frequency.months().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "Structured credit stochastic pricing requires month-based payment frequencies"
                    .to_string(),
            )
        })? as usize;
        let months_per_period = months_per_period.max(1);
        let payment_periods = self.payment_period_count(instrument);
        // Burnout is PATH state — it accumulates across the whole
        // path, so it is seeded once here and advanced month by month.
        let mut burnout = 1.0_f64;
        let mut shocks = Vec::with_capacity(payment_periods);

        for period in 0..payment_periods {
            let start = period * months_per_period;
            let end = (start + months_per_period).min(factors.len());
            let month_slice = if start < end {
                &factors[start..end]
            } else {
                &[][..]
            };
            // Prepayment reads its OWN factor slice, correlated with
            // the credit one at the configured rho.
            let prepay_slice = if start < end {
                &prepay_factors[start..end]
            } else {
                &[][..]
            };
            let mut shock = self.aggregate_monthly_shocks(
                prepared,
                start as u32,
                month_slice,
                prepay_slice,
                &mut burnout,
            );
            shock.systematic_z = self.period_systematic_z(prepared, month_slice);
            shock.per_name = self.copula_period_input(prepared, start as u32, month_slice);
            shocks.push(shock);
        }

        Ok(shocks)
    }

    /// Build the per-name copula plan for one payment period.
    ///
    /// Returns the period systematic factor `Z` and the *unconditional*
    /// period marginal default probability `PDₜ`. The period systematic
    /// factor aggregates all `M` of the period's monthly `N(0,1)` factors as
    /// `(Σ Zₘ)/√M`, the period-representative systematic shock — itself
    /// `N(0,1)` — so the copula channel conditions on the same months the
    /// LHP/MDR channel integrates (item 10). A monthly-pay deal recovers a
    /// single fresh `N(0,1)` per period. The marginal PD compounds the
    /// model's unconditional monthly MDR (`expected_mdr`) over the period's
    /// months.
    fn copula_period_input(
        &self,
        prepared: &PreparedRun,
        start_month: u32,
        factors: &[f64],
    ) -> Option<PerNamePeriodInput> {
        // Per-name simulation applies only to copula default models; other
        // stochastic specs keep the pool-wide MDR path even though they also
        // carry a built default model.
        prepared.copula_rho?;
        let model = prepared.default_model.as_deref()?;
        let months = factors.len().max(1) as u32;

        // Unconditional period marginal PD: 1 − ∏(1 − monthly_unconditional_MDR).
        let mut survival = 1.0;
        for offset in 0..months {
            let seasoning = self
                .config
                .tree_config
                .initial_seasoning
                .saturating_add(start_month)
                .saturating_add(offset + 1);
            survival *= 1.0 - model.expected_mdr(seasoning).clamp(0.0, 1.0);
        }
        let marginal_pd = (1.0 - survival).clamp(0.0, 1.0);

        Some(PerNamePeriodInput {
            systematic_z: self.period_systematic_z(prepared, factors),
            marginal_pd,
        })
    }

    /// Period systematic factor (item 10).
    ///
    /// Multi-month periods use `Z_period = (Σ Zₘ)/s` so the copula
    /// conditions on the whole period (matching LHP/MDR aggregation).
    /// `s = period_factor_scale` keeps `Z_period ~ N(0,1)` under the AR(1)
    /// autocorrelation `φ`; `√M` is correct only for independent months.
    /// Zero without stochastic credit.
    fn period_systematic_z(&self, prepared: &PreparedRun, factors: &[f64]) -> f64 {
        if self.has_stochastic_rates() && !factors.is_empty() {
            let sum: f64 = factors.iter().sum();
            sum / Self::period_factor_scale(factors.len(), prepared.factor_phi)
        } else {
            0.0
        }
    }

    fn aggregate_monthly_shocks(
        &self,
        prepared: &PreparedRun,
        start_month: u32,
        factors: &[f64],
        prepay_factors: &[f64],
        burnout: &mut f64,
    ) -> PeriodPoolShock {
        if factors.is_empty() {
            return self.monthly_shock(prepared, start_month.saturating_add(1), 0.0, 0.0, burnout);
        }

        let mut prepay_survival = 1.0;
        let mut default_survival = 1.0;
        let mut recovery_sum = 0.0;
        for (offset, factor) in factors.iter().enumerate() {
            let prepay_factor = prepay_factors.get(offset).copied().unwrap_or(*factor);
            let shock = self.monthly_shock(
                prepared,
                start_month.saturating_add(offset as u32 + 1),
                *factor,
                prepay_factor,
                burnout,
            );
            prepay_survival *= 1.0 - shock.smm;
            default_survival *= 1.0 - shock.mdr;
            recovery_sum += shock.recovery_rate;
        }

        let months = factors.len() as f64;
        PeriodPoolShock::pool_wide(
            1.0 - prepay_survival.powf(1.0 / months),
            1.0 - default_survival.powf(1.0 / months),
            recovery_sum / months,
        )
    }

    fn monthly_shock(
        &self,
        prepared: &PreparedRun,
        month_offset: u32,
        z: f64,
        z_prepay: f64,
        burnout: &mut f64,
    ) -> PeriodPoolShock {
        let stochastic = self.has_stochastic_rates();
        let factor = if stochastic { z } else { 0.0 };
        // Prepayment reads its own factor. With no configured
        // correlation the caller passes the credit factor through, so this is
        // exact identity with the previous single-factor behaviour.
        let prepay_factor = if stochastic { z_prepay } else { 0.0 };
        let seasoning = self
            .config
            .tree_config
            .initial_seasoning
            .saturating_add(month_offset);
        let credit_factors = [factor];
        let prepay_factors = [prepay_factor];

        PeriodPoolShock::pool_wide(
            self.conditional_smm(prepared, seasoning, &prepay_factors, burnout),
            self.conditional_mdr(prepared, seasoning, &credit_factors),
            // Recovery is a CREDIT quantity and stays on the credit factor, so
            // defaults and recoveries continue to co-move as the sign
            // convention requires.
            self.recovery_rate(factor),
        )
    }

    /// Conditional SMM for one month, advancing the path's burnout state.
    ///
    /// Hard-coding `burnout = 1.0` here (or never calling
    /// `update_burnout`) leaves the Richard-Roll burnout channel inert:
    /// seasoned pools that have already refinanced heavily get modelled with
    /// full prepayment propensity while `has_burnout()` still returns true.
    ///
    /// Burnout is path-state: it accumulates as realized prepayment runs above
    /// or below expectation, so it must be threaded through the month loop
    /// rather than recomputed. `expected_smm` is the unconditional speed at
    /// this seasoning, and the realized/expected ratio is what drives the
    /// update (fast prepayers leave the pool; slow ones rejuvenate it).
    fn conditional_smm(
        &self,
        prepared: &PreparedRun,
        seasoning: u32,
        factors: &[f64],
        burnout: &mut f64,
    ) -> f64 {
        if let Some(model) = prepared.prepay_model.as_deref() {
            let realized = model
                .conditional_smm(
                    seasoning,
                    factors,
                    self.config.tree_config.market_refi_rate,
                    *burnout,
                )
                .clamp(0.0, 0.50);
            if model.has_burnout() {
                let expected = model.expected_smm(seasoning);
                *burnout = model.update_burnout(*burnout, realized, expected);
            }
            return realized;
        }
        match &self.config.tree_config.prepay_spec {
            StochasticPrepaySpec::Deterministic(spec) => {
                spec.smm(seasoning).unwrap_or(0.0).clamp(0.0, 0.50)
            }
            _ => self
                .config
                .tree_config
                .prepay_spec
                .base_smm()
                .clamp(0.0, 0.50),
        }
    }

    fn conditional_mdr(&self, prepared: &PreparedRun, seasoning: u32, factors: &[f64]) -> f64 {
        if let Some(model) = prepared.default_model.as_deref() {
            return model
                .conditional_mdr(seasoning, factors, &MacroCreditFactors::default())
                .clamp(0.0, 0.50);
        }
        match &self.config.tree_config.default_spec {
            StochasticDefaultSpec::Deterministic(spec) => {
                spec.mdr(seasoning).unwrap_or(0.0).clamp(0.0, 0.50)
            }
            _ => self
                .config
                .tree_config
                .default_spec
                .base_mdr()
                .clamp(0.0, 0.50),
        }
    }

    fn recovery_rate(&self, factor: f64) -> f64 {
        match &self.config.tree_config.recovery_spec {
            RecoverySpec::Constant { rate } => *rate,
            RecoverySpec::MarketCorrelated {
                mean_recovery,
                recovery_volatility,
                factor_correlation,
            } => {
                (mean_recovery + factor_correlation * recovery_volatility * factor).clamp(0.0, 1.0)
            }
        }
    }

    /// Idiosyncratic (name-specific) recovery volatility for the per-name
    /// engine.
    ///
    /// The recovery volatility `σ_R` of the market-correlated recovery model
    /// splits into a systematic loading and an idiosyncratic residual.
    /// [`Self::recovery_rate`] already applies the systematic `ρ_R·σ_R·Z`
    /// channel (shared by every name in a period); this returns the residual
    /// `σ_R·√(1−ρ_R²)` that the per-name engine scatters independently across
    /// defaulted obligors. `Constant` recovery has no dispersion.
    fn idiosyncratic_recovery_vol(&self) -> f64 {
        match &self.config.tree_config.recovery_spec {
            RecoverySpec::Constant { .. } => 0.0,
            RecoverySpec::MarketCorrelated {
                recovery_volatility,
                factor_correlation,
                ..
            } => {
                let systematic_share = (factor_correlation * factor_correlation).min(1.0);
                recovery_volatility * (1.0 - systematic_share).max(0.0).sqrt()
            }
        }
    }

    fn has_stochastic_rates(&self) -> bool {
        self.config.tree_config.prepay_spec.is_stochastic()
            || self.config.tree_config.default_spec.is_stochastic()
            || matches!(
                self.config.tree_config.recovery_spec,
                RecoverySpec::MarketCorrelated { .. }
            )
    }

    fn month_count(&self, instrument: &StructuredCredit) -> usize {
        let periods = self.payment_period_count(instrument);
        let months_per_period = instrument.frequency.months().unwrap_or(1).max(1) as usize;
        periods.saturating_mul(months_per_period).max(1)
    }

    fn payment_period_count(&self, instrument: &StructuredCredit) -> usize {
        let months_per_period = instrument.frequency.months().unwrap_or(1).max(1) as usize;
        let base_periods = self
            .config
            .tree_config
            .num_periods
            .saturating_add(months_per_period - 1)
            / months_per_period;
        base_periods.saturating_add(2).max(1)
    }
}

struct PathScenarioOutput {
    deal_pv: f64,
    deal_loss: f64,
    tranches: Vec<(usize, PathTrancheMetrics)>,
    /// A collateral draw could not be funded somewhere on the path.
    unfunded_draws: bool,
    /// Collateral draws funded on the path.
    collateral_draws: f64,
    /// Draw option cost of the path (sum of the tranche shares).
    draw_option_cost: f64,
}

#[derive(Clone, Copy, Default)]
struct PathTrancheMetrics {
    pv: f64,
    loss: f64,
    wal: f64,
    /// Principal the tranche received on the path after the valuation date.
    principal: f64,
    duration: f64,
    /// Present value less the counterfactual (fair draw interest) present value.
    option_cost: f64,
}

impl PathTrancheMetrics {
    fn from_cashflows(
        cashflows: &crate::instruments::fixed_income::structured_credit::TrancheCashflows,
        as_of: Date,
        discount_curve: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    ) -> Result<Self> {
        let mut pv = 0.0;
        let mut positive_pv = 0.0;
        let mut weighted_duration = 0.0;
        for (date, amount) in &cashflows.cashflows {
            if *date <= as_of {
                continue;
            }
            let df = discount_curve.df_between_dates(as_of, *date)?;
            let flow_pv = amount.amount() * df;
            pv += flow_pv;
            if flow_pv > 0.0 {
                let t =
                    DayCount::Act365F.year_fraction(as_of, *date, DayCountContext::default())?;
                positive_pv += flow_pv;
                weighted_duration += flow_pv * t;
            }
        }

        let wal =
            weighted_average_life_from_principal(cashflows.principal_flows.iter().copied(), as_of)?;
        let principal: f64 = cashflows
            .principal_flows
            .iter()
            .filter(|(date, _)| *date > as_of)
            .map(|(_, amount)| amount.amount())
            .sum();

        Ok(Self {
            pv,
            loss: cashflows.total_writedown.amount(),
            wal,
            principal,
            duration: if positive_pv > f64::EPSILON {
                weighted_duration / positive_pv
            } else {
                0.0
            },
            option_cost: 0.0,
        })
    }
}

struct TrancheScenarioStats {
    tranche_id: String,
    seniority: TrancheSeniority,
    attachment: f64,
    detachment: f64,
    /// Current note balance at the valuation date: the face the tranche
    /// price is quoted on.
    current_balance: f64,
    pv_stats: OnlineStats,
    loss_stats: OnlineStats,
    losses: Vec<f64>,
    /// Sum of path WALs over the paths that returned principal.
    wal_sum: f64,
    /// Paths on which the tranche received any principal: a wiped-out path
    /// has no WAL and must not pull the average toward zero.
    paths_with_principal: usize,
    duration_sum: f64,
    option_cost_stats: OnlineStats,
}

impl TrancheScenarioStats {
    fn new(tranche: &Tranche, num_paths: usize) -> Self {
        Self {
            tranche_id: tranche.id.to_string(),
            seniority: tranche.seniority,
            attachment: tranche.attachment_pct() / 100.0,
            detachment: tranche.detachment_pct() / 100.0,
            current_balance: tranche.current_balance.amount(),
            pv_stats: OnlineStats::new(),
            loss_stats: OnlineStats::new(),
            losses: Vec::with_capacity(num_paths),
            wal_sum: 0.0,
            paths_with_principal: 0,
            duration_sum: 0.0,
            option_cost_stats: OnlineStats::new(),
        }
    }

    fn record(&mut self, metrics: PathTrancheMetrics) {
        self.pv_stats.update(metrics.pv);
        self.loss_stats.update(metrics.loss);
        self.losses.push(metrics.loss);
        if metrics.principal > 0.0 {
            self.wal_sum += metrics.wal;
            self.paths_with_principal += 1;
        }
        self.duration_sum += metrics.duration;
        self.option_cost_stats.update(metrics.option_cost);
    }

    fn finalize(
        mut self,
        currency: finstack_quant_core::currency::Currency,
        num_paths: usize,
        es_confidence: f64,
    ) -> Result<TranchePricingResult> {
        let paths = num_paths.max(1) as f64;
        let mean_pv = self.pv_stats.mean();
        let mean_loss = self.loss_stats.mean();
        // Use population variance (n denominator), the established convention
        // for these Monte Carlo loss estimators.
        let loss_std = self.loss_stats.population_variance().sqrt();
        let es = expected_shortfall(&mut self.losses, es_confidence);

        let price_pct = if self.current_balance > f64::EPSILON {
            mean_pv / self.current_balance * 100.0
        } else {
            0.0
        };

        Ok(TranchePricingResult::new(
            self.tranche_id,
            self.seniority,
            Money::new(mean_pv, currency)?,
        )
        .with_price_pct(price_pct)
        .with_subordination(self.attachment, self.detachment)
        .with_risk_metrics(
            Money::new(mean_loss, currency)?,
            Money::new(loss_std, currency)?,
            Money::new(es, currency)?,
        )
        .with_average_life(if self.paths_with_principal > 0 {
            self.wal_sum / self.paths_with_principal as f64
        } else {
            0.0
        })
        .with_paths_with_principal(self.paths_with_principal)
        .with_credit_duration(self.duration_sum / paths)
        .with_draw_option_cost(Money::new(self.option_cost_stats.mean(), currency)?))
    }
}

struct ScenarioCollector {
    currency: finstack_quant_core::currency::Currency,
    num_paths: usize,
    /// `true` when paths were generated as antithetic pairs `(2k, 2k+1)`.
    /// Each pair is one negatively-correlated draw, *not* two i.i.d. samples,
    /// so the deal-PV std-error must be computed over the `n/2` pair means.
    antithetic: bool,
    deal_pv_stats: OnlineStats,
    deal_loss_stats: OnlineStats,
    deal_losses: Vec<f64>,
    /// Per-path deal PVs, recorded in path order. Retained so the std-error
    /// can be recomputed pair-aware under antithetic mode; the order matches
    /// the antithetic pairing `(2k, 2k+1)` because `record_output` is fed in
    /// path order (see `price_factor_sets`).
    deal_pvs: Vec<f64>,
    tranche_stats: Vec<TrancheScenarioStats>,
    /// Paths on which a collateral draw could not be funded.
    unfunded_paths: usize,
    /// Collateral draws funded per path.
    draw_stats: OnlineStats,
    /// Per-path draw option cost, retained only for pools with stochastic
    /// revolvers.
    option_cost_paths: Option<Vec<f64>>,
}

impl ScenarioCollector {
    fn new(
        instrument: &StructuredCredit,
        num_paths: usize,
        antithetic: bool,
        track_option_cost: bool,
    ) -> Result<Self> {
        if num_paths == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "stochastic scenario collector requires at least one path".to_string(),
            ));
        }
        Ok(Self {
            currency: instrument.pool.get_base_currency(),
            num_paths,
            antithetic,
            deal_pv_stats: OnlineStats::new(),
            deal_loss_stats: OnlineStats::new(),
            deal_losses: Vec::with_capacity(num_paths),
            deal_pvs: Vec::with_capacity(num_paths),
            tranche_stats: instrument
                .tranches
                .tranches
                .iter()
                .map(|tranche| TrancheScenarioStats::new(tranche, num_paths))
                .collect(),
            unfunded_paths: 0,
            draw_stats: OnlineStats::new(),
            option_cost_paths: track_option_cost.then(|| Vec::with_capacity(num_paths)),
        })
    }

    fn record_tranche(&mut self, idx: usize, metrics: PathTrancheMetrics) {
        if let Some(stats) = self.tranche_stats.get_mut(idx) {
            stats.record(metrics);
        }
    }

    fn record_deal(&mut self, pv: f64, loss: f64) {
        self.deal_pv_stats.update(pv);
        self.deal_loss_stats.update(loss);
        self.deal_losses.push(loss);
        self.deal_pvs.push(pv);
    }

    fn record_output(&mut self, output: PathScenarioOutput) {
        for (idx, metrics) in output.tranches {
            self.record_tranche(idx, metrics);
        }
        self.record_deal(output.deal_pv, output.deal_loss);
        if output.unfunded_draws {
            self.unfunded_paths += 1;
        }
        self.draw_stats.update(output.collateral_draws);
        if let Some(paths) = self.option_cost_paths.as_mut() {
            paths.push(output.draw_option_cost);
        }
    }

    fn finalize(
        mut self,
        pricer: &StochasticPricer,
        pricing_mode: PricingMode,
    ) -> Result<StochasticPricingResult> {
        let mean_pv = self.deal_pv_stats.mean();
        let mean_loss = self.deal_loss_stats.mean();
        // Welford population variance avoids catastrophic cancellation when
        // tranche PVs are large (≥ 1e7) and relative dispersion is small.
        let loss_pop_var = self.deal_loss_stats.population_variance();
        // Std-error: under antithetic mode each pair `(2k, 2k+1)` is one
        // negatively-correlated draw — dividing the per-path variance by
        // `√num_paths` would treat the `n/2` pairs as `n` i.i.d. samples and
        // report a CI that is wrong (typically too narrow). `deal_pv_std_error`
        // collapses each pair to its mean and computes the SE over the `n/2`
        // pair means, the genuine i.i.d. unit under antithetic sampling.
        let std_error = deal_pv_std_error(&self.deal_pvs, self.antithetic);
        let es = expected_shortfall(&mut self.deal_losses, pricer.config.es_confidence);

        let mut result = StochasticPricingResult::new(
            Money::new(mean_pv, self.currency)?,
            Money::new(mean_loss, self.currency)?,
            self.num_paths,
            pricing_mode,
        )
        .with_unexpected_loss(Money::new(loss_pop_var.sqrt(), self.currency)?)
        .with_expected_shortfall(Money::new(es, self.currency)?, pricer.config.es_confidence);

        // Prices are quoted per CURRENT note balance, the same face the
        // deterministic `dirty_price` metric uses, not per pool balance: a
        // deal whose notes total 60M against a 100M pool prices at 100, not 60.
        let notional: f64 = self
            .tranche_stats
            .iter()
            .map(|stats| stats.current_balance)
            .sum();
        if notional > f64::EPSILON {
            // `mean_pv` is the present value of all FUTURE cashflows
            // from the valuation date, which is by definition the DIRTY price —
            // it already contains the accrued portion of the next coupon.
            // Assigning it to `clean_price` first and then copying to
            // `dirty_price` had the labels backwards on the primary quantity.
            //
            // Clean = dirty − accrued. The deal-level stochastic result carries
            // no per-tranche interest flows, so accrued cannot be computed here
            // (the same constraint the accrued calculator documents). Rather than fabricate one, `clean_price` is set
            // equal to dirty and flagged as UNADJUSTED: understating accrued by
            // at most one period's interest, in a known direction, instead of
            // silently mislabelling which of the two is authoritative. Use
            // `calculate_tranche_metrics` for an accrued-adjusted clean price.
            result.dirty_price = mean_pv / notional * 100.0;
            result.clean_price = result.dirty_price;
        }
        result.pv_std_error = std_error;
        result.pv_confidence_interval = (mean_pv - 1.96 * std_error, mean_pv + 1.96 * std_error);
        result.unfunded_draw_path_fraction =
            self.unfunded_paths as f64 / self.num_paths.max(1) as f64;
        result.expected_collateral_draws = Money::new(self.draw_stats.mean(), self.currency)?;
        if let Some(paths) = self.option_cost_paths.take() {
            let mean = if paths.is_empty() {
                0.0
            } else {
                paths.iter().sum::<f64>() / paths.len() as f64
            };
            result.draw_option_cost = Money::new(mean, self.currency)?;
            result.draw_option_cost_paths = paths;
        }
        result.tranche_results = self
            .tranche_stats
            .into_iter()
            .map(|stats| stats.finalize(self.currency, self.num_paths, pricer.config.es_confidence))
            .collect::<Result<Vec<_>>>()?;

        Ok(result)
    }
}

/// Standard error of the deal-PV Monte Carlo mean.
///
/// In plain (non-antithetic) mode every path is an i.i.d. sample and the SE
/// is `√(population_variance / n)`.
///
/// Under antithetic mode the paths are generated as negatively-correlated
/// pairs `(2k, 2k+1)`: `path 2k+1` negates the systematic factors of `path 2k`.
/// The pair is *not* two independent samples — the genuine i.i.d. unit is the
/// pair mean `(pv_2k + pv_2k+1)/2`. Treating the `n` paths as `n` i.i.d.
/// samples (`√(per-path variance / n)`) misstates the SE and the reported
/// 95% CI. This routine collapses each complete pair to its mean and computes
/// the SE over the `n/2` pair means; a lone trailing path (odd `n`) is treated
/// as its own one-element "pair". When every pair averages to the same value
/// the pair-mean variance — and hence the SE — is zero even if per-path
/// dispersion is large.
fn deal_pv_std_error(deal_pvs: &[f64], antithetic: bool) -> f64 {
    if !antithetic {
        return sample_std_error(deal_pvs);
    }
    let pair_means: Vec<f64> = deal_pvs
        .chunks(2)
        .map(|pair| pair.iter().sum::<f64>() / pair.len() as f64)
        .collect();
    sample_std_error(&pair_means)
}

/// Standard error of the mean of an i.i.d. sample: `√(population_variance / n)`.
///
/// Population variance is accumulated with Welford's algorithm to avoid the
/// catastrophic cancellation of the `E[X²] − E[X]²` form when PVs are large
/// relative to their dispersion.
fn sample_std_error(samples: &[f64]) -> f64 {
    let n = samples.len();
    if n == 0 {
        return 0.0;
    }
    let mut stats = OnlineStats::new();
    for &x in samples {
        stats.update(x);
    }
    (stats.population_variance() / n as f64).sqrt()
}

fn expected_shortfall(losses: &mut [f64], confidence: f64) -> f64 {
    if losses.is_empty() {
        return 0.0;
    }
    // `total_cmp`, not `partial_cmp(..).unwrap_or(Equal)`.
    //
    // The old comparator is not a total order when a NaN is present — NaN
    // compares Equal to everything while the finite values retain their own
    // ordering — which violates `sort_by`'s contract. Since Rust 1.81 the
    // standard sort detects that and PANICS rather than producing garbage.
    // A NaN can reach here through `convert_clamped` (utils/rates.rs), so this
    // was a live panic path in a pricing loop, not a theoretical one.
    //
    // `total_cmp` is a genuine total order over all f64 including NaN, so the
    // sort is always well-defined. NaN sorts to the front under the reversed
    // comparator, which is the conservative placement for a tail-loss measure:
    // a corrupted path shows up as an extreme loss rather than being silently
    // averaged into the middle of the distribution.
    losses.sort_by(|a, b| b.total_cmp(a));
    let tail = (1.0 - confidence).clamp(0.0, 1.0);
    let tail_count = (tail * losses.len() as f64).ceil().max(1.0) as usize;
    let tail_count = tail_count.min(losses.len());
    losses.iter().take(tail_count).sum::<f64>() / tail_count as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::ScenarioTreeConfig;
    use crate::instruments::fixed_income::structured_credit::{
        AssetPool, DealType, DefaultModelSpec, PoolAsset, RecoveryModelSpec, Tranche,
        TrancheCashflows, TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DateExt};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
    }

    /// Minimal single-asset/single-tranche deal for tests that need a
    /// structurally valid instrument to prepare a simulation against.
    fn simple_deal(id: &str) -> StructuredCredit {
        let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::from((1_000_000_i64, Currency::USD)),
            0.06,
            Date::from_calendar_date(2029, Month::January, 1).expect("valid date"),
            finstack_quant_core::dates::DayCount::Thirty360,
        ));
        let senior = Tranche::new(
            "SENIOR",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((1_000_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
        )
        .expect("senior tranche");
        let mut deal = StructuredCredit::new_abs(
            id,
            pool,
            TrancheStructure::new(vec![senior]).expect("structure"),
            test_date(),
            Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
            "USD-OIS",
        );
        deal.payment_calendar_id = Some("nyse".to_string());
        deal
    }

    fn test_discount_curve() -> std::sync::Arc<DiscountCurve> {
        std::sync::Arc::new(
            DiscountCurve::builder("USD-OIS")
                .base_date(test_date())
                .knots([(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
                .build()
                .expect("curve"),
        )
    }

    #[test]
    fn path_metrics_wal_matches_the_canonical_kernel() {
        let as_of = test_date();
        let first = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let second = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let principal_flows = vec![
            (first, Money::from((40_i64, Currency::USD))),
            (second, Money::from((60_i64, Currency::USD))),
        ];
        let zero = Money::from((0_i64, Currency::USD));
        let cashflows = TrancheCashflows {
            tranche_id: "A".to_string(),
            cashflows: principal_flows.clone(),
            detailed_flows: Vec::new(),
            accrual_periods: Vec::new(),
            interest_flows: Vec::new(),
            principal_flows,
            pik_flows: Vec::new(),
            deferred_flows: Vec::new(),
            writedown_flows: Vec::new(),
            final_balance: zero,
            total_interest: zero,
            total_principal: Money::from((100_i64, Currency::USD)),
            total_pik: zero,
            total_deferred: zero,
            total_writedown: zero,
        };

        let metrics =
            PathTrancheMetrics::from_cashflows(&cashflows, as_of, test_discount_curve().as_ref())
                .expect("path metrics");
        let expected =
            weighted_average_life_from_principal(cashflows.principal_flows.iter().copied(), as_of)
                .expect("canonical WAL");

        assert_eq!(metrics.wal, expected);
    }

    fn test_instrument() -> StructuredCredit {
        let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::from((1_000_000_i64, Currency::USD)),
            0.06,
            maturity,
            DayCount::Thirty360,
        ));
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((1_000_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("tranche");
        let mut instrument = StructuredCredit::new_abs(
            "ABS",
            pool,
            TrancheStructure::new(vec![tranche]).expect("structure"),
            test_date(),
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse");
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        instrument
    }

    #[test]
    fn monte_carlo_one_path_prices_waterfall_cashflows() {
        let instrument = test_instrument();
        let market = MarketContext::new().insert((*test_discount_curve()).clone());
        let config = StochasticPricerConfig::new(
            test_date(),
            test_discount_curve(),
            ScenarioTreeConfig::new(test_date().months_until(instrument.maturity) as usize, 2),
        )
        .with_pricing_mode(PricingMode::MonteCarlo {
            num_paths: 1,
            antithetic: false,
        });
        let pricer = StochasticPricer::new(config);

        let result = pricer.price(&instrument, &market).expect("price");

        assert_eq!(result.num_paths, 1);
        assert_eq!(result.tranche_results.len(), 1);
        assert!(result.npv.amount().is_finite());
    }

    #[test]
    fn hybrid_mode_prices_tree_prefix_and_mc_suffix_paths() {
        let instrument = test_instrument();
        let market = MarketContext::new().insert((*test_discount_curve()).clone());
        let config = StochasticPricerConfig::new(
            test_date(),
            test_discount_curve(),
            ScenarioTreeConfig::new(test_date().months_until(instrument.maturity) as usize, 2),
        )
        .with_pricing_mode(PricingMode::Hybrid {
            tree_periods: 3,
            mc_paths: 100,
        });
        let pricer = StochasticPricer::new(config);

        let result = pricer.price(&instrument, &market).expect("hybrid price");

        assert_eq!(result.num_paths, 800);
        assert_eq!(result.tranche_results.len(), 1);
        assert!(result.npv.amount().is_finite());
        assert_eq!(
            result.pricing_mode,
            PricingMode::Hybrid {
                tree_periods: 3,
                mc_paths: 100,
            }
        );
    }

    /// Regression test: catastrophic cancellation in MC variance accumulation.
    ///
    /// The `E[X²] - E[X]²` form (`sq_sum / paths - mean * mean`) suffers
    /// catastrophic cancellation when `delta² ≪ ULP(mean²)`.  For `mean = 5e7`
    /// the ULP of `mean²` is `≈ 0.555` (since `2^−52 · (5e7)² ≈ 0.555`).
    /// When `delta = 0.05` (`delta² = 0.0025 ≪ 0.555`) the two terms in the
    /// subtraction are identical in f64, so the naive form returns **exactly
    /// zero**, collapsing `pv_std_error` to zero even though the true value is
    /// `0.05 / √1000 ≈ 0.00158`.
    ///
    /// This test drives `ScenarioCollector` directly with synthetic path outputs
    /// whose population variance is known exactly, then verifies that the
    /// computed `pv_std_error` is accurate.
    ///
    /// For the buggy `E[X²] - E[X]²` form the computed variance is exactly
    /// **0.0** (collapsed), making `pv_std_error = 0.0` even though the true
    /// variance is `0.0025`.  The Welford fix recovers the correct value.
    #[test]
    fn scenario_collector_variance_no_catastrophic_cancellation() {
        let instrument = test_instrument();
        let n = 1000usize;
        let mut collector =
            ScenarioCollector::new(&instrument, n, false, false).expect("collector");

        // Synthetic PVs: alternating mean ± delta where delta is tiny relative to mean.
        // True population variance = delta² = 0.0025.
        // True population std      = 0.05.
        //
        // At mean = 5e7, ULP(mean²) ≈ 0.555.  delta² = 0.0025 ≪ 0.555, so the
        // naive sq_sum/n − mean² subtraction cancels completely to 0.0 in f64.
        let mean_pv: f64 = 50_000_000.0; // $50 M — large enough for cancellation
        let delta: f64 = 0.05; // $0.05 spread → sigma/mean = 1e-9, delta² ≪ ULP(mean²)

        for i in 0..n {
            let pv = if i % 2 == 0 {
                mean_pv + delta
            } else {
                mean_pv - delta
            };
            // Feed as deal-level output (no tranche sub-paths needed here).
            collector.record_deal(pv, 0.0);
        }

        // Extract the deal-level variance directly via the finalize path.
        // We use a minimal StochasticPricer config just to call finalize.
        use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::ScenarioTreeConfig;
        use crate::instruments::fixed_income::structured_credit::pricing::stochastic::pricer::config::StochasticPricerConfig;
        let config = StochasticPricerConfig::new(
            test_date(),
            test_discount_curve(),
            ScenarioTreeConfig::new(12, 2),
        );
        let pricer = StochasticPricer::new(config);
        let result = collector
            .finalize(&pricer, PricingMode::Tree)
            .expect("valid pricing result");

        // True population variance = delta² = 0.0025
        // True std_error of the mean = 0.05 / sqrt(1000) ≈ 0.001581
        let true_pop_var: f64 = delta * delta; // = 0.0025
        let true_std_error = true_pop_var.sqrt() / (n as f64).sqrt();

        // The E[X²]-E[X]² form collapses sq_sum/n − mean² to exactly 0.0 here:
        // delta² = 0.0025 is below the ~0.555 ULP of mean², so the subtraction
        // rounds to zero, making pv_std_error = 0. The Welford form is immune.
        assert!(
            result.pv_std_error > 0.0,
            "pv_std_error must be strictly positive (true value ≈ {true_std_error:.8}); \
             got {}. Catastrophic cancellation in sq_sum/n - mean² collapses to 0 \
             when delta²={:.6} ≪ ULP(mean²)≈0.555.",
            result.pv_std_error,
            delta * delta,
        );

        // Relative error must be small (< 0.5%). The E[X²]-E[X]² form
        // produces 100% relative error here; Welford is accurate to rounding.
        let rel_err = (result.pv_std_error - true_std_error).abs() / true_std_error;
        assert!(
            rel_err < 0.005,
            "pv_std_error relative error {rel_err:.4} exceeds 0.5%: \
             computed={}, true={true_std_error:.8}. \
             This indicates the E[X²]-E[X]² form is being used instead of Welford.",
            result.pv_std_error
        );
    }

    /// W-23 — antithetic pairs are negatively-correlated draws, not i.i.d.
    /// samples; the deal-PV std-error must be computed over the `n/2` pair
    /// means, not over the `n` per-path values.
    ///
    /// Pathology: per-path PVs alternate `mean ± delta`. Each antithetic pair
    /// `(2k, 2k+1)` therefore averages to *exactly* `mean` — the pair-mean
    /// variance is zero even though the per-path population variance is
    /// `delta²`. The plain i.i.d. estimator `√(delta²/n)` is non-zero and
    /// hence wrong; the pair-aware estimator must report `SE = 0`.
    #[test]
    fn antithetic_std_error_uses_pair_means_not_per_path() {
        let pvs: Vec<f64> = (0..1000)
            .map(|i| if i % 2 == 0 { 1.0e7 + 5.0 } else { 1.0e7 - 5.0 })
            .collect();

        // Plain i.i.d. estimator treats all 1000 paths as independent: it
        // sees per-path population variance = 25 and reports a non-zero SE.
        let iid_se = deal_pv_std_error(&pvs, false);
        assert!(
            iid_se > 0.0,
            "i.i.d. estimator should see per-path dispersion (got {iid_se})"
        );

        // Pair-aware estimator: every pair averages to exactly 1e7, so the
        // pair-mean variance — and therefore the SE — is zero.
        let pair_se = deal_pv_std_error(&pvs, true);
        assert!(
            pair_se.abs() < 1e-9,
            "antithetic SE must be ~0 when every pair averages identically; \
             got {pair_se} (i.i.d. estimator would wrongly report {iid_se})"
        );

        // The two estimators must genuinely disagree on this pathology — the
        // whole point of the fix.
        assert!(
            iid_se > 1e-3,
            "the i.i.d. and pair-aware estimators must differ materially here"
        );
    }

    /// W-23 — the pair-aware std-error must match an independent recomputation
    /// over the pair-mean sample, and antithetic mode must not *increase* the
    /// reported estimator variance versus the i.i.d. interpretation when the
    /// pairs carry genuine negative correlation.
    #[test]
    fn antithetic_std_error_matches_pair_mean_recomputation() {
        // Negatively-correlated pairs: within each pair the two paths move in
        // opposite directions about a slowly-drifting pair mean. This is the
        // regime antithetic sampling targets.
        let n_pairs = 500usize;
        let mut pvs = Vec::with_capacity(2 * n_pairs);
        for k in 0..n_pairs {
            let pair_mean = 1.0e7 + (k as f64) * 0.01;
            let spread = 100.0; // large per-path swing, cancels within the pair
            pvs.push(pair_mean + spread);
            pvs.push(pair_mean - spread);
        }

        let reported = deal_pv_std_error(&pvs, true);

        // Independent recomputation: collapse each pair to its mean, then take
        // the plain SE over the n/2 pair means.
        let pair_means: Vec<f64> = pvs.chunks(2).map(|p| (p[0] + p[1]) / 2.0).collect();
        let expected = sample_std_error(&pair_means);
        assert!(
            (reported - expected).abs() < 1e-9,
            "pair-aware SE {reported} must match pair-mean recomputation {expected}"
        );

        // The i.i.d. estimator sees the huge ±100 per-path swing and reports a
        // far larger SE; antithetic mode must NOT inflate variance beyond it.
        let iid = deal_pv_std_error(&pvs, false);
        assert!(
            reported <= iid,
            "antithetic SE {reported} must not exceed the i.i.d. SE {iid}"
        );
    }

    /// M2.16 — `evolved_factors` must implement the exact stationary AR(1)/OU
    /// recursion `Z_m = φ·Z_{m−1} + √(1−φ²)·ε_m` with `φ = e^{−κ/12}`.
    #[test]
    fn evolved_factors_applies_stationary_ar1_recursion() {
        let kappa = 1.5_f64;
        let phi = (-kappa / 12.0).exp();
        let scale = (1.0 - phi * phi).sqrt();
        let innovations = [0.3, -1.2, 0.7];

        let evolved = StochasticPricer::evolved_factors(&innovations, kappa);

        assert!((evolved[0] - 0.3).abs() < 1e-15, "Z_1 = ε_1");
        let expected_1 = phi * evolved[0] + scale * innovations[1];
        assert!((evolved[1] - expected_1).abs() < 1e-15);
        let expected_2 = phi * evolved[1] + scale * innovations[2];
        assert!((evolved[2] - expected_2).abs() < 1e-15);

        // Antithetic linearity: negated innovations give the negated path.
        let negated: Vec<f64> = innovations.iter().map(|e| -e).collect();
        let evolved_neg = StochasticPricer::evolved_factors(&negated, kappa);
        for (a, b) in evolved.iter().zip(&evolved_neg) {
            assert!((a + b).abs() < 1e-15, "AR(1) must commute with negation");
        }
    }

    /// Configured `mean_reversion` must change Monte Carlo path statistics.
    #[test]
    fn mean_reversion_changes_path_statistics() {
        let instrument = test_instrument();
        let market = MarketContext::new().insert((*test_discount_curve()).clone());
        let price_with_kappa = |kappa: f64| {
            let mut tree_config = ScenarioTreeConfig::new(24, 2);
            tree_config.factor_spec = LatentFactorSpec::single_factor(1.0, kappa);
            tree_config.default_spec =
                StochasticDefaultSpec::intensity_process(0.10, 1.0, 0.5, 0.8);
            let config =
                StochasticPricerConfig::new(test_date(), test_discount_curve(), tree_config)
                    .with_pricing_mode(PricingMode::MonteCarlo {
                        num_paths: 64,
                        antithetic: false,
                    });
            StochasticPricer::new(config)
                .price(&instrument, &market)
                .expect("MC price")
        };

        let iid = price_with_kappa(0.0);
        let persistent = price_with_kappa(2.0);

        assert!(
            (iid.npv.amount() - persistent.npv.amount()).abs() > 0.0
                || (iid.pv_std_error - persistent.pv_std_error).abs() > 0.0,
            "mean_reversion must change MC path statistics: \
             npv {} vs {}, std_error {} vs {}",
            iid.npv.amount(),
            persistent.npv.amount(),
            iid.pv_std_error,
            persistent.pv_std_error
        );
    }

    /// M2.13 — canonical sign convention invariant for the MC engine: under
    /// the shipped RMBS/CLO default and recovery calibrations, the two must
    /// co-move NEGATIVELY across systematic-factor realizations (stress = low
    /// factor ⇒ high MDR and low recovery).
    #[test]
    fn mc_engine_defaults_and_recoveries_co_move_negatively() {
        let mut rmbs = ScenarioTreeConfig::new(24, 3);
        rmbs.default_spec = rmbs_default_spec();
        rmbs.recovery_spec = RecoverySpec::market_standard_stochastic();

        let mut clo = ScenarioTreeConfig::new(24, 3);
        clo.default_spec = clo_default_spec();
        clo.recovery_spec = RecoverySpec::MarketCorrelated {
            mean_recovery: 0.40,
            recovery_volatility: 0.30,
            factor_correlation: 0.50,
        };

        for (label, tree_config) in [("rmbs", rmbs), ("clo", clo)] {
            let config =
                StochasticPricerConfig::new(test_date(), test_discount_curve(), tree_config);
            let pricer = StochasticPricer::new(config);

            let zs = [-2.0, -1.0, 0.0, 1.0, 2.0];
            let deal = simple_deal("CO-MOVE-DEAL");
            let prepared = pricer
                .prepare_run(&deal, &MarketContext::new())
                .expect("prepared run");
            let shocks: Vec<_> = zs
                .iter()
                .map(|&z| pricer.monthly_shock(&prepared, 36, z, z, &mut 1.0))
                .collect();
            let mdrs: Vec<f64> = shocks.iter().map(|s| s.mdr).collect();
            let recoveries: Vec<f64> = shocks.iter().map(|s| s.recovery_rate).collect();

            let corr = pearson(&mdrs, &recoveries);
            assert!(
                corr < 0.0,
                "{label}: corr(MDR, recovery) across factor realizations must be \
                 negative, got {corr} (mdrs {mdrs:?}, recoveries {recoveries:?})"
            );
        }
    }

    fn pearson(xs: &[f64], ys: &[f64]) -> f64 {
        let n = xs.len() as f64;
        let mean_x = xs.iter().sum::<f64>() / n;
        let mean_y = ys.iter().sum::<f64>() / n;
        let mut cov = 0.0;
        let mut var_x = 0.0;
        let mut var_y = 0.0;
        for (x, y) in xs.iter().zip(ys) {
            cov += (x - mean_x) * (y - mean_y);
            var_x += (x - mean_x).powi(2);
            var_y += (y - mean_y).powi(2);
        }
        cov / (var_x.sqrt() * var_y.sqrt()).max(f64::MIN_POSITIVE)
    }
}

/// Tests for per-name copula default simulation (finite-pool Monte Carlo).
///
/// These exercise the path that replaced the pool-wide-MDR-applied-to-all-
/// names defect: the engine now realizes each pool asset's default
/// individually via the copula latent variable, with a documented LHP
/// fast-path for genuinely granular pools.
#[cfg(test)]
mod per_name_copula_tests {
    /// The expected-shortfall sort must use a TOTAL order.
    ///
    /// `partial_cmp(..).unwrap_or(Equal)` is not a total order when a NaN is
    /// present: NaN compares Equal to everything while the finite values keep
    /// their own ordering. Since Rust 1.81 the standard sort detects that and
    /// PANICS. A NaN can reach here via `convert_clamped`, so this was a live
    /// panic path in a pricing loop.
    ///
    /// Positive-sign NaNs are the representation emitted by the live
    /// `convert_clamped` corruption path. They must sort into the worst tail so
    /// the measure surfaces the corrupted path instead of returning a plausible
    /// finite value.
    #[test]
    fn expected_shortfall_places_nan_in_worst_tail() {
        let mut losses = vec![10.0, f64::NAN, 5.0, 100.0, 1.0, f64::NAN, 50.0];
        let es = expected_shortfall(&mut losses, 0.95);
        assert!(
            es.is_nan(),
            "a corrupted worst-tail path must surface as NaN, got {es}"
        );
        assert!(losses[..2].iter().all(|loss| loss.is_nan()));
        assert_eq!(&losses[2..], &[100.0, 50.0, 10.0, 5.0, 1.0]);
    }

    /// With clean input the measure is unchanged.
    #[test]
    fn expected_shortfall_is_unchanged_for_finite_losses() {
        let mut losses = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        // 95% confidence over 10 paths -> ceil(0.5) = 1 worst loss.
        let es = expected_shortfall(&mut losses, 0.95);
        assert!(
            (es - 10.0).abs() < 1e-12,
            "the 95% ES over 10 sorted losses is the single worst (10.0), got {es}"
        );
    }

    use super::*;
    use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::ScenarioTreeConfig;
    use crate::instruments::fixed_income::structured_credit::{
        AssetPool, DealType, DefaultModelSpec, PoolAsset, RecoveryModelSpec, Tranche,
        TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_models::credit::pool::PoolGranularity;
    use time::Month;

    fn close() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
    }

    fn maturity() -> Date {
        Date::from_calendar_date(2027, Month::January, 1).expect("valid date")
    }

    fn discount_curve() -> std::sync::Arc<DiscountCurve> {
        std::sync::Arc::new(
            DiscountCurve::builder("USD-OIS")
                .base_date(close())
                .knots([(0.0, 1.0), (1.0, 0.97), (3.0, 0.91), (5.0, 0.85)])
                .build()
                .expect("curve"),
        )
    }

    /// Build a CLO-style deal: `n_assets` identical fixed-rate loans summing
    /// to $100M, tranched senior (0-80%) / mezzanine (80-92%) / equity
    /// (92-100%). Larger `n_assets` ⇒ more granular pool.
    fn clo_deal(n_assets: usize) -> StructuredCredit {
        let total = 100_000_000.0;
        let per_asset = total / n_assets as f64;
        let mut pool = AssetPool::new("CLO-POOL", DealType::Clo, Currency::USD);
        for i in 0..n_assets {
            pool.assets.push(PoolAsset::fixed_rate_bond(
                format!("L{i}"),
                Money::new(per_asset, Currency::USD).expect("valid money fixture"),
                0.07,
                maturity(),
                DayCount::Thirty360,
            ));
        }
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "SR",
                0.0,
                80.0,
                TrancheSeniority::Senior,
                Money::new(total * 0.80, Currency::USD).expect("valid money fixture"),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity(),
            )
            .expect("senior"),
            Tranche::new(
                "MEZZ",
                80.0,
                92.0,
                TrancheSeniority::Mezzanine,
                Money::new(total * 0.12, Currency::USD).expect("valid money fixture"),
                TrancheCoupon::Fixed { rate: 0.08 },
                maturity(),
            )
            .expect("mezz"),
            Tranche::new(
                "EQ",
                92.0,
                100.0,
                TrancheSeniority::Equity,
                Money::new(total * 0.08, Currency::USD).expect("valid money fixture"),
                TrancheCoupon::Fixed { rate: 0.0 },
                maturity(),
            )
            .expect("equity"),
        ])
        .expect("structure");
        let mut sc = StructuredCredit::new_abs(
            "CLO-PER-NAME",
            pool,
            tranches,
            close(),
            maturity(),
            "USD-OIS",
        )
        .with_payment_calendar("nyse");
        sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        sc.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        sc
    }

    /// Build a pricer config with a Gaussian-copula default model.
    fn copula_config(
        base_cdr: f64,
        correlation: f64,
        num_periods: usize,
        granularity: PoolGranularity,
        num_paths: usize,
    ) -> StochasticPricerConfig {
        copula_config_with_spec(
            StochasticDefaultSpec::gaussian_copula(base_cdr, correlation),
            num_periods,
            granularity,
            num_paths,
        )
    }

    /// Build a copula config with antithetic Monte Carlo sampling enabled.
    fn antithetic_copula_config(
        base_cdr: f64,
        correlation: f64,
        num_periods: usize,
        granularity: PoolGranularity,
        num_paths: usize,
    ) -> StochasticPricerConfig {
        let mut tree_config = ScenarioTreeConfig::new(num_periods, 2);
        tree_config.default_spec = StochasticDefaultSpec::gaussian_copula(base_cdr, correlation);
        StochasticPricerConfig::new(close(), discount_curve(), tree_config)
            .with_pricing_mode(PricingMode::MonteCarlo {
                num_paths,
                antithetic: true,
            })
            .with_pool_granularity(granularity)
    }

    /// Build a pricer config with a Student-t-copula default model.
    fn student_t_copula_config(
        base_cdr: f64,
        correlation: f64,
        degrees_of_freedom: f64,
        num_periods: usize,
        granularity: PoolGranularity,
        num_paths: usize,
    ) -> StochasticPricerConfig {
        copula_config_with_spec(
            StochasticDefaultSpec::student_t_copula(base_cdr, correlation, degrees_of_freedom),
            num_periods,
            granularity,
            num_paths,
        )
    }

    /// Build a pricer config from an explicit copula default spec.
    fn copula_config_with_spec(
        default_spec: StochasticDefaultSpec,
        num_periods: usize,
        granularity: PoolGranularity,
        num_paths: usize,
    ) -> StochasticPricerConfig {
        let mut tree_config = ScenarioTreeConfig::new(num_periods, 2);
        tree_config.default_spec = default_spec;
        StochasticPricerConfig::new(close(), discount_curve(), tree_config)
            .with_pricing_mode(PricingMode::MonteCarlo {
                num_paths,
                antithetic: false,
            })
            .with_pool_granularity(granularity)
    }

    fn tranche_pv(result: &StochasticPricingResult, id: &str) -> f64 {
        result
            .tranche_results
            .iter()
            .find(|t| t.tranche_id == id)
            .map(|t| t.npv.amount())
            .unwrap_or_else(|| panic!("tranche {id} missing"))
    }

    /// **LHP-limit parity** — the correctness anchor.
    ///
    /// A large, granular, homogeneous pool priced per-name must converge to
    /// the closed-form LHP result for the *same* pool: per-name → LHP as
    /// `N → ∞`. This both validates the per-name engine and shows the LHP
    /// fast-path is the genuine large-pool limit.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn large_granular_pool_per_name_converges_to_lhp() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        // 600 names ⇒ granular: per-name realized fraction ≈ LHP conditional.
        let deal = clo_deal(600);

        let per_name = StochasticPricer::new(copula_config(
            0.03,
            0.20,
            36,
            PoolGranularity::PerName,
            1_500,
        ))
        .price(&deal, &market)
        .expect("per-name pricing");
        let lhp = StochasticPricer::new(copula_config(
            0.03,
            0.20,
            36,
            PoolGranularity::LargeHomogeneous,
            1_500,
        ))
        .price(&deal, &market)
        .expect("LHP pricing");

        // Each tranche PV must agree within a few MC standard errors. The
        // per-name engine has its own idiosyncratic dispersion, but for a
        // 600-name pool that dispersion is small relative to the systematic
        // channel, so the means converge.
        for id in ["SR", "MEZZ", "EQ"] {
            let pn = tranche_pv(&per_name, id);
            let lh = tranche_pv(&lhp, id);
            let tol = (0.01 * lh.abs()).max(150_000.0);
            assert!(
                (pn - lh).abs() < tol,
                "{id}: per-name PV {pn:.0} should converge to LHP PV {lh:.0} \
                 (|diff|={:.0}, tol={tol:.0})",
                (pn - lh).abs()
            );
        }
    }

    /// Per-name simulation must be deterministic and bit-identical between
    /// repeated runs (seeded `PhiloxRng` substreams).
    #[test]
    fn per_name_pricing_is_deterministic() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        let deal = clo_deal(80);

        let run = || {
            StochasticPricer::new(copula_config(0.04, 0.25, 36, PoolGranularity::PerName, 500))
                .price(&deal, &market)
                .expect("per-name pricing")
        };
        let a = run();
        let b = run();

        assert_eq!(
            a.npv.amount(),
            b.npv.amount(),
            "repeated per-name MC runs must be bit-identical"
        );
        for id in ["SR", "MEZZ", "EQ"] {
            assert_eq!(
                tranche_pv(&a, id),
                tranche_pv(&b, id),
                "{id}: repeated per-name runs must produce bit-identical tranche PV"
            );
        }
    }

    /// Item 3 — per-name copula mask / asset-loop alignment.
    ///
    /// The default-indicator mask is sized by the builder from the
    /// performing-asset count at period start; the asset loop claims one
    /// entry per performing asset in pool-index order. With ≥2 defaults per
    /// period the loop mutates `is_defaulted` mid-iteration — the alignment
    /// must survive that. A misalignment is now a hard `Error` (the engine's
    /// pre-loop length guard), so a successfully-priced, deterministic run
    /// over a high-default scenario proves the mask stays index-aligned.
    ///
    /// This drives a concentrated pool with a high base CDR and high
    /// correlation so multiple names default in the same period across many
    /// paths; if the guard ever tripped the run would error rather than
    /// return a price.
    #[test]
    fn per_name_mask_stays_aligned_with_multiple_defaults_per_period() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        // 30 names ⇒ concentrated; high CDR + high correlation drives several
        // simultaneous defaults per period on a meaningful share of paths.
        let deal = clo_deal(30);

        let run = || {
            StochasticPricer::new(copula_config(
                0.12, // high base CDR
                0.45, // high correlation ⇒ clustered (multi-) defaults
                36,
                PoolGranularity::PerName,
                400,
            ))
            .price(&deal, &market)
            // `.expect` fails loudly if the mask-alignment guard ever errors.
            .expect("per-name pricing must not trip the mask-alignment guard")
        };
        let a = run();
        let b = run();

        // The run completed (guard never tripped) and is bit-reproducible.
        assert!(a.npv.amount().is_finite(), "priced NPV must be finite");
        assert_eq!(
            a.npv.amount(),
            b.npv.amount(),
            "per-name pricing under multi-default periods must be deterministic"
        );
        // The high-default scenario must actually realize losses — otherwise
        // the test would not be exercising the multi-default code path.
        let total_loss: f64 = a
            .tranche_results
            .iter()
            .map(|t| t.expected_loss.amount())
            .sum();
        assert!(
            total_loss > 0.0,
            "high-CDR per-name scenario must realize pool losses (got {total_loss}); \
             otherwise multi-default periods are not exercised"
        );
    }

    /// Item 10 — for a multi-month payment period the per-name copula
    /// systematic factor must aggregate ALL the period's monthly factors as
    /// `(Σ Zₘ)/√M`, not just the first month's. White-box test on
    /// `path_shocks_from_factors`: a quarterly deal with crafted month
    /// factors must produce a period systematic `Z` equal to the `√M`-scaled
    /// sum, so months 2..M are not silently ignored.
    #[test]
    fn multi_month_copula_systematic_factor_aggregates_all_months() {
        // Quarterly deal ⇒ 3 months per payment period.
        let mut deal = clo_deal(60);
        deal.frequency = finstack_quant_core::dates::Tenor::quarterly();

        let mut cfg = copula_config(
            0.05,
            0.30,
            12, // 12 monthly tree periods
            PoolGranularity::PerName,
            16,
        );
        // Fast mean reversion (κ = 6 ⇒ φ ≈ 0.61) so months within a period
        // stay distinct; at κ = 0 (φ = 1) later months equal month 1.
        cfg.tree_config.factor_spec = LatentFactorSpec::SingleFactor {
            volatility: 1.0,
            mean_reversion: 6.0,
        };
        let pricer = StochasticPricer::new(cfg);

        // Craft 6 monthly factors covering two quarterly periods. The first
        // quarter is benign in month 1 but stressed in months 2 and 3 — a
        // month-1-only systematic factor would miss that stress entirely.
        let factors = vec![0.10_f64, -2.0, -1.5, 0.3, 0.4, 0.5];
        let prepared = pricer
            .prepare_run(&deal, &MarketContext::new())
            .expect("prepared run");
        let shocks = pricer
            .path_shocks_from_factors(&deal, &factors, (0, false), &prepared)
            .expect("path shocks");

        assert!(!shocks.is_empty(), "must produce at least one period shock");
        let period0 = shocks[0]
            .per_name
            .expect("per-name plan must be present for a copula deal");

        // Expected period factor: aggregate the evolved AR(1) path, not the
        // raw innovations, with the same scale the engine uses.
        let phi = pricer.factor_phi();
        let evolved = StochasticPricer::evolved_factors(&factors, pricer.factor_mean_reversion());
        let scale = StochasticPricer::period_factor_scale(3, phi);
        let expected_z = (evolved[0] + evolved[1] + evolved[2]) / scale;
        assert!(
            (period0.systematic_z - expected_z).abs() < 1e-9,
            "quarter-1 copula systematic factor {} must be the √M-scaled sum \
             of all 3 monthly factors ({expected_z}); a month-1-only factor \
             would be {}",
            period0.systematic_z,
            factors[0],
        );
        // The aggregated factor must be materially stressed (negative), unlike
        // the benign month-1 factor — proving months 2-3 are not ignored.
        assert!(
            period0.systematic_z < -1.0,
            "aggregated systematic factor {} must reflect the months-2-3 \
             stress, not the benign month-1 value {}",
            period0.systematic_z,
            factors[0],
        );
    }

    /// Aggregated period factor `Z_period = (Σ Zₘ)/s` stays `N(0,1)` at every κ.
    ///
    /// The copula barrier `Φ⁻¹(PD)` assumes a standard-normal factor, so
    /// `s` must equal `sqrt(Var(Σ Zₘ))` under the AR(1) autocorrelation.
    #[test]
    fn aggregated_period_factor_is_standard_normal_at_every_kappa() {
        const MONTHS: usize = 6;
        const PATHS: usize = 200_000;

        for kappa in [0.0_f64, 0.25, 1.0, 6.0, 50.0] {
            let phi = (-kappa / 12.0).exp();
            let scale = StochasticPricer::period_factor_scale(MONTHS, phi);

            let mut sum = 0.0_f64;
            let mut sum_sq = 0.0_f64;
            for path in 0..PATHS {
                let mut rng = PhiloxRng::new(20_260_719).substream(path as u64);
                let innovations: Vec<f64> = (0..MONTHS).map(|_| rng.next_std_normal()).collect();
                let evolved = StochasticPricer::evolved_factors(&innovations, kappa);
                let z_period: f64 = evolved.iter().sum::<f64>() / scale;
                sum += z_period;
                sum_sq += z_period * z_period;
            }

            let n = PATHS as f64;
            let mean = sum / n;
            let variance = sum_sq / n - mean * mean;

            assert!(
                mean.abs() < 0.02,
                "kappa={kappa}: aggregated period factor mean {mean:.4} must be ~0"
            );
            assert!(
                (variance - 1.0).abs() < 0.03,
                "kappa={kappa} (phi={phi:.4}): aggregated period factor variance \
                 {variance:.4} must be ~1, or the copula barrier Phi^-1(PD) is \
                 de-calibrated. Divisor used was {scale:.4}; the naive sqrt(M) \
                 would be {:.4}.",
                (MONTHS as f64).sqrt()
            );
        }
    }

    /// Persistent systematic factor (κ = 0) widens loss dispersion vs i.i.d.
    /// months (large κ), holding asset correlation fixed.
    ///
    /// Under the canonical single-factor copula one systematic draw governs the
    /// horizon, so defaults cluster; independent monthly factors dilute that.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn persistent_systematic_factor_widens_the_loss_distribution() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        let deal = clo_deal(60);

        let unexpected_loss_for = |kappa: f64| -> f64 {
            let mut cfg = copula_config(0.06, 0.30, 36, PoolGranularity::PerName, 6_000);
            cfg.tree_config.factor_spec = LatentFactorSpec::SingleFactor {
                volatility: 1.0,
                mean_reversion: kappa,
            };
            StochasticPricer::new(cfg)
                .price(&deal, &market)
                .expect("per-name copula pricing")
                .unexpected_loss
                .amount()
        };

        // kappa = 0  -> phi = 1     -> one systematic draw for the horizon.
        // kappa = 600 -> phi ~ 0    -> independent monthly factors (the bug).
        let persistent = unexpected_loss_for(0.0);
        let independent = unexpected_loss_for(600.0);

        assert!(
            persistent > independent * 1.10,
            "persistent UL {persistent:.0} should exceed i.i.d. UL \
             {independent:.0} by more than 10%"
        );
    }

    /// Explicit deal `CorrelationStructure` override moves unexpected loss.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn explicit_correlation_structure_changes_the_price() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        let deal = clo_deal(60);

        let unexpected_loss_for = |override_rho: Option<f64>| -> f64 {
            let mut cfg = copula_config(0.06, 0.10, 36, PoolGranularity::PerName, 4_000);
            cfg.tree_config.asset_correlation_override = override_rho;
            StochasticPricer::new(cfg)
                .price(&deal, &market)
                .expect("per-name copula pricing")
                .unexpected_loss
                .amount()
        };

        // No override: the copula spec's own 10% correlation applies.
        let baseline = unexpected_loss_for(None);
        // An explicit structure at 45% must widen the loss distribution.
        let high = unexpected_loss_for(Some(0.45));

        assert!(
            high > baseline * 1.10,
            "UL at 45% override ({high:.0}) must exceed baseline at 10% \
             ({baseline:.0}) by more than 10%"
        );
    }

    /// The intensity model's `mean_reversion` must drive the factor.
    ///
    /// `IntensityProcessDefault` documents `dX = kappa(theta - X)dt + sigma dW`
    /// and cites Duffie-Singleton, but `kappa` was stored, clamped, exposed by
    /// a getter and used in NO computation — the model was a static lognormal
    /// shock. The systematic factor is now a genuine OU path, so
    /// sourcing its kappa from this spec realizes the documented model.
    #[test]
    fn intensity_mean_reversion_drives_the_systematic_factor() {
        let mut deal = clo_deal(20);
        deal.credit_model.stochastic_default_spec = Some(StochasticDefaultSpec::intensity_process(
            0.03, 0.8, 2.5, 0.4,
        ));

        let as_of = deal.closing_date;
        let tree_config = deal
            .build_scenario_tree_config(as_of)
            .expect("scenario tree config");

        match tree_config.factor_spec {
            LatentFactorSpec::SingleFactor { mean_reversion, .. } => assert!(
                (mean_reversion - 2.5).abs() < 1e-12,
                "the factor's kappa must come from the intensity spec (2.5), \
                 got {mean_reversion}. Zero means the parameter is still inert \
                ."
            ),
            other => panic!("expected a single-factor spec, got {other:?}"),
        }
    }

    /// Regression: `TwoFactor { correlation: 0.0 }` means the prepayment factor
    /// is INDEPENDENT of credit. Collapsing it into the single-factor arm
    /// (`|rho| <= f64::EPSILON` returning `None`) would share one factor —
    /// implied correlation +1, the exact opposite of what was asked for.
    #[test]
    fn zero_correlation_is_independent_not_shared() {
        let mut two = copula_config(0.06, 0.20, 12, PoolGranularity::PerName, 16);
        two.tree_config.factor_spec = LatentFactorSpec::two_factor(0.20, 0.25, 0.0);
        assert_eq!(
            StochasticPricer::new(two)
                .factor_correlation()
                .expect("two-factor spec is supported"),
            Some(0.0),
            "rho = 0 must stay a two-factor spec, not collapse to single-factor"
        );

        let mut single = copula_config(0.06, 0.20, 12, PoolGranularity::PerName, 16);
        single.tree_config.factor_spec = LatentFactorSpec::SingleFactor {
            volatility: 0.20,
            mean_reversion: 0.0,
        };
        assert_eq!(
            StochasticPricer::new(single)
                .factor_correlation()
                .expect("single-factor spec is supported"),
            None,
            "only SingleFactor shares the credit factor"
        );
    }

    /// Prepayment and default must be driven by SEPARATE factors
    /// correlated at the configured level.
    ///
    /// `monthly_shock` built `let factors = [factor]` — one scalar handed to
    /// both `conditional_smm` and `conditional_mdr` — so the implied
    /// prepay/default correlation was +/-1 regardless of the -0.30 configured
    /// in the shipped RMBS/CLO calibrations, so a user calibrating a two-factor
    /// model silently got a single-factor one.
    ///
    /// The correlation is measured ACROSS PATHS, not across months. Under the
    /// canonical single-draw copula (kappa = 0 is a random walk) each
    /// path's factor is constant over the horizon, so a time-series
    /// correlation is degenerate — any two constant series correlate at 1.
    /// Cross-sectional is also the quantity that matters: it is the dependence
    /// between the two channels' realizations.
    #[test]
    fn prepay_factor_is_correlated_not_identical_to_the_credit_factor() {
        const RHO: f64 = -0.30;
        const PATHS: usize = 20_000;

        let mut cfg = copula_config(0.06, 0.20, 12, PoolGranularity::PerName, 16);
        cfg.tree_config.factor_spec = LatentFactorSpec::two_factor(0.20, 0.25, RHO);
        let pricer = StochasticPricer::new(cfg);

        let rho = pricer
            .factor_correlation()
            .expect("factor spec is supported")
            .expect("a two-factor spec must expose its correlation");
        assert!(
            (rho - RHO).abs() < 1e-12,
            "the configured correlation must reach the engine, got {rho}"
        );
        let scale = (1.0 - rho * rho).sqrt();

        // Reconstruct each path's period-0 credit and prepay factor exactly as
        // the engine builds them.
        let (mut sum_c, mut sum_p, mut sum_cc, mut sum_pp, mut sum_cp) =
            (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
        for path in 0..PATHS as u64 {
            let mut credit_rng = PhiloxRng::new(pricer.config.tree_config.seed).substream(path);
            let credit = credit_rng.next_std_normal();

            let mut indep_rng =
                PhiloxRng::new(pricer.config.tree_config.seed ^ PREPAY_FACTOR_SEED_SALT)
                    .substream(path);
            let indep = indep_rng.next_std_normal();

            let prepay = rho * credit + scale * indep;

            sum_c += credit;
            sum_p += prepay;
            sum_cc += credit * credit;
            sum_pp += prepay * prepay;
            sum_cp += credit * prepay;
        }

        let n = PATHS as f64;
        let mean_c = sum_c / n;
        let mean_p = sum_p / n;
        let var_c = sum_cc / n - mean_c * mean_c;
        let var_p = sum_pp / n - mean_p * mean_p;
        let cov = sum_cp / n - mean_c * mean_p;
        let realized = cov / (var_c.sqrt() * var_p.sqrt());

        assert!(
            (realized - RHO).abs() < 0.02,
            "realized prepay/default factor correlation {realized:.4} must be \
             near the configured {RHO}. A value near +/-1 means both channels \
             still share one factor."
        );
        assert!(
            (var_p - 1.0).abs() < 0.05,
            "the prepay factor must keep a unit-variance marginal so the \
             prepayment model's calibration is untouched, got {var_p:.4}"
        );

        // The formula above is only meaningful if the ENGINE actually consumes
        // the second factor. Same deal, same seed, same credit factors — only
        // the factor spec differs, so any difference in realized prepayment
        // proves the prepay channel is reading its own series.
        let deal = clo_deal(40);
        let smm_for = |spec: LatentFactorSpec| -> f64 {
            let mut cfg = copula_config(0.06, 0.20, 36, PoolGranularity::PerName, 16);
            cfg.tree_config.factor_spec = spec;
            // A STOCHASTIC prepay model, or the factor is ignored entirely and
            // the comparison is vacuous.
            cfg.tree_config.prepay_spec = rmbs_prepay_spec(0.06);
            let pricer = StochasticPricer::new(cfg);
            let prepared = pricer
                .prepare_run(&deal, &MarketContext::new())
                .expect("prepared run");
            let factors: Vec<f64> = (0..24).map(|m| ((m as f64) * 0.37).sin()).collect();
            pricer
                .path_shocks_from_factors(&deal, &factors, (0, false), &prepared)
                .expect("path shocks")
                .iter()
                .map(|s| s.smm)
                .sum()
        };

        let two_factor = smm_for(LatentFactorSpec::two_factor(0.20, 0.25, RHO));
        let single_factor = smm_for(LatentFactorSpec::SingleFactor {
            volatility: 1.0,
            mean_reversion: 0.0,
        });
        assert!(
            (two_factor - single_factor).abs() > 1e-9,
            "a two-factor spec must produce different prepayment from a \
             single-factor one on identical credit factors: {two_factor} vs \
             {single_factor}. Equal values mean the engine is still handing one \
             scalar to both channels."
        );
    }

    /// Antithetic pairs `(2k, 2k+1)` negate the credit factors, so the
    /// pair's prepayment factors `rho * Z_credit + sqrt(1 - rho^2) * Z_indep`
    /// must be exact negatives too: the independent draw has to come from
    /// the pair's shared substream and be negated on the odd member. Drawing
    /// it from `substream(path_index)` left the pair's prepayment channel
    /// independent.
    #[test]
    fn antithetic_pair_prepayment_factors_sum_to_zero() {
        let mut cfg = copula_config(0.06, 0.20, 12, PoolGranularity::PerName, 16);
        cfg.tree_config.factor_spec = LatentFactorSpec::two_factor(0.20, 0.25, -0.30);
        let pricer = StochasticPricer::new(cfg);
        let prepared = pricer
            .prepare_run(&clo_deal(10), &MarketContext::new())
            .expect("prepared run");
        let credit: Vec<f64> = (0..12).map(|m| ((m as f64) * 0.61).cos()).collect();
        let negated: Vec<f64> = credit.iter().map(|z| -z).collect();
        for pair in [0usize, 3] {
            let first = pricer
                .prepay_factors(&credit, (2 * pair, true), &prepared)
                .expect("two-factor spec");
            let second = pricer
                .prepay_factors(&negated, (2 * pair + 1, true), &prepared)
                .expect("two-factor spec");
            for (a, b) in first.iter().zip(&second) {
                assert!(a.abs() > 0.0);
                assert!((a + b).abs() < 1e-15, "pair {pair}: {a} + {b} != 0");
            }
        }
        // Without antithetic pairing, neighbouring paths stay independent.
        let first = pricer
            .prepay_factors(&credit, (0, false), &prepared)
            .expect("two-factor spec");
        let second = pricer
            .prepay_factors(&negated, (1, false), &prepared)
            .expect("two-factor spec");
        assert!(first.iter().zip(&second).any(|(a, b)| (a + b).abs() > 1e-3));
    }

    /// Burnout must ACCUMULATE across a path, not sit at 1.0.
    ///
    /// `update_burnout` had zero production callers and the engine passed a
    /// hard-coded 1.0, so the burnout channel of Richard-Roll was inert:
    /// seasoned pools that had already refinanced heavily were modelled with
    /// full prepayment propensity, while `has_burnout()` returned true.
    ///
    /// Burnout is PATH state — it accumulates as realized prepayment runs
    /// above or below expectation — so it must be threaded through the month
    /// loop rather than recomputed per month.
    #[test]
    fn burnout_accumulates_across_a_path() {
        let deal = clo_deal(60);
        let mut cfg = copula_config(0.06, 0.20, 36, PoolGranularity::PerName, 16);
        // Richard-Roll with a live burnout rate.
        cfg.tree_config.prepay_spec = rmbs_prepay_spec(0.06);
        let pricer = StochasticPricer::new(cfg);

        // Drive a strongly-prepaying path so realized runs above expected.
        let prepared = pricer
            .prepare_run(&deal, &MarketContext::new())
            .expect("prepared run");
        let factors: Vec<f64> = (0..24).map(|_| 1.5_f64).collect();
        let mut burnout = 1.0_f64;
        let _ = pricer
            .path_shocks_from_factors(&deal, &factors, (0, false), &prepared)
            .expect("path shocks");

        // Exercise the month loop directly so the state is observable.
        for month in 1..=24u32 {
            let _ = pricer.monthly_shock(&prepared, month, 1.5, 1.5, &mut burnout);
        }

        assert!(
            burnout < 1.0,
            "after 24 months of above-expectation prepayment the burnout factor \
             must have decayed below 1.0, got {burnout}. A value pinned at 1.0 \
             means the channel is still inert."
        );
        assert!(burnout > 0.0, "burnout must stay in (0, 1], got {burnout}");
    }

    /// `period_factor_scale` hits its analytic limits at φ = 0 and φ = 1.
    #[test]
    fn period_factor_scale_matches_its_analytic_limits() {
        let m = 6usize;
        // phi = 0 (independent months): Var(sum) = M, so s = sqrt(M).
        let independent = StochasticPricer::period_factor_scale(m, 0.0);
        assert!(
            (independent - (m as f64).sqrt()).abs() < 1e-12,
            "at phi=0 the divisor must be sqrt(M)={}, got {independent}",
            (m as f64).sqrt()
        );
        // phi = 1 (fully persistent): Var(sum) = M^2, so s = M and Z_period = Z_1.
        let persistent = StochasticPricer::period_factor_scale(m, 1.0);
        assert!(
            (persistent - m as f64).abs() < 1e-9,
            "at phi=1 the divisor must be M={m}, got {persistent}"
        );
        // Single-month periods are the identity under either convention.
        assert!((StochasticPricer::period_factor_scale(1, 0.5) - 1.0).abs() < 1e-12);
    }

    /// Item 5 — antithetic per-name pricing must be deterministic and produce
    /// a finite, sensible result. The per-name idiosyncratic substreams are
    /// now paired antithetically (paired paths share a substream; the odd
    /// member negates `εᵢ`), so repeated runs must stay bit-identical.
    #[test]
    fn antithetic_per_name_pricing_is_deterministic() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        let deal = clo_deal(60);

        let run = || {
            StochasticPricer::new(antithetic_copula_config(
                0.05,
                0.30,
                36,
                PoolGranularity::PerName,
                400, // even ⇒ antithetic pairing is active
            ))
            .price(&deal, &market)
            .expect("antithetic per-name pricing")
        };
        let a = run();
        let b = run();

        assert!(a.npv.amount().is_finite(), "antithetic NPV must be finite");
        assert_eq!(
            a.npv.amount(),
            b.npv.amount(),
            "repeated antithetic per-name MC runs must be bit-identical"
        );
        for id in ["SR", "MEZZ", "EQ"] {
            assert_eq!(
                tranche_pv(&a, id),
                tranche_pv(&b, id),
                "{id}: repeated antithetic per-name runs must produce \
                 bit-identical tranche PV"
            );
        }
        // The reported MC confidence interval must be a valid interval.
        let (lo, hi) = a.pv_confidence_interval;
        assert!(
            lo.is_finite() && hi.is_finite() && lo <= hi,
            "antithetic CI must be a valid finite interval: ({lo}, {hi})"
        );
    }

    /// Concentrated pools must carry strictly more loss dispersion than
    /// granular pools under per-name simulation: with fewer names, the same
    /// correlation, name-level lumpiness fattens the loss tail.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn concentration_increases_loss_dispersion() {
        let market = MarketContext::new().insert((*discount_curve()).clone());

        let granular = StochasticPricer::new(copula_config(
            0.05,
            0.20,
            36,
            PoolGranularity::PerName,
            3_000,
        ))
        .price(&clo_deal(600), &market)
        .expect("granular per-name pricing");
        let concentrated = StochasticPricer::new(copula_config(
            0.05,
            0.20,
            36,
            PoolGranularity::PerName,
            3_000,
        ))
        .price(&clo_deal(40), &market)
        .expect("concentrated per-name pricing");

        assert!(
            concentrated.unexpected_loss.amount() > granular.unexpected_loss.amount(),
            "concentrated pool (40 names) loss dispersion {:.0} must exceed \
             granular pool (600 names) dispersion {:.0}",
            concentrated.unexpected_loss.amount(),
            granular.unexpected_loss.amount()
        );
    }

    /// Per-name idiosyncratic recovery dispersion widens deal-loss variance.
    ///
    /// Both runs share a flat 40% systematic recovery; only `σ_R = 0.30`
    /// idiosyncratic scatter differs (`ρ_R = 0`). Asset correlation is kept
    /// low (2%) so the systematic factor does not swamp the channel under test.
    #[test]
    fn per_name_recovery_dispersion_widens_loss_distribution() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        let deal = clo_deal(40); // concentrated pool — name-level scatter shows

        let mut constant_cfg = copula_config(0.06, 0.02, 36, PoolGranularity::PerName, 4_000);
        constant_cfg.tree_config.recovery_spec = RecoverySpec::Constant { rate: 0.40 };

        let mut dispersed_cfg = copula_config(0.06, 0.02, 36, PoolGranularity::PerName, 4_000);
        // ρ_R = 0 ⇒ the systematic recovery is a flat 0.40, bit-identical to
        // the constant model; only the idiosyncratic σ_R = 0.30 channel differs.
        dispersed_cfg.tree_config.recovery_spec = RecoverySpec::market_correlated(0.40, 0.30, 0.0)
            .expect("valid dispersed recovery inputs");

        let constant = StochasticPricer::new(constant_cfg)
            .price(&deal, &market)
            .expect("constant-recovery per-name pricing");
        let dispersed = StochasticPricer::new(dispersed_cfg)
            .price(&deal, &market)
            .expect("dispersed-recovery per-name pricing");

        assert!(
            dispersed.unexpected_loss.amount() > constant.unexpected_loss.amount(),
            "per-name recovery dispersion must widen the loss distribution: \
             dispersed UL {:.0} should exceed constant-recovery UL {:.0}",
            dispersed.unexpected_loss.amount(),
            constant.unexpected_loss.amount(),
        );
    }

    /// Sum of realized pool credit losses across all tranches, used as a
    /// granularity-independent proxy for the pool's total default experience.
    fn deal_credit_loss(result: &StochasticPricingResult) -> f64 {
        result
            .tranche_results
            .iter()
            .map(|t| t.expected_loss.amount())
            .sum()
    }

    /// Student-t per-name pricing converges to LHP for a large granular pool.
    ///
    /// LHP must condition on the Student-t factor `M = Z/√W`, not the Gaussian `Z`.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn student_t_large_granular_pool_per_name_converges_to_lhp() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        let deal = clo_deal(600);

        let per_name = StochasticPricer::new(student_t_copula_config(
            0.05,
            0.30,
            6.0,
            36,
            PoolGranularity::PerName,
            2_000,
        ))
        .price(&deal, &market)
        .expect("Student-t per-name pricing");
        let lhp = StochasticPricer::new(student_t_copula_config(
            0.05,
            0.30,
            6.0,
            36,
            PoolGranularity::LargeHomogeneous,
            2_000,
        ))
        .price(&deal, &market)
        .expect("Student-t LHP pricing");

        let loss_pn = deal_credit_loss(&per_name);
        let loss_lhp = deal_credit_loss(&lhp);
        let loss_tol = (0.05 * loss_pn.abs()).max(250_000.0);
        assert!(
            (loss_pn - loss_lhp).abs() < loss_tol,
            "Student-t per-name credit loss {loss_pn:.0} should converge to \
             LHP credit loss {loss_lhp:.0} (|diff|={:.0}, tol={loss_tol:.0})",
            (loss_pn - loss_lhp).abs()
        );

        for id in ["SR", "MEZZ", "EQ"] {
            let pn = tranche_pv(&per_name, id);
            let lh = tranche_pv(&lhp, id);
            let tol = (0.015 * lh.abs()).max(250_000.0);
            assert!(
                (pn - lh).abs() < tol,
                "{id}: Student-t per-name PV {pn:.0} should converge to LHP \
                 PV {lh:.0} (|diff|={:.0}, tol={tol:.0})",
                (pn - lh).abs()
            );
        }
    }

    /// Student-t copula default tail dependence: at a fixed correlation, a
    /// concentrated pool priced per-name with a Student-t copula must carry
    /// strictly more loss dispersion than a granular one — the per-name
    /// engine must work end-to-end for the Student-t copula, not just
    /// Gaussian.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn student_t_concentration_increases_loss_dispersion() {
        let market = MarketContext::new().insert((*discount_curve()).clone());

        let granular = StochasticPricer::new(student_t_copula_config(
            0.05,
            0.25,
            6.0,
            36,
            PoolGranularity::PerName,
            3_000,
        ))
        .price(&clo_deal(600), &market)
        .expect("Student-t granular per-name pricing");
        let concentrated = StochasticPricer::new(student_t_copula_config(
            0.05,
            0.25,
            6.0,
            36,
            PoolGranularity::PerName,
            3_000,
        ))
        .price(&clo_deal(40), &market)
        .expect("Student-t concentrated per-name pricing");

        assert!(
            concentrated.unexpected_loss.amount() > granular.unexpected_loss.amount(),
            "Student-t concentrated pool (40 names) loss dispersion {:.0} \
             must exceed granular pool (600 names) dispersion {:.0}",
            concentrated.unexpected_loss.amount(),
            granular.unexpected_loss.amount()
        );
    }

    /// Student-t per-name pricing must be deterministic and bit-identical
    /// between repeated runs — the shared mixing `W` is drawn from a seeded
    /// per-path stream.
    #[test]
    fn student_t_per_name_pricing_is_deterministic() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        let deal = clo_deal(80);

        let run = || {
            StochasticPricer::new(student_t_copula_config(
                0.04,
                0.25,
                6.0,
                36,
                PoolGranularity::PerName,
                500,
            ))
            .price(&deal, &market)
            .expect("Student-t per-name pricing")
        };
        let a = run();
        let b = run();

        assert_eq!(
            a.npv.amount(),
            b.npv.amount(),
            "repeated Student-t per-name MC runs must be bit-identical"
        );
        for id in ["SR", "MEZZ", "EQ"] {
            assert_eq!(
                tranche_pv(&a, id),
                tranche_pv(&b, id),
                "{id}: repeated Student-t per-name runs must produce \
                 bit-identical tranche PV"
            );
        }
    }

    /// Student-t LHP pricing must be deterministic: the LHP fast-path now
    /// draws a shared mixing `W` per period from the seeded per-path stream,
    /// so repeated runs must stay bit-identical.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn student_t_lhp_pricing_is_deterministic() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        let deal = clo_deal(600);

        let run = || {
            StochasticPricer::new(student_t_copula_config(
                0.04,
                0.25,
                6.0,
                36,
                PoolGranularity::LargeHomogeneous,
                500,
            ))
            .price(&deal, &market)
            .expect("Student-t LHP pricing")
        };
        let a = run();
        let b = run();

        assert_eq!(
            a.npv.amount(),
            b.npv.amount(),
            "repeated Student-t LHP MC runs must be bit-identical"
        );
    }
}
