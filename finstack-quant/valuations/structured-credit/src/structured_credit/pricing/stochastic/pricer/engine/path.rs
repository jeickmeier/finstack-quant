//! Per-path waterfall execution, per-name copula setup, and instrument-path streams.

use super::collector::{PathScenarioOutput, PathTrancheMetrics};
use super::{PreparedRun, StochasticPricer};
use crate::structured_credit::pricing::simulation_engine::{
    InstrumentPathFlowSource, PathShocks, PerNameDefaultEngine, PeriodPoolShock,
    StochasticPathFlowSource, simulate_prepared,
};
use crate::structured_credit::types::{StructuredCredit, TrancheCashflows};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{FxHashMap, Result};
use finstack_quant_models::correlation::CopulaSpec;
use finstack_quant_models::credit::pool::{PerNameCopulaDefault, StochasticDefaultSpec};
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use std::sync::Arc;

/// Seed salt for the per-name idiosyncratic-draw RNG.
///
/// Per-name copula simulation draws each name's idiosyncratic shock `εᵢ` from
/// a Philox stream seeded with `config.seed ^ PER_NAME_SEED_SALT`. XOR-salting
/// the seed places the idiosyncratic streams in a stream space fully disjoint
/// from the systematic-factor streams (which use the unsalted seed), so the
/// two never collide regardless of path count or pricing mode. The salt value
/// itself is arbitrary; only its disjointness from `0` matters.
const PER_NAME_SEED_SALT: u64 = 0x5350_4552_4E41_4D45; // "SPERNAME"

/// Seed salt for the instrument-collateral process draws.
///
/// Revolver spread and utilization shocks of instrument pools are drawn from
/// a Philox stream seeded with `config.seed ^ INSTRUMENT_PATH_SEED_SALT`,
/// disjoint from the systematic-factor and per-name stream spaces, so adding
/// instrument collateral never perturbs the draws of the other channels.
const INSTRUMENT_PATH_SEED_SALT: u64 = 0x494E_5354_5255_4D50; // "INSTRUMP"

impl StochasticPricer {
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
    pub(super) fn copula_rho(&self) -> Option<f64> {
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
    pub(super) fn per_name_simulator(&self) -> Result<Option<Arc<PerNameCopulaDefault>>> {
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
    pub(super) fn per_name_engine(
        &self,
        simulator: &Arc<PerNameCopulaDefault>,
        path_index: usize,
        antithetic: bool,
        prepared: &PreparedRun,
    ) -> PerNameDefaultEngine {
        let base = PhiloxRng::new(self.config.tree_config.seed ^ PER_NAME_SEED_SALT);
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
                    Arc::clone(&prepared.recovery_model),
                )
            } else {
                PerNameDefaultEngine::new(
                    Arc::clone(simulator),
                    self.config.pool_granularity,
                    rng,
                    Arc::clone(&prepared.recovery_model),
                )
            }
        } else {
            let rng = base.substream(path_index as u64);
            PerNameDefaultEngine::new(
                Arc::clone(simulator),
                self.config.pool_granularity,
                rng,
                Arc::clone(&prepared.recovery_model),
            )
        }
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

    pub(super) fn price_path(
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
        let mut counterfactual: Option<FxHashMap<String, TrancheCashflows>> = None;
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
}
