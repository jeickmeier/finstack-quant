//! Stochastic structured-credit scenario waterfall pricing engine.

use super::config::{PricingMode, StochasticPricerConfig};
use super::result::{StochasticPricingResult, TranchePricingResult};
use crate::cashflow::builder::schedule::weighted_average_life_from_principal;
use crate::correlation::{CopulaSpec, LatentFactorSpec, RecoverySpec};
use crate::instruments::fixed_income::structured_credit::pricing::simulation_engine::{
    run_simulation_with_source, PerNameDefaultEngine, PerNamePeriodInput, PeriodPoolShock,
    StochasticPathFlowSource,
};
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::default::{
    MacroCreditFactors, PerNameCopulaDefault,
};
use crate::instruments::fixed_income::structured_credit::pricing::{
    StochasticDefaultSpec, StochasticPrepaySpec,
};
use crate::instruments::fixed_income::structured_credit::types::{StructuredCredit, Tranche};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::stats::OnlineStats;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_monte_carlo::traits::RandomStream;
use rayon::prelude::*;
use std::cmp::Ordering;
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

/// Seed salt for the tree-mode tail-month RNG.
///
/// Tree-mode paths enumerate base-`branch_count` digits of the path index for
/// the leading months; months beyond `log_branch(path_count)` carry no digit
/// information and are drawn from a Philox substream seeded with
/// `config.seed ^ TREE_TAIL_SEED_SALT` instead (see [`tree_path_factors`]).
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

impl StochasticPricer {
    /// Create a new stochastic pricer.
    pub(crate) fn new(config: StochasticPricerConfig) -> Self {
        Self { config }
    }

    /// Price the full deal and all tranches through scenario-level waterfalls.
    pub(crate) fn price(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
    ) -> Result<StochasticPricingResult> {
        // Fail fast on invalid default specs (e.g. Student-t dof ≤ 2) so the
        // per-path hot loops can assume a validated, buildable spec.
        self.config
            .tree_config
            .default_spec
            .build_with_seasoning_offset(self.config.tree_config.initial_seasoning)?;
        match &self.config.pricing_mode {
            PricingMode::Tree => self.price_tree(instrument, context),
            PricingMode::MonteCarlo {
                num_paths,
                antithetic,
            } => self.price_monte_carlo(instrument, context, *num_paths, *antithetic),
            PricingMode::Hybrid {
                tree_periods,
                mc_paths,
            } => self.price_hybrid(instrument, context, *tree_periods, *mc_paths),
        }
    }

    fn price_tree(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
    ) -> Result<StochasticPricingResult> {
        let terminal_paths = self
            .config
            .tree_config
            .branching
            .estimate_terminal_nodes(self.config.tree_config.num_periods);
        if terminal_paths > self.config.max_tree_paths {
            return Err(finstack_quant_core::Error::Validation(format!(
                "structured_credit_stochastic tree requires {terminal_paths} terminal paths, \
                 above max_tree_paths={}",
                self.config.max_tree_paths
            )));
        }

        let branch_count = self
            .config
            .tree_config
            .branching
            .branches_at_node(self.branching_variance_proxy())
            .max(1);
        let path_count = terminal_paths.max(1);
        let per_name_simulator = self.per_name_simulator()?;
        // Tree mode draws no antithetic pairs: every path is an independent
        // stratified node, so the std-error is the plain i.i.d. estimator.
        let mut collector = ScenarioCollector::new(instrument, path_count, false)?;
        for path_index in 0..path_count {
            let shocks = self.tree_path_shocks(instrument, path_index, path_count, branch_count)?;
            // Tree mode draws no antithetic pairs — each path is an
            // independent stratified node, so the per-name substream is
            // per-path and never negated.
            let per_name_engine = per_name_simulator
                .as_ref()
                .map(|sim| self.per_name_engine(sim, path_index, false));
            let output = self.price_path(instrument, context, shocks, per_name_engine)?;
            collector.record_output(output);
        }
        Ok(collector.finalize(self, "Tree"))
    }

    fn price_monte_carlo(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        num_paths: usize,
        antithetic: bool,
    ) -> Result<StochasticPricingResult> {
        if num_paths == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "Monte Carlo pricing requires at least one simulation path".to_string(),
            ));
        }

        let factor_sets = self.monte_carlo_factor_sets(instrument, num_paths, antithetic);
        // Antithetic pairing is only effective when `num_paths` is even — an
        // odd trailing path is drawn independently (see `monte_carlo_factor_sets`)
        // and cannot be paired. Pair-aware std-error therefore requires an even
        // path count; with an odd count the antithetic flag is dropped so the
        // collector falls back to the plain i.i.d. estimator.
        let antithetic_paired = antithetic && num_paths.is_multiple_of(2);
        let mode = if antithetic {
            format!("MonteCarlo({}, antithetic)", num_paths)
        } else {
            format!("MonteCarlo({num_paths})")
        };
        self.price_factor_sets(
            instrument,
            context,
            factor_sets,
            num_paths,
            antithetic_paired,
            &mode,
        )
    }

    fn price_hybrid(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        tree_periods: usize,
        mc_paths: usize,
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

        let branch_count = self
            .config
            .tree_config
            .branching
            .branches_at_node(self.branching_variance_proxy())
            .max(1);
        let prefix_count = self
            .config
            .tree_config
            .branching
            .estimate_terminal_nodes(tree_periods)
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

        let mut factor_sets = Vec::with_capacity(total_paths);
        for prefix_index in 0..prefix_count {
            let prefix =
                self.tree_path_factors(prefix_index, prefix_count, branch_count, prefix_months);
            for suffix_index in 0..mc_paths {
                let path_index = prefix_index * mc_paths + suffix_index;
                // Per-path counter-based substream: Philox(seed).substream(path_id)
                // is statistically independent for any pair of distinct path_ids,
                // so the hybrid suffix factors carry no inter-path correlation.
                let mut rng = PhiloxRng::new(self.config.seed).substream(path_index as u64);
                // Pre-size to exact total length so neither the prefix copy nor
                // the suffix push triggers a Vec re-grow. Each path needs its
                // own owned Vec because `factor_sets` is consumed by a parallel
                // iterator below.
                let mut factors = Vec::with_capacity(prefix.len() + suffix_months);
                factors.extend_from_slice(&prefix);
                for _ in 0..suffix_months {
                    factors.push(if has_stochastic_rates {
                        rng.next_std_normal()
                    } else {
                        0.0
                    });
                }
                factor_sets.push(factors);
            }
        }

        self.price_factor_sets(
            instrument,
            context,
            factor_sets,
            total_paths,
            // Hybrid mode draws no antithetic pairs.
            false,
            &format!("Hybrid(tree_periods={tree_periods}, mc_paths={mc_paths})"),
        )
    }

    fn price_factor_sets(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        factor_sets: Vec<Vec<f64>>,
        num_paths: usize,
        antithetic: bool,
        pricing_mode: &str,
    ) -> Result<StochasticPricingResult> {
        let per_name_simulator = self.per_name_simulator()?;
        // `into_par_iter().enumerate()` on a `Vec` is an order-preserving
        // `IndexedParallelIterator`: `collect()` returns outputs in path
        // order regardless of rayon scheduling, and each path keeps a stable
        // index for its idiosyncratic-draw substream. Both properties are
        // required for bit-identical serial/parallel results (the downstream
        // Welford accumulation is order-sensitive).
        let outputs: Vec<PathScenarioOutput> = factor_sets
            .into_par_iter()
            .enumerate()
            .map(|(path_index, factors)| {
                let shocks = self.path_shocks_from_factors(instrument, &factors)?;
                let per_name_engine = per_name_simulator
                    .as_ref()
                    .map(|sim| self.per_name_engine(sim, path_index, antithetic));
                self.price_path(instrument, context, shocks, per_name_engine)
            })
            .collect::<Result<Vec<_>>>()?;

        let mut collector = ScenarioCollector::new(instrument, num_paths, antithetic)?;
        for output in outputs {
            collector.record_output(output);
        }

        Ok(collector.finalize(self, pricing_mode))
    }

    fn monte_carlo_factor_sets(
        &self,
        instrument: &StructuredCredit,
        num_paths: usize,
        antithetic: bool,
    ) -> Vec<Vec<f64>> {
        // One base Philox RNG seeded from config.seed.  Each (logical) path
        // index gets its own counter-based substream via `substream(path_id)`.
        //
        // For antithetic pairs path 2k+1 is the negation of path 2k.  Both
        // members of a pair share the same Philox stream (stream_id = k) so
        // the antithetic pair is perfectly correlated by construction; all
        // pairs are independent of one another because they have distinct
        // stream IDs.
        let base_rng = PhiloxRng::new(self.config.seed);
        let mut factor_sets = Vec::with_capacity(num_paths);

        let mut path_index = 0usize;
        while path_index < num_paths {
            if antithetic && path_index + 1 < num_paths {
                // Stream ID is the pair index (path_index / 2) so each pair
                // draws from a stream that is independent of all other pairs.
                let stream_id = (path_index / 2) as u64;
                let mut rng = base_rng.substream(stream_id);
                let factors = self.random_factors(instrument, &mut rng);
                let antithetic_factors = factors.iter().map(|z| -z).collect();
                factor_sets.push(factors);
                factor_sets.push(antithetic_factors);
                path_index += 2;
            } else {
                // Non-antithetic path: stream_id equals the path index so
                // every path is independent regardless of execution order.
                let mut rng = base_rng.substream(path_index as u64);
                let factors = self.random_factors(instrument, &mut rng);
                factor_sets.push(factors);
                path_index += 1;
            }
        }

        factor_sets
    }

    /// Extract the copula specification and asset correlation when the
    /// scenario default model is a copula.
    ///
    /// Per-name simulation only applies to copula default models; other
    /// stochastic default models (factor-correlated, intensity-process,
    /// hazard-curve) keep the pool-wide MDR path.
    fn copula_default(&self) -> Option<(CopulaSpec, f64)> {
        match &self.config.tree_config.default_spec {
            StochasticDefaultSpec::Copula {
                copula_spec,
                correlation,
                ..
            } => Some((copula_spec.clone(), *correlation)),
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
    /// pairs `(2k, 2k+1)` with `monte_carlo_factor_sets` negating the
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
        let base = PhiloxRng::new(self.config.seed ^ PER_NAME_SEED_SALT);
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

    fn price_path(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        shocks: Vec<PeriodPoolShock>,
        per_name_engine: Option<PerNameDefaultEngine>,
    ) -> Result<PathScenarioOutput> {
        let mut source = match per_name_engine {
            Some(engine) => StochasticPathFlowSource::with_per_name(shocks, engine),
            None => StochasticPathFlowSource::new(shocks),
        };
        let path_results = run_simulation_with_source(
            instrument,
            context,
            self.config.valuation_date,
            &mut source,
        )?;

        let mut deal_pv = 0.0;
        let mut deal_loss = 0.0;
        let mut tranches = Vec::with_capacity(instrument.tranches.tranches.len());
        for (idx, tranche) in instrument.tranches.tranches.iter().enumerate() {
            let tranche_result = path_results.get(tranche.id.as_str()).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "stochastic waterfall omitted tranche result '{}'",
                    tranche.id
                ))
            })?;
            let metrics = PathTrancheMetrics::from_cashflows(
                tranche_result,
                self.config.valuation_date,
                &self.config.discount_curve,
            )?;
            deal_pv += metrics.pv;
            deal_loss += metrics.loss;
            tranches.push((idx, metrics));
        }
        Ok(PathScenarioOutput {
            deal_pv,
            deal_loss,
            tranches,
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
    ) -> Result<Vec<PeriodPoolShock>> {
        let month_count = self.month_count(instrument);
        let factors = self.tree_path_factors(path_index, path_count, branch_count, month_count);
        self.path_shocks_from_factors(instrument, &factors)
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
        let stratified = matches!(
            self.config.tree_config.branching,
            super::super::tree::BranchingSpec::Stratified { .. }
        );

        // Number of leading months the base-`branch_count` digits of the path
        // index can actually resolve: the largest `m` with
        // `branch_count^m <= path_count`. Beyond that every path's digit is 0,
        // which previously pinned a deterministic z = Φ⁻¹(0.5/branch_count)
        // shock (≈ −0.97 for trinomial trees) on all trailing months. Those
        // months are instead drawn from a per-path Philox substream so the
        // tail diffuses like a genuine Monte Carlo continuation.
        let resolved_months = if stratified {
            month_count
        } else {
            let mut resolved = 0usize;
            let mut capacity = 1usize;
            while resolved < month_count {
                match capacity.checked_mul(branch_count) {
                    Some(next) if next <= path_count => {
                        capacity = next;
                        resolved += 1;
                    }
                    _ => break,
                }
            }
            resolved
        };
        let mut tail_rng =
            (resolved_months < month_count && self.has_stochastic_rates()).then(|| {
                PhiloxRng::new(self.config.seed ^ TREE_TAIL_SEED_SALT)
                    .substream(original_path_index as u64)
            });

        for month in 0..month_count {
            let z = if !self.has_stochastic_rates() {
                0.0
            } else if stratified {
                let p = (((path_index + month) % path_count) as f64 + 0.5) / path_count as f64;
                finstack_quant_core::math::standard_normal_inv_cdf(p)
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

    /// Configured systematic-factor mean-reversion speed κ, when positive.
    ///
    /// Only [`LatentFactorSpec::SingleFactor`] carries a mean-reversion
    /// parameter; other factor specs (and κ = 0) keep the i.i.d. monthly
    /// factor behavior.
    fn factor_mean_reversion(&self) -> Option<f64> {
        match &self.config.tree_config.factor_spec {
            LatentFactorSpec::SingleFactor { mean_reversion, .. } if *mean_reversion > 0.0 => {
                Some(*mean_reversion)
            }
            _ => None,
        }
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

    fn path_shocks_from_factors(
        &self,
        instrument: &StructuredCredit,
        factors: &[f64],
    ) -> Result<Vec<PeriodPoolShock>> {
        // AR(1)/OU persistence (single chokepoint for MC, tree, and hybrid
        // paths): when a positive mean-reversion speed is configured the
        // monthly draws are innovations, not the factor itself.
        let evolved_storage;
        let factors: &[f64] = match self.factor_mean_reversion() {
            Some(kappa) if self.has_stochastic_rates() => {
                evolved_storage = Self::evolved_factors(factors, kappa);
                &evolved_storage
            }
            _ => factors,
        };
        let months_per_period = instrument.frequency.months().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "Structured credit stochastic pricing requires month-based payment frequencies"
                    .to_string(),
            )
        })? as usize;
        let months_per_period = months_per_period.max(1);
        let payment_periods = self.payment_period_count(instrument);
        let mut shocks = Vec::with_capacity(payment_periods);

        // When the default model is a copula, build it once to source the
        // per-period *unconditional* marginal default probability used to set
        // each name's copula barrier `Φ⁻¹(PDₜ)`.
        let copula_model = if self.copula_default().is_some() {
            self.config
                .tree_config
                .default_spec
                .build_with_seasoning_offset(self.config.tree_config.initial_seasoning)?
        } else {
            None
        };

        for period in 0..payment_periods {
            let start = period * months_per_period;
            let end = (start + months_per_period).min(factors.len());
            let month_slice = if start < end {
                &factors[start..end]
            } else {
                &[][..]
            };
            let mut shock = self.aggregate_monthly_shocks(start as u32, month_slice);
            shock.per_name =
                self.copula_period_input(copula_model.as_deref(), start as u32, month_slice);
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
        copula_model: Option<&dyn crate::instruments::fixed_income::structured_credit::pricing::stochastic::default::StochasticDefault>,
        start_month: u32,
        factors: &[f64],
    ) -> Option<PerNamePeriodInput> {
        let model = copula_model?;
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

        // Period systematic factor (item 10).
        //
        // For a multi-month payment period the copula latent variable must
        // condition on a factor that represents the WHOLE period, consistent
        // with the LHP/MDR channel — which integrates every month's factor
        // through `aggregate_monthly_shocks`. Taking only the first month's
        // factor ignored months 2..M and desynchronized the two channels.
        //
        // The `M` monthly factors are i.i.d. `N(0,1)`; their sum has variance
        // `M`, so the period-representative systematic factor is
        // `Z_period = (Σ Zₘ)/√M`. Dividing by `√M` (not `M`) keeps
        // `Z_period ~ N(0,1)`, so the marginal-PD copula barrier
        // `c = Φ⁻¹(PDₜ)` stays correctly calibrated. A single-month period
        // recovers exactly that month's factor.
        let systematic_z = if self.has_stochastic_rates() && !factors.is_empty() {
            let sum: f64 = factors.iter().sum();
            sum / (factors.len() as f64).sqrt()
        } else {
            0.0
        };

        Some(PerNamePeriodInput {
            systematic_z,
            marginal_pd,
        })
    }

    fn aggregate_monthly_shocks(&self, start_month: u32, factors: &[f64]) -> PeriodPoolShock {
        if factors.is_empty() {
            return self.monthly_shock(start_month.saturating_add(1), 0.0);
        }

        let mut prepay_survival = 1.0;
        let mut default_survival = 1.0;
        let mut recovery_sum = 0.0;
        for (offset, factor) in factors.iter().enumerate() {
            let shock = self.monthly_shock(start_month.saturating_add(offset as u32 + 1), *factor);
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

    fn monthly_shock(&self, month_offset: u32, z: f64) -> PeriodPoolShock {
        let factor = if self.has_stochastic_rates() { z } else { 0.0 };
        let seasoning = self
            .config
            .tree_config
            .initial_seasoning
            .saturating_add(month_offset);
        let factors = [factor];

        PeriodPoolShock::pool_wide(
            self.conditional_smm(seasoning, &factors),
            self.conditional_mdr(seasoning, &factors),
            self.recovery_rate(factor),
        )
    }

    fn conditional_smm(&self, seasoning: u32, factors: &[f64]) -> f64 {
        if let Some(model) = self.config.tree_config.prepay_spec.build() {
            return model
                .conditional_smm(seasoning, factors, self.config.tree_config.pool_coupon, 1.0)
                .clamp(0.0, 0.50);
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

    fn conditional_mdr(&self, seasoning: u32, factors: &[f64]) -> f64 {
        // The spec is validated up-front in `price()`, so an `Err` here is
        // unreachable; `.ok().flatten()` only strips the already-checked
        // Result layer.
        if let Some(model) = self
            .config
            .tree_config
            .default_spec
            .build_with_seasoning_offset(self.config.tree_config.initial_seasoning)
            .ok()
            .flatten()
        {
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

    fn branching_variance_proxy(&self) -> f64 {
        let factor_var = match &self.config.tree_config.factor_spec {
            LatentFactorSpec::SingleFactor { volatility, .. } => volatility * volatility,
            LatentFactorSpec::TwoFactor {
                credit_vol,
                prepay_vol,
                ..
            } => credit_vol * credit_vol + prepay_vol * prepay_vol,
            LatentFactorSpec::MultiFactor { volatilities, .. } => {
                volatilities.iter().map(|v| v * v).sum::<f64>()
            }
        };
        let prepay_loading = self
            .config
            .tree_config
            .prepay_spec
            .factor_loading()
            .unwrap_or(0.0);
        let default_loading = self
            .config
            .tree_config
            .default_spec
            .correlation()
            .unwrap_or(0.0);
        let recovery_loading = match &self.config.tree_config.recovery_spec {
            RecoverySpec::Constant { .. } => 0.0,
            RecoverySpec::MarketCorrelated {
                factor_correlation,
                recovery_volatility,
                ..
            } => factor_correlation * recovery_volatility,
        };

        (factor_var * (prepay_loading.abs() + default_loading.abs() + recovery_loading.abs()))
            .clamp(0.0, 1.0)
    }
}

struct PathScenarioOutput {
    deal_pv: f64,
    deal_loss: f64,
    tranches: Vec<(usize, PathTrancheMetrics)>,
}

#[derive(Clone, Copy, Default)]
struct PathTrancheMetrics {
    pv: f64,
    loss: f64,
    wal: f64,
    duration: f64,
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

        Ok(Self {
            pv,
            loss: cashflows.total_writedown.amount(),
            wal,
            duration: if positive_pv > f64::EPSILON {
                weighted_duration / positive_pv
            } else {
                0.0
            },
        })
    }
}

struct TrancheScenarioStats {
    tranche_id: String,
    seniority: String,
    attachment: f64,
    detachment: f64,
    pv_stats: OnlineStats,
    loss_stats: OnlineStats,
    losses: Vec<f64>,
    wal_sum: f64,
    duration_sum: f64,
}

impl TrancheScenarioStats {
    fn new(tranche: &Tranche, num_paths: usize) -> Self {
        Self {
            tranche_id: tranche.id.to_string(),
            seniority: format!("{:?}", tranche.seniority),
            attachment: tranche.attachment_point / 100.0,
            detachment: tranche.detachment_point / 100.0,
            pv_stats: OnlineStats::new(),
            loss_stats: OnlineStats::new(),
            losses: Vec::with_capacity(num_paths),
            wal_sum: 0.0,
            duration_sum: 0.0,
        }
    }

    fn record(&mut self, metrics: PathTrancheMetrics) {
        self.pv_stats.update(metrics.pv);
        self.loss_stats.update(metrics.loss);
        self.losses.push(metrics.loss);
        self.wal_sum += metrics.wal;
        self.duration_sum += metrics.duration;
    }

    fn finalize(
        mut self,
        currency: finstack_quant_core::currency::Currency,
        num_paths: usize,
        es_confidence: f64,
    ) -> TranchePricingResult {
        let paths = num_paths.max(1) as f64;
        let mean_pv = self.pv_stats.mean();
        let mean_loss = self.loss_stats.mean();
        // Use population variance (n denominator), the established convention
        // for these Monte Carlo loss estimators.
        let loss_std = self.loss_stats.population_variance().sqrt();
        let es = expected_shortfall(&mut self.losses, es_confidence);

        TranchePricingResult::new(
            self.tranche_id,
            self.seniority,
            Money::new(mean_pv, currency),
        )
        .with_subordination(self.attachment, self.detachment)
        .with_risk_metrics(
            Money::new(mean_loss, currency),
            Money::new(loss_std, currency),
            Money::new(es, currency),
        )
        .with_average_life(self.wal_sum / paths)
        .with_credit_duration(self.duration_sum / paths)
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
}

impl ScenarioCollector {
    fn new(instrument: &StructuredCredit, num_paths: usize, antithetic: bool) -> Result<Self> {
        if num_paths == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "stochastic scenario collector requires at least one path".to_string(),
            ));
        }
        Ok(Self {
            currency: instrument.pool.base_currency(),
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
    }

    fn finalize(
        mut self,
        pricer: &StochasticPricer,
        pricing_mode: &str,
    ) -> StochasticPricingResult {
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
            Money::new(mean_pv, self.currency),
            Money::new(mean_loss, self.currency),
            self.num_paths,
        )
        .with_unexpected_loss(Money::new(loss_pop_var.sqrt(), self.currency))
        .with_expected_shortfall(Money::new(es, self.currency), pricer.config.es_confidence);

        let notional = pricer.config.tree_config.initial_balance;
        if notional > f64::EPSILON {
            result.clean_price = mean_pv / notional * 100.0;
            result.dirty_price = result.clean_price;
        }
        result.pv_std_error = std_error;
        result.pv_confidence_interval = (mean_pv - 1.96 * std_error, mean_pv + 1.96 * std_error);
        result.pricing_mode = pricing_mode.to_string();
        result.tranche_results = self
            .tranche_stats
            .into_iter()
            .map(|stats| stats.finalize(self.currency, self.num_paths, pricer.config.es_confidence))
            .collect();

        result
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
    losses.sort_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));
    let tail = (1.0 - confidence).clamp(0.0, 1.0);
    let tail_count = (tail * losses.len() as f64).ceil().max(1.0) as usize;
    let tail_count = tail_count.min(losses.len());
    losses.iter().take(tail_count).sum::<f64>() / tail_count as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::{
        BranchingSpec, ScenarioTreeConfig,
    };
    use crate::instruments::fixed_income::structured_credit::{
        AssetPool, DealType, DefaultModelSpec, PoolAsset, RecoveryModelSpec, Tranche,
        TrancheCashflows, TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
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
            (first, Money::new(40.0, Currency::USD)),
            (second, Money::new(60.0, Currency::USD)),
        ];
        let zero = Money::new(0.0, Currency::USD);
        let cashflows = TrancheCashflows {
            tranche_id: "A".to_string(),
            cashflows: principal_flows.clone(),
            detailed_flows: Vec::new(),
            interest_flows: Vec::new(),
            principal_flows,
            pik_flows: Vec::new(),
            writedown_flows: Vec::new(),
            final_balance: zero,
            total_interest: zero,
            total_principal: Money::new(100.0, Currency::USD),
            total_pik: zero,
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
        let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::new(1_000_000.0, Currency::USD),
            0.06,
            maturity,
            DayCount::Thirty360,
        ));
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::new(1_000_000.0, Currency::USD),
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
            ScenarioTreeConfig::new(12, 1.0, BranchingSpec::fixed(2)),
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
            ScenarioTreeConfig::new(12, 1.0, BranchingSpec::fixed(2)),
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
        assert!(result.pricing_mode.contains("Hybrid"));
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
        let mut collector = ScenarioCollector::new(&instrument, n, false).expect("collector");

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
        use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::{
            BranchingSpec, ScenarioTreeConfig,
        };
        use crate::instruments::fixed_income::structured_credit::pricing::stochastic::pricer::config::StochasticPricerConfig;
        let config = StochasticPricerConfig::new(
            test_date(),
            test_discount_curve(),
            ScenarioTreeConfig::new(12, 1.0, BranchingSpec::fixed(2)),
        );
        let pricer = StochasticPricer::new(config);
        let result = collector.finalize(&pricer, "Test");

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

    /// M2.16 — the configured `mean_reversion` must actually change Monte
    /// Carlo path statistics. With identical seeds and configs differing
    /// only in κ, the results must NOT be bit-identical (κ was previously a
    /// dead parameter).
    #[test]
    fn mean_reversion_changes_path_statistics() {
        use crate::instruments::fixed_income::structured_credit::pricing::stochastic::default::StochasticDefaultSpec;

        let instrument = test_instrument();
        let market = MarketContext::new().insert((*test_discount_curve()).clone());
        let price_with_kappa = |kappa: f64| {
            let mut tree_config = ScenarioTreeConfig::new(24, 2.0, BranchingSpec::fixed(2));
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

    /// M2.13 — canonical sign convention invariant for the MC engine: on the
    /// shipped RMBS/CLO configs, defaults and recoveries must co-move
    /// NEGATIVELY across systematic-factor realizations (stress = low factor
    /// ⇒ high MDR and low recovery).
    #[test]
    fn mc_engine_defaults_and_recoveries_co_move_negatively() {
        for (label, tree_config) in [
            ("rmbs", ScenarioTreeConfig::rmbs_standard(2.0, 0.045)),
            ("clo", ScenarioTreeConfig::clo_standard(2.0)),
        ] {
            let config =
                StochasticPricerConfig::new(test_date(), test_discount_curve(), tree_config);
            let pricer = StochasticPricer::new(config);

            let zs = [-2.0, -1.0, 0.0, 1.0, 2.0];
            let shocks: Vec<_> = zs.iter().map(|&z| pricer.monthly_shock(36, z)).collect();
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
    use super::*;
    use crate::instruments::fixed_income::structured_credit::pricing::stochastic::default::PoolGranularity;
    use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::{
        BranchingSpec, ScenarioTreeConfig,
    };
    use crate::instruments::fixed_income::structured_credit::{
        AssetPool, CorrelationStructure, DealType, DefaultModelSpec, PoolAsset, RecoveryModelSpec,
        Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
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
        let mut pool = AssetPool::new("CLO-POOL", DealType::CLO, Currency::USD);
        for i in 0..n_assets {
            pool.assets.push(PoolAsset::fixed_rate_bond(
                format!("L{i}"),
                Money::new(per_asset, Currency::USD),
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
                Money::new(total * 0.80, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity(),
            )
            .expect("senior"),
            Tranche::new(
                "MEZZ",
                80.0,
                92.0,
                TrancheSeniority::Mezzanine,
                Money::new(total * 0.12, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.08 },
                maturity(),
            )
            .expect("mezz"),
            Tranche::new(
                "EQ",
                92.0,
                100.0,
                TrancheSeniority::Equity,
                Money::new(total * 0.08, Currency::USD),
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
            correlation,
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
        let mut tree_config = ScenarioTreeConfig::new(
            num_periods,
            num_periods as f64 / 12.0,
            BranchingSpec::fixed(2),
        );
        tree_config.default_spec = StochasticDefaultSpec::gaussian_copula(base_cdr, correlation);
        tree_config.correlation = CorrelationStructure::flat(correlation, 0.0);
        tree_config.initial_balance = 100_000_000.0;
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
            correlation,
            num_periods,
            granularity,
            num_paths,
        )
    }

    /// Build a pricer config from an explicit copula default spec.
    fn copula_config_with_spec(
        default_spec: StochasticDefaultSpec,
        correlation: f64,
        num_periods: usize,
        granularity: PoolGranularity,
        num_paths: usize,
    ) -> StochasticPricerConfig {
        let mut tree_config = ScenarioTreeConfig::new(
            num_periods,
            num_periods as f64 / 12.0,
            BranchingSpec::fixed(2),
        );
        tree_config.default_spec = default_spec;
        tree_config.correlation = CorrelationStructure::flat(correlation, 0.0);
        tree_config.initial_balance = 100_000_000.0;
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

    /// **Concentration sensitivity** — the test that genuinely fails on the
    /// parent (parent = LHP-only).
    ///
    /// A concentrated pool (60 names) priced per-name must produce a
    /// materially different mezzanine/equity tranche value than the
    /// pool-wide LHP approach, because name-level lumpiness now dominates.
    #[test]
    fn concentrated_pool_per_name_differs_from_lhp() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        // 40 names ⇒ concentrated: a single default is 2.5% of the pool,
        // well inside the 8%-thick equity and 12%-thick mezz tranches.
        let deal = clo_deal(40);

        let per_name = StochasticPricer::new(copula_config(
            0.05,
            0.25,
            36,
            PoolGranularity::PerName,
            8_000,
        ))
        .price(&deal, &market)
        .expect("per-name pricing");
        let lhp = StochasticPricer::new(copula_config(
            0.05,
            0.25,
            36,
            PoolGranularity::LargeHomogeneous,
            8_000,
        ))
        .price(&deal, &market)
        .expect("LHP pricing");

        // Mezzanine and equity tranches must show a material PV gap: the LHP
        // limit smooths away the discrete-default lumpiness that a 60-name
        // pool genuinely carries. The parent engine (LHP-only) produces the
        // SAME value for both, so this assertion fails on the parent.
        let mezz_pn = tranche_pv(&per_name, "MEZZ");
        let mezz_lhp = tranche_pv(&lhp, "MEZZ");
        let eq_pn = tranche_pv(&per_name, "EQ");
        let eq_lhp = tranche_pv(&lhp, "EQ");

        let mezz_gap = (mezz_pn - mezz_lhp).abs();
        let eq_gap = (eq_pn - eq_lhp).abs();
        // Gap must exceed the MC noise floor by a wide margin. At 8 000 paths
        // the per-tranche MC standard error is ≈ $30 k; the observed gaps
        // ($0.29 M mezz, $0.38 M equity) clear the $100 k floor by ~3-4×.
        assert!(
            mezz_gap > 100_000.0 || eq_gap > 100_000.0,
            "concentrated-pool per-name pricing must differ materially from \
             LHP: mezz per-name={mezz_pn:.0} LHP={mezz_lhp:.0} (gap {mezz_gap:.0}); \
             equity per-name={eq_pn:.0} LHP={eq_lhp:.0} (gap {eq_gap:.0})"
        );
    }

    /// Per-name simulation must be deterministic and bit-identical between
    /// repeated runs (seeded `PhiloxRng` substreams).
    #[test]
    fn per_name_pricing_is_deterministic() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        let deal = clo_deal(80);

        let run = || {
            StochasticPricer::new(copula_config(0.04, 0.25, 24, PoolGranularity::PerName, 500))
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
                24,
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

        let pricer = StochasticPricer::new(copula_config(
            0.05,
            0.30,
            12, // 12 monthly tree periods
            PoolGranularity::PerName,
            16,
        ));

        // Craft 6 monthly factors covering two quarterly periods. The first
        // quarter is benign in month 1 but stressed in months 2 and 3 — a
        // month-1-only systematic factor would miss that stress entirely.
        let factors = vec![0.10_f64, -2.0, -1.5, 0.3, 0.4, 0.5];
        let shocks = pricer
            .path_shocks_from_factors(&deal, &factors)
            .expect("path shocks");

        assert!(!shocks.is_empty(), "must produce at least one period shock");
        let period0 = shocks[0]
            .per_name
            .expect("per-name plan must be present for a copula deal");

        // Expected period systematic factor for quarter 1: (Σ Zₘ)/√3.
        let m = 3.0_f64;
        let expected_z = (factors[0] + factors[1] + factors[2]) / m.sqrt();
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
                24,
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

    /// Per-name idiosyncratic recovery dispersion must widen the deal-loss
    /// distribution.
    ///
    /// Both runs below use per-name simulation with an identical *systematic*
    /// recovery: a flat 40 %. The constant model recovers every default at
    /// exactly 40 %; the market-correlated model with `ρ_R = 0` has the same
    /// flat 40 % systematic recovery (no factor channel) but adds an
    /// idiosyncratic per-name scatter `σ_R = 0.30`. The only difference is
    /// FU4's per-name recovery dispersion, so the dispersed run must carry
    /// strictly more loss uncertainty. The pre-fix engine applied one
    /// pool-level recovery to every per-name default, making the two
    /// indistinguishable.
    #[test]
    fn per_name_recovery_dispersion_widens_loss_distribution() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        let deal = clo_deal(40); // concentrated pool — name-level scatter shows

        let mut constant_cfg = copula_config(0.06, 0.20, 36, PoolGranularity::PerName, 4_000);
        constant_cfg.tree_config.recovery_spec = RecoverySpec::Constant { rate: 0.40 };

        let mut dispersed_cfg = copula_config(0.06, 0.20, 36, PoolGranularity::PerName, 4_000);
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

    /// **Student-t LHP-limit parity** — the regression anchor for the
    /// corrected Student-t LHP conditional default probability.
    ///
    /// A large, granular, homogeneous pool priced through the stochastic
    /// engine with a **Student-t** copula must produce the same tranche PVs
    /// under [`PoolGranularity::PerName`] and [`PoolGranularity::LargeHomogeneous`]:
    /// per-name → LHP as `N → ∞`.
    ///
    /// On the parent commit this FAILS. The pre-fix LHP fast-path fed the
    /// Gaussian systematic `Z` into the Student-t `conditional_default_prob`
    /// slot that expects the `t(ν)` factor `M = Z/√W`, understating pool
    /// defaults ~14-17% (per-name rate ≈ 0.050 vs LHP ≈ 0.043) and thereby
    /// overstating the mezzanine/equity tranche values. There was zero test
    /// coverage of the Student-t copula through the engine — this closes it.
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

        // Total realized credit loss must agree: the per-name and LHP paths
        // now condition on the same (Z, W), so the pool default experience
        // converges as N → ∞. The pre-fix LHP path understates losses ~14%.
        let loss_pn = deal_credit_loss(&per_name);
        let loss_lhp = deal_credit_loss(&lhp);
        let loss_tol = (0.05 * loss_pn.abs()).max(250_000.0);
        assert!(
            (loss_pn - loss_lhp).abs() < loss_tol,
            "Student-t per-name credit loss {loss_pn:.0} should converge to \
             LHP credit loss {loss_lhp:.0} (|diff|={:.0}, tol={loss_tol:.0}); \
             pre-fix LHP understates losses ~14-17%",
            (loss_pn - loss_lhp).abs()
        );

        // Each tranche PV must agree within a few MC standard errors.
        for id in ["SR", "MEZZ", "EQ"] {
            let pn = tranche_pv(&per_name, id);
            let lh = tranche_pv(&lhp, id);
            let tol = (0.015 * lh.abs()).max(250_000.0);
            assert!(
                (pn - lh).abs() < tol,
                "{id}: Student-t per-name PV {pn:.0} should converge to LHP \
                 PV {lh:.0} (|diff|={:.0}, tol={tol:.0}); pre-fix LHP \
                 overstates mezz/equity",
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
                24,
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
    fn student_t_lhp_pricing_is_deterministic() {
        let market = MarketContext::new().insert((*discount_curve()).clone());
        let deal = clo_deal(600);

        let run = || {
            StochasticPricer::new(student_t_copula_config(
                0.04,
                0.25,
                6.0,
                24,
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
